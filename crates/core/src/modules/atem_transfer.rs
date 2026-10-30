//! File transfers: media pool stills, clip frames and clip audio, macros and
//! multiviewer labels, up to and down from the switcher.
//!
//! A transfer runs in the background of the session. Its command answers at
//! once with the transfer's number; `transfers.<number>` in the state follows
//! it (queued, reading, waiting, locking, transferring, writing, then done,
//! failed or canceled) with its progress, and `transfer_started` and
//! `transfer_finished` message events mark its ends. One transfer is on the
//! wire at a time, in the order asked (Blackmagic's ATEM Switchers SDK
//! manual, section 4.3.8.9: no more than one transfer at a time).
//!
//! The flow, from Sofie `src/dataTransfer/` and `src/commands/DataTransfer/`
//! (MIT), with the lock rules of the SDK manual (sections 4.1 and 4.3.6 to
//! 4.3.12) and behavior measured on hardware as bmdwire's
//! `atemwire/docs/MEDIA_LOCK.md` describes it (prose only; the code is
//! LGPL and was not read):
//!
//! - Stills and each clip have a lock (`LOCK` u16 store, u8 state; stills 0,
//!   clip n at n). The switcher grants it to this client alone with `LKOB`
//!   and tells everyone with `LKST`. The lock is held from `LKOB` to this
//!   client's own release (`LOCK` state 0); an `LKST` is never taken as the
//!   end of it, since the echo of a release can arrive after the next grant
//!   (bmdwire). Requests made while another client holds the store are
//!   queued by the switcher (bmdwire), so a grant can come after this client
//!   gave up waiting: a grant nobody here wants is released at once.
//! - Each lock is released after its transfer, as the SDK manual advises so
//!   other clients get a turn; a further transfer to the same store waits a
//!   frame (60 ms) after the release, and while another client is seen
//!   holding the store, until it is free or 10 s have passed (bmdwire).
//! - An upload asks with `FTSD` (id, store, index, raw size, mode), the
//!   switcher answers with budgets (`FTCD`: chunk size and count), the first
//!   budget is answered with the description (`FTFD`: name, description and
//!   MD5 of the raw data) and every budget with that many `FTDa` chunks; the
//!   switcher ends it with `FTDC`. A download asks with `FTSU`, each `FTDa`
//!   is acknowledged with `FTUA`, and `FTDC` ends it. `FTDC` goes to every
//!   client with the owner's transfer id (bmdwire), so it ends a transfer
//!   here only when its id matches and the data is all sent or some arrived.
//! - `FTDE` reports an error: 1 retry (Sofie), 2 not found (Sofie), 5 the
//!   transfer is not locked, 6 another client won the store (bmdwire): the
//!   loser still holds its lock, releases it and asks again in turn.
//! - Clips are cleared (`CMPC`) before their first frame and named and made
//!   valid (`SMPC`) after the last (Sofie; the SDK manual's SetInvalid and
//!   SetValid). Macros and labels take no lock (Sofie's simple queues).
//! - Clearing a still or a clip (`CSTL`, `CMPC`) runs in the same queue,
//!   moving no data: the store's lock is taken, the clear sent and the
//!   lock released. The SDK manual (4.3.8.6, 4.3.12.6) says a clear works
//!   only for a client holding the lock or while no other client holds
//!   it, and bmdwire reports a 1 M/E Production Studio 4K ignoring a bare
//!   `CSTL`.
//!
//! Every path ends with the lock released: done, failed, canceled, timed
//! out and closed. A lost connection ends with a goodbye on the old
//! session, which frees its locks at once; a switcher that does not hear it
//! frees a dead session's locks when it times the session out (bmdwire:
//! about 5 s).

use std::collections::{BTreeMap, VecDeque};

use crate::{
    json,
    value::{Map, Value},
};
use md5::{Digest, Md5};

use super::label::{self, Resolutions};
use super::media;
use super::{
    invalid, opt_int, opt_str, put16, put32, u16_at, unsupported, Atem, CommandError, CommandId,
    Cx, Key, Level, Millis, Out, Outcome, Params, Phase, HEADER, MAX_PACKET,
};
use crate::module::FileInput;

/// The commands this file handles.
pub(super) const COMMANDS: [&str; 12] = [
    "clear_still",
    "clear_clip",
    "upload_still",
    "download_still",
    "upload_clip",
    "download_clip",
    "upload_clip_audio",
    "upload_macro",
    "download_macro",
    "upload_multiviewer_label",
    "set_multiviewer_label",
    "cancel_transfer",
];

pub(super) const TIMER: Key = "transfer";
const READ: Key = "atem-read";
/// An upload's file, converted off the session.
const PREPARE: Key = "atem-prepare";
/// Downloaded frames converted off the session, each slot then written by
/// the writer of the same position.
const CONVERTERS: [Key; 4] = [
    "atem-convert-1",
    "atem-convert-2",
    "atem-convert-3",
    "atem-convert-4",
];
const WRITERS: [Key; 4] = [
    "atem-write-1",
    "atem-write-2",
    "atem-write-3",
    "atem-write-4",
];

/// How long a lock request may wait: another client may be loading a whole
/// pool (ATEM Software Control downloads every still on connecting, about
/// 1.8 s each for 1080p, per bmdwire).
const LOCK_WAIT: Millis = 120_000;
/// Silence from the switcher during a transfer.
const STALL: Millis = 5_000;
/// A frame's grace after a release before asking for the same store again.
const COURTESY: Millis = 60;
/// The longest wait for another client seen holding a store.
const LOCK_QUIET: Millis = 10_000;
/// Retries of one frame or file after `FTDE` 1, 5 or 6.
const RETRIES: u32 = 20;
/// Finished transfers kept in the state.
const KEEP_FINISHED: usize = 32;

/// The largest chunk that fits a datagram: header, command header, FTDa's
/// id and length.
const MAX_CHUNK: usize = MAX_PACKET - HEADER - 8 - 4;
/// FTUA commands per packet.
const ACKS_PER_PACKET: usize = (MAX_PACKET - HEADER) / 12;

/// Stores that are not stills or clips (Sofie: macros and labels).
const STORE_OTHER: u16 = 0xFFFF;
/// FTSU types (Sofie `dataTransferDownloadStill.ts`, `dataTransferDownloadMacro.ts`).
const TYPE_FRAME: u16 = 0x00F9;
const TYPE_MACRO: u16 = 0x03FE;
/// FTSD modes (Sofie `dataTransferUpload*.ts`).
const MODE_FRAME: u16 = 0x0001;
const MODE_AUDIO: u16 = 0x0100;
const MODE_LABEL: u16 = 0x0201;
const MODE_MACRO: u16 = 0x0300;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Format {
    Png,
    Rgba,
    Native,
}

#[derive(Debug)]
enum What {
    UploadStill {
        slot: u16,
        name: String,
        description: String,
    },
    DownloadStill {
        slot: u16,
    },
    UploadClip {
        clip: u8,
        name: String,
    },
    DownloadClip {
        clip: u8,
        first: u16,
    },
    UploadAudio {
        clip: u8,
        name: String,
    },
    UploadMacro {
        slot: u16,
        name: String,
    },
    DownloadMacro {
        slot: u16,
    },
    UploadLabel {
        source: u16,
        /// Text drawn into the label here, rather than a bitmap file.
        text: Option<(String, Resolutions)>,
    },
    /// A clear: no data moves, but it runs under the store's lock like a
    /// transfer, since the switcher takes it only from a client holding the
    /// lock or while no other client holds it (SDK manual 4.3.8.6 and
    /// 4.3.12.6), and some models ignore it unless the lock is held
    /// (bmdwire).
    ClearStill {
        slot: u16,
    },
    ClearClip {
        clip: u8,
    },
}

impl What {
    fn kind(&self) -> &'static str {
        match self {
            What::ClearStill { .. } => "clear_still",
            What::ClearClip { .. } => "clear_clip",
            What::UploadStill { .. } => "upload_still",
            What::DownloadStill { .. } => "download_still",
            What::UploadClip { .. } => "upload_clip",
            What::DownloadClip { .. } => "download_clip",
            What::UploadAudio { .. } => "upload_clip_audio",
            What::UploadMacro { .. } => "upload_macro",
            What::DownloadMacro { .. } => "download_macro",
            What::UploadLabel { text: None, .. } => "upload_multiviewer_label",
            What::UploadLabel { .. } => "set_multiviewer_label",
        }
    }

    fn upload(&self) -> bool {
        !matches!(
            self,
            What::DownloadStill { .. }
                | What::DownloadClip { .. }
                | What::DownloadMacro { .. }
                | What::ClearStill { .. }
                | What::ClearClip { .. }
        )
    }

    fn frames(&self) -> bool {
        matches!(
            self,
            What::UploadStill { .. }
                | What::DownloadStill { .. }
                | What::UploadClip { .. }
                | What::DownloadClip { .. }
        )
    }

    /// The store whose lock the transfer needs: stills 0, clip n at n.
    fn lock(&self) -> Option<u16> {
        match self {
            What::UploadStill { .. } | What::DownloadStill { .. } | What::ClearStill { .. } => {
                Some(0)
            }
            What::UploadClip { clip, .. }
            | What::DownloadClip { clip, .. }
            | What::UploadAudio { clip, .. }
            | What::ClearClip { clip } => Some(*clip as u16 + 1),
            _ => None,
        }
    }

    fn store(&self) -> u16 {
        self.lock().unwrap_or(STORE_OTHER)
    }

    /// The index the transfer names for its `unit`th file.
    fn index(&self, unit: usize) -> u16 {
        match self {
            What::UploadStill { slot, .. }
            | What::DownloadStill { slot }
            | What::UploadMacro { slot, .. }
            | What::DownloadMacro { slot }
            | What::ClearStill { slot } => *slot,
            What::ClearClip { clip } => *clip as u16,
            What::UploadClip { .. } => unit as u16,
            What::DownloadClip { first, .. } => first + unit as u16,
            What::UploadAudio { .. } => 0,
            What::UploadLabel { source, .. } => *source,
        }
    }

    /// What it names, numbered as the commands number it.
    fn target(&self) -> Map<String, Value> {
        let mut m = Map::new();
        match self {
            What::UploadStill { slot, .. }
            | What::DownloadStill { slot }
            | What::ClearStill { slot } => {
                m.insert("still".into(), json!(slot + 1));
            }
            What::UploadClip { clip, .. }
            | What::DownloadClip { clip, .. }
            | What::UploadAudio { clip, .. }
            | What::ClearClip { clip } => {
                m.insert("clip".into(), json!(clip + 1));
            }
            What::UploadMacro { slot, .. } | What::DownloadMacro { slot } => {
                m.insert("macro".into(), json!(slot + 1));
            }
            What::UploadLabel { source, .. } => {
                m.insert("source".into(), json!(source));
            }
        }
        m
    }
}

