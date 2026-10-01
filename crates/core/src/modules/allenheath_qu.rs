//! Qu-16, Qu-24, Qu-32, Qu-Pac and Qu-SB, from "Qu Mixer MIDI Protocol -
//! Firmware V1.9+" (Issue 2, October 2021).
//!
//! - Qu uses one MIDI channel N, set on the mixer (p.3).
//! - Mutes are Note On (7F on, 3F off, then 00) with the channel number as
//!   the note (p.3, p.5).
//! - Every other parameter is NRPN with the channel as parameter MSB, the
//!   parameter ID as LSB, the value as data MSB and an index (a mix, a GEQ
//!   band, a dSNAKE socket, or 07/00 where there is none) as data LSB
//!   (p.5-9).
//! - SysEx header `F0 00 00 1A 50 11 01 00 0N`: names, and "Get System
//!   State", after which the mixer pushes every parameter as NRPN and ends
//!   with `... 14 F7` (p.10-11).
//! - Qu sends Active Sensing (FE) when a connection opens and every 300 ms
//!   or so when idle; a client that sends FE must keep sending something at
//!   least every 12 s (p.3, p.10).
//! - Scenes 1-100 are Bank 1 (CC 00 00, CC 20 00) and Program Change (p.9).

use serde_json::{json, Value};

use super::allenheath::{invalid, midi_channel, unknown, Args, Dialect, Plan};
use super::allenheath_midi::{self as midi, level_updates, Event, Law, Update};
use crate::catalog::Params;
use crate::module::CommandError;

/// p.13 "Fader / Send Level".
pub(crate) static LEVEL_TABLE: &[(f64, u16)] = &[
    (-45.0, 0x0C),
    (-40.0, 0x10),
    (-35.0, 0x17),
    (-30.0, 0x1F),
    (-25.0, 0x27),
    (-20.0, 0x2F),
    (-15.0, 0x36),
    (-10.0, 0x3F),
    (-5.0, 0x4F),
    (0.0, 0x62),
    (5.0, 0x72),
    (10.0, 0x7F),
];
const LEVEL: Law = Law::Table(LEVEL_TABLE);

/// p.13 "Local Gain" (NRPN 19).
pub(crate) static LOCAL_GAIN_TABLE: &[(f64, u16)] = &[
    (-5.0, 0x00),
    (0.0, 0x0A),
    (5.0, 0x13),
    (10.0, 0x1D),
    (20.0, 0x30),
    (30.0, 0x44),
    (40.0, 0x57),
    (50.0, 0x6B),
    (60.0, 0x7F),
];
const LOCAL_GAIN: Law = Law::Table(LOCAL_GAIN_TABLE);

/// p.13 "dSNAKE Gain" (NRPN 58).
pub(crate) static DSNAKE_GAIN_TABLE: &[(f64, u16)] = &[
    (5.0, 0x00),
    (10.0, 0x0B),
    (20.0, 0x22),
    (25.0, 0x2E),
    (30.0, 0x39),
    (35.0, 0x45),
    (40.0, 0x50),
    (50.0, 0x67),
    (60.0, 0x7F),
];
const DSNAKE_GAIN: Law = Law::Table(DSNAKE_GAIN_TABLE);

/// p.7: "Trim -24 to +24dB = 00 to 7F 0dB = 40".
const TRIM: Law = Law::Linear {
    lo: -24.0,
    hi: 24.0,
    max: 127,
};

/// Pan p.5: "Full Left = 00, to Centre = 25, to Full Right = 4A", as a
/// percentage from -100 (left) to 100 (right).
const PAN: Law = Law::Linear {
    lo: -100.0,
    hi: 100.0,
    max: 0x4A,
};

