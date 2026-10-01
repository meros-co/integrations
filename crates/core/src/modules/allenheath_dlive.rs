//! dLive and Avantis, from "dLive MIDI Over TCP/IP Protocol, Firmware V2.0"
//! and "Avantis MIDI TCP/IP Protocol" (firmware V2.0 and later). Page numbers
//! are the dLive document's.
//!
//! - The base MIDI channel N is set on the console; the channel type is N
//!   plus an offset 0-4 and the channel the note number (p.1-2). Avantis has
//!   fewer channels of each type and puts DCAs, mute groups and their
//!   assignment values elsewhere.
//! - Mute: Note On, velocity 7F on or 3F off, then velocity 00; received
//!   velocity 40-7F is on, 01-3F off, 00 and Note Off ignored (p.2).
//! - Fader (NRPN 17), main assignment (18), DCA and mute group assignment
//!   (40), PEQ (1A-29), HPF (30, 31): NRPN with the channel as parameter MSB
//!   and a 7-bit value (p.2-7).
//! - SysEx header `F0 00 00 1A 50 10 01 00`, then `0N` (the channel type's
//!   MIDI channel): names, colours, send levels and assignments, preamp pad
//!   and phantom power, and every "get" (`0N 05 ...`) (p.2-7). Avantis
//!   documents only the name and colour gets.
//! - Preamp gain is Pitch Bend on the base channel, `EN MP GV` (p.4).
//! - Scenes are bank (CC 00) and Program Change on the base channel; the
//!   MixRack and Avantis recall scenes 1-500 and send the same message when a
//!   scene is recalled on the console; a dLive Surface recalls cues by
//!   Recall Id 0-1999 instead (p.4-5).

use serde_json::Value;

use super::allenheath::{invalid, midi_channel, setting_flag, unknown, Args, Dialect, Plan};
use super::allenheath_midi::{self as midi, level_updates, Event, Law, Update};
use crate::catalog::Params;
use crate::module::CommandError;

const HEADER: [u8; 7] = [0x00, 0x00, 0x1A, 0x50, 0x10, 0x01, 0x00];

pub(crate) const MIXRACK_PORT: u16 = 51325;
pub(crate) const SURFACE_PORT: u16 = 51328;

/// dLive p.9 and Avantis "Fader Level": LV = [(dB + 54) / 64] * 7F. The
/// documents' tables agree with this rounded down (except dLive's "+5 74",
/// whose decimal 117 is 75 hex), and list 00 as -inf.
pub(crate) const FADER: Law = Law::Floor {
    lo: -54.0,
    hi: 10.0,
    max: 127,
    floor_raw: 0,
};

/// dLive p.9 "Gain Value": +5 to +60 dB, the table's own values. Its formula
/// [(Gain - 5) / 55] * 7F rounds inconsistently against the table (+40 is 50
/// but +30 is 3A), so the table is used.
pub(crate) static GAIN_TABLE: &[(f64, u16)] = &[
    (5.0, 0x00),
    (10.0, 0x0C),
    (15.0, 0x17),
    (20.0, 0x22),
    (25.0, 0x2E),
    (30.0, 0x3A),
    (35.0, 0x45),
    (40.0, 0x50),
    (45.0, 0x5C),
    (50.0, 0x67),
    (55.0, 0x73),
    (60.0, 0x7F),
];
const GAIN: Law = Law::Table(GAIN_TABLE);

/// dLive p.6: PEQ gain Vv = (GAIN + 15) * 126 / 30. The table's "15db 126 7F"
/// gives 126 in decimal; 7F is a typo for 7E.
const PEQ_GAIN: Law = Law::Linear {
    lo: -15.0,
    hi: 15.0,
    max: 126,
};

pub(crate) const COLOURS: [&str; 8] = [
    "off",
    "red",
    "green",
    "yellow",
    "blue",
    "purple",
    "light_blue",
    "white",
];

/// dLive p.6: PEQ width values 00-18.
pub(crate) const WIDTHS: [&str; 25] = [
    "1.5", "1.4", "1.3", "1.2", "1.1", "1", "0.95", "0.9", "0.85", "0.8", "3/4", "0.7", "2/3",
    "0.6", "0.55", "0.5", "0.45", "0.4", "1/3", "0.3", "1/4", "0.2", "1/6", "0.13", "1/9",
];

/// dLive p.6: PEQ types and the bands each may be used on.
const PEQ_TYPES: [(&str, &[u8]); 5] = [
    ("bell", &[0, 3]),
    ("lf_shelf", &[0]),
    ("hf_shelf", &[3]),
    ("low_pass", &[3]),
    ("high_pass", &[0]),
];

const KEYS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const SCALES: [&str; 3] = ["major", "minor", "chromatic"];

/// A channel type: MIDI channel offset from N, first note, count.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ChType {
    pub name: &'static str,
    pub offset: u8,
    pub first: u8,
    pub count: u8,
}

const fn t(name: &'static str, offset: u8, first: u8, count: u8) -> ChType {
    ChType {
        name,
        offset,
        first,
        count,
    }
}

/// dLive p.2 "Channel Selection".
const DLIVE_TYPES: &[ChType] = &[
    t("input", 0, 0x00, 128),
    t("mono_group", 1, 0x00, 62),
    t("stereo_group", 1, 0x40, 31),
    t("mono_aux", 2, 0x00, 62),
    t("stereo_aux", 2, 0x40, 31),
    t("mono_matrix", 3, 0x00, 62),
    t("stereo_matrix", 3, 0x40, 31),
    t("mono_fx_send", 4, 0x00, 16),
    t("stereo_fx_send", 4, 0x10, 16),
    t("fx_return", 4, 0x20, 16),
    t("main", 4, 0x30, 6),
    t("dca", 4, 0x36, 24),
    t("mute_group", 4, 0x4E, 8),
    t("ufx_send", 4, 0x56, 8),
    t("ufx_return", 4, 0x5E, 8),
];