struct Job {
    number: u64,
    what: What,
    format: Format,
    /// Host files, one per unit (a clip's frames; one for anything else).
    paths: Vec<String>,
    /// The frame size in the video mode when it was asked.
    size: (u32, u32),
    unit: usize,
    retries: u32,
    /// The current unit: what goes out (encoded), or what came in.
    data: Vec<u8>,
    raw_len: u32,
    hash: [u8; 16],
    rle: bool,
    sent: usize,
    described: bool,
    /// File writes not yet confirmed.
    writes: usize,
    /// A write that failed.
    write_error: Option<String>,
    bytes: u64,
    /// Chunks of the current unit received, for progress.
    chunks: u32,
}

impl Job {
    fn frame_bytes(&self) -> usize {
        (self.size.0 * self.size.1 * 4) as usize
    }

    fn progress(&self) -> f64 {
        let units = self.paths.len().max(1) as f64;
        let part = if self.what.upload() {
            if self.data.is_empty() {
                0.0
            } else {
                self.sent as f64 / self.data.len() as f64
            }
        } else if self.what.frames() && self.frame_bytes() > 0 {
            (self.data.len() as f64 / self.frame_bytes() as f64).min(1.0)
        } else {
            0.0
        };
        ((self.unit as f64 + part) / units).min(1.0)
    }
}

#[derive(Debug, PartialEq)]
enum Step {
    /// Reading the current unit's file from the host.
    Reading,
    /// Converting it for the switcher, off the session.
    Preparing,
    /// Giving other clients a turn before asking for the store again.
    Yielding,
    Locking {
        since: Millis,
    },
    /// The request is sent; nothing has come back yet.
    Started,
    Moving,
    /// A downloaded frame waits for a free file writer.
    WaitingWriter,
}

#[derive(Default)]
pub(super) struct Transfers {
    queue: VecDeque<Job>,
    active: Option<Job>,
    step: Option<Step>,
    /// The active unit's transfer id.
    id: u16,
    next_id: Option<u16>,
    numbers: u64,
    /// The store this client holds the lock of (granted, not yet released).
    held: Option<u16>,
    /// The store this client asked for and has not been granted.
    asked: Option<u16>,
    /// What the switcher last announced for each store (LKST).
    locks: BTreeMap<u16, bool>,
    /// The store last released and when.
    released: Option<(u16, Millis)>,
    activity: Millis,
    /// File writers in use, with their transfer.
    writing: BTreeMap<Key, u64>,
    /// Downloaded units being converted, with their transfer and file.
    converting: BTreeMap<Key, (u64, String)>,
    /// Downloads off the wire whose files are still being written.
    draining: BTreeMap<u64, Job>,
    finished: VecDeque<u64>,
    /// FTUA acknowledgements to send after the packet being read.
    acks: Vec<(u16, u8)>,
}

fn lines(params: &Params) -> Result<Vec<String>, CommandError> {
    let text = opt_str(params, "paths").unwrap_or("");
    let paths: Vec<String> = text
        .trim_end_matches(['\r', '\n'])
        .split('\n')
        .map(|l| l.trim_end_matches('\r').to_owned())
        .collect();
    if paths.iter().any(String::is_empty) {
        return Err(invalid("paths has an empty line".into()));
    }
    if paths.len() > u16::MAX as usize {
        return Err(invalid("more frames than a clip can hold".into()));
    }
    Ok(paths)
}

/// The last part of a host path, cut to fit `max` bytes, for a default name.
fn base_name(path: &str, max: usize) -> String {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let mut end = base.len().min(max);
    while !base.is_char_boundary(end) {
        end -= 1;
    }
    base[..end].to_owned()
}

fn name_param(params: &Params, name: &str, max: usize) -> Result<Option<String>, CommandError> {
    match opt_str(params, name) {
        Some(s) if s.len() > max => Err(invalid(format!(
            "{name} is {} bytes; the switcher holds {max}",
            s.len()
        ))),
        other => Ok(other.map(str::to_owned)),
    }
}

fn format_param(params: &Params) -> Format {
    match opt_str(params, "format") {
        Some("rgba") => Format::Rgba,
        Some("native") => Format::Native,
        _ => Format::Png,
    }
}

fn md5(data: &[u8]) -> [u8; 16] {
    Md5::digest(data).into()
}

/// How an upload's file becomes what is sent.
#[derive(Clone, Copy)]
enum Prep {
    Frame { format: Format, size: (u32, u32) },
    Audio { reversed: bool },
    Macro,
    Label,
}

/// An upload's file as it is sent: the raw length, the MD5 for FTFD,
/// whether it is run-length encoded, then the bytes, packed for a task's
/// result. Runs off the session: a large frame takes a while.
fn encode_upload(prep: Prep, file: Vec<u8>) -> Result<Vec<u8>, String> {
    let (raw_len, hash, rle, data) = match prep {
        Prep::Frame {
            format,
            size: (w, h),
        } => {
            let frame = (w * h * 4) as usize;
            let yuva = match format {
                Format::Png => {
                    let (pw, ph, rgba) = media::png_decode(&file)?;
                    if (pw, ph) != (w, h) {
                        return Err(format!(
                            "the picture is {pw} by {ph}; the switcher's video mode takes {w} by {h}"
                        ));
                    }
                    media::rgba_to_yuva(w, h, &rgba)
                }
                Format::Rgba | Format::Native if file.len() != frame => {
                    return Err(format!(
                        "{} bytes; a {w} by {h} frame is {frame}",
                        file.len()
                    ))
                }
                Format::Rgba => media::rgba_to_yuva(w, h, &file),
                Format::Native => {
                    if !media::plausible_yuva(&file) {
                        return Err(
                            "not the switcher's 10-bit YUVA words (a word has its top bits set)"
                                .into(),
                        );
                    }
                    file
                }
            };
            (yuva.len(), md5(&yuva), true, media::rle_encode(&yuva))
        }
        Prep::Audio { reversed } => {
            let audio = media::wav_to_clip_audio(&file, reversed)?;
            (audio.len(), md5(&audio), false, audio)
        }
        Prep::Macro => {
            if file.is_empty() {
                return Err("the macro file is empty".into());
            }
            // Sofie sends a macro with no hash.
            (file.len(), [0; 16], false, file)
        }
        Prep::Label => {
            // Values 254 and 255 crash a switcher (Sofie `atem.ts:572-578`).
            if file.is_empty() || file.iter().any(|b| *b >= 254) {
                return Err("a label holds bytes from 0 to 253 only".into());
            }
            (file.len(), md5(&file), false, file)
        }
    };
    // FTSD gives the size in 32 bits: the protocol's own bound, not ours.
    let raw_len = u32::try_from(raw_len)
        .map_err(|_| "the file is larger than the switcher's 32-bit transfer size".to_string())?;
    let mut out = Vec::with_capacity(21 + data.len());
    out.extend_from_slice(&raw_len.to_be_bytes());
    out.extend_from_slice(&hash);
    out.push(rle as u8);
    out.extend_from_slice(&data);
    Ok(out)
}

/// A downloaded unit as its host file holds it. Runs off the session.
fn decode_download(
    frames: bool,
    format: Format,
    (w, h): (u32, u32),
    data: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if !frames {
        return Ok(data);
    }
    let yuva = media::rle_decode(&data, (w * h * 4) as usize)
        .map_err(|e| format!("the switcher's frame is damaged: {e}"))?;
    Ok(match format {
        Format::Png => media::png_encode(w, h, &media::yuva_to_rgba(w, h, &yuva)),
        Format::Rgba => media::yuva_to_rgba(w, h, &yuva),
        Format::Native => yuva,
    })
}

fn lock_body(store: u16, locked: bool) -> Vec<u8> {
    let mut b = vec![0u8; 4];
    put16(&mut b, 0, store);
    b[2] = locked as u8;
    b
}

impl Atem {
    /// Handle a transfer command; false when `name` is not one.
    pub(super) fn transfer_command(
        &mut self,
        cx: &mut Cx,
        id: CommandId,
        name: &str,
        params: &Params,
    ) -> bool {
        if !COMMANDS.contains(&name) {
            return false;
        }
        let result = if name == "cancel_transfer" {
            self.cancel_transfers(cx, params)
        } else {
            self.queue_transfer(cx, name, params)
        };
        cx.complete(id, result.map(|value| Outcome::Value { value }));
        true
    }

    fn frame_size_now(&self) -> Result<(u32, u32), CommandError> {
        self.topology
            .video_mode
            .and_then(media::frame_size)
            .ok_or_else(|| {
                invalid("the switcher has not reported a video mode it has frames for".into())
            })
    }

    fn check_clip(&self, name: &str, params: &Params) -> Result<u8, CommandError> {
        if self.topology.clips == 0 {
            return Err(unsupported(name, "this switcher (no clip pool)"));
        }
        self.check_index(params, "clip", self.topology.clips)
    }