/// Processing parameters (p.6-8) with their NRPN ID and the index byte the
/// document gives: 07 for values, 00 for in/out switches.
const PROCESSING: &[(&str, u8, u8)] = &[
    ("peq_lf_gain", 0x01, 0x07),
    ("peq_lf_frequency", 0x02, 0x07),
    ("peq_lf_width", 0x03, 0x07),
    ("peq_lf_type", 0x04, 0x07),
    ("peq_lm_gain", 0x05, 0x07),
    ("peq_lm_frequency", 0x06, 0x07),
    ("peq_lm_width", 0x07, 0x07),
    ("peq_hm_gain", 0x09, 0x07),
    ("peq_hm_frequency", 0x0A, 0x07),
    ("peq_hm_width", 0x0B, 0x07),
    ("peq_hf_gain", 0x0D, 0x07),
    ("peq_hf_frequency", 0x0E, 0x07),
    ("peq_hf_width", 0x0F, 0x07),
    ("peq_hf_type", 0x10, 0x07),
    ("peq_in", 0x11, 0x00),
    ("hpf_frequency", 0x13, 0x07),
    ("hpf_in", 0x14, 0x00),
    ("gate_attack", 0x41, 0x07),
    ("gate_release", 0x42, 0x07),
    ("gate_hold", 0x43, 0x07),
    ("gate_threshold", 0x44, 0x07),
    ("gate_depth", 0x45, 0x07),
    ("gate_in", 0x46, 0x00),
    ("compressor_type", 0x61, 0x07),
    ("compressor_attack", 0x62, 0x07),
    ("compressor_release", 0x63, 0x07),
    ("compressor_knee", 0x64, 0x07),
    ("compressor_ratio", 0x65, 0x07),
    ("compressor_threshold", 0x66, 0x07),
    ("compressor_gain", 0x67, 0x07),
    ("compressor_in", 0x68, 0x00),
    ("polarity", 0x6A, 0x07),
    ("insert_in", 0x6B, 0x07),
    ("delay_time", 0x6C, 0x07),
    ("delay_in", 0x6D, 0x00),
    ("geq_in", 0x71, 0x00),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Model {
    Qu16,
    Qu24,
    /// Qu-32, Qu-Pac and Qu-SB share the 32-channel tables (p.12).
    Qu32,
}

pub(crate) struct Qu {
    model: Model,
    /// The mixer's MIDI channel, 0-based.
    n: u8,
}

/// Channel numbers, p.3: the note for mutes and the NRPN MSB for the rest.
fn channel_code(kind: &str, number: i64) -> Option<u8> {
    let n = u8::try_from(number).ok()?;
    match kind {
        "fx_send" if (1..=4).contains(&n) => Some(n - 1),
        "fx_return" if (1..=4).contains(&n) => Some(0x08 + n - 1),
        "dca" if (1..=4).contains(&n) => Some(0x10 + n - 1),
        "input" if (1..=32).contains(&n) => Some(0x20 + n - 1),
        "stereo_input" if (1..=3).contains(&n) => Some(0x40 + n - 1),
        "mute_group" if (1..=4).contains(&n) => Some(0x50 + n - 1),
        "mix" => match n {
            1..=4 => Some(0x60 + n - 1),
            5 => Some(0x64),
            7 => Some(0x65),
            9 => Some(0x66),
            _ => None,
        },
        "main" if n == 1 => Some(0x67),
        "group" => match n {
            1 => Some(0x68),
            3 => Some(0x69),
            5 => Some(0x6A),
            7 => Some(0x6B),
            _ => None,
        },
        "matrix" => match n {
            1 => Some(0x6C),
            3 => Some(0x6D),
            _ => None,
        },
        _ => None,
    }
}

fn channel_name(code: u8) -> Option<(&'static str, u32)> {
    let c = code as u32;
    Some(match code {
        0x00..=0x03 => ("fx_send", c + 1),
        0x08..=0x0B => ("fx_return", c - 0x08 + 1),
        0x10..=0x13 => ("dca", c - 0x10 + 1),
        0x20..=0x3F => ("input", c - 0x20 + 1),
        0x40..=0x42 => ("stereo_input", c - 0x40 + 1),
        0x50..=0x53 => ("mute_group", c - 0x50 + 1),
        0x60..=0x63 => ("mix", c - 0x60 + 1),
        0x64..=0x66 => ("mix", 5 + (c - 0x64) * 2),
        0x67 => ("main", 1),
        0x68..=0x6B => ("group", 1 + (c - 0x68) * 2),
        0x6C..=0x6D => ("matrix", 1 + (c - 0x6C) * 2),
        _ => return None,
    })
}

/// Send destinations (the data LSB "VX"), p.5-6.
fn destination_code(kind: &str, number: i64) -> Option<u8> {
    let n = u8::try_from(number).ok()?;
    match kind {
        "mix" => match n {
            1..=4 => Some(n - 1),
            5 => Some(0x04),
            7 => Some(0x05),
            9 => Some(0x06),
            _ => None,
        },
        "main" if n == 1 => Some(0x07),
        "group" => match n {
            1 => Some(0x08),
            3 => Some(0x09),
            5 => Some(0x0A),
            7 => Some(0x0B),
            _ => None,
        },
        "matrix" => match n {
            1 => Some(0x0C),
            3 => Some(0x0D),
            _ => None,
        },
        "fx_send" if (1..=4).contains(&n) => Some(0x10 + n - 1),
        _ => None,
    }
}

fn destination_name(code: u8) -> Option<(&'static str, u32)> {
    let c = code as u32;
    Some(match code {
        0x00..=0x03 => ("mix", c + 1),
        0x04..=0x06 => ("mix", 5 + (c - 0x04) * 2),
        0x07 => ("main", 1),
        0x08..=0x0B => ("group", 1 + (c - 0x08) * 2),
        0x0C..=0x0D => ("matrix", 1 + (c - 0x0C) * 2),
        0x10..=0x13 => ("fx_send", c - 0x10 + 1),
        _ => return None,
    })
}

impl Qu {
    pub(crate) fn new(model: &str, settings: &Params) -> Result<Qu, String> {
        let model = match model {
            "qu-16" => Model::Qu16,
            "qu-24" => Model::Qu24,
            "qu-32" | "qu-pac" | "qu-sb" => Model::Qu32,
            other => return Err(format!("unknown Qu model '{other}'")),
        };
        Ok(Qu {
            model,
            n: midi_channel(settings, 16)?,
        })
    }

    fn header(&self, body: &[u8]) -> Vec<u8> {
        let mut h = vec![0x00, 0x00, 0x1A, 0x50, 0x11, 0x01, 0x00, self.n];
        h.extend_from_slice(body);
        midi::sysex(&h, &[])
    }

    fn inputs(&self) -> i64 {
        match self.model {
            Model::Qu16 => 16,
            Model::Qu24 => 24,
            Model::Qu32 => 32,
        }
    }

    /// The channel code, refusing what the model lacks: Qu-16 has 16
    /// inputs, and no groups or matrices (p.2).
    fn code(&self, kind: &str, number: i64) -> Result<u8, CommandError> {
        if kind == "input" && number > self.inputs() {
            return Err(invalid(format!(
                "this model has {} input channels",
                self.inputs()
            )));
        }
        if self.model == Model::Qu16 && matches!(kind, "group" | "matrix") {
            return Err(invalid("Qu-16 has no groups or matrices"));
        }
        channel_code(kind, number).ok_or_else(|| {
            invalid(format!(
                "no {kind} {number}: stereo mixes, groups and matrices are addressed by their odd number"
            ))
        })
    }

    fn arg_channel(&self, a: &Args) -> Result<(u8, String), CommandError> {
        let kind = a.str("channel_type")?;
        let n = a.int("channel")?;
        Ok((self.code(kind, n)?, format!("channels.{kind}.{n}")))
    }

    fn arg_destination(&self, a: &Args, allow_main: bool) -> Result<(u8, String), CommandError> {
        let kind = a.str("destination_type")?;
        let n = a.int("destination")?;
        if kind == "main" && !allow_main {
            return Err(invalid("this parameter has no main (LR) destination"));
        }
        if self.model == Model::Qu16 {
            if matches!(kind, "group" | "matrix") {
                return Err(invalid("Qu-16 has no groups or matrices"));
            }
            if kind == "fx_send" && n > 2 {
                return Err(invalid("Qu-16 sends to FX 1 and 2 only"));
            }
        }
        let code = destination_code(kind, n)
            .ok_or_else(|| invalid(format!("no {kind} {n} destination")))?;
        Ok((code, format!("{kind}.{n}")))
    }

    fn nrpn(&self, ch: u8, id: u8, value: u8, index: u8) -> Vec<u8> {
        midi::nrpn_fine(self.n, ch, id, value, index)
    }

    fn get_name(&self, code: u8) -> Vec<u8> {
        self.header(&[0x01, code])
    }

    fn all_channels(&self) -> Vec<u8> {
        let mut codes: Vec<u8> = (0x00..=0x03).chain(0x08..=0x0B).collect();
        codes.extend(0x10..=0x13);
        codes.extend((0x20..0x20 + self.inputs() as u8).chain(0x40..=0x42));
        codes.extend(0x60..=0x67);
        if self.model != Model::Qu16 {
            codes.extend(0x68..=0x6D);
        }
        codes
    }
}

impl Dialect for Qu {
    fn command(&mut self, name: &str, a: &Args) -> Result<Plan, CommandError> {
        let n = self.n;
        let flag = |key: &str| a.flag(key).map(u8::from);
        Ok(match name {
            "set_mute" => {
                let (ch, _) = self.arg_channel(a)?;
                let vel = if a.flag("muted")? { 0x7F } else { 0x3F };
                let mut b = midi::note_on(n, ch, vel);
                b.extend(midi::note_on(n, ch, 0x00));
                Plan::write(b)
            }
            "set_level" => {
                let (ch, _) = self.arg_channel(a)?;
                let v = a.level("level", &LEVEL, 127)? as u8;
                Plan::write(self.nrpn(ch, 0x17, v, 0x07))
            }
            "set_pan" => {
                let (ch, _) = self.arg_channel(a)?;
                let (dest, _) = self.arg_destination(a, true)?;
                if !matches!(dest, 0x04..=0x0D) {
                    return Err(invalid(
                        "pan is to main, stereo mixes 5-6, 7-8, 9-10, groups or matrices",
                    ));
                }
                let v = a.value("pan", "pan_raw", &PAN, 0x4A)? as u8;
                Plan::write(self.nrpn(ch, 0x16, v, dest))
            }
            "set_main_assign" => {
                let (ch, _) = self.arg_channel(a)?;
                Plan::write(self.nrpn(ch, 0x18, flag("assigned")?, 0x07))
            }
            "set_send_assign" => {
                let (ch, _) = self.arg_channel(a)?;
                let (dest, _) = self.arg_destination(a, true)?;
                Plan::write(self.nrpn(ch, 0x55, flag("assigned")?, dest))
            }
            "set_send_level" | "set_send_pre_post" => {
                let (ch, _) = self.arg_channel(a)?;
                let (dest, _) = self.arg_destination(a, false)?;
                if name == "set_send_level" {
                    let v = a.level("level", &LEVEL, 127)? as u8;
                    Plan::write(self.nrpn(ch, 0x20, v, dest))
                } else {
                    Plan::write(self.nrpn(ch, 0x50, flag("pre")?, dest))
                }
            }
            "set_mute_group_assign" | "set_dca_assign" => {
                let (ch, _) = self.arg_channel(a)?;
                let (key, id) = if name == "set_dca_assign" {
                    ("dca", 0x40)
                } else {
                    ("mute_group", 0x5C)
                };
                let g = a.int(key)?;
                if !(1..=4).contains(&g) {
                    return Err(invalid(format!("{key} is 1-4")));
                }
                let v = (g - 1) as u8 + if a.flag("assigned")? { 0x40 } else { 0 };
                Plan::write(self.nrpn(ch, id, v, 0x07))
            }
            "set_pafl" => {
                let (ch, _) = self.arg_channel(a)?;
                Plan::write(self.nrpn(ch, 0x51, flag("enabled")?, 0x07))
            }
            "set_usb_source" => {
                let (ch, _) = self.arg_channel(a)?;
                Plan::write(self.nrpn(ch, 0x12, flag("enabled")?, 0x00))
            }
            "set_preamp_source" => {
                let (ch, _) = self.arg_channel(a)?;
                let v = u8::from(a.str("source")? == "dsnake");
                Plan::write(self.nrpn(ch, 0x57, v, 0x00))
            }
            "set_preamp_gain" => {
                let (ch, _) = self.arg_channel(a)?;
                let v = a.level("gain", &LOCAL_GAIN, 127)? as u8;
                Plan::write(self.nrpn(ch, 0x19, v, 0x07))
            }
            "set_phantom_power" => {
                let (ch, _) = self.arg_channel(a)?;
                Plan::write(self.nrpn(ch, 0x69, flag("enabled")?, 0x07))
            }
            "set_dsnake_gain" | "set_dsnake_pad" | "set_dsnake_phantom_power" => {
                let (ch, _) = self.arg_channel(a)?;
                let socket = a.int("socket")?;
                if !(1..=40).contains(&socket) {
                    return Err(invalid("socket is 1-40 (dSNAKE index 00-27)"));
                }
                let vx = (socket - 1) as u8;
                let (id, v) = match name {
                    "set_dsnake_gain" => (0x58, a.level("gain", &DSNAKE_GAIN, 127)? as u8),
                    "set_dsnake_pad" => (0x59, flag("enabled")?),
                    _ => (0x5A, flag("enabled")?),
                };
                Plan::write(self.nrpn(ch, id, v, vx))
            }
            "set_trim" => {
                let kind = a.str("channel_type")?;
                let id = match kind {
                    "input" => 0x52,
                    "stereo_input" => 0x54,
                    _ => {
                        return Err(invalid(
                            "trim is for input (USB digital trim) or stereo_input",
                        ))
                    }
                };
                let (ch, _) = self.arg_channel(a)?;
                let v = a.level("trim", &TRIM, 127)? as u8;
                Plan::write(self.nrpn(ch, id, v, 0x07))
            }
            "set_processing" => {
                let (ch, _) = self.arg_channel(a)?;
                let param = a.str("parameter")?;
                let (_, id, index) = PROCESSING
                    .iter()
                    .find(|(n, ..)| *n == param)
                    .ok_or_else(|| invalid(format!("unknown parameter '{param}'")))?;
                let v = a.int("value")?;
                if !(0..=127).contains(&v) {
                    return Err(invalid("value is 0-127"));
                }
                Plan::write(self.nrpn(ch, *id, v as u8, *index))
            }
            "set_geq_band" => {
                let (ch, _) = self.arg_channel(a)?;
                let band = a.int("band")?;
                let v = a.int("value")?;
                if !(1..=28).contains(&band) || !(0..=127).contains(&v) {
                    return Err(invalid("band is 1-28 and value 0-127"));
                }
                Plan::write(self.nrpn(ch, 0x70, v as u8, (band - 1) as u8))
            }
            "set_fx_delay_time" => {
                let (ch, _) = self.arg_channel(a)?;
                let vx = match a.str("tap")? {
                    "right" => 0x07,
                    _ => 0x05,
                };
                let coarse = a.int("coarse")?;
                let fine = a.opt_int("fine");
                if !(0..=127).contains(&coarse) || fine.is_some_and(|f| !(0..=127).contains(&f)) {
                    return Err(invalid("coarse and fine are 0-127"));
                }
                // p.9: the fine (LSB, ID 49) message goes first, then the
                // coarse (MSB, ID 48) one.
                let mut b = Vec::new();
                if let Some(f) = fine {
                    b.extend(self.nrpn(ch, 0x49, f as u8, vx));
                }
                b.extend(self.nrpn(ch, 0x48, coarse as u8, vx));
                Plan::write(b)
            }
            "set_fx_delay_link" => {
                let (ch, _) = self.arg_channel(a)?;
                let v = if a.flag("linked")? { 0x7F } else { 0x00 };
                Plan::write(self.nrpn(ch, 0x48, v, 0x06))
            }
            "recall_scene" => {
                let s = a.int("scene")?;
                if !(1..=100).contains(&s) {
                    return Err(invalid("scene is 1-100"));
                }
                let mut b = midi::cc(n, 0x00, 0x00);
                b.extend(midi::cc(n, 0x20, 0x00));
                b.extend([0xC0 | n, (s - 1) as u8]);
                Plan::write(b)
            }
            "set_channel_name" => {
                let (ch, _) = self.arg_channel(a)?;
                let mut body = vec![0x03, ch];
                body.extend(midi::ascii_name(a.str("name")?).map_err(invalid)?);
                Plan::write_then(self.header(&body), self.get_name(ch))
            }
            "get_channel_name" => {
                let (ch, p) = self.arg_channel(a)?;
                Plan::read(self.get_name(ch), format!("{p}.name"))
            }
            "remote_shutdown" => {
                // p.8 gives this message with channel 1 written out (B0).
                Plan::write(midi::nrpn_fine(0, 0x00, 0x5F, 0x00, 0x00))
            }
            other => return Err(unknown(other)),
        })
    }

    fn event(&mut self, event: &Event) -> Vec<Update> {
        match *event {
            Event::Note { ch, note, velocity } if ch == self.n => channel_name(note)
                .map(|(k, i)| {
                    vec![(
                        format!("channels.{k}.{i}.mute"),
                        Value::Bool(velocity >= 0x40),
                    )]
                })
                .unwrap_or_default(),
            Event::NrpnFine {
                ch,
                msb,
                lsb,
                coarse: Some(va),
                fine: vx,
            } if ch == self.n => self.nrpn_update(msb, lsb, va, vx),
            Event::Program { ch, program, .. } if ch == self.n => {
                vec![("scene.current".into(), json!(program as u32 + 1))]
            }
            Event::SysEx(ref body) => {
                let head = [0x00, 0x00, 0x1A, 0x50, 0x11];
                if body.len() < 9 || body[..5] != head {
                    return Vec::new();
                }
                match &body[8..] {
                    [0x02, code, name @ ..] => channel_name(*code)
                        .map(|(k, i)| {
                            vec![(
                                format!("channels.{k}.{i}.name"),
                                json!(midi::read_name(name)),
                            )]
                        })
                        .unwrap_or_default(),
                    [0x11, box_id, major, minor] => vec![
                        (
                            "device.model".into(),
                            json!(["qu-16", "qu-24", "qu-32", "qu-pac", "qu-sb"]
                                .get((*box_id as usize).wrapping_sub(1))),
                        ),
                        ("device.firmware".into(), json!(format!("{major}.{minor}"))),
                    ],
                    [0x14] => vec![("device.synced".into(), json!(true))],
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    fn sync(&self) -> Vec<Vec<u8>> {
        // p.10: the All Call header (channel 7F), 10, iPadFlag 0.
        let mut out = vec![midi::sysex(
            &[0x00, 0x00, 0x1A, 0x50, 0x11, 0x01, 0x00, 0x7F],
            &[0x10, 0x00],
        )];
        out.extend(self.all_channels().into_iter().map(|c| self.get_name(c)));
        out
    }

    fn probe(&self) -> Vec<u8> {
        self.get_name(0x20)
    }

    fn keepalive(&self) -> Option<Vec<u8>> {
        Some(vec![0xFE])
    }

    fn silence_hint(&self) -> &'static str {
        "Qu accepts one TCP MIDI connection at a time: close A&H MIDI Control or other \
         clients, and check the midi_channel setting matches the mixer"
    }
}

impl Qu {
    fn nrpn_update(&self, ch: u8, id: u8, va: u8, vx: u8) -> Vec<Update> {
        let Some((kind, num)) = channel_name(ch) else {
            return Vec::new();
        };
        let p = format!("channels.{kind}.{num}");
        let on = |field: &str| vec![(format!("{p}.{field}"), Value::Bool(va != 0))];
        let dest = || destination_name(vx).map(|(k, i)| format!("sends.{kind}.{num}.{k}.{i}"));
        match id {
            0x17 => level_updates(&p, "level", &LEVEL, va as u16),
            0x16 => match destination_name(vx) {
                Some(("main", _)) => level_updates(&p, "pan", &PAN, va as u16)
                    .into_iter()
                    .map(|(k, v)| (k.replace("_db", ""), v))
                    .collect(),
                Some(_) => dest()
                    .map(|d| {
                        level_updates(&d, "pan", &PAN, va as u16)
                            .into_iter()
                            .map(|(k, v)| (k.replace("_db", ""), v))
                            .collect()
                    })
                    .unwrap_or_default(),
                None => Vec::new(),
            },
            0x18 => on("main_assign"),
            0x55 => dest()
                .map(|d| vec![(format!("{d}.assigned"), Value::Bool(va != 0))])
                .unwrap_or_default(),
            0x50 => dest()
                .map(|d| vec![(format!("{d}.pre"), Value::Bool(va != 0))])
                .unwrap_or_default(),
            0x20 => dest()
                .map(|d| level_updates(&d, "level", &LEVEL, va as u16))
                .unwrap_or_default(),
            0x5C | 0x40 => {
                let what = if id == 0x40 { "dca" } else { "mute_group" };
                vec![(
                    format!("{p}.{what}.{}", (va & 0x3F) as u32 + 1),
                    Value::Bool(va >= 0x40),
                )]
            }
            0x51 => on("pafl"),
            0x12 => on("usb_source"),
            0x57 => vec![(
                format!("{p}.preamp_source"),
                json!(if va != 0 { "dsnake" } else { "local" }),
            )],
            0x5D => vec![(format!("{p}.dsnake_socket"), json!(va as u32 + 1))],
            0x5E => vec![(
                format!("{p}.group_mode"),
                json!(if va != 0 { "mix" } else { "group" }),
            )],
            0x19 => level_updates(&p, "gain", &LOCAL_GAIN, va as u16),
            0x69 => on("phantom_power"),
            0x58 => {
                let mut u = level_updates(&format!("{p}.dsnake"), "gain", &DSNAKE_GAIN, va as u16);
                u.push((format!("{p}.dsnake_socket"), json!(vx as u32 + 1)));
                u
            }
            0x59 => on("dsnake.pad"),
            0x5A => on("dsnake.phantom_power"),
            0x52 | 0x54 => level_updates(&p, "trim", &TRIM, va as u16),
            0x70 => vec![(
                format!("{p}.processing.geq_band.{}", vx as u32 + 1),
                json!(va),
            )],
            0x48 | 0x49 => {
                let tap = if vx == 0x07 { "right" } else { "left" };
                if id == 0x48 && vx == 0x06 {
                    vec![(format!("{p}.fx_delay.linked"), Value::Bool(va != 0))]
                } else {
                    let part = if id == 0x48 { "coarse" } else { "fine" };
                    vec![(format!("{p}.fx_delay.{tap}.{part}"), json!(va))]
                }
            }
            _ => PROCESSING
                .iter()
                .find(|(_, i, _)| *i == id)
                .map(|(name, ..)| vec![(format!("{p}.processing.{name}"), json!(va))])
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::allenheath::testing::*;
    use super::*;

    fn qu(model: &str) -> Box<Qu> {
        Box::new(Qu::new(model, &params(json!({"midi_channel": 1}))).unwrap())
    }

    #[test]
    fn tables_read_back_exactly() {
        for (law, table) in [
            (LEVEL, LEVEL_TABLE),
            (LOCAL_GAIN, LOCAL_GAIN_TABLE),
            (DSNAKE_GAIN, DSNAKE_GAIN_TABLE),
        ] {
            for &(db, raw) in table {
                assert_eq!(law.encode(db), Some(raw));
                assert_eq!(law.decode(raw), Some(db));
            }
        }
        // 0 dB is 62, the document's own note (p.5).
        assert_eq!(LEVEL.encode(0.0), Some(0x62));
        // Trim: "0dB = 40" (p.7); pan centre 25 (p.5).
        assert_eq!(TRIM.encode(0.0), Some(0x40));
        assert_eq!(PAN.encode(0.0), Some(0x25));
        assert_eq!(PAN.encode(100.0), Some(0x4A));
    }

    #[test]
    fn documented_messages_byte_for_byte() {
        let (mut m, a) = connected(qu("qu-32"), &[0xFE]);
        // Active Sensing on connecting, then Get System State with the All
        // Call header (p.10).
        let s = sent(&a);
        assert_eq!(s[0], [0xFE]);
        assert_eq!(s[1], hex("F0 00 00 1A 50 11 01 00 7F 10 00 F7"));
        let b = |m: &mut _, name, p| bytes(m, name, p).unwrap();
        // Mute on input 1 (CH 20), p.5.
        assert_eq!(
            b(
                &mut m,
                "set_mute",
                json!({"channel_type": "input", "channel": 1, "muted": true})
            ),
            hex("90 20 7F 90 20 00")
        );
        // Fader LR to 0 dB = 62, index 07, p.5.
        assert_eq!(
            b(
                &mut m,
                "set_level",
                json!({"channel_type": "main", "channel": 1, "level_db": 0.0})
            ),
            hex("B0 63 67 B0 62 17 B0 06 62 B0 26 07")
        );
        // Send level from ST1 to Mix 9-10 (VX 06), p.6.
        assert_eq!(
            b(
                &mut m,
                "set_send_level",
                json!({"channel_type": "stereo_input", "channel": 1,
                "destination_type": "mix", "destination": 9, "level_db": -10.0})
            ),
            hex("B0 63 40 B0 62 20 B0 06 3F B0 26 06")
        );
        // Pan centre to LR (VX 07), p.5.
        assert_eq!(
            b(
                &mut m,
                "set_pan",
                json!({"channel_type": "input", "channel": 2,
                "destination_type": "main", "destination": 1, "pan": 0.0})
            ),
            hex("B0 63 21 B0 62 16 B0 06 25 B0 26 07")
        );
        // DCA 4 on, p.6.
        assert_eq!(
            b(
                &mut m,
                "set_dca_assign",
                json!({"channel_type": "input", "channel": 1, "dca": 4, "assigned": true})
            ),
            hex("B0 63 20 B0 62 40 B0 06 43 B0 26 07")
        );
        // dSNAKE gain +60 on socket 40 (VX 27), p.6.
        assert_eq!(
            b(
                &mut m,
                "set_dsnake_gain",
                json!({"channel_type": "input", "channel": 1, "socket": 40, "gain_db": 60.0})
            ),
            hex("B0 63 20 B0 62 58 B0 06 7F B0 26 27")
        );
        // Scene 100: Bank 1 then program 63, p.9.
        assert_eq!(
            b(&mut m, "recall_scene", json!({"scene": 100})),
            hex("B0 00 00 B0 20 00 C0 63")
        );
        // Name, p.11.
        assert_eq!(
            b(
                &mut m,
                "set_channel_name",
                json!({"channel_type": "input", "channel": 1, "name": "Vox"})
            ),
            [
                hex("F0 00 00 1A 50 11 01 00 00 03 20 56 6F 78 F7"),
                hex("F0 00 00 1A 50 11 01 00 00 01 20 F7")
            ]
            .concat()
        );
        assert_eq!(
            b(&mut m, "remote_shutdown", json!({})),
            hex("B0 63 00 B0 62 5F B0 06 00 B0 26 00")
        );
        // Stereo mixes are addressed by their odd number.
        assert!(bytes(
            &mut m,
            "set_mute",
            json!({"channel_type": "mix", "channel": 6, "muted": true})
        )
        .is_err());
    }

    #[test]
    fn qu16_lacks_groups_matrices_and_fx3() {
        let (mut m, _) = connected(qu("qu-16"), &[0xFE]);
        assert!(bytes(
            &mut m,
            "set_mute",
            json!({"channel_type": "input", "channel": 17, "muted": true})
        )
        .is_err());
        assert!(bytes(
            &mut m,
            "set_mute",
            json!({"channel_type": "group", "channel": 1, "muted": true})
        )
        .is_err());
        assert!(bytes(
            &mut m,
            "set_send_level",
            json!({"channel_type": "input", "channel": 1,
            "destination_type": "fx_send", "destination": 3, "level_raw": 1})
        )
        .is_err());
    }

    #[test]
    fn the_system_state_push_becomes_state() {
        let (mut m, _) = connected(qu("qu-24"), &[0xFE]);
        let s = state(&feed(
            &mut m,
            10,
            &[
                hex("F0 00 00 1A 50 11 01 00 00 11 02 01 0A F7"),
                hex("B0 63 20 B0 62 17 B0 06 62 B0 26 07"),
                hex("B0 63 21 62 20 06 4F 26 05"),
                hex("B0 63 22 62 16 06 00 26 07"),
                hex("B0 63 20 62 5C 06 41 26 07"),
                hex("90 67 7F 90 67 00"),
                hex("F0 00 00 1A 50 11 01 00 00 02 20 4B 69 63 6B F7"),
                hex("F0 00 00 1A 50 11 01 00 00 14 F7"),
                hex("B0 00 00 B0 20 00 C0 04"),
            ]
            .concat(),
        ));
        assert_eq!(s["device"]["model"], "qu-24");
        assert_eq!(s["device"]["firmware"], "1.10");
        assert_eq!(s["channels"]["input"]["1"]["level_db"], 0.0);
        assert_eq!(s["sends"]["input"]["2"]["mix"]["7"]["level_db"], -5.0);
        assert_eq!(s["channels"]["input"]["3"]["pan"], -100.0);
        assert_eq!(s["channels"]["input"]["1"]["mute_group"]["2"], true);
        assert_eq!(s["channels"]["main"]["1"]["mute"], true);
        assert_eq!(s["channels"]["input"]["1"]["name"], "Kick");
        assert_eq!(s["device"]["synced"], true);
        assert_eq!(s["scene"]["current"], 5);
    }
}