/// Avantis "Channel Selection".
const AVANTIS_TYPES: &[ChType] = &[
    t("input", 0, 0x00, 96),
    t("mono_group", 1, 0x00, 54),
    t("stereo_group", 1, 0x40, 27),
    t("mono_aux", 2, 0x00, 54),
    t("stereo_aux", 2, 0x40, 27),
    t("mono_matrix", 3, 0x00, 54),
    t("stereo_matrix", 3, 0x40, 27),
    t("mono_fx_send", 4, 0x00, 12),
    t("stereo_fx_send", 4, 0x10, 12),
    t("fx_return", 4, 0x20, 12),
    t("main", 4, 0x30, 3),
    t("dca", 4, 0x36, 16),
    t("mute_group", 4, 0x46, 8),
    t("ufx_send", 4, 0x56, 8),
    t("ufx_return", 4, 0x5E, 8),
];

const SEND_LEVEL_DESTINATIONS: &[&str] = &[
    "mono_aux",
    "stereo_aux",
    "mono_matrix",
    "stereo_matrix",
    "mono_fx_send",
    "stereo_fx_send",
    "ufx_send",
];
const SEND_ASSIGN_DESTINATIONS: &[&str] = &["mono_group", "stereo_group", "mono_aux", "stereo_aux"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Console {
    MixRack,
    Surface,
    Avantis,
}

pub(crate) struct Dlive {
    console: Console,
    /// Base MIDI channel N, 0-based.
    base: u8,
    types: &'static [ChType],
    sync_preamps: bool,
}

fn peq_param(id: u8) -> Option<(u8, &'static str)> {
    (0x1A..=0x29).contains(&id).then(|| {
        let i = id - 0x1A;
        (
            i / 4 + 1,
            ["type", "frequency", "width", "gain"][(i % 4) as usize],
        )
    })
}

/// dLive p.6: Vv = INT(127 * ((4608 * LOG10(F / 4) / LOG10(2)) - 10699) /
/// divisor), with 45922 for PEQ and 41314 for the HPF (p.7).
fn freq_to_raw(hz: f64, divisor: f64) -> Option<u8> {
    let v = (127.0 * (4608.0 * (hz / 4.0).log2() - 10699.0) / divisor).floor();
    (0.0..=127.0).contains(&v).then_some(v as u8)
}

/// The lowest frequency that gives `raw`, rounded to 0.1 Hz.
fn raw_to_freq(raw: u8, divisor: f64) -> f64 {
    let hz = 4.0 * 2f64.powf((raw as f64 * divisor / 127.0 + 10699.0) / 4608.0);
    (hz * 10.0).round() / 10.0
}

impl Dlive {
    pub(crate) fn new(model: &str, settings: &Params) -> Result<Dlive, String> {
        let console = match model {
            "dlive-mixrack" => Console::MixRack,
            "dlive-surface" => Console::Surface,
            "avantis" => Console::Avantis,
            other => return Err(format!("unknown dLive/Avantis model '{other}'")),
        };
        Ok(Dlive {
            console,
            // N to N+4 must fit in 16 channels; Avantis says N "cannot exceed 12".
            base: midi_channel(settings, 12)?,
            types: if console == Console::Avantis {
                AVANTIS_TYPES
            } else {
                DLIVE_TYPES
            },
            sync_preamps: setting_flag(settings, "sync_preamps", true),
        })
    }

    fn avantis(&self) -> bool {
        self.console == Console::Avantis
    }

    fn sx(&self, body: &[u8]) -> Vec<u8> {
        midi::sysex(&HEADER, body)
    }

    fn find(&self, name: &str) -> Option<&'static ChType> {
        self.types.iter().find(|t| t.name == name)
    }

    /// (MIDI channel, note, type) for a channel type and 1-based number.
    fn channel(&self, kind: &str, number: i64) -> Result<(u8, u8, &'static ChType), CommandError> {
        let t = self
            .find(kind)
            .ok_or_else(|| invalid(format!("this console has no '{kind}' channels")))?;
        if number < 1 || number > t.count as i64 {
            return Err(invalid(format!(
                "{kind} {number} is outside 1 to {}",
                t.count
            )));
        }
        Ok((self.base + t.offset, t.first + (number - 1) as u8, t))
    }

    fn arg_channel(&self, a: &Args) -> Result<(u8, u8, &'static ChType, i64), CommandError> {
        let n = a.int("channel")?;
        let (ch, note, t) = self.channel(a.str("channel_type")?, n)?;
        Ok((ch, note, t, n))
    }

    fn arg_destination(
        &self,
        a: &Args,
        allowed: &[&str],
    ) -> Result<(u8, u8, &'static ChType, i64), CommandError> {
        let kind = a.str("destination_type")?;
        if !allowed.contains(&kind) {
            return Err(invalid(format!(
                "destination_type must be one of {allowed:?}"
            )));
        }
        let n = a.int("destination")?;
        let (ch, note, t) = self.channel(kind, n)?;
        Ok((ch, note, t, n))
    }

    /// The type and 1-based number a MIDI channel and note address.
    fn decode(&self, midi_ch: u8, note: u8) -> Option<(&'static str, u32)> {
        let offset = midi_ch.checked_sub(self.base)?;
        self.types
            .iter()
            .find(|t| t.offset == offset && note >= t.first && note < t.first + t.count)
            .map(|t| (t.name, (note - t.first) as u32 + 1))
    }

    fn path(kind: &str, n: impl std::fmt::Display) -> String {
        format!("channels.{kind}.{n}")
    }

    /// DCA count and the NRPN 40 values: (DCA on, mute group off, mute group on).
    fn assign_values(&self) -> (u8, u8, u8) {
        if self.avantis() {
            (16, 0x10, 0x50)
        } else {
            (24, 0x18, 0x58)
        }
    }

    fn socket(a: &Args) -> Result<(u8, i64), CommandError> {
        let s = a.int("socket")?;
        if !(1..=128).contains(&s) {
            return Err(invalid("socket is 1-128"));
        }
        Ok(((s - 1) as u8, s))
    }

    fn get(&self, ch: u8, body: &[u8]) -> Vec<u8> {
        let mut b = vec![ch, 0x05];
        b.extend_from_slice(body);
        self.sx(&b)
    }

    fn level_arg(a: &Args, prefix: &str, law: &Law) -> Result<u8, CommandError> {
        a.level(prefix, law, 127).map(|v| v as u8)
    }

    fn peq_band(a: &Args) -> Result<u8, CommandError> {
        let b = a.int("band")?;
        if !(1..=4).contains(&b) {
            return Err(invalid("band is 1-4"));
        }
        Ok((b - 1) as u8)
    }

    fn index_of(list: &[&str], value: &str, what: &str) -> Result<u8, CommandError> {
        list.iter()
            .position(|v| *v == value)
            .map(|i| i as u8)
            .ok_or_else(|| invalid(format!("unknown {what} '{value}'")))
    }

    fn sync_channel(&self, out: &mut Vec<Vec<u8>>, t: &ChType, ch: u8, note: u8) {
        out.push(self.sx(&[ch, 0x01, note]));
        out.push(self.sx(&[ch, 0x04, note]));
        if self.avantis() {
            return;
        }
        out.push(self.get(ch, &[0x09, note]));
        if t.name != "mute_group" {
            out.push(self.get(ch, &[0x0B, 0x17, note]));
        }
        if t.name == "input" {
            out.push(self.get(ch, &[0x0B, 0x18, note]));
        }
    }
}