    /// A transfer the parameters describe, checked against the switcher.
    fn transfer_job(&self, name: &str, params: &Params) -> Result<Job, CommandError> {
        let path = || opt_str(params, "path").unwrap_or_default().to_owned();
        let mut size = (0, 0);
        let mut paths = vec![path()];
        let what = match name {
            "clear_still" => {
                paths.clear();
                What::ClearStill {
                    slot: self.check_index(params, "still", self.topology.stills)? as u16,
                }
            }
            "clear_clip" => {
                paths.clear();
                What::ClearClip {
                    clip: self.check_clip(name, params)?,
                }
            }
            "upload_still" => {
                let slot = self.check_index(params, "still", self.topology.stills)? as u16;
                size = self.frame_size_now()?;
                What::UploadStill {
                    slot,
                    name: name_param(params, "name", 63)?.unwrap_or_else(|| base_name(&path(), 63)),
                    description: name_param(params, "description", 127)?.unwrap_or_default(),
                }
            }
            "download_still" => {
                let slot = self.check_index(params, "still", self.topology.stills)? as u16;
                if !self.topology.stills_used.contains(&slot) {
                    return Err(invalid(format!("still {} is empty", slot + 1)));
                }
                size = self.frame_size_now()?;
                What::DownloadStill { slot }
            }
            "upload_clip" => {
                let clip = self.check_clip(name, params)?;
                size = self.frame_size_now()?;
                paths = lines(params)?;
                let max = self
                    .topology
                    .clip_max_frames
                    .map(|m| m[clip as usize])
                    .filter(|m| *m > 0);
                if let Some(max) = max {
                    if paths.len() > max as usize {
                        return Err(invalid(format!(
                            "{} frames; clip {} holds {max} (set_clip_lengths shares the clip memory)",
                            paths.len(),
                            clip + 1
                        )));
                    }
                }
                What::UploadClip {
                    clip,
                    name: name_param(params, "name", 43)?.unwrap_or_default(),
                }
            }
            "download_clip" => {
                let clip = self.check_clip(name, params)?;
                size = self.frame_size_now()?;
                paths = lines(params)?;
                let frames = self.topology.clip_frames.get(&clip).copied().unwrap_or(0);
                if frames == 0 {
                    return Err(invalid(format!("clip {} is empty", clip + 1)));
                }
                let first = opt_int(params, "first_frame").unwrap_or(1).max(1);
                let last = first + paths.len() as i64 - 1;
                if last > frames as i64 {
                    return Err(invalid(format!(
                        "frames {first} to {last} asked; clip {} has {frames}",
                        clip + 1
                    )));
                }
                What::DownloadClip {
                    clip,
                    first: (first - 1) as u16,
                }
            }
            "upload_clip_audio" => {
                let clip = self.check_clip(name, params)?;
                What::UploadAudio {
                    clip,
                    name: name_param(params, "name", 63)?.unwrap_or_else(|| base_name(&path(), 63)),
                }
            }
            "upload_macro" => {
                let slot = self.check_index(params, "macro", self.topology.macros)? as u16;
                What::UploadMacro {
                    slot,
                    name: name_param(params, "name", 63)?.unwrap_or_default(),
                }
            }
            "download_macro" => {
                let slot = self.check_index(params, "macro", self.topology.macros)? as u16;
                if self.topology.macros_used.get(&slot) != Some(&true) {
                    return Err(invalid(format!("macro {} is empty", slot + 1)));
                }
                What::DownloadMacro { slot }
            }
            _ => {
                // Switchers from the ATEM Mini on draw their own labels
                // (Sofie `lib/multiviewLabel.ts:365-367`).
                if self.topology.model_id >= 13 {
                    return Err(unsupported(
                        name,
                        "this switcher (it draws its own multiviewer labels)",
                    ));
                }
                let text = if name == "set_multiviewer_label" {
                    let resolutions = self.topology.label_resolutions;
                    if !resolutions.any() {
                        return Err(invalid(
                            "the switcher has reported no multiviewer resolution a label is drawn for"
                                .into(),
                        ));
                    }
                    Some((
                        opt_str(params, "text").unwrap_or_default().to_owned(),
                        resolutions,
                    ))
                } else {
                    None
                };
                What::UploadLabel {
                    source: self.check_source(params)?,
                    text,
                }
            }
        };
        Ok(Job {
            number: 0,
            what,
            format: format_param(params),
            paths,
            size,
            unit: 0,
            retries: 0,
            data: Vec::new(),
            raw_len: 0,
            hash: [0; 16],
            rle: false,
            sent: 0,
            described: false,
            writes: 0,
            write_error: None,
            bytes: 0,
            chunks: 0,
        })
    }

    fn queue_transfer(
        &mut self,
        cx: &mut Cx,
        name: &str,
        params: &Params,
    ) -> Result<Value, CommandError> {
        let mut job = self.transfer_job(name, params)?;
        let t = &mut self.transfers;
        t.numbers += 1;
        job.number = t.numbers;
        let mut entry = job.what.target();
        entry.insert("kind".into(), json!(job.what.kind()));
        entry.insert("status".into(), json!("queued"));
        entry.insert("progress".into(), json!(0.0));
        entry.insert("frames".into(), json!(job.paths.len()));
        entry.insert("error".into(), Value::Null);
        cx.state(json!({"transfers": {job.number.to_string(): entry}}));
        let number = job.number;
        t.queue.push_back(job);
        let queued = t.queue.len() + t.active.is_some() as usize - 1;
        self.pump_transfers(cx);
        Ok(json!({"transfer": number, "ahead": queued}))
    }

    fn cancel_transfers(&mut self, cx: &mut Cx, params: &Params) -> Result<Value, CommandError> {
        let only = opt_int(params, "transfer").map(|n| n as u64);
        let mut canceled = Vec::new();
        let queued: Vec<Job> = {
            let t = &mut self.transfers;
            let (out, keep): (VecDeque<Job>, VecDeque<Job>) = t
                .queue
                .drain(..)
                .partition(|j| only.is_none_or(|n| n == j.number));
            t.queue = keep;
            out.into_iter().collect()
        };
        for job in queued {
            canceled.push(job.number);
            self.finish_transfer(cx, job, "canceled", None);
        }
        let active = self.transfers.active.as_ref().map(|j| j.number);
        if let Some(n) = active.filter(|n| only.is_none_or(|o| o == *n)) {
            canceled.push(n);
            self.end_active(cx, "canceled", None);
        }
        if let Some(n) = only {
            if canceled.is_empty() {
                return Err(invalid(format!("transfer {n} is not queued or running")));
            }
        }
        canceled.sort_unstable();
        Ok(json!({"canceled": canceled}))
    }

    fn transfer_patch(&self, cx: &mut Cx, job: &Job, status: &str) {
        let mut entry = Map::new();
        entry.insert("status".into(), json!(status));
        entry.insert(
            "progress".into(),
            json!((job.progress() * 1000.0).round() / 1000.0),
        );
        if job.paths.len() > 1 {
            entry.insert("frame".into(), json!(job.unit + 1));
        }
        cx.state(json!({"transfers": {job.number.to_string(): entry}}));
    }

    /// Start the next transfer, if none is on the wire.
    pub(super) fn pump_transfers(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Ready || self.transfers.active.is_some() {
            return;
        }
        let Some(job) = self.transfers.queue.pop_front() else {
            return;
        };
        let mut args = job.what.target();
        args.insert("transfer".into(), json!(job.number));
        args.insert("kind".into(), json!(job.what.kind()));
        cx.message("transfer_started", None, Value::Object(args), self.device);
        self.transfers.active = Some(job);
        self.begin_unit(cx);
    }

    fn begin_unit(&mut self, cx: &mut Cx) {
        // Frames are the size of the video mode now, which may have changed
        // since the transfer was asked for.
        let size = self.topology.video_mode.and_then(media::frame_size);
        let Some(job) = self.transfers.active.as_mut() else {
            return;
        };
        if job.what.frames() {
            match size {
                Some(s) => job.size = s,
                None => {
                    self.end_active(
                        cx,
                        "failed",
                        Some("the switcher's video mode has no frame size known here".into()),
                    );
                    return;
                }
            }
        }
        job.data.clear();
        job.sent = 0;
        job.described = false;
        job.chunks = 0;
        if let What::UploadLabel {
            text: Some((text, resolutions)),
            ..
        } = &job.what
        {
            // Drawn off the session, then sent as an uploaded label is.
            let (text, resolutions) = (text.clone(), *resolutions);
            self.transfers.step = Some(Step::Preparing);
            cx.compute(PREPARE, move || {
                encode_upload(Prep::Label, label::render(&text, resolutions)?)
            });
            let job = self.transfers.active.as_ref().unwrap();
            self.transfer_patch(cx, job, "reading");
        } else if job.what.upload() {
            // Whatever its size: a picture is checked against the video
            // mode once decoded, and anything else the switcher refuses is
            // the transfer's error.
            cx.file_read(READ, job.paths[job.unit].clone());
            self.transfers.step = Some(Step::Reading);
            let job = self.transfers.active.as_ref().unwrap();
            self.transfer_patch(cx, job, "reading");
        } else {
            self.acquire(cx);
        }
    }

    /// What converting the active upload's file needs, captured for a task.
    fn prep(&self, job: &Job) -> Prep {
        match job.what {
            What::UploadStill { .. } | What::UploadClip { .. } => Prep::Frame {
                format: job.format,
                size: job.size,
            },
            // Unknown models are taken as newer, as Sofie does.
            What::UploadAudio { .. } => Prep::Audio {
                reversed: self.topology.model_id == 0 || self.topology.model_id >= 4,
            },
            What::UploadMacro { .. } => Prep::Macro,
            _ => Prep::Label,
        }
    }

    /// Take the lock the active transfer needs, or start it when it needs
    /// none or holds it already.
    fn acquire(&mut self, cx: &mut Cx) {
        let Some(job) = self.transfers.active.as_ref() else {
            return;
        };
        let Some(store) = job.what.lock() else {
            self.start_unit(cx);
            return;
        };
        let t = &mut self.transfers;
        if t.held == Some(store) {
            self.start_unit(cx);
            return;
        }
        if let Some(other) = t.held {
            self.release(cx, other);
        }
        let now = cx.now();
        let t = &mut self.transfers;
        if let Some((s, at)) = t.released {
            let foreign = t.locks.get(&store) == Some(&true);
            if s == store && (now < at + COURTESY || (foreign && now < at + LOCK_QUIET)) {
                t.step = Some(Step::Yielding);
                let job = self.transfers.active.as_ref().unwrap();
                self.transfer_patch(cx, job, "waiting");
                self.arm_transfer_timer(cx);
                return;
            }
        }
        t.released = None;
        t.asked = Some(store);
        t.step = Some(Step::Locking { since: now });
        self.send_commands(cx, None, &[(*b"LOCK", lock_body(store, true))]);
        let job = self.transfers.active.as_ref().unwrap();
        self.transfer_patch(cx, job, "locking");
        self.arm_transfer_timer(cx);
    }

    fn release(&mut self, cx: &mut Cx, store: u16) {
        self.send_commands(cx, None, &[(*b"LOCK", lock_body(store, false))]);
        let t = &mut self.transfers;
        if t.held == Some(store) {
            t.held = None;
        }
        t.released = Some((store, cx.now()));
    }

    fn start_unit(&mut self, cx: &mut Cx) {
        let clear = match self.transfers.active.as_ref().map(|j| &j.what) {
            // Slot, 3 rsv (Sofie `Media/MediaPoolClearStillCommand.ts:10-14`
            // and `MediaPoolClearClipCommand.ts`).
            Some(What::ClearStill { slot }) => Some((*b"CSTL", *slot as u8)),
            Some(What::ClearClip { clip }) => Some((*b"CMPC", *clip)),
            _ => None,
        };
        if let Some((name, index)) = clear {
            // Sent under the lock; the release goes in a later packet, which
            // the switcher takes after this one.
            self.send_commands(cx, None, &[(name, vec![index, 0, 0, 0])]);
            self.end_active(cx, "done", None);
            return;
        }
        let t = &mut self.transfers;
        let id = t
            .next_id
            .unwrap_or(((cx.unix_millis() as u16) ^ 0x5A5A) | 1);
        t.next_id = Some(id.wrapping_add(1));
        t.id = id;
        let job = t.active.as_mut().unwrap();
        let store = job.what.store();
        let index = job.what.index(job.unit);
        let mut out: Out = Vec::new();
        if let (What::UploadClip { clip, .. }, 0) = (&job.what, job.unit) {
            out.push((*b"CMPC", vec![*clip, 0, 0, 0]));
        }
        if job.what.upload() {
            // u16 id, u16 store, 2 rsv, u16 index, u32 size, u16 mode (Sofie
            // `DataTransferUploadRequestCommand.ts:18-26`).
            let mode = match job.what {
                What::UploadAudio { .. } => MODE_AUDIO,
                What::UploadMacro { .. } => MODE_MACRO,
                What::UploadLabel { .. } => MODE_LABEL,
                _ => MODE_FRAME,
            };
            let mut b = vec![0u8; 16];
            put16(&mut b, 0, id);
            put16(&mut b, 2, store);
            put16(&mut b, 6, index);
            put32(&mut b, 8, job.raw_len);
            put16(&mut b, 12, mode);
            out.push((*b"FTSD", b));
        } else {
            // u16 id, u16 store, 2 rsv, u16 index, u16 type (Sofie
            // `DataTransferDownloadRequestCommand.ts:15-24`).
            let kind = match job.what {
                What::DownloadMacro { .. } => TYPE_MACRO,
                _ => TYPE_FRAME,
            };
            let mut b = vec![0u8; 12];
            put16(&mut b, 0, id);
            put16(&mut b, 2, store);
            put16(&mut b, 6, index);
            put16(&mut b, 8, kind);
            out.push((*b"FTSU", b));
        }
        t.step = Some(Step::Started);
        t.activity = cx.now();
        self.send_commands(cx, None, &out);
        let job = self.transfers.active.as_ref().unwrap();
        self.transfer_patch(cx, job, "transferring");
        self.arm_transfer_timer(cx);
    }

