//! VTR remote control over the Sony 9-pin protocol (RS-422) and Grass Valley's
//! Advanced Media Protocol (AMP), which extends it: the `sony-9pin` and
//! `grassvalley-amp` integrations.
//!
//! - A message is CMD-1 (the command type in the high nibble, the number of
//!   data bytes in the low one), CMD-2 (the command), the data and, in the
//!   binary form, a checksum: the low byte of the sum of the bytes before it.
//!   The device answers every message: ACK (10 01), NAK (11 12 and an error
//!   byte), or the data asked for (12 11 the device type, 7X the sense
//!   returns, and for AMP 8X its own returns). One message is in flight at a
//!   time and the next goes once it is answered.
//! - AMP's extended messages (a clip name of any length) put 2, A or E in the
//!   count nibble and the real count in the two bytes after CMD-2; so do its
//!   8X returns that carry names.
//! - Sony 9-pin (RS-422 on the device) goes through a serial-to-Ethernet
//!   converter, which carries the binary messages unchanged. AMP goes over
//!   TCP 3811, where each message is ASCII: `CMDS`, its length in characters as four decimal
//!   digits, the message's bytes as hexadecimal digits, then LF, with no
//!   checksum; the server answers in hexadecimal digits too. A TCP
//!   connection first opens an AMP channel (`CRAT`, the channel's name) or a
//!   channel-less session, answered 1001, and ends with `STOP0000`.
//! - Nothing is pushed: monitored, the status bytes (61.20) and the current
//!   time (61.0C) are asked every 250 ms, and the edit points and other
//!   settings every 5 s.
//!
//! Opened for commands only (`monitor` false), nothing is asked but what
//! commands ask; a device type request (00.11) every 10 s while idle is the
//! liveness check. The AMP channel is still opened, since commands need it.

use std::collections::VecDeque;

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

const SOCKET: Key = "vtr";
const REPLY_TIMER: Key = "reply";
const FAST_TIMER: Key = "fast";
const SLOW_TIMER: Key = "slow";
const RETRY_TIMER: Key = "retry";

const REPLY_TIMEOUT: Millis = 1_000;
const FAST_EVERY: Millis = 250;
const SLOW_EVERY: Millis = 5_000;
const PROBE_EVERY: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

/// The protocol a device speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(all(feature = "sony-9pin", feature = "grassvalley-amp")),
    allow(dead_code)
)]
pub(crate) enum Dialect {
    Sony,
    Amp,
}

/// How messages travel: binary with a checksum (Sony 9-pin through a
/// converter), or AMP's hexadecimal text over TCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wire {
    Binary,
    Hex,
}

/// One message without its checksum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Msg {
    pub cmd1: u8,
    pub cmd2: u8,
    pub data: Vec<u8>,
}

impl Msg {
    /// A message whose count nibble is the number of data bytes.
    pub(crate) fn new(kind: u8, cmd2: u8, data: &[u8]) -> Msg {
        Msg {
            cmd1: (kind << 4) | (data.len() as u8 & 0x0F),
            cmd2,
            data: data.to_vec(),
        }
    }

    /// An extended message: a count nibble (2, A or E) and the payload's
    /// real length in the two bytes after CMD-2.
    pub(crate) fn extended(kind: u8, nibble: u8, cmd2: u8, payload: &[u8]) -> Msg {
        let mut data = (payload.len() as u16).to_be_bytes().to_vec();
        data.extend_from_slice(payload);
        Msg {
            cmd1: (kind << 4) | nibble,
            cmd2,
            data,
        }
    }

    fn kind(&self) -> u8 {
        self.cmd1 >> 4
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = vec![self.cmd1, self.cmd2];
        out.extend_from_slice(&self.data);
        out
    }
}

pub(crate) fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

/// A message as it goes out on the wire.
pub(crate) fn frame(msg: &Msg, hex: bool) -> Vec<u8> {
    let bytes = msg.bytes();
    if hex {
        let digits: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        format!("CMDS{:04}{digits}\n", digits.len()).into_bytes()
    } else {
        let mut out = bytes.clone();
        out.push(checksum(&bytes));
        out
    }
}

/// How many data bytes a received message carries, from its first bytes;
/// `None` until enough have arrived. AMP's 8X returns with 2, A or E in the
/// count nibble give the real count in their first two data bytes.
fn data_len(dialect: Dialect, head: &[u8]) -> Option<usize> {
    let cmd1 = *head.first()?;
    let nibble = (cmd1 & 0x0F) as usize;
    if dialect == Dialect::Amp && cmd1 >> 4 == 0x8 && matches!(nibble, 0x2 | 0xA | 0xE) {
        let hi = *head.get(2)? as usize;
        let lo = *head.get(3)? as usize;
        return Some(2 + (hi << 8 | lo));
    }
    Some(nibble)
}

/// BCD timecode from "HH:MM:SS:FF" (or with ';' or '.' before the frames),
/// as the protocol sends it: frames, seconds, minutes, hours.
pub(crate) fn timecode(text: &str) -> Result<[u8; 4], CommandError> {
    let parts: Vec<&str> = text.split([':', ';', '.']).collect();
    let bad = || invalid(format!("'{text}' is not a timecode HH:MM:SS:FF"));
    if parts.len() != 4 {
        return Err(bad());
    }
    let mut v = [0u8; 4];
    for (i, (part, max)) in parts.iter().zip([23u8, 59, 59, 59]).enumerate() {
        let n: u8 = part.parse().map_err(|_| bad())?;
        if n > max || part.len() != 2 {
            return Err(bad());
        }
        v[i] = (n / 10) << 4 | (n % 10);
    }
    Ok([v[3], v[2], v[1], v[0]])
}

/// "HH:MM:SS:FF" from BCD frames, seconds, minutes, hours; the drop-frame
/// and color-frame flags (bits 6 and 7 of the frames) are left out.
pub(crate) fn tc_text(data: &[u8]) -> String {
    let d = |b: u8, mask: u8| {
        let b = b & mask;
        (b >> 4) * 10 + (b & 0x0F)
    };
    let f = data.first().copied().unwrap_or(0);
    let s = data.get(1).copied().unwrap_or(0);
    let m = data.get(2).copied().unwrap_or(0);
    let h = data.get(3).copied().unwrap_or(0);
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        d(h, 0x3F),
        d(m, 0x7F),
        d(s, 0x7F),
        d(f, 0x3F)
    )
}

/// User bits as eight hexadecimal digits in the order the bytes carry them,
/// each byte's low nibble first (Drastic's example: 60 63 44 45 is
/// 06364454).
fn ub_text(data: &[u8]) -> String {
    data.iter()
        .take(4)
        .map(|b| format!("{:x}{:x}", b & 0x0F, b >> 4))
        .collect::<String>()
        .to_uppercase()
}

fn ub_bytes(text: &str) -> Result<[u8; 4], CommandError> {
    if text.len() != 8 || !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid("'user_bits' is eight hexadecimal digits"));
    }
    let n: Vec<u8> = text
        .chars()
        .map(|c| c.to_digit(16).unwrap_or(0) as u8)
        .collect();
    Ok([
        n[0] | n[1] << 4,
        n[2] | n[3] << 4,
        n[4] | n[5] << 4,
        n[6] | n[7] << 4,
    ])
}