impl Dialect for Dlive {
    fn port(&self) -> u16 {
        match self.console {
            Console::Surface => SURFACE_PORT,
            _ => MIXRACK_PORT,
        }
    }

    fn command(&mut self, name: &str, a: &Args) -> Result<Plan, CommandError> {
        let n = self.base;
        Ok(match name {
            "set_mute" => {
                let (ch, note, ..) = self.arg_channel(a)?;
                let vel = if a.flag("muted")? { 0x7F } else { 0x3F };
                let mut b = midi::note_on(ch, note, vel);
                b.extend(midi::note_on(ch, note, 0x00));
                if self.avantis() {
                    Plan::write(b)
                } else {
                    Plan::write_then(b, self.get(ch, &[0x09, note]))
                }
            }
            "get_mute" => {
                let (ch, note, t, num) = self.arg_channel(a)?;
                Plan::read(
                    self.get(ch, &[0x09, note]),
                    format!("{}.mute", Self::path(t.name, num)),
                )
            }
            "set_level" => {
                let (ch, note, ..) = self.arg_channel(a)?;
                let lv = Self::level_arg(a, "level", &FADER)?;
                let b = midi::nrpn(ch, note, 0x17, lv);
                if self.avantis() {
                    Plan::write(b)
                } else {
                    Plan::write_then(b, self.get(ch, &[0x0B, 0x17, note]))
                }
            }
            "get_level" => {
                let (ch, note, t, num) = self.arg_channel(a)?;
                Plan::read_fields(
                    self.get(ch, &[0x0B, 0x17, note]),
                    format!("{}.level_raw", Self::path(t.name, num)),
                    &["level_db", "level_raw"],
                )
            }
            "set_main_assign" => {
                let (ch, note, ..) = self.arg_channel(a)?;
                let v = if a.flag("assigned")? { 0x7F } else { 0x3F };
                let b = midi::nrpn(ch, note, 0x18, v);
                if self.avantis() {
                    Plan::write(b)
                } else {
                    Plan::write_then(b, self.get(ch, &[0x0B, 0x18, note]))
                }
            }
            "get_main_assign" => {
                let (ch, note, t, num) = self.arg_channel(a)?;
                Plan::read(
                    self.get(ch, &[0x0B, 0x18, note]),
                    format!("{}.main_assign", Self::path(t.name, num)),
                )
            }
            "set_send_level" | "get_send_level" => {
                let (ch, note, t, num) = self.arg_channel(a)?;
                let (dch, dnote, dt, dnum) = self.arg_destination(a, SEND_LEVEL_DESTINATIONS)?;
                let get = self.get(ch, &[0x0F, 0x0D, note, dch, dnote]);
                if name == "get_send_level" {
                    Plan::read_fields(
                        get,
                        format!("sends.{}.{num}.{}.{dnum}.level_raw", t.name, dt.name),
                        &["level_db", "level_raw"],
                    )
                } else {
                    let lv = Self::level_arg(a, "level", &FADER)?;
                    let b = self.sx(&[ch, 0x0D, note, dch, dnote, lv]);
                    if self.avantis() {
                        Plan::write(b)
                    } else {
                        Plan::write_then(b, get)
                    }
                }
            }
            "set_send_assign" | "get_send_assign" => {
                let (ch, note, t, num) = self.arg_channel(a)?;
                let (dch, dnote, dt, dnum) = self.arg_destination(a, SEND_ASSIGN_DESTINATIONS)?;
                let get = self.get(ch, &[0x0F, 0x0E, note, dch, dnote]);
                if name == "get_send_assign" {
                    Plan::read(
                        get,
                        format!("sends.{}.{num}.{}.{dnum}.assigned", t.name, dt.name),
                    )
                } else {
                    let v = if a.flag("assigned")? { 0x7F } else { 0x3F };
                    Plan::write_then(self.sx(&[ch, 0x0E, note, dch, dnote, v]), get)
                }
            }
            "set_dca_assign" => {
                let (ch, note, ..) = self.arg_channel(a)?;
                let (dcas, ..) = self.assign_values();
                let d = a.int("dca")?;
                if d < 1 || d > dcas as i64 {
                    return Err(invalid(format!("dca is 1-{dcas}")));
                }
                let v = (d - 1) as u8 + if a.flag("assigned")? { 0x40 } else { 0x00 };
                Plan::write(midi::nrpn(ch, note, 0x40, v))
            }
            "set_mute_group_assign" => {
                let (ch, note, ..) = self.arg_channel(a)?;
                let (_, off, on) = self.assign_values();
                let g = a.int("mute_group")?;
                if !(1..=8).contains(&g) {
                    return Err(invalid("mute_group is 1-8"));
                }
                let v = (g - 1) as u8 + if a.flag("assigned")? { on } else { off };
                Plan::write(midi::nrpn(ch, note, 0x40, v))
            }
            "set_preamp_gain" | "get_preamp_gain" => {
                let (mp, s) = Self::socket(a)?;
                // p.4 prints the get as "0N, 05, 0B, 19, CH"; the socket
                // number MP is sent in the CH position.
                let get = self.get(n, &[0x0B, 0x19, mp]);
                if name == "get_preamp_gain" {
                    Plan::read_fields(
                        get,
                        format!("preamps.{s}.gain_raw"),
                        &["gain_db", "gain_raw"],
                    )
                } else {
                    let gv = Self::level_arg(a, "gain", &GAIN)?;
                    Plan::write_then(vec![0xE0 | n, mp, gv], get)
                }
            }
            "set_preamp_pad" | "get_preamp_pad" => {
                let (mp, s) = Self::socket(a)?;
                let get = self.sx(&[n, 0x07, mp]);
                if name == "get_preamp_pad" {
                    Plan::read(get, format!("preamps.{s}.pad"))
                } else {
                    let v = if a.flag("enabled")? { 0x7F } else { 0x00 };
                    Plan::write_then(self.sx(&[n, 0x09, mp, v]), get)
                }
            }
            "set_phantom_power" | "get_phantom_power" => {
                let (mp, s) = Self::socket(a)?;
                let get = self.sx(&[n, 0x0A, mp]);
                if name == "get_phantom_power" {
                    Plan::read(get, format!("preamps.{s}.phantom_power"))
                } else {
                    let v = if a.flag("enabled")? { 0x7F } else { 0x00 };
                    Plan::write_then(self.sx(&[n, 0x0C, mp, v]), get)
                }
            }
            "set_channel_name" => {
                let (ch, note, ..) = self.arg_channel(a)?;
                let name = a.str("name")?;
                if self.avantis() && name.chars().count() > 8 {
                    return Err(invalid("Avantis names are at most 8 characters"));
                }
                let mut b = vec![ch, 0x03, note];
                b.extend(midi::ascii_name(name).map_err(invalid)?);
                Plan::write_then(self.sx(&b), self.sx(&[ch, 0x01, note]))
            }
            "get_channel_name" => {
                let (ch, note, t, num) = self.arg_channel(a)?;
                Plan::read(
                    self.sx(&[ch, 0x01, note]),
                    format!("{}.name", Self::path(t.name, num)),
                )
            }
            "set_channel_colour" => {
                let (ch, note, ..) = self.arg_channel(a)?;
                let col = Self::index_of(&COLOURS, a.str("colour")?, "colour")?;
                Plan::write_then(self.sx(&[ch, 0x06, note, col]), self.sx(&[ch, 0x04, note]))
            }
            "get_channel_colour" => {
                let (ch, note, t, num) = self.arg_channel(a)?;
                Plan::read(
                    self.sx(&[ch, 0x04, note]),
                    format!("{}.colour", Self::path(t.name, num)),
                )
            }
            "recall_scene" => {
                let s = a.int("scene")?;
                if !(1..=500).contains(&s) {
                    return Err(invalid("scene is 1-500"));
                }
                let i = (s - 1) as u16;
                Plan::write(midi::bank_program(n, (i / 128) as u8, (i % 128) as u8))
            }
            "recall_cue" => {
                let r = a.int("recall_id")?;
                if !(0..=1999).contains(&r) {
                    return Err(invalid("recall_id is 0-1999"));
                }
                Plan::write(midi::bank_program(n, (r / 128) as u8, (r % 128) as u8))
            }
            "set_peq_type" | "set_peq_frequency" | "set_peq_width" | "set_peq_gain" => {
                let (ch, note, ..) = self.arg_channel(a)?;
                let band = Self::peq_band(a)?;
                let (offset, value) = match name {
                    "set_peq_type" => {
                        let kind = a.str("type")?;
                        let (i, (_, bands)) = PEQ_TYPES
                            .iter()
                            .enumerate()
                            .find(|(_, (n, _))| *n == kind)
                            .ok_or_else(|| invalid(format!("unknown type '{kind}'")))?;
                        if !bands.contains(&band) {
                            return Err(invalid(format!(
                                "{kind} is for band(s) {:?} only (1-based)",
                                bands.iter().map(|b| b + 1).collect::<Vec<_>>()
                            )));
                        }
                        (0, i as u8)
                    }
                    "set_peq_frequency" => {
                        let hz = a
                            .opt_float("frequency_hz")
                            .ok_or_else(|| invalid("'frequency_hz' is required"))?;
                        let v = freq_to_raw(hz, 45922.0)
                            .ok_or_else(|| invalid("frequency_hz is 20-20000"))?;
                        (1, v)
                    }
                    "set_peq_width" => (2, Self::index_of(&WIDTHS, a.str("width")?, "width")?),
                    _ => (3, Self::level_arg(a, "gain", &PEQ_GAIN)?),
                };
                let id = 0x1A + band * 4 + offset;
                Plan::write_then(
                    midi::nrpn(ch, note, id, value),
                    self.get(ch, &[0x0B, id, note]),
                )
            }
            "get_peq" => {
                let (ch, note, t, num) = self.arg_channel(a)?;
                let band = Self::peq_band(a)?;
                let param = a.str("parameter")?;
                let offset = ["type", "frequency", "width", "gain"]
                    .iter()
                    .position(|p| *p == param)
                    .ok_or_else(|| invalid(format!("unknown parameter '{param}'")))?;
                let id = 0x1A + band * 4 + offset as u8;
                let base = format!("{}.peq.{}", Self::path(t.name, num), band + 1);
                let get = self.get(ch, &[0x0B, id, note]);
                match param {
                    "type" => Plan::read(get, format!("{base}.type")),
                    "width" => Plan::read(get, format!("{base}.width")),
                    "frequency" => Plan::read_fields(
                        get,
                        format!("{base}.frequency_raw"),
                        &["frequency_hz", "frequency_raw"],
                    ),
                    _ => {
                        Plan::read_fields(get, format!("{base}.gain_raw"), &["gain_db", "gain_raw"])
                    }
                }
            }
            "set_hpf_frequency" | "get_hpf_frequency" => {
                let (ch, note, t, num) = self.arg_channel(a)?;
                let get = self.get(ch, &[0x0B, 0x30, note]);
                if name == "get_hpf_frequency" {
                    Plan::read_fields(
                        get,
                        format!("{}.hpf.frequency_raw", Self::path(t.name, num)),
                        &["frequency_hz", "frequency_raw"],
                    )
                } else {
                    let hz = a
                        .opt_float("frequency_hz")
                        .ok_or_else(|| invalid("'frequency_hz' is required"))?;
                    let v = freq_to_raw(hz, 41314.0).ok_or_else(|| {
                        invalid("frequency_hz is outside the range the HPF formula covers (20 Hz to about 10 kHz)")
                    })?;
                    Plan::write_then(midi::nrpn(ch, note, 0x30, v), get)
                }
            }
            "set_hpf" | "get_hpf" => {
                let (ch, note, t, num) = self.arg_channel(a)?;
                let get = self.get(ch, &[0x0B, 0x31, note]);
                if name == "get_hpf" {
                    Plan::read(get, format!("{}.hpf.enabled", Self::path(t.name, num)))
                } else {
                    let v = if a.flag("enabled")? { 0x7F } else { 0x00 };
                    Plan::write_then(midi::nrpn(ch, note, 0x31, v), get)
                }
            }
            "set_ufx_global_key" => {
                let key = Self::index_of(&KEYS, a.str("key")?, "key")?;
                Plan::write(midi::cc(n, 0x0C, key))
            }
            "set_ufx_global_scale" => {
                let scale = Self::index_of(&SCALES, a.str("scale")?, "scale")?;
                if scale == 2 && !self.avantis() {
                    return Err(invalid(
                        "dLive documents scale values 00-01 (major, minor) only",
                    ));
                }
                Plan::write(midi::cc(n, 0x0D, scale))
            }
            "control_change" => {
                let ch = a.int("midi_channel")?;
                let c = a.int("controller")?;
                let v = a.int("value")?;
                if !(1..=16).contains(&ch) || !(0..=127).contains(&c) || !(0..=127).contains(&v) {
                    return Err(invalid("midi_channel 1-16, controller and value 0-127"));
                }
                Plan::write(midi::cc((ch - 1) as u8, c as u8, v as u8))
            }
            other => return Err(unknown(other)),
        })
    }