    fn arm_transfer_timer(&mut self, cx: &mut Cx) {
        let t = &self.transfers;
        let now = cx.now();
        let at = match t.step {
            Some(Step::Yielding) => match t.released {
                Some((s, at)) if t.locks.get(&s) == Some(&true) => at + LOCK_QUIET,
                Some((_, at)) => at + COURTESY,
                None => now,
            },
            Some(Step::Locking { since }) => since + LOCK_WAIT,
            Some(Step::Started) | Some(Step::Moving) => t.activity + STALL,
            _ => {
                cx.cancel_timer(TIMER);
                return;
            }
        };
        cx.set_timer(TIMER, at.saturating_sub(now).max(1));
    }

    pub(super) fn transfer_timer(&mut self, cx: &mut Cx) {
        let now = cx.now();
        match self.transfers.step {
            Some(Step::Yielding) => self.acquire(cx),
            Some(Step::Locking { since }) if now >= since + LOCK_WAIT => self.end_active(
                cx,
                "failed",
                Some(format!(
                    "the switcher did not grant the lock within {} s; another client holds it",
                    LOCK_WAIT / 1000
                )),
            ),
            Some(Step::Started) | Some(Step::Moving) if now >= self.transfers.activity + STALL => {
                self.end_active(
                    cx,
                    "failed",
                    Some("the switcher stopped answering the transfer".into()),
                )
            }
            _ => self.arm_transfer_timer(cx),
        }
    }

    /// Take the active transfer off the wire: release what it holds and
    /// report how it ended.
    fn end_active(&mut self, cx: &mut Cx, status: &str, error: Option<String>) {
        let Some(job) = self.transfers.active.take() else {
            return;
        };
        if let Some(store) = self.transfers.held {
            self.release(cx, store);
        }
        if let Some(store) = self.transfers.asked.take() {
            // Withdraw the request; a grant that still comes is released.
            self.send_commands(cx, None, &[(*b"LOCK", lock_body(store, false))]);
        }
        self.transfers.step = None;
        cx.cancel_timer(TIMER);
        if job.writes > 0 && status == "done" {
            self.transfer_patch(cx, &job, "writing");
            self.transfers.draining.insert(job.number, job);
        } else {
            self.finish_transfer(cx, job, status, error);
        }
        self.pump_transfers(cx);
    }

    fn finish_transfer(&mut self, cx: &mut Cx, mut job: Job, status: &str, error: Option<String>) {
        let (status, error) = match (status, job.write_error.take()) {
            ("done", Some(e)) => ("failed", Some(e)),
            (s, _) => (s, error),
        };
        if status == "failed" {
            cx.log(
                Level::Warning,
                format!(
                    "transfer {} ({}) failed: {}",
                    job.number,
                    job.what.kind(),
                    error.as_deref().unwrap_or("")
                ),
            );
        }
        let mut entry = Map::new();
        entry.insert("status".into(), json!(status));
        if status == "done" {
            entry.insert("progress".into(), json!(1.0));
        }
        entry.insert("error".into(), json!(error));
        entry.insert("bytes".into(), json!(job.bytes));
        let key = job.number.to_string();
        cx.state(json!({"transfers": {key: entry}}));
        let mut args = job.what.target();
        args.insert("transfer".into(), json!(job.number));
        args.insert("kind".into(), json!(job.what.kind()));
        args.insert("status".into(), json!(status));
        args.insert("error".into(), json!(error));
        args.insert("bytes".into(), json!(job.bytes));
        if job.what.frames() {
            args.insert("width".into(), json!(job.size.0));
            args.insert("height".into(), json!(job.size.1));
        }
        cx.message("transfer_finished", None, Value::Object(args), self.device);
        let t = &mut self.transfers;
        t.finished.push_back(job.number);
        while t.finished.len() > KEEP_FINISHED {
            let old = t.finished.pop_front().unwrap();
            cx.state(json!({"transfers": {old.to_string(): null}}));
        }
    }

    /// A host file read or written for a transfer.
    pub(super) fn transfer_file(&mut self, cx: &mut Cx, key: Key, input: FileInput) {
        if key == READ {
            if self.transfers.step != Some(Step::Reading) {
                return;
            }
            let Some(job) = self.transfers.active.as_ref() else {
                return;
            };
            match input {
                FileInput::Read { data } => {
                    let prep = self.prep(job);
                    self.transfers.step = Some(Step::Preparing);
                    cx.compute(PREPARE, move || encode_upload(prep, data));
                }
                FileInput::Failed { message } => self.end_active(cx, "failed", Some(message)),
                _ => {}
            }
            return;
        }
        let Some(number) = self.transfers.writing.remove(key) else {
            return;
        };
        let failed = match input {
            FileInput::Failed { message } => Some(message),
            FileInput::Closed { .. } => None,
            _ => {
                self.transfers.writing.insert(key, number);
                return;
            }
        };
        self.file_stored(cx, number, failed);
    }

    /// A task this module handed off finished: an upload's file prepared,
    /// or a downloaded unit converted for its file.
    pub(super) fn transfer_computed(
        &mut self,
        cx: &mut Cx,
        key: Key,
        result: Result<Vec<u8>, String>,
    ) {
        if key == PREPARE {
            if self.transfers.step != Some(Step::Preparing) {
                return;
            }
            let Some(job) = self.transfers.active.as_mut() else {
                return;
            };
            match result {
                Ok(mut packed) if packed.len() >= 21 => {
                    job.raw_len = u32::from_be_bytes(packed[0..4].try_into().unwrap());
                    job.hash.copy_from_slice(&packed[4..20]);
                    job.rle = packed[20] == 1;
                    packed.drain(..21);
                    job.data = packed;
                    self.acquire(cx);
                }
                Ok(_) => self.end_active(cx, "failed", Some("nothing to send".into())),
                Err(e) => self.end_active(cx, "failed", Some(e)),
            }
            return;
        }
        let Some(slot) = CONVERTERS.iter().position(|k| *k == key) else {
            return;
        };
        let Some((number, path)) = self.transfers.converting.remove(key) else {
            return;
        };
        let alive = self.transfers.draining.contains_key(&number)
            || self
                .transfers
                .active
                .as_ref()
                .is_some_and(|j| j.number == number);
        if alive {
            match result {
                Ok(file) => {
                    let writer = WRITERS[slot];
                    cx.file_open(writer, path);
                    cx.file_write(writer, file);
                    cx.file_close(writer);
                    self.transfers.writing.insert(writer, number);
                }
                Err(e) => self.file_stored(cx, number, Some(e)),
            }
        }
        if self.transfers.step == Some(Step::WaitingWriter) {
            self.store_download(cx);
        }
    }

    /// One downloaded file written, or not.
    fn file_stored(&mut self, cx: &mut Cx, number: u64, failed: Option<String>) {
        if let Some(job) = self.transfers.draining.get_mut(&number) {
            job.writes -= 1;
            if failed.is_some() {
                job.write_error = failed;
            }
            if job.writes == 0 {
                let job = self.transfers.draining.remove(&number).unwrap();
                self.finish_transfer(cx, job, "done", None);
            }
        } else if let Some(job) = self
            .transfers
            .active
            .as_mut()
            .filter(|j| j.number == number)
        {
            job.writes -= 1;
            if let Some(e) = failed {
                self.end_active(cx, "failed", Some(e));
                return;
            }
        }
        if self.transfers.step == Some(Step::WaitingWriter) {
            self.store_download(cx);
        }
    }

    /// Lock and transfer messages from the switcher.
    pub(super) fn decode_transfer(
        &mut self,
        cx: &mut Cx,
        name: &[u8; 4],
        b: &[u8],
    ) -> Option<Value> {
        let need = |n: usize| (b.len() >= n).then_some(());
        match name {
            // u16 store, u8 locked (Sofie `LockStateCommand.ts:33-40`).
            b"LKST" => {
                need(3)?;
                let (store, locked) = (u16_at(b, 0), b[2] == 1);
                if store >= 100 {
                    return None;
                }
                self.transfers.locks.insert(store, locked);
                if !locked && self.transfers.step == Some(Step::Yielding) {
                    if let Some((s, _)) = self.transfers.released {
                        if s == store {
                            // Free now: one frame's grace, then ask.
                            self.transfers.released = Some((s, cx.now()));
                            self.arm_transfer_timer(cx);
                        }
                    }
                }
                Some(if store == 0 {
                    json!({"media_pool": {"locks": {"stills": locked}}})
                } else {
                    json!({"media_pool": {"locks": {"clips": {store.to_string(): locked}}}})
                })
            }
            // u16 store (Sofie `LockObtainedCommand.ts:13-17`).
            b"LKOB" => {
                need(2)?;
                let store = u16_at(b, 0);
                let t = &mut self.transfers;
                if t.asked == Some(store) {
                    t.asked = None;
                    t.held = Some(store);
                    if matches!(t.step, Some(Step::Locking { .. })) {
                        self.start_unit(cx);
                    }
                } else if t.held != Some(store) {
                    // A grant nobody here waits for (a request withdrawn or
                    // timed out, then served from the switcher's queue).
                    self.send_commands(cx, None, &[(*b"LOCK", lock_body(store, false))]);
                }
                None
            }
            // u16 id, 4 rsv, u16 chunk size, u16 chunk count (Sofie
            // `DataTransferUploadContinueCommand.ts:13-19`).
            b"FTCD" => {
                need(10)?;
                if !self.active_id(u16_at(b, 0), true) {
                    return None;
                }
                self.send_chunks(cx, u16_at(b, 6) as usize, u16_at(b, 8) as usize);
                None
            }
            // u16 id, u16 size, the data (Sofie `DataTransferDataCommand.ts:29-35`).
            b"FTDa" => {
                need(4)?;
                let id = u16_at(b, 0);
                if !self.active_id(id, false) {
                    return None;
                }
                let end = (4 + u16_at(b, 2) as usize).min(b.len());
                self.receive_chunk(cx, &b[4..end]);
                None
            }
            // u16 id (Sofie `DataTransferCompleteCommand.ts:6-12`).
            b"FTDC" => {
                need(2)?;
                let id = u16_at(b, 0);
                let t = &self.transfers;
                let job = t.active.as_ref()?;
                let ours = t.id == id
                    && t.step == Some(Step::Moving)
                    && if job.what.upload() {
                        job.sent >= job.data.len()
                    } else {
                        !job.data.is_empty()
                    };
                if ours {
                    self.unit_done(cx);
                }
                None
            }
            // u16 id, u8 code (Sofie `DataTransferErrorCommand.ts:17-24`).
            b"FTDE" => {
                need(3)?;
                let t = &self.transfers;
                if t.active.is_none()
                    || t.id != u16_at(b, 0)
                    || !matches!(t.step, Some(Step::Started) | Some(Step::Moving))
                {
                    return None;
                }
                self.transfer_error(cx, b[2]);
                None
            }
            _ => None,
        }
    }