/// A tape speed as the 9-pin speed bytes: 10^(N/32 - 2) times play speed
/// for N, with N' the 1/256 steps towards N+1.
pub(crate) fn speed_bytes(rate: f64) -> Vec<u8> {
    if rate <= 0.0 {
        return vec![0];
    }
    let n = (32.0 * (rate.log10() + 2.0)).clamp(0.0, 255.0);
    let whole = n.floor();
    let at = |x: f64| 10f64.powf(x / 32.0 - 2.0);
    let fine = if whole >= 255.0 {
        0.0
    } else {
        ((rate - at(whole)) / (at(whole + 1.0) - at(whole)) * 256.0).clamp(0.0, 255.0)
    };
    let fine = fine.round() as u8;
    if fine == 0 {
        vec![whole as u8]
    } else {
        vec![whole as u8, fine]
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn text<'a>(p: &'a Params, key: &str) -> Result<&'a str, CommandError> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn flag(p: &Params, key: &str) -> Result<bool, CommandError> {
    p.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn int(p: &Params, key: &str) -> Result<i64, CommandError> {
    p.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn num(p: &Params, key: &str) -> Result<f64, CommandError> {
    p.get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn pick(p: &Params, key: &str, table: &[(&str, u8)]) -> Result<u8, CommandError> {
    let v = text(p, key)?;
    table
        .iter()
        .find(|(n, _)| *n == v)
        .map(|(_, c)| *c)
        .ok_or_else(|| invalid(format!("'{key}' has no value '{v}'")))
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A clip or folder name as AMP's extended format writes it: a two-byte
/// length, then the name.
fn name_field(name: &str) -> Vec<u8> {
    let mut out = (name.len() as u16).to_be_bytes().to_vec();
    out.extend_from_slice(name.as_bytes());
    out
}

/// Names in AMP's extended format, back to back, from a return's payload.
fn names(mut payload: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    while payload.len() >= 2 {
        let len = (payload[0] as usize) << 8 | payload[1] as usize;
        if payload.len() < 2 + len {
            break;
        }
        out.push(String::from_utf8_lossy(&payload[2..2 + len]).into_owned());
        payload = &payload[2 + len..];
    }
    out
}

const DIRECTION: &[(&str, u8)] = &[("forward", 0x00), ("reverse", 0x10)];
const POINT: &[(&str, u8)] = &[("in", 0), ("out", 1), ("audio_in", 2), ("audio_out", 3)];

/// What a reply is read as.
#[derive(Debug, Clone, PartialEq)]
enum Read {
    /// ACK expected.
    Ack,
    DeviceType,
    Status,
    Time,
    Generator,
    Point(u8),
    PrerollTime,
    TimerMode,
    EditPreset,
    Raw,
    DeviceId,
    Loaded,
    ClipTime(&'static str),
    WorkingFolder,
    Folders(Vec<String>),
    Clips(Vec<String>),
    ClipStatus,
    ClipData,
}

#[derive(Debug, Clone)]
struct Job {
    command: Option<CommandId>,
    msg: Msg,
    read: Read,
    /// Messages sent after an ACK, before the command completes (a set's
    /// read back); their replies update state.
    then: Vec<(Msg, Read)>,
}

impl Job {
    fn ack(msg: Msg) -> Job {
        Job {
            command: None,
            msg,
            read: Read::Ack,
            then: Vec::new(),
        }
    }

    fn read(msg: Msg, read: Read) -> Job {
        Job {
            command: None,
            msg,
            read,
            then: Vec::new(),
        }
    }

    fn then(mut self, msg: Msg, read: Read) -> Job {
        self.then.push((msg, read));
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Connecting,
    /// AMP over TCP: the channel request is answered.
    Channel,
    Ready,
}

pub(crate) struct Vtr {
    dialect: Dialect,
    wire: Wire,
    to: std::net::SocketAddr,
    monitor: bool,
    channel: String,
    phase: Phase,
    connected: bool,
    buf: Vec<u8>,
    commands: VecDeque<Job>,
    polls: VecDeque<Job>,
    in_flight: Option<Job>,
    sent_at: Millis,
    retry_after: Millis,
    /// The status bytes asked for (61.20's DATA-1).
    status_request: u8,
}

/// The status bits both dialects share and the Sony-only ones, by byte and
/// bit, as the 7X.20 status data gives them.
const SONY_BITS: &[(usize, u8, &str)] = &[
    (0, 0, "local"),
    (0, 4, "servo_ref_missing"),
    (0, 5, "tape_out"),
    (1, 0, "play"),
    (1, 1, "record"),
    (1, 2, "fast_forward"),
    (1, 3, "rewind"),
    (1, 4, "eject"),
    (1, 5, "stop"),
    (1, 6, "tension_release"),
    (1, 7, "standby"),
    (2, 0, "cue_complete"),
    (2, 1, "still"),
    (2, 2, "reverse"),
    (2, 3, "var"),
    (2, 4, "jog"),
    (2, 5, "shuttle"),
    (2, 6, "tso"),
    (2, 7, "servo_lock"),
    (3, 0, "in_point"),
    (3, 1, "out_point"),
    (3, 2, "audio_in_point"),
    (3, 3, "audio_out_point"),
    (3, 4, "cf_mode"),
    (3, 6, "freeze"),
    (3, 7, "auto_mode"),
    (4, 0, "preroll"),
    (4, 1, "preview"),
    (4, 2, "auto_edit"),
    (4, 3, "review"),
    (4, 4, "edit"),
    (4, 6, "full_ee"),
    (4, 7, "select_ee"),
    (5, 0, "a1"),
    (5, 1, "a2"),
    (5, 2, "a3"),
    (5, 3, "a4"),
    (5, 4, "video"),
    (5, 5, "assemble"),
    (5, 6, "insert"),
    (6, 4, "lamp_reverse"),
    (6, 5, "lamp_forward"),
    (6, 6, "lamp_still"),
    (7, 0, "in_out"),
    (7, 2, "spot_erase"),
    (7, 4, "sync_active"),
    (7, 5, "audio_split"),
    (8, 0, "record_inhibit"),
    (8, 1, "system_alarm"),
    (8, 2, "servo_alarm"),
    (8, 3, "cf_lock"),
    (8, 4, "eot"),
    (8, 5, "near_eot"),
    (8, 6, "lost_lock"),
    (8, 7, "buzzer"),
    (9, 7, "function_abort"),
];

/// AMP's status bits (Grass Valley's T2 AMP manual lists them).
const AMP_BITS: &[(usize, u8, &str)] = &[
    (0, 0, "local"),
    (0, 1, "remote_and_local"),
    (0, 2, "hard_error"),
    (0, 3, "general_error"),
    (0, 5, "tape_out"),
    (0, 7, "busy"),
    (1, 0, "play"),
    (1, 1, "record"),
    (1, 2, "fast_forward"),
    (1, 3, "rewind"),
    (1, 4, "eject"),
    (1, 5, "stop"),
    (1, 6, "tension_release"),
    (1, 7, "standby"),
    (2, 0, "cue_complete"),
    (2, 1, "still"),
    (2, 2, "reverse"),
    (2, 3, "var"),
    (2, 4, "jog"),
    (2, 5, "shuttle"),
    (2, 7, "servo_lock"),
    (3, 0, "in_point"),
    (3, 1, "out_point"),
    (3, 2, "folder_exists"),
    (3, 3, "invalid_folder_name"),
    (3, 4, "folder_delete_failed"),
    (3, 5, "jog_reject"),
    (3, 6, "source_missing"),
    (3, 7, "auto_mode"),
    (4, 0, "preroll"),
    (4, 1, "schedule_failed"),
    (4, 4, "mute"),
    (4, 5, "loop"),
    (4, 6, "ee"),
    (9, 0, "preview_in_point"),
    (9, 1, "preview_out_point"),
    (9, 2, "folder_not_found"),
    (9, 3, "disk_overflow"),
    (9, 4, "metadata_not_found"),
    (9, 5, "clips_dropped"),
    (9, 6, "out_preset_failed"),
    (9, 7, "overwrite_clip_name"),
    (10, 0, "id_not_found"),
    (10, 1, "timecode_not_found"),
    (10, 2, "transfer_complete"),
    (10, 3, "transfer_abort_complete"),
    (10, 4, "delete_complete"),
    (10, 5, "transfer_failed"),
    (10, 6, "transfer_abort_failed"),
    (10, 7, "delete_failed"),
    (13, 0, "time_of_day"),
    (13, 1, "widescreen"),
    (13, 2, "drop_frame"),
    (13, 3, "vitc"),
    (13, 4, "timer"),
    (13, 5, "ltc"),
    (13, 6, "tape_end"),
    (13, 7, "tape_top"),
];

/// Device types (12.11) Sony's and others' documentation lists; X is 0 for
/// 525-line and 1 for 625-line models.
const DEVICE_TYPES: &[(u8, u8, &str)] = &[
    (0x20, 0x00, "BVW-10"),
    (0x20, 0x02, "BVW-11"),
    (0x20, 0x03, "BVW-15"),
    (0x20, 0x10, "BVW-35"),
    (0x20, 0x01, "BVW-40"),
    (0x20, 0x30, "BVW-50"),
    (0x20, 0x20, "BVW-60"),
    (0x20, 0x21, "BVW-65"),
    (0x20, 0x22, "BVW-95"),
    (0x20, 0x23, "BVW-96"),
    (0x20, 0x24, "BVW-70"),
    (0x20, 0x25, "BVW-75"),
    (0x20, 0x46, "BVW-D75"),
    (0x20, 0x47, "BVW-9000"),
    (0x20, 0x40, "PVW-2600"),
    (0x20, 0x41, "PVW-2800"),
    (0x30, 0x10, "DVR-2000"),
    (0x30, 0x11, "DVR-2100"),
    (0x10, 0x00, "BVU-800"),
    (0xF0, 0x1A, "TASCAM DA-6400"),
];

fn model_of(d1: u8, d2: u8) -> Option<&'static str> {
    DEVICE_TYPES
        .iter()
        .find(|(a, b, _)| *a == d1 & 0xF0 && *b == d2)
        .map(|(_, _, n)| *n)
}

fn nak_text(bits: u8) -> String {
    let names = [
        (0, "undefined command"),
        (2, "checksum error"),
        (4, "parity error"),
        (5, "overrun error"),
        (6, "framing error"),
        (7, "time out"),
    ];
    let found: Vec<&str> = names
        .iter()
        .filter(|(b, _)| bits & (1 << b) != 0)
        .map(|(_, n)| *n)
        .collect();
    if found.is_empty() {
        "the device refused the command".into()
    } else {
        format!("the device refused the command: {}", found.join(", "))
    }
}

impl Vtr {
    pub(crate) fn new(dialect: Dialect, ctx: OpenContext) -> Result<Vtr, String> {
        let (to, wire) = match dialect {
            Dialect::Amp => (crate::module::tcp_address(&ctx, 3811)?, Wire::Hex),
            Dialect::Sony => {
                if ctx.port.is_none() {
                    return Err(
                        "a serial-to-Ethernet converter has no standard port: give the converter's port"
                            .into(),
                    );
                }
                (crate::module::tcp_address(&ctx, 0)?, Wire::Binary)
            }
        };
        let channel = ctx
            .settings
            .get("channel")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Ok(Vtr {
            dialect,
            wire,
            to,
            monitor: ctx.monitor,
            channel,
            phase: Phase::Closed,
            connected: false,
            buf: Vec::new(),
            commands: VecDeque::new(),
            polls: VecDeque::new(),
            in_flight: None,
            sent_at: 0,
            retry_after: RETRY_MIN,
            status_request: match dialect {
                Dialect::Sony => 0x0A,
                Dialect::Amp => 0x0F,
            },
        })
    }

    fn time_request(&self) -> u8 {
        match self.dialect {
            Dialect::Sony => 0x03,
            Dialect::Amp => 0x01,
        }
    }

    /// The job for a command, or why it cannot be sent.
    fn job(&self, name: &str, p: &Params) -> Result<Job, CommandError> {
        let ack = |kind: u8, cmd2: u8, data: &[u8]| Ok(Job::ack(Msg::new(kind, cmd2, data)));
        let read =
            |kind: u8, cmd2: u8, data: &[u8], r: Read| Ok(Job::read(Msg::new(kind, cmd2, data), r));
        let status = || Msg::new(6, 0x20, &[self.status_request]);
        let point_sense = |pt: u8| Msg::new(6, 0x10 + pt, &[]);
        let amp = self.dialect == Dialect::Amp;
        match name {
            "local_disable" => ack(0, 0x0C, &[]),
            "local_enable" => ack(0, 0x1D, &[]),
            "get_device_type" => read(0, 0x11, &[], Read::DeviceType),
            "stop" => ack(2, 0x00, &[]),
            "play" => ack(2, 0x01, &[]),
            "record" => ack(2, 0x02, &[]),
            "play_at" | "stop_at" | "record_at" => {
                let code = match name {
                    "stop_at" => 0x00,
                    "play_at" => 0x01,
                    _ => 0x02,
                };
                ack(2, code, &timecode(text(p, "timecode")?)?)
            }
            "standby" => ack(2, if flag(p, "enabled")? { 0x05 } else { 0x04 }, &[]),
            "eject" => ack(2, 0x0F, &[]),
            "fast_forward" => ack(2, 0x10, &[]),
            "rewind" => ack(2, 0x20, &[]),
            "jog" | "var" | "shuttle" => {
                let base = match name {
                    "jog" => 0x11,
                    "var" => 0x12,
                    _ => 0x13,
                };
                let dir = pick(p, "direction", DIRECTION)?;
                let rate = num(p, "rate")?;
                ack(2, base + dir, &speed_bytes(rate))
            }
            "jog_fields" => {
                let fields = int(p, "fields")?;
                let base = if fields < 0 { 0x21 } else { 0x11 };
                ack(2, base, &(fields.unsigned_abs() as u32).to_be_bytes())
            }
            "frame_step" => {
                let dir = pick(p, "direction", DIRECTION)?;
                ack(2, 0x14 + dir, &[])
            }
            "preroll" => ack(2, 0x30, &[]),
            "cue_up" => ack(2, 0x31, &timecode(text(p, "timecode")?)?),
            "sync_play" => ack(2, 0x34, &[]),
            "program_speed_play" => {
                let percent = num(p, "percent")?;
                let code = if percent < 0.0 { 0x39 } else { 0x38 };
                let steps = (percent.abs() * 10.0).round().min(255.0) as u8;
                ack(2, code, &[steps])
            }
            "preview" => ack(2, 0x40, &[]),
            "review" => ack(2, 0x41, &[]),
            "auto_edit" => ack(2, 0x42, &[]),
            "outpoint_preview" => ack(2, 0x43, &[]),
            "anti_clog_timer" => ack(2, if flag(p, "enabled")? { 0x55 } else { 0x54 }, &[]),
            "full_ee" => ack(2, if flag(p, "enabled")? { 0x61 } else { 0x60 }, &[]),
            "select_ee_on" => ack(2, 0x63, &[]),
            "edit" => ack(2, if flag(p, "enabled")? { 0x65 } else { 0x64 }, &[]),
            "freeze" => ack(2, if flag(p, "enabled")? { 0x6B } else { 0x6A }, &[]),
            "set_mute" => ack(2, 0x62, &[u8::from(flag(p, "enabled")?)]),
            "set_timer1" => ack(4, 0x00, &timecode(text(p, "timecode")?)?),
            "reset_timer1" => ack(4, 0x08, &[]),
            "set_generator" => {
                let mut tc = timecode(text(p, "timecode")?)?;
                if flag(p, "drop_frame")? {
                    tc[0] |= 0x40;
                }
                if flag(p, "color_frame")? {
                    tc[0] |= 0x80;
                }
                Ok(Job::ack(Msg::new(4, 0x04, &tc))
                    .then(Msg::new(6, 0x0A, &[0x11]), Read::Generator))
            }
            "set_user_bits" => Ok(
                Job::ack(Msg::new(4, 0x05, &ub_bytes(text(p, "user_bits")?)?))
                    .then(Msg::new(6, 0x0A, &[0x11]), Read::Generator),
            ),
            "enter_point" => {
                let pt = pick(p, "point", POINT)?;
                Ok(Job::ack(Msg::new(4, 0x10 + pt, &[])).then(point_sense(pt), Read::Point(pt)))
            }
            "set_point" => {
                let pt = pick(p, "point", POINT)?;
                let tc = timecode(text(p, "timecode")?)?;
                Ok(Job::ack(Msg::new(4, 0x14 + pt, &tc)).then(point_sense(pt), Read::Point(pt)))
            }
            "shift_point" => {
                let pt = pick(p, "point", POINT)?;
                let minus = pick(p, "direction", &[("plus", 0), ("minus", 1)])?;
                Ok(Job::ack(Msg::new(4, 0x18 + pt * 2 + minus, &[]))
                    .then(point_sense(pt), Read::Point(pt)))
            }
            "reset_point" => {
                let pt = pick(p, "point", POINT)?;
                Ok(Job::ack(Msg::new(4, 0x20 + pt, &[])).then(point_sense(pt), Read::Point(pt)))
            }
            "recall_point" => {
                let pt = pick(p, "point", POINT)?;
                Ok(Job::ack(Msg::new(4, 0x24 + pt, &[])).then(point_sense(pt), Read::Point(pt)))
            }
            "reset_lost_lock" => ack(4, 0x2D, &[]),
            "set_edit_preset" => {
                let b = |k: &str, bit: u8| -> Result<u8, CommandError> {
                    Ok(u8::from(flag(p, k)?) << bit)
                };
                let d1 = b("insert", 6)?
                    | b("assemble", 5)?
                    | b("video", 4)?
                    | b("timecode", 2)?
                    | b("cue_a2", 1)?
                    | b("cue_a1", 0)?;
                let d2 = b("audio4", 3)? | b("audio3", 2)? | b("audio2", 1)? | b("audio1", 0)?;
                Ok(Job::ack(Msg::new(4, 0x30, &[d1, d2]))
                    .then(Msg::new(6, 0x30, &[0x02]), Read::EditPreset))
            }
            "get_edit_preset" => read(6, 0x30, &[0x02], Read::EditPreset),
            "set_preroll_time" => {
                let tc = timecode(text(p, "timecode")?)?;
                Ok(
                    Job::ack(Msg::new(4, 0x31, &tc))
                        .then(Msg::new(6, 0x31, &[]), Read::PrerollTime),
                )
            }
            "set_tape_auto" => ack(
                4,
                0x32,
                &[pick(
                    p,
                    "mode",
                    &[("auto", 0x00), ("tape", 0x01), ("local", 0xFF)],
                )?],
            ),
            "set_servo_reference" => ack(
                4,
                0x33,
                &[pick(
                    p,
                    "reference",
                    &[
                        ("auto", 0x00),
                        ("external", 0x01),
                        ("input", 0x02),
                        ("local", 0xFF),
                    ],
                )?],
            ),
            "set_color_frame" => ack(
                4,
                0x35,
                &[pick(
                    p,
                    "mode",
                    &[
                        ("two_field", 0x01),
                        ("four_field", 0x02),
                        ("eight_field", 0x03),
                        ("local", 0xFF),
                    ],
                )?],
            ),
            "set_timer_mode" => {
                let m = pick(
                    p,
                    "mode",
                    &[
                        ("timecode", 0x00),
                        ("timer1", 0x01),
                        ("timer2", 0x02),
                        ("local", 0xFF),
                    ],
                )?;
                Ok(Job::ack(Msg::new(4, 0x36, &[m])).then(Msg::new(6, 0x36, &[]), Read::TimerMode))
            }
            "set_timecode_mode" => ack(
                4,
                0x36,
                &[pick(
                    p,
                    "mode",
                    &[
                        ("ltc", 0x00),
                        ("timer", 0x01),
                        ("vitc", 0x02),
                        ("time_of_day", 0x03),
                    ],
                )?],
            ),
            "input_check" => ack(4, 0x37, &[u8::from(flag(p, "enabled")?)]),
            "set_edit_field" => ack(
                4,
                0x3A,
                &[pick(
                    p,
                    "field",
                    &[
                        ("either", 0x00),
                        ("field1", 0x01),
                        ("field2", 0x02),
                        ("local", 0xFF),
                    ],
                )?],
            ),
            "set_freeze_mode" => ack(
                4,
                0x3B,
                &[pick(
                    p,
                    "mode",
                    &[
                        ("either_field", 0x00),
                        ("field1", 0x01),
                        ("field2", 0x10),
                        ("frame", 0x11),
                    ],
                )?],
            ),
            "auto_mode" => ack(4, if flag(p, "enabled")? { 0x41 } else { 0x40 }, &[]),
            "spot_erase" => ack(4, if flag(p, "enabled")? { 0x43 } else { 0x42 }, &[]),
            "audio_split" => ack(4, if flag(p, "enabled")? { 0x45 } else { 0x44 }, &[]),
            "set_loop" => ack(4, 0x42, &[u8::from(flag(p, "enabled")?)]),
            "set_drop_frame" => ack(0, 0x06, &[u8::from(flag(p, "enabled")?)]),
            "get_status" => Ok(Job::read(status(), Read::Status)),
            "get_time" => {
                let which = pick(
                    p,
                    "source",
                    &[
                        ("best", 0x03),
                        ("ltc", 0x01),
                        ("vitc", 0x02),
                        ("timer1", 0x04),
                        ("timer2", 0x08),
                        ("ltc_user_bits", 0x10),
                        ("vitc_user_bits", 0x20),
                        ("windows_time", 0x08),
                        ("ltc_source", 0x40),
                    ],
                )?;
                read(6, 0x0C, &[which], Read::Time)
            }
            "get_generator" => read(6, 0x0A, &[0x11], Read::Generator),
            "get_point" => {
                let pt = pick(p, "point", POINT)?;
                Ok(Job::read(point_sense(pt), Read::Point(pt)))
            }
            "get_preroll_time" => read(6, 0x31, &[], Read::PrerollTime),
            "get_timer_mode" => read(6, 0x36, &[], Read::TimerMode),
            "send_raw" => {
                let bytes = unhex(text(p, "message")?)
                    .filter(|b| b.len() >= 2)
                    .ok_or_else(|| {
                        invalid("'message' is CMD-1, CMD-2 and the data as hex bytes")
                    })?;
                let msg = Msg {
                    cmd1: bytes[0],
                    cmd2: bytes[1],
                    data: bytes[2..].to_vec(),
                };
                Ok(Job::read(msg, Read::Raw))
            }
            // AMP's own commands.
            "get_device_id" if amp => read(0xA, 0x21, &[], Read::DeviceId),
            "load_clip" if amp => {
                let mut payload = Vec::new();
                for clip in clip_list(p)? {
                    payload.extend(name_field(&clip));
                }
                Ok(Job::ack(Msg::extended(4, 0xA, 0x14, &payload))
                    .then(Msg::new(0xA, 0x16, &[]), Read::Loaded))
            }
            "cue_clip" if amp => {
                let mut payload = vec![b'E'];
                payload.extend(name_len(text(p, "clip")?));
                payload.extend(timecode(text(p, "timecode")?)?);
                payload.extend(text(p, "clip")?.as_bytes());
                Ok(Job::ack(Msg::extended(4, 0xE, 0x14, &payload))
                    .then(Msg::new(0xA, 0x16, &[]), Read::Loaded))
            }
            "set_clip_out" if amp => {
                let mut payload = vec![b'E'];
                payload.extend(name_len(text(p, "clip")?));
                payload.extend(timecode(text(p, "timecode")?)?);
                payload.extend(text(p, "clip")?.as_bytes());
                Ok(Job::ack(Msg::extended(4, 0xE, 0x15, &payload)))
            }
            "reset_in" if amp => ack(4, 0x20, &[]),
            "reset_out" if amp => ack(4, 0x21, &[]),
            "preview_clip" if amp => {
                let mut payload = Vec::new();
                for clip in clip_list(p)? {
                    payload.extend(name_field(&clip));
                }
                Ok(Job::ack(Msg::extended(0xA, 0xA, 0x04, &payload)))
            }
            "preview_clip_at" if amp => {
                let mut payload = vec![b'E'];
                payload.extend(name_len(text(p, "clip")?));
                payload.extend(timecode(text(p, "timecode")?)?);
                payload.extend(text(p, "clip")?.as_bytes());
                Ok(Job::ack(Msg::extended(0xA, 0xE, 0x04, &payload)))
            }
            "set_preview_out" if amp => ack(0xA, 0x05, &timecode(text(p, "timecode")?)?),
            "reset_preview_in" if amp => ack(0xA, 0x06, &[]),
            "reset_preview_out" if amp => ack(0xA, 0x07, &[]),
            "record_cue" if amp => Ok(Job::ack(Msg::extended(
                0xA,
                0xA,
                0x02,
                &name_field(text(p, "clip")?),
            ))),
            "record_cue_at" if amp => {
                let mut payload = timecode(text(p, "timecode")?)?.to_vec();
                payload.extend(name_field(text(p, "clip")?));
                Ok(Job::ack(Msg::extended(0xA, 0xE, 0x02, &payload)))
            }
            "get_loaded_clip" if amp => read(0xA, 0x16, &[], Read::Loaded),
            "get_clip_start" if amp => Ok(Job::read(
                Msg::extended(0xA, 0x2, 0x25, text(p, "clip")?.as_bytes()),
                Read::ClipTime("start"),
            )),
            "get_clip_duration" if amp => Ok(Job::read(
                Msg::extended(0xA, 0x2, 0x17, text(p, "clip")?.as_bytes()),
                Read::ClipTime("duration"),
            )),
            "get_working_folder" if amp => read(0xA, 0x0F, &[], Read::WorkingFolder),
            "set_working_folder" if amp => {
                Ok(
                    Job::ack(Msg::extended(0xA, 0x2, 0x0E, text(p, "folder")?.as_bytes()))
                        .then(Msg::new(0xA, 0x0F, &[]), Read::WorkingFolder),
                )
            }
            "list_folders" if amp => read(0xA, 0x2A, &[], Read::Folders(Vec::new())),
            "list_clips" if amp => Ok(Job::read(
                Msg::extended(0xA, 0x2, 0x14, &[]),
                Read::Clips(Vec::new()),
            )),
            "get_clip_status" if amp => Ok(Job::read(
                Msg::extended(0xA, 0xA, 0x18, text(p, "clip")?.as_bytes()),
                Read::ClipStatus,
            )),
            "get_clip_data" if amp => {
                let mut payload = vec![b'C'];
                payload.extend(name_field(text(p, "clip")?));
                Ok(Job::read(
                    Msg::extended(0xA, 0xA, 0x13, &payload),
                    Read::ClipData,
                ))
            }
            "erase_clip" if amp => Ok(Job::ack(Msg::extended(
                0xA,
                0x2,
                0x10,
                text(p, "clip")?.as_bytes(),
            ))),
            "create_folder" if amp => Ok(Job::ack(Msg::extended(
                0xA,
                0x2,
                0x31,
                text(p, "folder")?.as_bytes(),
            ))),
            "delete_folder" if amp => Ok(Job::ack(Msg::extended(
                0xA,
                0x2,
                0x29,
                text(p, "folder")?.as_bytes(),
            ))),
            other => Err(CommandError::UnknownCommand {
                command: other.to_string(),
            }),
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Ready || self.in_flight.is_some() {
            return;
        }
        let Some(job) = self.commands.pop_front().or_else(|| self.polls.pop_front()) else {
            return;
        };
        self.buf.clear();
        cx.tcp_send(SOCKET, frame(&job.msg, self.wire == Wire::Hex));
        self.sent_at = cx.now();
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        self.in_flight = Some(job);
    }

    fn queue_poll(&mut self, msg: Msg, read: Read) {
        let same = |j: &Job| j.msg == msg;
        if self.polls.iter().any(same) || self.in_flight.as_ref().is_some_and(same) {
            return;
        }
        self.polls.push_back(Job::read(msg, read));
    }

    fn fast_poll(&mut self) {
        self.queue_poll(Msg::new(6, 0x20, &[self.status_request]), Read::Status);
        self.queue_poll(Msg::new(6, 0x0C, &[self.time_request()]), Read::Time);
    }

    fn slow_poll(&mut self) {
        let points: &[u8] = match self.dialect {
            Dialect::Sony => &[0, 1, 2, 3],
            Dialect::Amp => &[0, 1],
        };
        for pt in points {
            self.queue_poll(Msg::new(6, 0x10 + pt, &[]), Read::Point(*pt));
        }
        match self.dialect {
            Dialect::Sony => {
                self.queue_poll(Msg::new(6, 0x0C, &[0x04]), Read::Time);
                self.queue_poll(Msg::new(6, 0x0C, &[0x10]), Read::Time);
                self.queue_poll(Msg::new(6, 0x0A, &[0x11]), Read::Generator);
                self.queue_poll(Msg::new(6, 0x31, &[]), Read::PrerollTime);
                self.queue_poll(Msg::new(6, 0x36, &[]), Read::TimerMode);
                self.queue_poll(Msg::new(6, 0x30, &[0x02]), Read::EditPreset);
            }
            Dialect::Amp => {
                self.queue_poll(Msg::new(0xA, 0x16, &[]), Read::Loaded);
                self.queue_poll(Msg::new(0xA, 0x0F, &[]), Read::WorkingFolder);
            }
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        self.phase = Phase::Connecting;
        self.buf.clear();
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.to);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        let error = CommandError::Transport {
            message: reason.clone(),
        };
        if let Some(job) = self.in_flight.take() {
            if let Some(id) = job.command {
                cx.complete(id, Err(error.clone()));
            }
        }
        for job in self.commands.drain(..) {
            if let Some(id) = job.command {
                cx.complete(id, Err(error.clone()));
            }
        }
        self.polls.clear();
        cx.cancel_timer(REPLY_TIMER);
        self.phase = Phase::Closed;
        self.connected = false;
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// The link is ready for traffic: queue what monitoring asks first.
    fn ready(&mut self, cx: &mut Cx) {
        self.phase = Phase::Ready;
        self.queue_poll(Msg::new(0, 0x11, &[]), Read::DeviceType);
        if self.monitor {
            if self.dialect == Dialect::Amp {
                self.queue_poll(Msg::new(0xA, 0x21, &[]), Read::DeviceId);
            }
            self.fast_poll();
            self.slow_poll();
            cx.set_timer(FAST_TIMER, FAST_EVERY);
            cx.set_timer(SLOW_TIMER, SLOW_EVERY);
        } else {
            cx.set_timer(FAST_TIMER, PROBE_EVERY);
        }
        self.pump(cx);
    }

    /// One whole message at the front of the buffer, as bytes (CMD-1,
    /// CMD-2, data); `None` until it has all arrived.
    fn take_message(&mut self) -> Option<Result<Msg, String>> {
        match self.wire {
            Wire::Binary => {
                let n = data_len(self.dialect, &self.buf)?;
                let total = 2 + n + 1;
                if self.buf.len() < total {
                    return None;
                }
                let bytes: Vec<u8> = self.buf.drain(..total).collect();
                if checksum(&bytes[..total - 1]) != bytes[total - 1] {
                    return Some(Err(format!("a reply with a bad checksum: {}", hex(&bytes))));
                }
                Some(Ok(Msg {
                    cmd1: bytes[0],
                    cmd2: bytes[1],
                    data: bytes[2..total - 1].to_vec(),
                }))
            }
            Wire::Hex => {
                // Hexadecimal digits; line ends and spaces between replies
                // are skipped.
                self.buf.retain(|b| b.is_ascii_hexdigit());
                let digits = |buf: &[u8], from: usize, count: usize| -> Option<Vec<u8>> {
                    let s = std::str::from_utf8(buf.get(from..from + count * 2)?).ok()?;
                    unhex(s)
                };
                let head = digits(&self.buf, 0, (self.buf.len() / 2).min(4))?;
                let n = data_len(self.dialect, &head)?;
                let total = 2 + n;
                let bytes = digits(&self.buf, 0, total)?;
                self.buf.drain(..total * 2);
                Some(Ok(Msg {
                    cmd1: bytes[0],
                    cmd2: bytes[1],
                    data: bytes[2..].to_vec(),
                }))
            }
        }
    }

    fn on_data(&mut self, cx: &mut Cx, data: &[u8]) {
        self.buf.extend_from_slice(data);
        if self.phase == Phase::Channel {
            // The channel request's answer: 1001 (ACK) or 1111.
            let digits: Vec<u8> = self
                .buf
                .iter()
                .copied()
                .filter(u8::is_ascii_hexdigit)
                .collect();
            if digits.len() < 4 {
                return;
            }
            self.buf = digits[4..].to_vec();
            cx.cancel_timer(REPLY_TIMER);
            if &digits[..4] == b"1001" {
                self.ready(cx);
            } else {
                let answer = String::from_utf8_lossy(&digits[..4]).into_owned();
                cx.tcp_close(SOCKET);
                self.lost(
                    cx,
                    format!(
                        "the server refused AMP channel '{}' ({answer})",
                        self.channel
                    ),
                );
                return;
            }
        }
        loop {
            if self.in_flight.is_none() {
                self.buf.clear();
                return;
            }
            let Some(message) = self.take_message() else {
                return;
            };
            cx.cancel_timer(REPLY_TIMER);
            cx.round_trip(cx.now().saturating_sub(self.sent_at));
            cx.alive();
            if !self.connected {
                self.connected = true;
                self.retry_after = RETRY_MIN;
                cx.connection(Connection::Connected);
            }
            let job = self.in_flight.take().unwrap();
            self.answer(cx, job, message);
            self.pump(cx);
        }
    }

    /// A reply to the job in flight.
    fn answer(&mut self, cx: &mut Cx, mut job: Job, reply: Result<Msg, String>) {
        let reply = match reply {
            Ok(m) => m,
            Err(message) => {
                return self.finish(
                    cx,
                    job,
                    Err(CommandError::DeviceError {
                        code: None,
                        message,
                    }),
                );
            }
        };
        // NAK: 11 12 and the error bits.
        if reply.cmd1 == 0x11 && reply.cmd2 == 0x12 {
            let bits = reply.data.first().copied().unwrap_or(0);
            if job.read == Read::Status && self.status_request == 0x0A && job.command.is_none() {
                // A device keeping fewer status bytes: ask four from now on.
                self.status_request = 0x04;
            }
            return self.finish(
                cx,
                job,
                Err(CommandError::DeviceError {
                    code: Some(format!("{bits:02x}")),
                    message: nak_text(bits),
                }),
            );
        }
        // 70.0D: the time asked for is missing (no VITC, say).
        if reply.cmd1 == 0x70 && reply.cmd2 == 0x0D {
            return self.finish(
                cx,
                job,
                Err(CommandError::DeviceError {
                    code: Some("70.0d".into()),
                    message: "the device has no time data of that kind".into(),
                }),
            );
        }
        if job.read == Read::Raw {
            let mut bytes = vec![reply.cmd1, reply.cmd2];
            bytes.extend_from_slice(&reply.data);
            return self.finish(
                cx,
                job,
                Ok(Outcome::Value {
                    value: json!(hex(&bytes)),
                }),
            );
        }
        if reply.cmd1 == 0x10 && reply.cmd2 == 0x01 {
            if job.read != Read::Ack {
                return self.finish(cx, job, Ok(Outcome::Ack));
            }
            // A set's read back goes first, and its answer goes to state.
            let then = std::mem::take(&mut job.then);
            for (msg, read) in then.into_iter().rev() {
                if self.monitor || job.command.is_some() {
                    self.commands.push_front(Job::read(msg, read));
                }
            }
            return self.finish(cx, job, Ok(Outcome::Ack));
        }
        // AMP's failure returns: 80.xx with nothing.
        if reply.kind() == 0x8
            && reply.cmd1 & 0x0F == 0
            && !matches!(job.read, Read::Loaded | Read::Folders(_) | Read::Clips(_))
        {
            return self.finish(
                cx,
                job,
                Err(CommandError::DeviceError {
                    code: Some(format!("{:02x}.{:02x}", reply.cmd1, reply.cmd2)),
                    message: "the server found nothing for the request".into(),
                }),
            );
        }
        let read = job.read.clone();
        let value = match read {
            Read::DeviceType if reply.cmd1 == 0x12 && reply.cmd2 == 0x11 => {
                let d = &reply.data;
                let (d1, d2) = (
                    d.first().copied().unwrap_or(0),
                    d.get(1).copied().unwrap_or(0),
                );
                let mut v = json!({"device_type": hex(d)});
                if let Some(model) = model_of(d1, d2) {
                    v["model"] = json!(model);
                    v["line_625"] = json!(d1 & 0x0F == 1);
                }
                cx.state(json!({"device": v}));
                v
            }
            Read::Status if reply.kind() == 7 && reply.cmd2 == 0x20 => {
                let start = (self.status_request >> 4) as usize;
                let v = self.decode_status(start, &reply.data);
                cx.state(json!({"status": v.clone(), "transport": transport_of(&v)}));
                v
            }
            Read::Time if reply.kind() == 7 => self.time(cx, &reply),
            Read::Generator if reply.kind() == 7 => {
                let mut v = Map::new();
                match reply.cmd2 {
                    0x08 => {
                        v.insert("timecode".into(), json!(tc_text(&reply.data)));
                        if reply.data.len() >= 8 {
                            v.insert("user_bits".into(), json!(ub_text(&reply.data[4..])));
                        }
                    }
                    0x09 => {
                        v.insert("user_bits".into(), json!(ub_text(&reply.data)));
                    }
                    _ => {}
                }
                cx.state(json!({"generator": v.clone()}));
                Value::Object(v)
            }
            Read::Point(pt) if reply.kind() == 7 => {
                let name = POINT[pt as usize].0;
                let tc = tc_text(&reply.data);
                cx.state(json!({"points": {name: tc}}));
                json!(tc)
            }
            Read::PrerollTime if reply.kind() == 7 => {
                let tc = tc_text(&reply.data);
                cx.state(json!({"preroll_time": tc}));
                json!(tc)
            }
            Read::TimerMode if reply.kind() == 7 => {
                let mode = match reply.data.first() {
                    Some(0) => "timecode",
                    Some(1) => "timer1",
                    Some(2) => "timer2",
                    _ => "other",
                };
                cx.state(json!({"timer_mode": mode}));
                json!(mode)
            }
            Read::EditPreset if reply.kind() == 7 && reply.cmd2 == 0x30 => {
                let d1 = reply.data.first().copied().unwrap_or(0);
                let d2 = reply.data.get(1).copied().unwrap_or(0);
                let bit = |b: u8, n: u8| b & (1 << n) != 0;
                let v = json!({
                    "insert": bit(d1, 6), "assemble": bit(d1, 5), "video": bit(d1, 4),
                    "timecode": bit(d1, 2), "cue_a2": bit(d1, 1), "cue_a1": bit(d1, 0),
                    "audio4": bit(d2, 3), "audio3": bit(d2, 2), "audio2": bit(d2, 1), "audio1": bit(d2, 0),
                });
                cx.state(json!({"edit_preset": v.clone()}));
                v
            }
            Read::DeviceId if reply.kind() == 8 => {
                let id = String::from_utf8_lossy(&reply.data)
                    .trim_end_matches(['\0', ' '])
                    .to_string();
                cx.state(json!({"device_id": id}));
                json!(id)
            }
            Read::Loaded if reply.kind() == 8 => {
                let clip = names(reply.data.get(2..).unwrap_or(&[]))
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                cx.state(json!({"clip": {"loaded": clip}}));
                json!(clip)
            }
            Read::ClipTime(which) if reply.kind() == 8 => {
                if reply.data.len() >= 4 {
                    json!(tc_text(&reply.data))
                } else {
                    return self.finish(
                        cx,
                        job,
                        Err(CommandError::DeviceError {
                            code: None,
                            message: format!("no {which} for that clip"),
                        }),
                    );
                }
            }
            Read::WorkingFolder if reply.kind() == 8 => {
                let folder = names(reply.data.get(2..).unwrap_or(&[]))
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                cx.state(json!({"working_folder": folder}));
                json!(folder)
            }
            Read::Folders(mut acc) if reply.kind() == 8 => {
                if reply.cmd1 & 0x0F == 0 {
                    return self.listed(cx, job, "folders", acc);
                }
                acc.extend(names(reply.data.get(2..).unwrap_or(&[])));
                job.read = Read::Folders(acc);
                job.msg = Msg::new(0xA, 0x2B, &[]);
                self.commands.push_front(job);
                return;
            }
            Read::Clips(mut acc) if reply.kind() == 8 => {
                if reply.cmd1 & 0x0F == 0 {
                    return self.listed(cx, job, "clips", acc);
                }
                let payload = if matches!(reply.cmd1 & 0x0F, 0x2 | 0xA | 0xE) {
                    reply.data.get(2..).unwrap_or(&[]).to_vec()
                } else {
                    // Name format: eight characters, space padded.
                    let n = String::from_utf8_lossy(&reply.data).trim_end().to_string();
                    let mut v = (n.len() as u16).to_be_bytes().to_vec();
                    v.extend_from_slice(n.as_bytes());
                    v
                };
                acc.extend(names(&payload));
                job.read = Read::Clips(acc);
                job.msg = Msg::new(0xA, 0x15, &[0x0A]);
                self.commands.push_front(job);
                return;
            }
            Read::ClipStatus if reply.kind() == 8 => {
                json!(reply.data.first().copied().unwrap_or(0))
            }
            Read::ClipData if reply.kind() == 8 => clip_data(reply.data.get(2..).unwrap_or(&[])),
            _ => {
                let mut bytes = vec![reply.cmd1, reply.cmd2];
                bytes.extend_from_slice(&reply.data);
                return self.finish(
                    cx,
                    job,
                    Err(CommandError::DeviceError {
                        code: None,
                        message: format!("an unexpected reply: {}", hex(&bytes)),
                    }),
                );
            }
        };
        self.finish(cx, job, Ok(Outcome::Value { value }))
    }

    /// A list read whole (folders or clips): into state, replacing the last.
    fn listed(&mut self, cx: &mut Cx, job: Job, key: &str, list: Vec<String>) {
        let mut cleared = Map::new();
        cleared.insert(key.into(), Value::Null);
        cx.state(Value::Object(cleared));
        let items: Map<String, Value> = list
            .iter()
            .enumerate()
            .map(|(i, n)| ((i + 1).to_string(), json!(n)))
            .collect();
        let mut patch = Map::new();
        patch.insert(key.into(), Value::Object(items));
        cx.state(Value::Object(patch));
        self.finish(cx, job, Ok(Outcome::Value { value: json!(list) }));
    }

    fn time(&mut self, cx: &mut Cx, reply: &Msg) -> Value {
        let tc = tc_text(&reply.data);
        let (key, source) = match reply.cmd2 {
            0x00 => ("timer1", "timer1"),
            0x01 => ("timer2", "timer2"),
            0x04 => ("timecode", "ltc"),
            0x06 => ("timecode", "vitc"),
            0x14 => ("timecode", "ltc_corrected"),
            0x16 => ("timecode", "vitc_hold"),
            0x0A => ("windows_time", "windows_time"),
            0x0B => ("timecode", "ltc_source"),
            0x0C => ("timecode", "vitc_source"),
            0x05 | 0x15 => {
                let ub = ub_text(&reply.data);
                cx.state(json!({"user_bits": {"ltc": ub}}));
                return json!({"user_bits": ub, "source": "ltc"});
            }
            0x07 | 0x17 => {
                let ub = ub_text(&reply.data);
                cx.state(json!({"user_bits": {"vitc": ub}}));
                return json!({"user_bits": ub, "source": "vitc"});
            }
            _ => ("timecode", "other"),
        };
        let drop = reply.data.first().is_some_and(|f| f & 0x40 != 0);
        let mut patch = Map::new();
        patch.insert(key.into(), json!(tc));
        if key == "timecode" {
            patch.insert("timecode_source".into(), json!(source));
            patch.insert("drop_frame".into(), json!(drop));
            // 78.xx carries the user bits after the time.
            if reply.kind() == 7 && reply.cmd1 & 0x0F == 8 {
                let ub = ub_text(&reply.data[4..]);
                let which = if source.starts_with("vitc") {
                    "vitc"
                } else {
                    "ltc"
                };
                patch.insert("user_bits".into(), json!({ which: ub }));
            }
        }
        cx.state(Value::Object(patch));
        json!({"timecode": tc, "source": source, "drop_frame": drop})
    }

    fn decode_status(&self, start: usize, data: &[u8]) -> Value {
        let bits = match self.dialect {
            Dialect::Sony => SONY_BITS,
            Dialect::Amp => AMP_BITS,
        };
        let mut v = Map::new();
        for (byte, bit, name) in bits {
            if *byte < start {
                continue;
            }
            if let Some(b) = data.get(byte - start) {
                v.insert((*name).into(), json!(b & (1 << bit) != 0));
            }
        }
        Value::Object(v)
    }

    fn finish(&mut self, cx: &mut Cx, job: Job, result: Result<Outcome, CommandError>) {
        match (job.command, result) {
            (Some(id), r) => cx.complete(id, r),
            (None, Err(e)) => cx.log(Level::Debug, format!("VTR: {e}")),
            _ => {}
        }
    }
}

/// The transport state the status bits show, in one word.
fn transport_of(status: &Value) -> &'static str {
    let on = |k: &str| status.get(k).and_then(Value::as_bool).unwrap_or(false);
    if on("eject") {
        "eject"
    } else if on("record") {
        "record"
    } else if on("fast_forward") {
        "fast_forward"
    } else if on("rewind") {
        "rewind"
    } else if on("shuttle") {
        "shuttle"
    } else if on("jog") {
        "jog"
    } else if on("var") {
        "var"
    } else if on("play") {
        "play"
    } else if on("still") {
        "still"
    } else if on("stop") {
        "stop"
    } else if on("standby") {
        "standby"
    } else {
        "unknown"
    }
}

/// The clip names a command gives: `clips` (several) or `clip` (one).
fn clip_list(p: &Params) -> Result<Vec<String>, CommandError> {
    if let Some(list) = p.get("clips").and_then(Value::as_array) {
        let clips: Vec<String> = list
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        if clips.is_empty() || clips.len() != list.len() {
            return Err(invalid("'clips' is a list of clip names"));
        }
        return Ok(clips);
    }
    let clip = text(p, "clip")?;
    if clip.is_empty() {
        return Err(invalid("give 'clip' or 'clips'"));
    }
    Ok(vec![clip.to_string()])
}

fn name_len(name: &str) -> [u8; 2] {
    (name.len() as u16).to_be_bytes()
}

/// AMP's clip data (8A.13 for 'C'): dates, duration, format and flags.
fn clip_data(d: &[u8]) -> Value {
    if d.len() < 33 {
        return json!({"data": hex(d)});
    }
    let filetime = |b: &[u8]| u64::from_le_bytes(b.try_into().unwrap_or([0; 8]));
    let compression = [
        "mpeg1", "mpeg2", "d10", "dv25", "dv50", "dvcam", "mpeg2hd", "dv100", "avci",
    ];
    let status = u32::from_be_bytes(d[29..33].try_into().unwrap_or([0; 4]));
    json!({
        "created": filetime(&d[0..8]),
        "modified": filetime(&d[8..16]),
        "duration": tc_text(&d[16..20]),
        "compression": compression.get(d[20] as usize).copied().unwrap_or("other"),
        "video_format": d[21],
        "video_tracks": d[22],
        "audio_tracks": d[23],
        "ancillary_tracks": d[24],
        "timecode_tracks": d[25],
        "ready_to_play": status & 1 != 0,
        "under_construction": status & 2 != 0,
        "read_only": status & 4 != 0,
        "locked": status & 8 != 0,
        "drop_frame": status & 16 != 0,
    })
}

impl Module for Vtr {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.phase == Phase::Closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.job(name, params) {
            Ok(mut job) => {
                job.command = Some(id);
                self.commands.push_back(job);
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                if self.phase != Phase::Connecting {
                    return;
                }
                if self.wire == Wire::Hex {
                    // Open the AMP channel, or a channel-less session.
                    let request = if self.channel.is_empty() {
                        "CRAT00014\n".to_string()
                    } else {
                        let body = format!("2{:02}{}", self.channel.len(), self.channel);
                        format!("CRAT{:04}{body}\n", body.len())
                    };
                    self.phase = Phase::Channel;
                    self.sent_at = cx.now();
                    cx.tcp_send(SOCKET, request.into_bytes());
                    cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
                } else {
                    // A converter's connection shows nothing of the
                    // device: it is reported connected once it answers.
                    self.ready(cx);
                }
            }
            TcpInput::Data(data) => {
                if matches!(self.phase, Phase::Ready | Phase::Channel) {
                    self.on_data(cx, &data);
                }
            }
            TcpInput::Closed { reason } => {
                if self.phase != Phase::Closed {
                    self.lost(cx, reason);
                }
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => {
                if self.phase == Phase::Closed {
                    self.open(cx);
                }
            }
            FAST_TIMER => {
                if self.phase != Phase::Ready {
                    return;
                }
                if self.monitor {
                    self.fast_poll();
                    cx.set_timer(FAST_TIMER, FAST_EVERY);
                } else {
                    let idle = self.in_flight.is_none() && self.commands.is_empty();
                    if idle && cx.now().saturating_sub(self.sent_at) >= PROBE_EVERY {
                        self.queue_poll(Msg::new(0, 0x11, &[]), Read::DeviceType);
                    }
                    cx.set_timer(FAST_TIMER, PROBE_EVERY);
                }
                self.pump(cx);
            }
            SLOW_TIMER => {
                if self.phase == Phase::Ready && self.monitor {
                    self.slow_poll();
                    cx.set_timer(SLOW_TIMER, SLOW_EVERY);
                    self.pump(cx);
                }
            }
            REPLY_TIMER => {
                if self.phase == Phase::Channel {
                    cx.tcp_close(SOCKET);
                    self.lost(
                        cx,
                        "the AMP server did not answer the channel request".into(),
                    );
                    return;
                }
                if let Some(job) = self.in_flight.take() {
                    self.buf.clear();
                    if let Some(id) = job.command {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    if self.connected {
                        self.connected = false;
                        cx.connection(Connection::Disconnected {
                            reason: "the device did not answer".into(),
                        });
                    }
                    self.pump(cx);
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Closed {
            if self.wire == Wire::Hex && self.phase == Phase::Ready {
                cx.tcp_send(SOCKET, b"STOP0000\n".to_vec());
            }
            cx.tcp_close(SOCKET);
        }
        self.phase = Phase::Closed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use std::net::{IpAddr, Ipv4Addr};

    fn module(spec: &str, monitor: bool, settings: Value) -> Vtr {
        let s = Catalog::source_tree().devices[spec].clone();
        let settings = validate(&s.settings, settings.as_object().unwrap()).unwrap();
        let dialect = if spec == "grassvalley-amp" {
            Dialect::Amp
        } else {
            Dialect::Sony
        };
        Vtr::new(
            dialect,
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: Some(5000),
                model: s.models[0].id.clone(),
                channels: None,
                settings,
                monitor,
            },
        )
        .unwrap()
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn bin(bytes: &[u8]) -> Vec<u8> {
        let mut v = bytes.to_vec();
        v.push(checksum(bytes));
        v
    }

    fn feed(m: &mut Vtr, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut Vtr, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    /// A Sony deck opened for commands only, past its device type request.
    #[cfg(feature = "sony-9pin")]
    fn sony_idle() -> Vtr {
        let mut m = module("sony-9pin", false, json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&cx.take()), [bin(&[0x00, 0x11])]);
        let a = feed(&mut m, 0, &bin(&[0x12, 0x11, 0x20, 0x25]));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::State(
            json!({"device": {"device_type": "2025", "model": "BVW-75", "line_625": false}})
        )));
        m
    }

    #[test]
    fn frames_carry_a_checksum_or_go_as_hex() {
        // Play, and cue up to 02:16:58:13 (the bytes of Drastic's worked
        // example; its printed checksum leaves out CMD-2).
        assert_eq!(frame(&Msg::new(2, 0x01, &[]), false), [0x20, 0x01, 0x21]);
        let cue = Msg::new(2, 0x31, &timecode("02:16:58:13").unwrap());
        assert_eq!(
            frame(&cue, false),
            [0x24, 0x31, 0x13, 0x58, 0x16, 0x02, 0xD8]
        );
        assert_eq!(frame(&Msg::new(2, 0x01, &[]), true), b"CMDS00042001\n");
        // SimplyLive's example: an ID status request for clip01, extended.
        let status = Msg::extended(0xA, 0xA, 0x18, b"clip01");
        assert_eq!(frame(&status, true), b"CMDS0020aa180006636c69703031\n");
    }

    #[test]
    fn speeds_follow_the_documented_curve() {
        assert_eq!(speed_bytes(1.0), [64]);
        assert_eq!(speed_bytes(0.1), [32]);
        assert_eq!(speed_bytes(0.0), [0]);
        let fine = speed_bytes(1.5);
        assert_eq!(fine[0], 69);
        assert_eq!(fine.len(), 2);
    }

    #[test]
    fn user_bits_round_trip() {
        let b = ub_bytes("06364454").unwrap();
        assert_eq!(b, [0x60, 0x63, 0x44, 0x45]);
        assert_eq!(ub_text(&b), "06364454");
    }

    #[test]
    #[cfg(feature = "sony-9pin")]
    fn commands_send_their_bytes_and_read_back() {
        let mut m = sony_idle();
        let a = run(
            &mut m,
            1,
            "shuttle",
            json!({"direction": "reverse", "rate": 1.0}),
        );
        assert_eq!(sent(&a), [bin(&[0x21, 0x23, 0x40])]);
        let a = feed(&mut m, 0, &bin(&[0x10, 0x01]));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        // A point set is read back.
        let a = run(
            &mut m,
            2,
            "set_point",
            json!({"point": "in", "timecode": "04:25:16:21"}),
        );
        assert_eq!(sent(&a), [bin(&[0x44, 0x14, 0x21, 0x16, 0x25, 0x04])]);
        let a = feed(&mut m, 0, &bin(&[0x10, 0x01]));
        assert_eq!(completed(&a, 2), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a), [bin(&[0x60, 0x10])]);
        let a = feed(&mut m, 0, &bin(&[0x74, 0x10, 0x21, 0x16, 0x25, 0x04]));
        assert!(a.contains(&Action::State(json!({"points": {"in": "04:25:16:21"}}))));
        // A NAK fails the command with its error bits.
        run(&mut m, 3, "eject", json!({}));
        let a = feed(&mut m, 0, &bin(&[0x11, 0x12, 0x01]));
        assert!(matches!(
            completed(&a, 3),
            Some(Err(CommandError::DeviceError { code: Some(c), .. })) if c == "01"
        ));
    }

    #[test]
    #[cfg(feature = "sony-9pin")]
    fn status_bytes_become_state() {
        let mut m = sony_idle();
        run(&mut m, 1, "get_status", json!({}));
        let reply = [0x7A, 0x20, 0, 0x01, 0x80, 0, 0, 0, 0, 0, 0, 0];
        let a = feed(&mut m, 0, &bin(&reply));
        let state = a
            .iter()
            .find_map(|a| match a {
                Action::State(v) => Some(v.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(state["transport"], "play");
        assert_eq!(state["status"]["servo_lock"], true);
        assert_eq!(state["status"]["record"], false);
    }

    #[test]
    #[cfg(feature = "sony-9pin")]
    fn opened_for_commands_only_it_asks_only_the_device_type() {
        let mut m = sony_idle();
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, FAST_TIMER);
        assert_eq!(sent(&cx.take()), [bin(&[0x00, 0x11])]);
        // Monitored, the status and time follow the device type.
        let mut m = module("sony-9pin", true, json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        let a = feed(&mut m, 0, &bin(&[0x12, 0x11, 0x20, 0x25]));
        assert_eq!(sent(&a), [bin(&[0x61, 0x20, 0x0A])]);
    }

    #[test]
    #[cfg(feature = "sony-9pin")]
    fn the_time_to_each_answer_is_reported() {
        let mut m = sony_idle();
        let mut cx = Cx::new(1_000);
        m.command(
            &mut cx,
            1,
            "get_time",
            json!({"source": "best"}).as_object().unwrap(),
        );
        let a = feed(&mut m, 1_012, &bin(&[0x74, 0x04, 0x10, 0x20, 0x30, 0x01]));
        assert!(a.contains(&Action::RoundTrip(12)));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!({"timecode": "01:30:20:10", "source": "ltc", "drop_frame": false})
            }))
        );
    }

    #[test]
    #[cfg(feature = "grassvalley-amp")]
    fn amp_opens_its_channel_and_lists_clips_in_hex() {
        let mut m = module("grassvalley-amp", false, json!({"channel": "Vtr2"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&cx.take()), [b"CRAT0007204Vtr2\n".to_vec()]);
        let a = feed(&mut m, 0, b"1001");
        assert_eq!(sent(&a), [b"CMDS00040011\n".to_vec()]);
        let a = feed(&mut m, 0, b"12112050");
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // A clip list: first, then next until none is left.
        let a = run(&mut m, 1, "list_clips", json!({}));
        assert_eq!(sent(&a), [b"CMDS0008a2140000\n".to_vec()]);
        let a = feed(&mut m, 0, b"8a14000a00036f6e65000374776f");
        assert_eq!(sent(&a), [b"CMDS0006a1150a\n".to_vec()]);
        let a = feed(&mut m, 0, b"8014");
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!(["one", "two"])
            }))
        );
        assert!(a.contains(&Action::State(json!({"clips": {"1": "one", "2": "two"}}))));
        // Loading a clip is In Preset with the extended name, then the loaded clip is read.
        let a = run(&mut m, 2, "load_clip", json!({"clip": "clip01"}));
        assert_eq!(sent(&a), [b"CMDS00244a1400080006636c69703031\n".to_vec()]);
        let a = feed(&mut m, 0, b"1001");
        assert_eq!(completed(&a, 2), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a), [b"CMDS0004a016\n".to_vec()]);
        let a = feed(&mut m, 0, b"821600080006636c69703031");
        assert!(a.contains(&Action::State(json!({"clip": {"loaded": "clip01"}}))));
    }
}