    fn event(&mut self, event: &Event) -> Vec<Update> {
        let n = self.base;
        match *event {
            Event::Note { ch, note, velocity } => match self.decode(ch, note) {
                Some((kind, num)) => vec![(
                    format!("{}.mute", Self::path(kind, num)),
                    Value::Bool(velocity >= 0x40),
                )],
                None => Vec::new(),
            },
            Event::Nrpn {
                ch,
                msb,
                lsb,
                value,
            } => {
                let Some((kind, num)) = self.decode(ch, msb) else {
                    return Vec::new();
                };
                let p = Self::path(kind, num);
                match lsb {
                    0x17 => level_updates(&p, "level", &FADER, value as u16),
                    0x18 => vec![(format!("{p}.main_assign"), Value::Bool(value >= 0x40))],
                    0x40 => {
                        let (dcas, mg_off, mg_on) = self.assign_values();
                        let (what, index, on) = match value {
                            v if v < dcas => ("dca", v, false),
                            v if (mg_off..mg_off + 8).contains(&v) => {
                                ("mute_group", v - mg_off, false)
                            }
                            v if (0x40..0x40 + dcas).contains(&v) => ("dca", v - 0x40, true),
                            v if (mg_on..mg_on + 8).contains(&v) => ("mute_group", v - mg_on, true),
                            _ => return Vec::new(),
                        };
                        vec![(format!("{p}.{what}.{}", index + 1), Value::Bool(on))]
                    }
                    0x30 => vec![
                        (format!("{p}.hpf.frequency_raw"), Value::from(value)),
                        (
                            format!("{p}.hpf.frequency_hz"),
                            Value::from(raw_to_freq(value, 41314.0)),
                        ),
                    ],
                    0x31 => vec![(format!("{p}.hpf.enabled"), Value::Bool(value >= 0x40))],
                    id => match peq_param(id) {
                        Some((band, what)) => {
                            let b = format!("{p}.peq.{band}");
                            match what {
                                "type" => PEQ_TYPES
                                    .get(value as usize)
                                    .map(|(t, _)| vec![(format!("{b}.type"), Value::from(*t))])
                                    .unwrap_or_default(),
                                "width" => WIDTHS
                                    .get(value as usize)
                                    .map(|w| vec![(format!("{b}.width"), Value::from(*w))])
                                    .unwrap_or_default(),
                                "frequency" => vec![
                                    (format!("{b}.frequency_raw"), Value::from(value)),
                                    (
                                        format!("{b}.frequency_hz"),
                                        Value::from(raw_to_freq(value, 45922.0)),
                                    ),
                                ],
                                _ => level_updates(&b, "gain", &PEQ_GAIN, value as u16),
                            }
                        }
                        None => Vec::new(),
                    },
                }
            }
            Event::PitchBend { ch, first, second } if ch == n => level_updates(
                &format!("preamps.{}", first as u32 + 1),
                "gain",
                &GAIN,
                second as u16,
            ),
            Event::Program {
                ch,
                bank_msb,
                program,
                ..
            } if ch == n => {
                let bank = bank_msb.unwrap_or(0) as u32;
                if self.console == Console::Surface {
                    vec![(
                        "cue.last_recall_id".into(),
                        Value::from(bank * 128 + program as u32),
                    )]
                } else {
                    vec![(
                        "scene.current".into(),
                        Value::from(bank * 128 + program as u32 + 1),
                    )]
                }
            }
            Event::SysEx(ref body) => self.sysex(body),
            _ => Vec::new(),
        }
    }