    fn active_id(&self, id: u16, upload: bool) -> bool {
        let t = &self.transfers;
        t.id == id
            && matches!(t.step, Some(Step::Started) | Some(Step::Moving))
            && t.active.as_ref().is_some_and(|j| j.what.upload() == upload)
    }

    fn send_chunks(&mut self, cx: &mut Cx, chunk: usize, count: usize) {
        let t = &mut self.transfers;
        t.step = Some(Step::Moving);
        t.activity = cx.now();
        let id = t.id;
        let job = t.active.as_mut().unwrap();
        let mut out: Vec<([u8; 4], Vec<u8>)> = Vec::new();
        if !job.described {
            // u16 id, name[64], description[128], MD5[16], padded to 212
            // (Sofie `DataTransferFileDescriptionCommand.ts:15-24`).
            job.described = true;
            let (name, description) = match &job.what {
                What::UploadStill {
                    name, description, ..
                } => (name.as_str(), description.as_str()),
                What::UploadAudio { name, .. } | What::UploadMacro { name, .. } => {
                    (name.as_str(), "")
                }
                What::UploadLabel { .. } => ("Label", ""),
                _ => ("", ""),
            };
            let mut b = vec![0u8; 212];
            put16(&mut b, 0, id);
            b[2..2 + name.len()].copy_from_slice(name.as_bytes());
            b[66..66 + description.len()].copy_from_slice(description.as_bytes());
            b[194..210].copy_from_slice(&job.hash);
            out.push((*b"FTFD", b));
        }
        // Whole 8-byte blocks, as Sofie takes them, and no more than a
        // datagram holds.
        let size = chunk.min(MAX_CHUNK) / 8 * 8;
        if size > 0 {
            for _ in 0..count {
                if job.sent >= job.data.len() {
                    break;
                }
                let end = media::chunk_end(&job.data, job.sent, size, job.rle);
                let mut b = vec![0u8; 4];
                put16(&mut b, 0, id);
                put16(&mut b, 2, (end - job.sent) as u16);
                b.extend_from_slice(&job.data[job.sent..end]);
                job.bytes += (end - job.sent) as u64;
                job.sent = end;
                out.push((*b"FTDa", b));
            }
        }
        for c in out {
            self.send_commands(cx, None, &[c]);
        }
        let job = self.transfers.active.as_ref().unwrap();
        self.transfer_patch(cx, job, "transferring");
        self.arm_transfer_timer(cx);
    }

    fn receive_chunk(&mut self, cx: &mut Cx, data: &[u8]) {
        let now = cx.now();
        let t = &mut self.transfers;
        t.step = Some(Step::Moving);
        t.activity = now;
        let id = t.id;
        let job = t.active.as_mut().unwrap();
        job.data.extend_from_slice(data);
        job.bytes += data.len() as u64;
        job.chunks += 1;
        let index = job.what.index(job.unit) as u8;
        // A frame's data, run-length encoded, is never longer than the
        // frame: more means the stream is broken. A macro has no such bound.
        let over = job.what.frames() && job.data.len() > job.frame_bytes() + 64;
        let report = job.chunks % 256 == 1;
        t.acks.push((id, index));
        if over {
            self.end_active(
                cx,
                "failed",
                Some("the switcher sent more data than the file can hold".into()),
            );
            return;
        }
        if report {
            let job = self.transfers.active.as_ref().unwrap();
            self.transfer_patch(cx, job, "transferring");
        }
    }

    /// Send the FTUA acknowledgements of the packet just read: u16 id, u8
    /// index (Sofie `DataTransferAckCommand.ts:25-31`).
    pub(super) fn flush_transfer_acks(&mut self, cx: &mut Cx) {
        let acks = std::mem::take(&mut self.transfers.acks);
        for batch in acks.chunks(ACKS_PER_PACKET) {
            let out: Out = batch
                .iter()
                .map(|(id, index)| {
                    let mut b = vec![0u8; 4];
                    put16(&mut b, 0, *id);
                    b[2] = *index;
                    (*b"FTUA", b)
                })
                .collect();
            self.send_commands(cx, None, &out);
        }
        if !acks.is_empty() {
            self.arm_transfer_timer(cx);
        }
    }

    fn transfer_error(&mut self, cx: &mut Cx, code: u8) {
        let job = self.transfers.active.as_mut().unwrap();
        job.retries += 1;
        let exhausted = job.retries > RETRIES;
        let lock = job.what.lock();
        match code {
            1 | 5 | 6 if !exhausted => {
                cx.log(
                    Level::Debug,
                    format!(
                        "transfer {}: switcher error {code}, trying again",
                        job.number
                    ),
                );
                if !job.what.upload() {
                    job.data.clear();
                }
                job.sent = 0;
                job.described = false;
                job.chunks = 0;
                match (code, lock) {
                    // Retry as it stands.
                    (1, _) | (_, None) => self.start_unit(cx),
                    // Not locked: let go of what this client thought it
                    // held, and ask again.
                    (5, Some(store)) => {
                        self.release(cx, store);
                        self.acquire(cx);
                    }
                    // Another client won: let go and wait for a turn.
                    (_, Some(store)) => {
                        self.release(cx, store);
                        self.transfers.locks.insert(store, true);
                        self.acquire(cx);
                    }
                }
            }
            _ => {
                let why = match code {
                    1 | 5 | 6 => format!(
                        "the switcher refused it {} times (error {code})",
                        RETRIES + 1
                    ),
                    2 => "the switcher has nothing there (error 2)".into(),
                    _ => format!("the switcher refused it (error {code})"),
                };
                self.end_active(cx, "failed", Some(why));
            }
        }
    }

    fn unit_done(&mut self, cx: &mut Cx) {
        let job = self.transfers.active.as_mut().unwrap();
        job.retries = 0;
        if !job.what.upload() {
            self.store_download(cx);
            return;
        }
        job.unit += 1;
        if job.unit < job.paths.len() {
            self.begin_unit(cx);
            return;
        }
        if let What::UploadClip { clip, name } = &job.what {
            // Name the clip and make it valid: 3, clip, name at 2, u16 frames
            // at 66 (Sofie `MediaPoolSetClipCommand.ts:15-25`).
            let mut b = vec![0u8; 68];
            b[0] = 3;
            b[1] = *clip;
            b[2..2 + name.len()].copy_from_slice(name.as_bytes());
            put16(&mut b, 66, job.paths.len() as u16);
            self.send_commands(cx, None, &[(*b"SMPC", b)]);
        }
        self.end_active(cx, "done", None);
    }

    /// Hand the downloaded unit to a converter for its host file, and go on.
    /// A slot (converter, then writer) is held from conversion until the
    /// file is closed; with all four busy, the next unit waits.
    fn store_download(&mut self, cx: &mut Cx) {
        let t = &mut self.transfers;
        let Some(slot) = (0..CONVERTERS.len()).find(|i| {
            !t.converting.contains_key(CONVERTERS[*i]) && !t.writing.contains_key(WRITERS[*i])
        }) else {
            t.step = Some(Step::WaitingWriter);
            return;
        };
        let job = t.active.as_mut().unwrap();
        let data = std::mem::take(&mut job.data);
        let (frames, format, size) = (job.what.frames(), job.format, job.size);
        job.writes += 1;
        t.converting
            .insert(CONVERTERS[slot], (job.number, job.paths[job.unit].clone()));
        cx.compute(CONVERTERS[slot], move || {
            decode_download(frames, format, size, data)
        });
        job.unit += 1;
        if job.unit < job.paths.len() {
            // The lock stays held for the clip's next frame.
            self.start_unit(cx);
        } else {
            self.end_active(cx, "done", None);
        }
    }

    /// The connection is gone: what was on the wire failed, and the
    /// switcher frees a dead session's locks itself. Queued transfers wait
    /// for the next connection.
    pub(super) fn transfers_lost(&mut self, cx: &mut Cx, reason: &str) {
        let t = &mut self.transfers;
        t.held = None;
        t.asked = None;
        t.released = None;
        t.locks.clear();
        t.acks.clear();
        if let Some(job) = t.active.take() {
            t.step = None;
            cx.cancel_timer(TIMER);
            self.finish_transfer(
                cx,
                job,
                "failed",
                Some(format!("connection lost: {reason}")),
            );
        }
    }