    fn sync(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for t in self.types {
            for i in 0..t.count {
                self.sync_channel(&mut out, t, self.base + t.offset, t.first + i);
            }
        }
        if self.console == Console::MixRack && self.sync_preamps {
            for mp in 0..128u8 {
                out.push(self.get(self.base, &[0x0B, 0x19, mp]));
                out.push(self.sx(&[self.base, 0x07, mp]));
                out.push(self.sx(&[self.base, 0x0A, mp]));
            }
        }
        out
    }

    fn probe(&self) -> Vec<u8> {
        self.sx(&[self.base, 0x01, 0x00])
    }

    fn silence_hint(&self) -> &'static str {
        "check that MIDI/TCP is enabled on the console and not in secure (TLS) mode, that the \
         midi_channel setting matches the console's base MIDI channel (Utility / Control / \
         MIDI), and the port (MixRack 51325, Surface 51328)"
    }
}

impl Dlive {
    fn sysex(&self, body: &[u8]) -> Vec<Update> {
        // Header bytes, then 0N and the message type. The version bytes are
        // not checked, so a later minor version still reads.
        if body.len() < HEADER.len() + 2 || body[..5] != HEADER[..5] {
            return Vec::new();
        }
        let m = &body[HEADER.len()..];
        let (ch, kind, rest) = (m[0], m[1], &m[2..]);
        let channel = |note: u8| self.decode(ch, note).map(|(k, n)| Self::path(k, n));
        match (kind, rest) {
            // Name reply (02), or a name set (03) seen from elsewhere.
            (0x02 | 0x03, [note, name @ ..]) => channel(*note)
                .map(|p| vec![(format!("{p}.name"), Value::from(midi::read_name(name)))])
                .unwrap_or_default(),
            // Colour reply (05) or set (06).
            (0x05 | 0x06, [note, col]) => match (channel(*note), COLOURS.get(*col as usize)) {
                (Some(p), Some(c)) => vec![(format!("{p}.colour"), Value::from(*c))],
                _ => Vec::new(),
            },
            // Pad reply (08) or set (09); phantom power reply (0B) or set (0C).
            (0x08 | 0x09, [mp, v]) if ch == self.base => vec![(
                format!("preamps.{}.pad", *mp as u32 + 1),
                Value::Bool(*v >= 0x40),
            )],
            (0x0B | 0x0C, [mp, v]) if ch == self.base => vec![(
                format!("preamps.{}.phantom_power", *mp as u32 + 1),
                Value::Bool(*v >= 0x40),
            )],
            // Send level (0D) and send assignment (0E).
            (0x0D | 0x0E, [note, snd_n, snd_ch, v]) => {
                match (self.decode(ch, *note), self.decode(*snd_n, *snd_ch)) {
                    (Some((k, n)), Some((dk, dn))) => {
                        let p = format!("sends.{k}.{n}.{dk}.{dn}");
                        if kind == 0x0D {
                            level_updates(&p, "level", &FADER, *v as u16)
                        } else {
                            vec![(format!("{p}.assigned"), Value::Bool(*v >= 0x40))]
                        }
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::allenheath::testing::*;
    use super::*;
    use serde_json::json;

    fn dlive(model: &str, channel: i64) -> Box<Dlive> {
        Box::new(Dlive::new(model, &params(json!({"midi_channel": channel}))).unwrap())
    }

    /// The dLive SysEx header, `F0 00 00 1A 50 10 01 00`.
    fn h(rest: &str) -> Vec<u8> {
        hex(&format!("F0 00 00 1A 50 10 01 00 {rest} F7"))
    }

    #[test]
    fn fader_formula_matches_the_tables() {
        // dLive p.9 (hex column) and Avantis "Fader Level".
        for (db, raw) in [
            (10.0, 0x7F),
            (0.0, 0x6B),
            (-5.0, 0x61),
            (-10.0, 0x57),
            (-15.0, 0x4D),
            (-20.0, 0x43),
            (-25.0, 0x39),
            (-30.0, 0x2F),
            (-35.0, 0x25),
            (-40.0, 0x1B),
            (-45.0, 0x11),
        ] {
            assert_eq!(FADER.encode(db), Some(raw), "{db} dB");
        }
        // "+5 74 117": the formula and the decimal column give 117 (75).
        assert_eq!(FADER.encode(5.0), Some(117));
        assert_eq!(FADER.decode(0), None, "00 is -inf");
        assert_eq!(FADER.decode(0x6B), Some(0.0));
        assert_eq!(FADER.decode(0x7F), Some(10.0));
        assert_eq!(FADER.decode(0x61), Some(-5.0));
        // Every value reads back as itself.
        for raw in 1..=127 {
            assert_eq!(FADER.encode(FADER.decode(raw).unwrap()), Some(raw));
        }
        assert_eq!(FADER.encode(10.5), None);
    }

    #[test]
    fn gain_peq_and_hpf_tables() {
        // p.9 Gain Value.
        for &(db, raw) in GAIN_TABLE {
            assert_eq!(GAIN.encode(db), Some(raw));
            assert_eq!(GAIN.decode(raw), Some(db));
        }
        // p.6 frequency examples.
        for (hz, raw) in [
            (20.0, 0x00),
            (50.0, 0x10),
            (100.0, 0x1D),
            (500.0, 0x3B),
            (1000.0, 0x47),
            (10000.0, 0x72),
            (20000.0, 0x7F),
        ] {
            assert_eq!(freq_to_raw(hz, 45922.0), Some(raw), "{hz} Hz");
        }
        assert_eq!(freq_to_raw(19.0, 45922.0), None);
        // p.6 gain examples (decimal column).
        for (db, raw) in [
            (-15.0, 0),
            (-10.0, 21),
            (-5.0, 42),
            (0.0, 63),
            (5.0, 84),
            (10.0, 105),
            (15.0, 126),
        ] {
            assert_eq!(PEQ_GAIN.encode(db), Some(raw));
        }
        assert!(freq_to_raw(12_000.0, 41314.0).is_none());
        assert_eq!(freq_to_raw(20.0, 41314.0), Some(0));
    }

    #[test]
    fn documented_messages_byte_for_byte() {
        // Base channel 12 (N = B), as in the document's running-status example.
        let (mut m, _) = connected(dlive("dlive-mixrack", 12), &[0xFE]);
        let b = |m: &mut _, name, p| bytes(m, name, p).unwrap();
        // Mute ON / OFF, p.2 (followed by the mute get).
        assert_eq!(
            b(
                &mut m,
                "set_mute",
                json!({"channel_type": "input", "channel": 1, "muted": true})
            ),
            [hex("9B 00 7F 9B 00 00"), h("0B 05 09 00")].concat()
        );
        assert_eq!(
            b(
                &mut m,
                "set_mute",
                json!({"channel_type": "dca", "channel": 1, "muted": false})
            )[..6],
            hex("9F 36 3F 9F 36 00")
        );
        // Fader, NRPN 17, p.2: 0 dB = 6B.
        assert_eq!(
            b(
                &mut m,
                "set_level",
                json!({"channel_type": "stereo_group", "channel": 2, "level_db": 0.0})
            )[..9],
            hex("BC 63 41 BC 62 17 BC 06 6B")
        );
        // Main mix assign, p.2-3.
        assert_eq!(
            b(
                &mut m,
                "set_main_assign",
                json!({"channel_type": "input", "channel": 3, "assigned": false})
            )[..9],
            hex("BB 63 02 BB 62 18 BB 06 3F")
        );
        // DCA 24 on, mute group 8 off, p.3.
        assert_eq!(
            b(
                &mut m,
                "set_dca_assign",
                json!({"channel_type": "input", "channel": 1, "dca": 24, "assigned": true})
            ),
            hex("BB 63 00 BB 62 40 BB 06 57")
        );
        assert_eq!(
            b(
                &mut m,
                "set_mute_group_assign",
                json!({"channel_type": "input", "channel": 1, "mute_group": 8, "assigned": false})
            ),
            hex("BB 63 00 BB 62 40 BB 06 1F")
        );
        // Aux send level, p.3: Input 1 to stereo aux 1 (N+2 = D, note 40).
        assert_eq!(
            b(
                &mut m,
                "set_send_level",
                json!({"channel_type": "input", "channel": 1,
                "destination_type": "stereo_aux", "destination": 1, "level_raw": 0x6B})
            ),
            [h("0B 0D 00 0D 40 6B"), h("0B 05 0F 0D 00 0D 40")].concat()
        );
        // Preamp: gain is Pitch Bend EN MP GV, pad and 48V SysEx, p.4.
        assert_eq!(
            b(
                &mut m,
                "set_preamp_gain",
                json!({"socket": 65, "gain_db": 40.0})
            ),
            [hex("EB 40 50"), h("0B 05 0B 19 40")].concat()
        );
        assert_eq!(
            b(
                &mut m,
                "set_preamp_pad",
                json!({"socket": 1, "enabled": true})
            ),
            [h("0B 09 00 7F"), h("0B 07 00")].concat()
        );
        assert_eq!(
            b(
                &mut m,
                "set_phantom_power",
                json!({"socket": 64, "enabled": false})
            ),
            [h("0B 0C 3F 00"), h("0B 0A 3F")].concat()
        );
        // Name and colour, p.4: "A" is 41, Red is 01.
        assert_eq!(
            b(
                &mut m,
                "set_channel_name",
                json!({"channel_type": "input", "channel": 1, "name": "Ab"})
            ),
            [h("0B 03 00 41 62"), h("0B 01 00")].concat()
        );
        assert_eq!(
            b(
                &mut m,
                "set_channel_colour",
                json!({"channel_type": "input", "channel": 1, "colour": "red"})
            ),
            [h("0B 06 00 01"), h("0B 04 00")].concat()
        );
        // Scenes, p.4-5: scene 129 is bank 01, program 00; 500 is bank 03, 73.
        assert_eq!(
            b(&mut m, "recall_scene", json!({"scene": 129})),
            hex("BB 00 01 CB 00")
        );
        assert_eq!(
            b(&mut m, "recall_scene", json!({"scene": 500})),
            hex("BB 00 03 CB 73")
        );
        // PEQ band 1 (document band 0) gain 0 dB = 3F, HPF on, p.6-7.
        assert_eq!(
            b(
                &mut m,
                "set_peq_gain",
                json!({"channel_type": "input", "channel": 1, "band": 1, "gain_db": 0.0})
            ),
            [hex("BB 63 00 BB 62 1D BB 06 3F"), h("0B 05 0B 1D 00")].concat()
        );
        assert_eq!(
            b(
                &mut m,
                "set_peq_frequency",
                json!({"channel_type": "input", "channel": 1, "band": 4, "frequency_hz": 1000.0})
            )[..9],
            hex("BB 63 00 BB 62 27 BB 06 47")
        );
        assert!(bytes(
            &mut m,
            "set_peq_type",
            json!({"channel_type": "input", "channel": 1, "band": 2, "type": "bell"})
        )
        .is_err());
        assert_eq!(
            b(
                &mut m,
                "set_hpf",
                json!({"channel_type": "input", "channel": 1, "enabled": true})
            )[..9],
            hex("BB 63 00 BB 62 31 BB 06 7F")
        );
        // UFX global key and scale, p.7.
        assert_eq!(
            b(&mut m, "set_ufx_global_key", json!({"key": "F#"})),
            hex("BB 0C 06")
        );
        assert!(bytes(
            &mut m,
            "set_ufx_global_scale",
            json!({"scale": "chromatic"})
        )
        .is_err());
        // Out of range for dLive.
        assert!(bytes(
            &mut m,
            "set_mute",
            json!({"channel_type": "input", "channel": 129, "muted": true})
        )
        .is_err());
    }

    #[test]
    fn avantis_tables_differ_and_writes_have_no_read_back() {
        let (mut m, _) = connected(dlive("avantis", 12), &[0xFE]);
        // Avantis DCA 16 is note 45 on N+4; mute group 1 is 46.
        assert_eq!(
            bytes(
                &mut m,
                "set_mute",
                json!({"channel_type": "mute_group", "channel": 1, "muted": true})
            )
            .unwrap(),
            hex("9F 46 7F 9F 46 00")
        );
        assert_eq!(
            bytes(
                &mut m,
                "set_dca_assign",
                json!({"channel_type": "input", "channel": 96, "dca": 16, "assigned": true})
            )
            .unwrap(),
            hex("BB 63 5F BB 62 40 BB 06 4F")
        );
        assert_eq!(
            bytes(
                &mut m,
                "set_mute_group_assign",
                json!({"channel_type": "input", "channel": 1, "mute_group": 1, "assigned": true})
            )
            .unwrap(),
            hex("BB 63 00 BB 62 40 BB 06 50")
        );
        assert!(bytes(
            &mut m,
            "set_mute",
            json!({"channel_type": "input", "channel": 97, "muted": true})
        )
        .is_err());
        assert!(bytes(
            &mut m,
            "set_channel_name",
            json!({"channel_type": "input", "channel": 1, "name": "123456789"})
        )
        .is_err());
        assert_eq!(
            bytes(
                &mut m,
                "set_ufx_global_scale",
                json!({"scale": "chromatic"})
            )
            .unwrap(),
            hex("BB 0D 02")
        );
    }

    #[test]
    fn console_messages_become_state_with_running_status() {
        let (mut m, _) = connected(dlive("dlive-mixrack", 12), &[0xFE]);
        // p.1's example: inputs 1-3 muted, with running status.
        let s = state(&feed(&mut m, 10, &hex("9B 00 7F 01 7F 02 7F 00 00")));
        assert_eq!(s["channels"]["input"]["1"]["mute"], true);
        assert_eq!(s["channels"]["input"]["3"]["mute"], true);
        let s = state(&feed(
            &mut m,
            20,
            &[
                hex("9F 4E 3F"),             // mute group 1 off
                hex("BC 63 41 62 17 06 6B"), // stereo group 2 fader, running status
                hex("BB 63 00 62 40 06 41"), // input 1 to DCA 2
                hex("BB 63 00 62 40 06 18"), // input 1 off mute group 1
                hex("BB 00 01 CB 05"),       // scene 134 recalled
                hex("EB 00 50"),             // socket 1 gain +40
                h("0B 02 00 4B 69 63 6B"),   // input 1 is "Kick"
                h("0B 05 00 04"),            // input 1 is blue
                h("0B 08 00 7F"),            // socket 1 pad on
                h("0B 0D 00 0D 40 6B"),      // input 1 to stereo aux 1 at 0 dB
            ]
            .concat(),
        ));
        assert_eq!(s["channels"]["mute_group"]["1"]["mute"], false);
        assert_eq!(s["channels"]["stereo_group"]["2"]["level_raw"], 0x6B);
        assert_eq!(s["channels"]["stereo_group"]["2"]["level_db"], 0.0);
        assert_eq!(s["channels"]["input"]["1"]["dca"]["2"], true);
        assert_eq!(s["channels"]["input"]["1"]["mute_group"]["1"], false);
        assert_eq!(s["scene"]["current"], 134);
        assert_eq!(s["preamps"]["1"]["gain_db"], 40.0);
        assert_eq!(s["preamps"]["1"]["pad"], true);
        assert_eq!(s["channels"]["input"]["1"]["name"], "Kick");
        assert_eq!(s["channels"]["input"]["1"]["colour"], "blue");
        assert_eq!(
            s["sends"]["input"]["1"]["stereo_aux"]["1"]["level_raw"],
            0x6B
        );
    }

    #[test]
    fn a_get_is_answered_by_the_console_message() {
        let (mut m, _) = connected(dlive("dlive-mixrack", 1), &[0xFE]);
        let a = run(
            &mut m,
            10,
            "get_level",
            json!({"channel_type": "input", "channel": 2}),
        );
        assert_eq!(sent(&a), [h("00 05 0B 17 01")]);
        let a = feed(&mut m, 20, &hex("B0 63 01 B0 62 17 B0 06 7F"));
        assert!(a.contains(&crate::module::Action::Complete {
            id: 1,
            result: Ok(crate::module::Outcome::Value {
                value: json!({"level_db": 10.0, "level_raw": 127})
            })
        }));
    }

    #[test]
    fn surface_recalls_cues_and_sync_covers_every_channel() {
        let (mut m, a) = connected(dlive("dlive-surface", 1), &[0xFE]);
        assert_eq!(
            bytes(&mut m, "recall_cue", json!({"recall_id": 1999})).unwrap(),
            hex("B0 00 0F C0 4F")
        );
        let s = state(&feed(&mut m, 10, &hex("B0 00 01 C0 00")));
        assert_eq!(s["cue"]["last_recall_id"], 128);
        let all = drain_sync(&mut m, &a);
        // Name and colour for 509 channels, mute for all, fader for all but
        // mute groups, main assignment for inputs; no preamps on a Surface.
        assert_eq!(all.len(), 509 * 3 + 501 + 128);
        assert_eq!(all[0], h("00 01 00"));
    }
}