    /// The session is closing: let go of any lock held or asked for.
    pub(super) fn transfers_stop(&mut self, cx: &mut Cx) {
        if self.phase == Phase::Hello {
            return;
        }
        let t = &self.transfers;
        let stores: Vec<u16> = t.held.into_iter().chain(t.asked).collect();
        for store in stores {
            self.send_commands(cx, None, &[(*b"LOCK", lock_body(store, false))]);
        }
        self.transfers.held = None;
        self.transfers.asked = None;
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{cmd, dump, feed, ready, ready_with, reliable, sent, state};
    use super::*;
    use crate::module::{Action, Module};
    use std::net::SocketAddr;

    const W: u32 = 640;
    const H: u32 = 480;

    /// The commands in the packets of some actions, by name.
    fn out(actions: &[Action]) -> Vec<([u8; 4], Vec<u8>)> {
        let mut list = Vec::new();
        for p in sent(actions) {
            // Commands go in reliable packets; the hello is not one.
            if p[0] >> 3 & 0x01 == 0 {
                continue;
            }
            let mut at = 12;
            while at + 8 <= p.len() {
                let len = u16_at(&p, at) as usize;
                assert!(len >= 8 && at + len <= p.len(), "a malformed command");
                let name: [u8; 4] = p[at + 4..at + 8].try_into().unwrap();
                list.push((name, p[at + 8..at + len].to_vec()));
                at += len;
            }
        }
        list
    }

    fn named<'a>(list: &'a [([u8; 4], Vec<u8>)], name: &[u8; 4]) -> Vec<&'a Vec<u8>> {
        list.iter()
            .filter(|(n, _)| n == name)
            .map(|(_, b)| b)
            .collect()
    }

    fn messages(actions: &[Action]) -> Vec<(String, Value)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Message { address, args, .. } => Some((address.clone(), args.clone())),
                _ => None,
            })
            .collect()
    }

    /// A switcher packet with the next id.
    fn from_switcher(m: &mut Atem, now: Millis, commands: &[Vec<u8>]) -> Vec<Action> {
        let id = (m.last_received + 1) % super::super::ID_MODULO;
        let a = feed(m, now, reliable(0x8001, id, commands));
        settle(m, now, a)
    }

    fn run(
        m: &mut Atem,
        now: Millis,
        name: &str,
        params: Value,
    ) -> (Result<Value, CommandError>, Vec<Action>) {
        let mut cx = Cx::new(now);
        m.command(&mut cx, 7, name, params.as_object().unwrap());
        let a = cx.take();
        let result = a
            .iter()
            .find_map(|x| match x {
                Action::Complete { result, .. } => Some(result.clone()),
                _ => None,
            })
            .expect("the command completes");
        (
            result.map(|o| match o {
                Outcome::Value { value } => value,
                other => panic!("{other:?}"),
            }),
            a,
        )
    }

    fn file(m: &mut Atem, now: Millis, key: Key, input: FileInput) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.file(&mut cx, key, input);
        settle(m, now, cx.take())
    }

    /// Run the tasks the actions hand off, as the session would, and add
    /// what their results lead to.
    fn settle(m: &mut Atem, now: Millis, mut actions: Vec<Action>) -> Vec<Action> {
        let mut at = 0;
        while at < actions.len() {
            if let Action::Compute { key, task } = &actions[at] {
                let (key, result) = (*key, task.run());
                let mut cx = Cx::new(now);
                m.computed(&mut cx, key, result);
                actions.extend(cx.take());
            }
            at += 1;
        }
        actions
    }

    fn timer(m: &mut Atem, now: Millis) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.timer(&mut cx, TIMER);
        cx.take()
    }

    fn read_of(actions: &[Action]) -> Option<String> {
        actions.iter().find_map(|a| match a {
            Action::FileRead { path, .. } => Some(path.clone()),
            _ => None,
        })
    }

    /// A small switcher in 525-line SD (640 by 480 frames), with still 1
    /// used, clip 1 holding 3 frames and macro 1 used.
    fn switcher() -> Atem {
        let (mut m, _) = ready();
        let mut mpfe = vec![0u8; 24];
        mpfe[4] = 1;
        mpfe[23] = 4;
        mpfe.extend_from_slice(b"logo");
        let mut mpcs = vec![0u8; 68];
        mpcs[1] = 1;
        put16(&mut mpcs, 66, 3);
        from_switcher(
            &mut m,
            25,
            &[
                cmd(b"VidM", &[0, 0, 0, 0]),
                cmd(b"MPfe", &mpfe),
                cmd(b"MPCS", &mpcs),
                cmd(b"MPSp", &[0, 4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0]),
            ],
        );
        m
    }

    fn picture() -> Vec<u8> {
        let mut rgba = vec![0u8; (W * H * 4) as usize];
        for (i, px) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            px.copy_from_slice(&[(i % 256) as u8, 128, 30, 255]);
        }
        rgba
    }

    fn ftcd(id: u16, size: u16, count: u16) -> Vec<u8> {
        let mut b = vec![0u8; 12];
        put16(&mut b, 0, id);
        put16(&mut b, 6, size);
        put16(&mut b, 8, count);
        cmd(b"FTCD", &b)
    }

    fn id_cmd(name: &[u8; 4], id: u16, extra: &[u8]) -> Vec<u8> {
        let mut b = id.to_be_bytes().to_vec();
        b.extend_from_slice(extra);
        b.resize(b.len().max(4), 0);
        cmd(name, &b)
    }

    fn lock_msg(name: &[u8; 4], store: u16, locked: bool) -> Vec<u8> {
        cmd(name, &[(store >> 8) as u8, store as u8, locked as u8, 0])
    }

    /// Upload a still through to its first budget; the transfer id.
    fn upload_to_lock(m: &mut Atem, picture: Vec<u8>) -> u16 {
        let (r, a) = run(
            m,
            100,
            "upload_still",
            json!({"still": 2, "path": "/img/a.png"}),
        );
        assert_eq!(r.unwrap(), json!({"transfer": 1, "ahead": 0}));
        assert_eq!(read_of(&a).as_deref(), Some("/img/a.png"));
        let a = file(m, 110, READ, FileInput::Read { data: picture });
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 1, 0]]);
        let a = from_switcher(m, 120, &[lock_msg(b"LKOB", 0, true)]);
        let ftsd = named(&out(&a), b"FTSD")[0].clone();
        assert_eq!(u16_at(&ftsd, 2), 0, "the stills store");
        assert_eq!(u16_at(&ftsd, 6), 1, "still 2");
        assert_eq!(super::super::u32_at(&ftsd, 8), W * H * 4);
        assert_eq!(u16_at(&ftsd, 12), 1);
        u16_at(&ftsd, 0)
    }

    #[test]
    fn a_png_still_is_uploaded_under_the_stills_lock() {
        let mut m = switcher();
        let rgba = picture();
        let id = upload_to_lock(&mut m, media::png_encode(W, H, &rgba));

        // The first budget brings the description, then the chunks.
        let a = from_switcher(&mut m, 130, &[ftcd(id, 1396, 3)]);
        let list = out(&a);
        let ftfd = named(&list, b"FTFD")[0];
        assert_eq!(u16_at(ftfd, 0), id);
        assert_eq!(&ftfd[2..8], b"a.png\0");
        let yuva = media::rgba_to_yuva(W, H, &rgba);
        assert_eq!(ftfd[194..210], md5(&yuva));
        let chunks = named(&list, b"FTDa");
        assert_eq!(chunks.len(), 3);
        // Each in its own datagram, none larger than one holds.
        assert_eq!(sent(&a).iter().filter(|p| p.len() > 12).count(), 4);
        assert!(sent(&a).iter().all(|p| p.len() <= MAX_PACKET));
        let mut data: Vec<u8> = chunks.iter().flat_map(|c| c[4..].to_vec()).collect();
        assert!(chunks
            .iter()
            .all(|c| c[4..].len() <= 1392 && c[4..].len() % 8 == 0));
        // FTDC before all is sent is someone else's, or early: ignored.
        let a = from_switcher(&mut m, 135, &[id_cmd(b"FTDC", id, &[])]);
        assert!(named(&out(&a), b"LOCK").is_empty());

        let a = from_switcher(&mut m, 140, &[ftcd(id, 1396, 60000)]);
        for c in named(&out(&a), b"FTDa") {
            data.extend_from_slice(&c[4..]);
        }
        assert_eq!(media::rle_decode(&data, yuva.len()).unwrap(), yuva);
        assert!(state(&a)["transfers"]["1"]["progress"].as_f64().unwrap() > 0.99);

        // Another client's FTDC with another id changes nothing.
        let a = from_switcher(&mut m, 145, &[id_cmd(b"FTDC", id.wrapping_add(9), &[])]);
        assert!(named(&out(&a), b"LOCK").is_empty());
        let a = from_switcher(&mut m, 150, &[id_cmd(b"FTDC", id, &[])]);
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 0, 0]], "released");
        let s = state(&a);
        assert_eq!(s["transfers"]["1"]["status"], "done");
        assert_eq!(s["transfers"]["1"]["error"], Value::Null);
        let msg = messages(&a);
        assert_eq!(msg[0].0, "transfer_finished");
        assert_eq!(msg[0].1["status"], "done");
        assert_eq!(msg[0].1["still"], 2);
        assert_eq!(m.transfers.held, None);
    }

    #[test]
    fn a_still_is_downloaded_acknowledged_and_written_as_png() {
        let mut m = switcher();
        let (r, a) = run(
            &mut m,
            100,
            "download_still",
            json!({"still": 1, "path": "/out/s.png"}),
        );
        assert_eq!(r.unwrap()["transfer"], 1);
        assert_eq!(messages(&a)[0].0, "transfer_started");
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 1, 0]]);
        let a = from_switcher(&mut m, 110, &[lock_msg(b"LKOB", 0, true)]);
        let ftsu = named(&out(&a), b"FTSU")[0].clone();
        let id = u16_at(&ftsu, 0);
        assert_eq!(
            (u16_at(&ftsu, 2), u16_at(&ftsu, 6), u16_at(&ftsu, 8)),
            (0, 0, 0x00F9)
        );

        let rgba = picture();
        let rle = media::rle_encode(&media::rgba_to_yuva(W, H, &rgba));
        let mut now = 120;
        for c in rle.chunks(1392) {
            let mut b = id.to_be_bytes().to_vec();
            b.extend_from_slice(&(c.len() as u16).to_be_bytes());
            b.extend_from_slice(c);
            let a = from_switcher(&mut m, now, &[cmd(b"FTDa", &b)]);
            let list = out(&a);
            let acks = named(&list, b"FTUA");
            assert_eq!(acks.len(), 1, "every chunk acknowledged");
            assert_eq!(acks[0][..3], [(id >> 8) as u8, id as u8, 0]);
            now += 1;
        }
        let a = from_switcher(&mut m, now, &[id_cmd(b"FTDC", id, &[])]);
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 0, 0]]);
        let written: Vec<u8> = a
            .iter()
            .find_map(|x| match x {
                Action::FileWrite { data, .. } => Some(data.clone()),
                _ => None,
            })
            .unwrap();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::FileOpen { path, .. } if path == "/out/s.png")));
        let (w, h, back) = media::png_decode(&written).unwrap();
        assert_eq!((w, h), (W, H));
        assert!(back
            .iter()
            .zip(&rgba)
            .all(|(a, b)| (*a as i16 - *b as i16).abs() <= 2));
        assert_eq!(state(&a)["transfers"]["1"]["status"], "writing");

        // Done once the file is written.
        let a = file(
            &mut m,
            now + 5,
            WRITERS[0],
            FileInput::Closed {
                bytes: written.len() as u64,
            },
        );
        assert_eq!(state(&a)["transfers"]["1"]["status"], "done");
    }

    #[test]
    fn a_lock_held_elsewhere_times_out_and_a_late_grant_is_released() {
        let mut m = switcher();
        let _ = run(
            &mut m,
            100,
            "download_still",
            json!({"still": 1, "path": "/s"}),
        );
        let a = from_switcher(&mut m, 110, &[lock_msg(b"LKST", 0, true)]);
        assert_eq!(state(&a)["media_pool"]["locks"]["stills"], true);
        assert!(timer(&mut m, 60_000)
            .iter()
            .all(|a| !matches!(a, Action::Message { .. })));
        let a = timer(&mut m, 100 + LOCK_WAIT);
        assert_eq!(state(&a)["transfers"]["1"]["status"], "failed");
        assert_eq!(
            named(&out(&a), b"LOCK"),
            [&vec![0, 0, 0, 0]],
            "the request withdrawn"
        );
        // The switcher served the queued request after all.
        let a = from_switcher(&mut m, 130_000, &[lock_msg(b"LKOB", 0, true)]);
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 0, 0]]);
        assert_eq!(m.transfers.held, None);
    }

    #[test]
    fn losing_the_store_to_another_client_waits_its_turn_and_tries_again() {
        let mut m = switcher();
        let id = upload_to_lock(&mut m, media::png_encode(W, H, &picture()));
        // Error 6: another client's transfer won.
        let a = from_switcher(&mut m, 130, &[id_cmd(b"FTDE", id, &[6])]);
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 0, 0]]);
        assert_eq!(state(&a)["transfers"]["1"]["status"], "waiting");
        // Still held elsewhere a frame later: no request yet.
        assert!(named(&out(&timer(&mut m, 200)), b"LOCK").is_empty());
        // Free: a frame's grace, then ask again.
        let a = from_switcher(&mut m, 2_000, &[lock_msg(b"LKST", 0, false)]);
        assert!(named(&out(&a), b"LOCK").is_empty());
        let a = timer(&mut m, 2_000 + COURTESY);
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 1, 0]]);
        let a = from_switcher(&mut m, 2_100, &[lock_msg(b"LKOB", 0, true)]);
        let list = out(&a);
        let ftsd = named(&list, b"FTSD");
        assert_eq!(ftsd.len(), 1, "the upload starts again");
        assert_ne!(u16_at(ftsd[0], 0), id);
    }

    #[test]
    fn errors_and_silence_end_a_transfer_with_the_lock_released() {
        let mut m = switcher();
        let id = upload_to_lock(&mut m, media::png_encode(W, H, &picture()));
        let a = from_switcher(&mut m, 130, &[id_cmd(b"FTDE", id, &[2])]);
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 0, 0]]);
        assert_eq!(state(&a)["transfers"]["1"]["status"], "failed");

        // A picture of the wrong size never asks for the lock.
        let (_, _) = run(
            &mut m,
            200,
            "upload_still",
            json!({"still": 1, "path": "/b.png"}),
        );
        let a = file(
            &mut m,
            210,
            READ,
            FileInput::Read {
                data: media::png_encode(2, 2, &[0; 16]),
            },
        );
        assert!(named(&out(&a), b"LOCK").is_empty());
        let s = state(&a);
        assert_eq!(s["transfers"]["2"]["status"], "failed");
        assert!(s["transfers"]["2"]["error"]
            .as_str()
            .unwrap()
            .contains("640 by 480"));

        // A missing file.
        let _ = run(
            &mut m,
            300,
            "upload_still",
            json!({"still": 1, "path": "/c.png"}),
        );
        let a = file(
            &mut m,
            310,
            READ,
            FileInput::Failed {
                message: "/c.png: not found".into(),
            },
        );
        assert_eq!(state(&a)["transfers"]["3"]["status"], "failed");

        // The switcher goes quiet mid-transfer.
        let _ = run(
            &mut m,
            400,
            "download_still",
            json!({"still": 1, "path": "/d"}),
        );
        from_switcher(&mut m, 410, &[lock_msg(b"LKOB", 0, true)]);
        let a = timer(&mut m, 410 + STALL);
        assert_eq!(state(&a)["transfers"]["4"]["status"], "failed");
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 0, 0]]);
    }

    #[test]
    fn a_lost_connection_fails_the_transfer_and_keeps_the_queue() {
        let mut m = switcher();
        upload_to_lock(&mut m, media::png_encode(W, H, &picture()));
        let (r, _) = run(
            &mut m,
            125,
            "download_macro",
            json!({"macro": 1, "path": "/m.bin"}),
        );
        assert_eq!(r.unwrap(), json!({"transfer": 2, "ahead": 1}));
        let mut cx = Cx::new(6_000);
        m.timer(&mut cx, super::super::SILENCE);
        let a = cx.take();
        let s = state(&a);
        assert_eq!(s["transfers"]["1"]["status"], "failed");
        assert!(s["transfers"]["1"]["error"]
            .as_str()
            .unwrap()
            .starts_with("connection lost"));
        assert!(
            named(&out(&a), b"LOCK").is_empty(),
            "the switcher frees a dead session's lock"
        );
        assert_eq!(m.transfers.held, None);
        assert_eq!(m.transfers.queue.len(), 1);
        // Connected again: the queued macro download goes ahead, with no lock.
        feed(&mut m, 6_010, super::super::tests::syn_reply());
        let a = feed(&mut m, 6_020, reliable(0x8001, 1, &dump()));
        let list = out(&a);
        let ftsu = named(&list, b"FTSU");
        assert_eq!(ftsu.len(), 1);
        assert_eq!(
            (u16_at(ftsu[0], 2), u16_at(ftsu[0], 6), u16_at(ftsu[0], 8)),
            (0xFFFF, 0, 0x03FE)
        );
        assert!(named(&out(&a), b"LOCK").is_empty());
    }

    #[test]
    fn canceling_releases_the_lock_and_empties_the_queue() {
        let mut m = switcher();
        upload_to_lock(&mut m, media::png_encode(W, H, &picture()));
        let _ = run(
            &mut m,
            125,
            "download_still",
            json!({"still": 1, "path": "/x"}),
        );
        let (r, _) = run(&mut m, 126, "cancel_transfer", json!({"transfer": 9}));
        assert!(matches!(r, Err(CommandError::InvalidParams { .. })));
        let (r, a) = run(&mut m, 127, "cancel_transfer", json!({}));
        assert_eq!(r.unwrap(), json!({"canceled": [1, 2]}));
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 0, 0]]);
        let s = state(&a);
        assert_eq!(s["transfers"]["1"]["status"], "canceled");
        assert_eq!(s["transfers"]["2"]["status"], "canceled");
        assert!(m.transfers.active.is_none() && m.transfers.queue.is_empty());
    }

    #[test]
    fn a_clip_is_cleared_uploaded_frame_by_frame_under_one_lock_and_named() {
        let mut m = switcher();
        let png = media::png_encode(W, H, &picture());
        let (r, a) = run(
            &mut m,
            100,
            "upload_clip",
            json!({"clip": 1, "name": "Sting", "paths": "/f1.png\r\n/f2.png\n"}),
        );
        assert_eq!(r.unwrap()["transfer"], 1);
        assert_eq!(read_of(&a).as_deref(), Some("/f1.png"));
        let a = file(&mut m, 110, READ, FileInput::Read { data: png.clone() });
        assert_eq!(
            named(&out(&a), b"LOCK"),
            [&vec![0, 1, 1, 0]],
            "clip 1's lock"
        );
        let a = from_switcher(&mut m, 120, &[lock_msg(b"LKOB", 1, true)]);
        let list = out(&a);
        assert_eq!(list[0], (*b"CMPC", vec![0, 0, 0, 0]), "cleared first");
        let ftsd = named(&list, b"FTSD")[0].clone();
        let id = u16_at(&ftsd, 0);
        assert_eq!((u16_at(&ftsd, 2), u16_at(&ftsd, 6)), (1, 0));
        from_switcher(&mut m, 130, &[ftcd(id, 1396, 60000)]);
        let a = from_switcher(&mut m, 140, &[id_cmd(b"FTDC", id, &[])]);
        assert!(named(&out(&a), b"LOCK").is_empty(), "the lock is kept");
        assert_eq!(read_of(&a).as_deref(), Some("/f2.png"));
        let a = file(&mut m, 150, READ, FileInput::Read { data: png });
        let list = out(&a);
        assert!(named(&list, b"LOCK").is_empty() && named(&list, b"CMPC").is_empty());
        let ftsd = named(&list, b"FTSD")[0].clone();
        let id = u16_at(&ftsd, 0);
        assert_eq!(u16_at(&ftsd, 6), 1, "frame 2");
        from_switcher(&mut m, 160, &[ftcd(id, 1396, 60000)]);
        let a = from_switcher(&mut m, 170, &[id_cmd(b"FTDC", id, &[])]);
        let list = out(&a);
        let smpc = named(&list, b"SMPC")[0];
        assert_eq!(
            (smpc[0], smpc[1], &smpc[2..7], u16_at(smpc, 66)),
            (3, 0, &b"Sting"[..], 2)
        );
        assert_eq!(named(&list, b"LOCK"), [&vec![0, 1, 0, 0]]);
        assert_eq!(state(&a)["transfers"]["1"]["status"], "done");
    }

    #[test]
    fn clears_are_sent_under_the_stores_lock_then_released() {
        let mut m = switcher();
        let (r, a) = run(&mut m, 100, "clear_still", json!({"still": 5}));
        assert_eq!(r.unwrap(), json!({"transfer": 1, "ahead": 0}));
        let list = out(&a);
        assert_eq!(named(&list, b"LOCK"), [&vec![0, 0, 1, 0]]);
        assert!(named(&list, b"CSTL").is_empty(), "not before the grant");
        assert_eq!(state(&a)["transfers"]["1"]["kind"], "clear_still");
        assert_eq!(state(&a)["transfers"]["1"]["still"], 5);
        let a = from_switcher(&mut m, 110, &[lock_msg(b"LKOB", 0, true)]);
        let list = out(&a);
        let order: Vec<&[u8; 4]> = list.iter().map(|(n, _)| n).collect();
        assert_eq!(order, [b"CSTL", b"LOCK"], "cleared, then released");
        assert_eq!(list[0].1, vec![4, 0, 0, 0]);
        assert_eq!(list[1].1, vec![0, 0, 0, 0]);
        assert_eq!(state(&a)["transfers"]["1"]["status"], "done");
        let done = messages(&a)
            .into_iter()
            .find(|(n, _)| n == "transfer_finished")
            .unwrap()
            .1;
        assert_eq!(
            (&done["kind"], &done["still"], &done["status"]),
            (&json!("clear_still"), &json!(5), &json!("done"))
        );

        // A clip's clear takes that clip's lock, and waits while another
        // client holds it.
        let a = from_switcher(&mut m, 2_000, &[lock_msg(b"LKST", 2, true)]);
        assert_eq!(state(&a)["media_pool"]["locks"]["clips"]["2"], true);
        let (r, a) = run(&mut m, 2_010, "clear_clip", json!({"clip": 2}));
        assert_eq!(r.unwrap()["transfer"], 2);
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 2, 1, 0]]);
        let a = from_switcher(&mut m, 3_000, &[lock_msg(b"LKOB", 2, true)]);
        let list = out(&a);
        assert_eq!(list[0], (*b"CMPC", vec![1, 0, 0, 0]));
        assert_eq!(named(&list, b"LOCK"), [&vec![0, 2, 0, 0]]);

        // Checked against the pool like any other command.
        assert!(matches!(
            run(&mut m, 3_100, "clear_still", json!({"still": 21})).0,
            Err(CommandError::InvalidParams { .. })
        ));
    }

    #[test]
    fn audio_macros_and_clip_frames_take_their_own_stores_and_modes() {
        let mut m = switcher();
        // 48 kHz 24-bit stereo, one sample pair.
        let mut wav = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x80\xbb\0\0\0\0\0\0\x06\0\x18\0data\x06\0\0\0".to_vec();
        wav.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let _ = run(
            &mut m,
            100,
            "upload_clip_audio",
            json!({"clip": 2, "path": "/a.wav", "name": "Hit"}),
        );
        file(&mut m, 110, READ, FileInput::Read { data: wav });
        let a = from_switcher(&mut m, 120, &[lock_msg(b"LKOB", 2, true)]);
        let ftsd = named(&out(&a), b"FTSD")[0].clone();
        let id = u16_at(&ftsd, 0);
        assert_eq!(
            (u16_at(&ftsd, 2), u16_at(&ftsd, 6), u16_at(&ftsd, 12)),
            (2, 0, 0x0100)
        );
        // The Mini Pro (model 14) takes every word reversed.
        assert_eq!(super::super::u32_at(&ftsd, 8), 12);
        let a = from_switcher(&mut m, 130, &[ftcd(id, 1396, 10)]);
        let list = out(&a);
        assert_eq!(&named(&list, b"FTFD")[0][2..5], b"Hit");
        assert_eq!(
            named(&list, b"FTDa")[0][4..],
            [6, 1, 2, 3, 0, 0, 4, 5, 0, 0, 0, 0]
        );
        let a = from_switcher(&mut m, 140, &[id_cmd(b"FTDC", id, &[])]);
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 2, 0, 0]]);

        // A macro upload takes no lock: store 0xFFFF, mode 0x300.
        let _ = run(
            &mut m,
            200,
            "upload_macro",
            json!({"macro": 3, "path": "/m", "name": "Go"}),
        );
        let a = file(&mut m, 210, READ, FileInput::Read { data: vec![9; 40] });
        let list = out(&a);
        assert!(named(&list, b"LOCK").is_empty());
        let ftsd = named(&list, b"FTSD")[0];
        assert_eq!(
            (u16_at(ftsd, 2), u16_at(ftsd, 6), u16_at(ftsd, 12)),
            (0xFFFF, 2, 0x0300)
        );

        // Clip frames download from the clip's store.
        let mut m = switcher();
        let (r, _) = run(
            &mut m,
            100,
            "download_clip",
            json!({"clip": 1, "paths": "/a\n/b", "first_frame": 2}),
        );
        assert_eq!(r.unwrap()["transfer"], 1);
        let a = from_switcher(&mut m, 110, &[lock_msg(b"LKOB", 1, true)]);
        let ftsu = named(&out(&a), b"FTSU")[0].clone();
        assert_eq!((u16_at(&ftsu, 2), u16_at(&ftsu, 6)), (1, 1));
    }

    #[test]
    fn transfers_are_checked_against_the_switcher() {
        let mut m = switcher();
        let refused = |m: &mut Atem, name: &str, p: Value| run(m, 100, name, p).0.unwrap_err();
        assert!(matches!(
            refused(&mut m, "download_still", json!({"still": 3, "path": "/x"})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            refused(&mut m, "upload_still", json!({"still": 21, "path": "/x"})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            refused(
                &mut m,
                "upload_clip",
                json!({"clip": 1, "paths": "/1\n/2\n/3\n/4\n/5"})
            ),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            refused(
                &mut m,
                "upload_clip",
                json!({"clip": 1, "paths": "/1\n\n/3"})
            ),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            refused(
                &mut m,
                "download_clip",
                json!({"clip": 1, "paths": "/1\n/2", "first_frame": 3})
            ),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            refused(&mut m, "download_macro", json!({"macro": 2, "path": "/x"})),
            CommandError::InvalidParams { .. }
        ));
        // The Mini Pro draws its own multiviewer labels.
        assert!(matches!(
            refused(
                &mut m,
                "upload_multiviewer_label",
                json!({"source": 1, "path": "/x"})
            ),
            CommandError::UnsupportedForModel { .. }
        ));
        // Before the video mode is known there is no frame size.
        let (mut m, _) = ready();
        assert!(matches!(
            refused(&mut m, "upload_still", json!({"still": 1, "path": "/x"})),
            CommandError::InvalidParams { .. }
        ));

        // An older switcher takes labels: store 0xFFFF, mode 0x201, the
        // source as the index, and no bytes from 254.
        let mut d = dump();
        let mut pin = vec![0u8; 44];
        pin[..12].copy_from_slice(b"ATEM TVS Pro");
        pin[40] = 9;
        d[1] = cmd(b"_pin", &pin);
        let mut m = ready_with(d);
        let _ = run(
            &mut m,
            100,
            "upload_multiviewer_label",
            json!({"source": 2, "path": "/l"}),
        );
        let a = file(
            &mut m,
            110,
            READ,
            FileInput::Read {
                data: vec![14, 255],
            },
        );
        assert_eq!(state(&a)["transfers"]["1"]["status"], "failed");
        let _ = run(
            &mut m,
            200,
            "upload_multiviewer_label",
            json!({"source": 2, "path": "/l"}),
        );
        let a = file(&mut m, 210, READ, FileInput::Read { data: vec![14; 20] });
        let ftsd = named(&out(&a), b"FTSD")[0].clone();
        assert_eq!(
            (u16_at(&ftsd, 2), u16_at(&ftsd, 6), u16_at(&ftsd, 12)),
            (0xFFFF, 2, 0x0201)
        );
    }

    #[test]
    fn pictures_are_converted_off_the_session_and_a_stale_result_is_dropped() {
        let mut m = switcher();
        let _ = run(
            &mut m,
            100,
            "upload_still",
            json!({"still": 1, "path": "/a"}),
        );
        let mut cx = Cx::new(110);
        m.file(
            &mut cx,
            READ,
            FileInput::Read {
                data: media::png_encode(W, H, &picture()),
            },
        );
        let a = cx.take();
        // Nothing is converted here: the work is handed off, and nothing is
        // asked of the switcher until it comes back.
        let task = a
            .iter()
            .find_map(|x| match x {
                Action::Compute { key, task } if *key == PREPARE => Some(task.clone()),
                _ => None,
            })
            .expect("the conversion is handed off");
        assert!(sent(&a).is_empty());
        // Canceled meanwhile: the result that comes back changes nothing.
        let _ = run(&mut m, 120, "cancel_transfer", json!({}));
        let mut cx = Cx::new(130);
        m.computed(&mut cx, PREPARE, task.run());
        assert!(sent(&cx.take()).is_empty());
        assert!(m.transfers.held.is_none() && m.transfers.asked.is_none());

        // A download's frame is converted off the session too, then written.
        let _ = run(
            &mut m,
            200,
            "download_still",
            json!({"still": 1, "path": "/d.png"}),
        );
        let a = from_switcher(&mut m, 210, &[lock_msg(b"LKOB", 0, true)]);
        let id = u16_at(named(&out(&a), b"FTSU")[0], 0);
        // A flat frame: one run, one chunk.
        let flat = [10u8, 20, 30, 255].repeat((W * H) as usize);
        let rle = media::rle_encode(&media::rgba_to_yuva(W, H, &flat));
        let mut body = id.to_be_bytes().to_vec();
        body.extend_from_slice(&(rle.len() as u16).to_be_bytes());
        body.extend_from_slice(&rle);
        from_switcher(&mut m, 220, &[cmd(b"FTDa", &body)]);
        let mut cx = Cx::new(230);
        let id_bytes = id.to_be_bytes();
        let ftdc = reliable(
            0x8001,
            (m.last_received + 1) % super::super::ID_MODULO,
            &[cmd(b"FTDC", &[id_bytes[0], id_bytes[1], 0, 0])],
        );
        m.datagram(
            &mut cx,
            super::super::SOCKET,
            SocketAddr::new(m.host(), 9910),
            &ftdc,
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Compute { key, .. } if *key == CONVERTERS[0])));
        assert!(!a.iter().any(|x| matches!(x, Action::FileWrite { .. })));
        let a = settle(&mut m, 240, a);
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::FileOpen { file, path } if *file == WRITERS[0] && path == "/d.png")));
    }

    #[test]
    fn a_label_is_drawn_from_text_and_sent_as_a_label_bitmap() {
        // An older switcher, whose 1080p50 multiviewer runs in 1080 and 720.
        let mut d = dump();
        let mut pin = vec![0u8; 44];
        pin[..12].copy_from_slice(b"ATEM TVS Pro");
        pin[40] = 9;
        d[1] = cmd(b"_pin", &pin);
        let without = ready_with(d.clone());
        // One video mode, 1080p50; its multiviewer runs in 1080p50 and
        // 720p50, with no reconfiguring.
        let mut vmc = vec![0, 1, 0, 0, 12, 0, 0, 0];
        vmc.extend_from_slice(&((1u32 << 12) | (1 << 4)).to_be_bytes());
        vmc.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
        d.insert(3, cmd(b"_VMC", &vmc));
        let mut m = ready_with(d);
        let resolutions = m.topology.label_resolutions;
        assert!(resolutions.hd1080 && resolutions.hd720 && !resolutions.uhd);
        let expected = label::render("CAM 2", resolutions).unwrap();
        assert_eq!(expected.len(), 320 * 90);

        // The drawing is handed off; settle it as the session would.
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            7,
            "set_multiviewer_label",
            json!({"source": 2, "text": "CAM 2"}).as_object().unwrap(),
        );
        let a = settle(&mut m, 100, cx.take());
        let list = out(&a);
        let ftsd = named(&list, b"FTSD")[0].clone();
        let id = u16_at(&ftsd, 0);
        assert_eq!(
            (u16_at(&ftsd, 2), u16_at(&ftsd, 6), u16_at(&ftsd, 12)),
            (0xFFFF, 2, 0x0201)
        );
        assert_eq!(super::super::u32_at(&ftsd, 8), expected.len() as u32);
        let a = from_switcher(&mut m, 120, &[ftcd(id, 1396, 1000)]);
        let list = out(&a);
        assert_eq!(&named(&list, b"FTFD")[0][2..8], b"Label\0");
        let sent_label: Vec<u8> = named(&list, b"FTDa")
            .iter()
            .flat_map(|c| c[4..].to_vec())
            .collect();
        assert_eq!(sent_label, expected);

        // Without the multiviewer modes there is nothing to draw for.
        let mut without = without;
        let (r, _) = run(
            &mut without,
            100,
            "set_multiviewer_label",
            json!({"source": 2, "text": "x"}),
        );
        assert!(matches!(r, Err(CommandError::InvalidParams { .. })));
        // A switcher that draws its own labels refuses it.
        let mut m = switcher();
        let (r, _) = run(
            &mut m,
            100,
            "set_multiviewer_label",
            json!({"source": 1, "text": "x"}),
        );
        assert!(matches!(r, Err(CommandError::UnsupportedForModel { .. })));
    }

    #[test]
    fn closing_lets_go_of_the_lock() {
        let mut m = switcher();
        upload_to_lock(&mut m, media::png_encode(W, H, &picture()));
        assert_eq!(m.transfers.held, Some(0));
        let mut cx = Cx::new(500);
        m.stop(&mut cx);
        let a = cx.take();
        assert_eq!(named(&out(&a), b"LOCK"), [&vec![0, 0, 0, 0]]);
    }
}
