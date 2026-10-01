//! AHM-16, AHM-32 and AHM-64, from "AHM TCP/IP Protocol, Firmware V1.4"
//! (Issue 2).
//!
//! - Fixed MIDI channels select the channel type: inputs 0, zones 1, control
//!   groups 2, rooms 3; the note or NRPN MSB selects the channel (p.1).
//! - Mutes are Note On (7F on, 3F off, then 00); levels, input trim, preamp
//!   gain, pad and phantom power are 7-bit NRPN (17, 18, 19, 1A, 1B), and
//!   NRPN 20 steps a level up (7F) or down (3F) (p.2-3).
//! - SysEx header `F0 00 00 1A 50 12 01 00`: crosspoint send levels and
//!   mutes, playback, source selectors, rooms, names and colours, and every
//!   "get" (`0N 01 ...`) (p.2-6).
//! - Presets are bank and Program Change on channel 0 (p.4).

use serde_json::{json, Value};

use super::allenheath::{invalid, unknown, Args, Dialect, Plan};
use super::allenheath_midi::{self as midi, level_updates, Event, Law, Update};
use crate::catalog::Params;
use crate::module::CommandError;

const HEADER: [u8; 7] = [0x00, 0x00, 0x1A, 0x50, 0x12, 0x01, 0x00];

/// p.7 "Channel Level": LV = [(dB + 48) / 58] * 127, rounded down as the
/// table's values are, with -48 dB at 01 and 00 for -inf.
pub(crate) const LEVEL: Law = Law::Floor {
    lo: -48.0,
    hi: 10.0,
    max: 127,
    floor_raw: 1,
};

/// p.3: "-24 to +24dB = 00 to 7F", given by its ends only.
const TRIM: Law = Law::Linear {
    lo: -24.0,
    hi: 24.0,
    max: 127,
};

/// p.3: "5dB to +60dB = 00 to 7F", given by its ends only.
const GAIN: Law = Law::Linear {
    lo: 5.0,
    hi: 60.0,
    max: 127,
};

/// p.7 "SourceColour".
const COLOURS: [&str; 8] = [
    "off", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

/// (name, MIDI channel, count), p.1.
const TYPES: [(&str, u8, u8); 4] = [
    ("input", 0, 64),
    ("zone", 1, 64),
    ("control_group", 2, 32),
    ("room", 3, 16),
];

pub(crate) struct Ahm;

impl Ahm {
    pub(crate) fn new(_settings: &Params) -> Ahm {
        Ahm
    }

    fn sx(body: &[u8]) -> Vec<u8> {
        midi::sysex(&HEADER, body)
    }

    fn channel(kind: &str, number: i64, allowed: &[&str]) -> Result<(u8, u8), CommandError> {
        if !allowed.contains(&kind) {
            return Err(invalid(format!("channel_type must be one of {allowed:?}")));
        }
        let (_, ch, count) = TYPES
            .iter()
            .find(|(n, ..)| *n == kind)
            .ok_or_else(|| invalid(format!("unknown channel_type '{kind}'")))?;
        if number < 1 || number > *count as i64 {
            return Err(invalid(format!("{kind} {number} is outside 1 to {count}")));
        }
        Ok((*ch, (number - 1) as u8))
    }

    fn arg_channel(a: &Args, allowed: &[&str]) -> Result<(u8, u8, String), CommandError> {
        let kind = a.str("channel_type")?;
        let n = a.int("channel")?;
        let (ch, note) = Self::channel(kind, n, allowed)?;
        Ok((ch, note, format!("channels.{kind}.{n}")))
    }

    fn input(a: &Args) -> Result<(u8, String), CommandError> {
        let n = a.int("channel")?;
        let (_, note) = Self::channel("input", n, &["input"])?;
        Ok((note, format!("channels.input.{n}")))
    }

    fn numbered(a: &Args, key: &str, max: i64) -> Result<u8, CommandError> {
        let n = a.int(key)?;
        if n < 1 || n > max {
            return Err(invalid(format!("{key} is 1-{max}")));
        }
        Ok((n - 1) as u8)
    }

    fn decode(ch: u8, note: u8) -> Option<String> {
        TYPES
            .iter()
            .find(|(_, c, count)| *c == ch && note < *count)
            .map(|(name, ..)| format!("channels.{name}.{}", note as u32 + 1))
    }

    /// Send source and destination: input or zone, to a zone.
    fn crosspoint(a: &Args) -> Result<(u8, u8, u8, String), CommandError> {
        let (ch, note, _) = Self::arg_channel(a, &["input", "zone"])?;
        let kind = a.str("channel_type")?;
        let n = a.int("channel")?;
        let z = Self::numbered(a, "zone", 64)?;
        Ok((
            ch,
            note,
            z,
            format!("sends.{kind}.{n}.zone.{}", z as u32 + 1),
        ))
    }

    fn get(ch: u8, body: &[u8]) -> Vec<u8> {
        let mut b = vec![ch, 0x01];
        b.extend_from_slice(body);
        Self::sx(&b)
    }

    fn on_off(on: bool) -> u8 {
        if on {
            0x7F
        } else {
            0x3F
        }
    }

    /// A source selector message after `00 08 CH`: either the get reply
    /// (count, current, a colour per source, then NUL-terminated names) or the
    /// message sent on a selection (number, colour, name), p.5.
    fn source_selector(zone: u8, rest: &[u8]) -> Vec<Update> {
        let p = format!("zones.{}", zone as u32 + 1);
        if let Some(sources) = Self::source_list(rest) {
            let current = rest[1] as usize;
            let mut out = vec![
                (format!("{p}.source"), json!(current + 1)),
                (format!("{p}.sources"), Value::Array(sources.clone())),
            ];
            if let Some(s) = sources.get(current) {
                out.push((format!("{p}.source_name"), s["name"].clone()));
                out.push((format!("{p}.source_colour"), s["colour"].clone()));
            }
            return out;
        }
        match rest {
            [number, colour, name @ ..] => vec![
                (format!("{p}.source"), json!(*number as u32 + 1)),
                (
                    format!("{p}.source_colour"),
                    json!(COLOURS.get(*colour as usize)),
                ),
                (format!("{p}.source_name"), json!(midi::read_name(name))),
            ],
            _ => Vec::new(),
        }
    }

    fn source_list(rest: &[u8]) -> Option<Vec<Value>> {
        let (&count, rest) = rest.split_first()?;
        let (&current, rest) = rest.split_first()?;
        let count = count as usize;
        if count == 0 || current as usize >= count || rest.len() < count {
            return None;
        }
        let (colours, mut names) = rest.split_at(count);
        let mut out = Vec::new();
        for (i, colour) in colours.iter().enumerate() {
            let end = names.iter().position(|&b| b == 0)?;
            out.push(json!({
                "number": i + 1,
                "name": midi::read_name(&names[..end]),
                "colour": COLOURS.get(*colour as usize),
            }));
            names = &names[end + 1..];
        }
        names.is_empty().then_some(out)
    }
}

const LEVEL_TYPES: &[&str] = &["input", "zone", "control_group"];
const NAMED_TYPES: &[&str] = &["input", "zone", "control_group", "room"];

impl Dialect for Ahm {
    fn command(&mut self, name: &str, a: &Args) -> Result<Plan, CommandError> {
        Ok(match name {
            "set_mute" => {
                let (ch, note, _) = Self::arg_channel(a, LEVEL_TYPES)?;
                let mut b = midi::note_on(ch, note, Self::on_off(a.flag("muted")?));
                b.extend(midi::note_on(ch, note, 0x00));
                Plan::write_then(b, Self::get(ch, &[0x09, note]))
            }
            "get_mute" => {
                let (ch, note, p) = Self::arg_channel(a, LEVEL_TYPES)?;
                Plan::read(Self::get(ch, &[0x09, note]), format!("{p}.mute"))
            }
            "set_level" => {
                let (ch, note, _) = Self::arg_channel(a, LEVEL_TYPES)?;
                let lv = a.level("level", &LEVEL, 127)? as u8;
                Plan::write_then(
                    midi::nrpn(ch, note, 0x17, lv),
                    Self::get(ch, &[0x0B, 0x17, note]),
                )
            }
            "adjust_level" => {
                let (ch, note, _) = Self::arg_channel(a, LEVEL_TYPES)?;
                let v = match a.str("direction")? {
                    "up" => 0x7F,
                    _ => 0x3F,
                };
                Plan::write_then(
                    midi::nrpn(ch, note, 0x20, v),
                    Self::get(ch, &[0x0B, 0x17, note]),
                )
            }
            "get_level" => {
                let (ch, note, p) = Self::arg_channel(a, LEVEL_TYPES)?;
                Plan::read_fields(
                    Self::get(ch, &[0x0B, 0x17, note]),
                    format!("{p}.level_raw"),
                    &["level_db", "level_raw"],
                )
            }
            "set_input_trim" | "get_input_trim" => {
                let (note, p) = Self::input(a)?;
                let get = Self::get(0, &[0x0B, 0x18, note]);
                if name == "get_input_trim" {
                    Plan::read_fields(get, format!("{p}.trim_raw"), &["trim_db", "trim_raw"])
                } else {
                    let v = a.level("trim", &TRIM, 127)? as u8;
                    Plan::write_then(midi::nrpn(0, note, 0x18, v), get)
                }
            }
            "set_preamp_gain" | "get_preamp_gain" => {
                let (note, p) = Self::input(a)?;
                let get = Self::get(0, &[0x0B, 0x19, note]);
                if name == "get_preamp_gain" {
                    Plan::read_fields(get, format!("{p}.gain_raw"), &["gain_db", "gain_raw"])
                } else {
                    let v = a.level("gain", &GAIN, 127)? as u8;
                    Plan::write_then(midi::nrpn(0, note, 0x19, v), get)
                }
            }
            "set_preamp_pad" | "get_preamp_pad" | "set_phantom_power" | "get_phantom_power" => {
                let (note, p) = Self::input(a)?;
                let (id, field) = if name.ends_with("pad") {
                    (0x1A, "pad")
                } else {
                    (0x1B, "phantom_power")
                };
                let get = Self::get(0, &[0x0B, id, note]);
                if name.starts_with("get") {
                    Plan::read(get, format!("{p}.{field}"))
                } else {
                    let v = if a.flag("enabled")? { 0x7F } else { 0x00 };
                    Plan::write_then(midi::nrpn(0, note, id, v), get)
                }
            }
            "set_send_level" | "get_send_level" => {
                let (ch, note, z, p) = Self::crosspoint(a)?;
                let get = Self::get(ch, &[0x0F, 0x02, note, 1, z]);
                if name == "get_send_level" {
                    Plan::read_fields(get, format!("{p}.level_raw"), &["level_db", "level_raw"])
                } else {
                    let lv = a.level("level", &LEVEL, 127)? as u8;
                    Plan::write_then(Self::sx(&[ch, 0x02, note, 1, z, lv]), get)
                }
            }
            "set_send_mute" | "get_send_mute" => {
                let (ch, note, z, p) = Self::crosspoint(a)?;
                let get = Self::get(ch, &[0x0F, 0x03, note, 1, z]);
                if name == "get_send_mute" {
                    Plan::read(get, format!("{p}.mute"))
                } else {
                    let v = Self::on_off(a.flag("muted")?);
                    Plan::write_then(Self::sx(&[ch, 0x03, note, 1, z, v]), get)
                }
            }
            "adjust_send_level" => {
                let (ch, note, z, _) = Self::crosspoint(a)?;
                let v = match a.str("direction")? {
                    "up" => 0x7F,
                    _ => 0x3F,
                };
                Plan::write_then(
                    Self::sx(&[ch, 0x04, note, 1, z, v]),
                    Self::get(ch, &[0x0F, 0x02, note, 1, z]),
                )
            }
            "recall_preset" => {
                let s = a.int("preset")?;
                if !(1..=500).contains(&s) {
                    return Err(invalid("preset is 1-500"));
                }
                let i = (s - 1) as u16;
                Plan::write(midi::bank_program(0, (i / 128) as u8, (i % 128) as u8))
            }
            "play_track" => {
                let track = a.int("track_id")?;
                if !(0..=127).contains(&track) {
                    return Err(invalid("track_id is 0-127"));
                }
                let channel = match a.str("playback_channel")? {
                    "mono_2" => 1,
                    _ => 0,
                };
                Plan::write(Self::sx(&[0x00, 0x06, channel, track as u8]))
            }
            "select_source" => {
                let z = Self::numbered(a, "zone", 64)?;
                let s = Self::numbered(a, "source", 20)?;
                Plan::write_then(
                    Self::sx(&[0x00, 0x08, z, s]),
                    Self::get(1, &[0x0F, 0x08, z]),
                )
            }
            "get_source" => {
                let z = Self::numbered(a, "zone", 64)?;
                // p.5 writes the get as "0N, 01, 0F, 08, CH"; N is taken as
                // the zones' channel, 1.
                Plan::read(
                    Self::get(1, &[0x0F, 0x08, z]),
                    format!("zones.{}.source", z as u32 + 1),
                )
            }
            "select_room_source" => {
                let r = Self::numbered(a, "room", 16)?;
                let s = Self::numbered(a, "source", 20)?;
                Plan::write(Self::sx(&[0x00, 0x0D, r, s]))
            }
            "set_room_divider" => {
                let r1 = Self::numbered(a, "room", 16)?;
                let r2 = Self::numbered(a, "other_room", 16)?;
                let v = if a.flag("divided")? { 0x7F } else { 0x00 };
                Plan::write(Self::sx(&[0x00, 0x0E, r1, r2, v]))
            }
            "get_channel_name" => {
                let (ch, note, p) = Self::arg_channel(a, NAMED_TYPES)?;
                Plan::read(Self::sx(&[ch, 0x09, note]), format!("{p}.name"))
            }
            "get_channel_colour" => {
                let (ch, note, p) = Self::arg_channel(a, NAMED_TYPES)?;
                Plan::read(Self::sx(&[ch, 0x0B, note]), format!("{p}.colour"))
            }
            other => return Err(unknown(other)),
        })
    }

    fn event(&mut self, event: &Event) -> Vec<Update> {
        match *event {
            Event::Note { ch, note, velocity } => Self::decode(ch, note)
                .filter(|_| ch < 3)
                .map(|p| vec![(format!("{p}.mute"), Value::Bool(velocity >= 0x40))])
                .unwrap_or_default(),
            Event::Nrpn {
                ch,
                msb,
                lsb,
                value,
            } if ch < 3 => {
                let Some(p) = Self::decode(ch, msb) else {
                    return Vec::new();
                };
                match (ch, lsb) {
                    (_, 0x17) => level_updates(&p, "level", &LEVEL, value as u16),
                    (0, 0x18) => level_updates(&p, "trim", &TRIM, value as u16),
                    (0, 0x19) => level_updates(&p, "gain", &GAIN, value as u16),
                    (0, 0x1A) => vec![(format!("{p}.pad"), Value::Bool(value >= 0x40))],
                    (0, 0x1B) => vec![(format!("{p}.phantom_power"), Value::Bool(value >= 0x40))],
                    _ => Vec::new(),
                }
            }
            Event::Program {
                ch: 0,
                bank_msb,
                program,
                ..
            } => vec![(
                "preset.current".into(),
                json!(bank_msb.unwrap_or(0) as u32 * 128 + program as u32 + 1),
            )],
            Event::SysEx(ref body) => {
                if body.len() < HEADER.len() + 2 || body[..5] != HEADER[..5] {
                    return Vec::new();
                }
                let m = &body[HEADER.len()..];
                let (ch, kind, rest) = (m[0], m[1], &m[2..]);
                match (kind, rest) {
                    (0x02 | 0x03, [note, 1, z, v]) if ch < 2 => match Self::decode(ch, *note) {
                        Some(src) => {
                            let p =
                                format!("sends{}.zone.{}", &src["channels".len()..], *z as u32 + 1);
                            if kind == 0x02 {
                                level_updates(&p, "level", &LEVEL, *v as u16)
                            } else {
                                vec![(format!("{p}.mute"), Value::Bool(*v >= 0x40))]
                            }
                        }
                        None => Vec::new(),
                    },
                    (0x08, [zone, rest @ ..]) if ch == 0 => Self::source_selector(*zone, rest),
                    (0x0D, [room, number, colour, name @ ..]) if ch == 0 => {
                        let p = format!("rooms.{}", *room as u32 + 1);
                        vec![
                            (format!("{p}.source"), json!(*number as u32 + 1)),
                            (
                                format!("{p}.source_colour"),
                                json!(COLOURS.get(*colour as usize)),
                            ),
                            (format!("{p}.source_name"), json!(midi::read_name(name))),
                        ]
                    }
                    (0x0E, [r1, r2, v]) if ch == 0 => vec![(
                        format!("rooms.{}.divided_from.{}", *r1 as u32 + 1, *r2 as u32 + 1),
                        Value::Bool(*v >= 0x40),
                    )],
                    (0x0A, [note, name @ ..]) => Self::decode(ch, *note)
                        .map(|p| vec![(format!("{p}.name"), json!(midi::read_name(name)))])
                        .unwrap_or_default(),
                    (0x0C, [note, colour]) => {
                        match (Self::decode(ch, *note), COLOURS.get(*colour as usize)) {
                            (Some(p), Some(c)) => vec![(format!("{p}.colour"), json!(c))],
                            _ => Vec::new(),
                        }
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    fn sync(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for (name, ch, count) in TYPES {
            for note in 0..count {
                out.push(Self::sx(&[ch, 0x09, note]));
                out.push(Self::sx(&[ch, 0x0B, note]));
                if name == "room" {
                    continue;
                }
                out.push(Self::get(ch, &[0x09, note]));
                out.push(Self::get(ch, &[0x0B, 0x17, note]));
                if name == "input" {
                    for id in [0x18, 0x19, 0x1A, 0x1B] {
                        out.push(Self::get(0, &[0x0B, id, note]));
                    }
                }
                if name == "zone" {
                    out.push(Self::get(1, &[0x0F, 0x08, note]));
                }
            }
        }
        out
    }

    fn probe(&self) -> Vec<u8> {
        Self::sx(&[0x00, 0x09, 0x00])
    }

    fn silence_hint(&self) -> &'static str {
        "check that external control is enabled in System Manager and that the port is the \
         unencrypted one (51325)"
    }
}

#[cfg(test)]
mod tests {
    use super::super::allenheath::testing::*;
    use super::*;
    use crate::module::{Action, Outcome};

    fn h(rest: &str) -> Vec<u8> {
        hex(&format!("F0 00 00 1A 50 12 01 00 {rest} F7"))
    }

    fn ahm() -> Box<Ahm> {
        Box::new(Ahm::new(&params(json!({}))))
    }

    #[test]
    fn level_formula_matches_the_table() {
        // p.7 "Channel Level".
        for (db, raw) in [
            (10.0, 0x7F),
            (5.0, 0x74),
            (0.0, 0x69),
            (-5.0, 0x5E),
            (-10.0, 0x53),
            (-15.0, 0x48),
            (-20.0, 0x3D),
            (-25.0, 0x32),
            (-30.0, 0x27),
            (-35.0, 0x1C),
            (-40.0, 0x11),
            (-45.0, 0x06),
            (-48.0, 0x01),
        ] {
            assert_eq!(LEVEL.encode(db), Some(raw), "{db} dB");
            assert_eq!(LEVEL.decode(raw), Some(db), "{raw:02X}");
        }
        assert_eq!(LEVEL.decode(0), None, "-inf");
    }

    #[test]
    fn documented_messages_byte_for_byte() {
        let (mut m, _) = connected(ahm(), &[0xFE]);
        let b = |m: &mut _, name, p| bytes(m, name, p).unwrap();
        // p.2: mute on zone 3, then its get.
        assert_eq!(
            b(
                &mut m,
                "set_mute",
                json!({"channel_type": "zone", "channel": 3, "muted": true})
            ),
            [hex("91 02 7F 91 02 00"), h("01 01 09 02")].concat()
        );
        // p.2: control group 1 level 0 dB = 69.
        assert_eq!(
            b(
                &mut m,
                "set_level",
                json!({"channel_type": "control_group", "channel": 1, "level_db": 0.0})
            ),
            [hex("B2 63 00 B2 62 17 B2 06 69"), h("02 01 0B 17 00")].concat()
        );
        // p.2: level increment.
        assert_eq!(
            b(
                &mut m,
                "adjust_level",
                json!({"channel_type": "input", "channel": 1, "direction": "up"})
            )[..9],
            hex("B0 63 00 B0 62 20 B0 06 7F")
        );
        // p.3: phantom power on input 64.
        assert_eq!(
            b(
                &mut m,
                "set_phantom_power",
                json!({"channel": 64, "enabled": true})
            ),
            [hex("B0 63 3F B0 62 1B B0 06 7F"), h("00 01 0B 1B 3F")].concat()
        );
        // p.4: input 2 to zone 5 send level and mute.
        assert_eq!(
            b(
                &mut m,
                "set_send_level",
                json!({"channel_type": "input", "channel": 2, "zone": 5, "level_raw": 0x69})
            ),
            [h("00 02 01 01 04 69"), h("00 01 0F 02 01 01 04")].concat()
        );
        assert_eq!(
            b(
                &mut m,
                "set_send_mute",
                json!({"channel_type": "zone", "channel": 1, "zone": 2, "muted": false})
            )[..15],
            h("01 03 00 01 01 3F")[..]
        );
        // p.4: preset 129 is bank 01 program 00 on channel 0.
        assert_eq!(
            b(&mut m, "recall_preset", json!({"preset": 129})),
            hex("B0 00 01 C0 00")
        );
        // p.5: playback, source selector, rooms.
        assert_eq!(
            b(
                &mut m,
                "play_track",
                json!({"track_id": 3, "playback_channel": "mono_2"})
            ),
            h("00 06 01 03")
        );
        assert_eq!(
            b(&mut m, "select_source", json!({"zone": 1, "source": 20})),
            [h("00 08 00 13"), h("01 01 0F 08 00")].concat()
        );
        assert_eq!(
            b(
                &mut m,
                "set_room_divider",
                json!({"room": 1, "other_room": 2, "divided": true})
            ),
            h("00 0E 00 01 7F")
        );
        // p.6: name and colour gets.
        assert_eq!(
            b(
                &mut m,
                "get_channel_name",
                json!({"channel_type": "room", "channel": 16})
            ),
            h("03 09 0F")
        );
    }

    #[test]
    fn pushed_and_requested_values_become_state() {
        let (mut m, _) = connected(ahm(), &[0xFE]);
        let s = state(&feed(
            &mut m,
            10,
            &[
                hex("90 00 7F"),
                hex("B1 63 04 B1 62 17 B1 06 69"),
                hex("B0 63 00 B0 62 1A B0 06 7F"),
                hex("B0 00 03 C0 73"),
                h("00 02 00 01 04 06"),
                h("00 0A 00 4D 69 63"),
                h("01 0C 00 05"),
                // Source selector push: zone 1, source 3, red, "TV".
                h("00 08 00 02 01 54 56"),
                h("00 0E 01 02 10"),
            ]
            .concat(),
        ));
        assert_eq!(s["channels"]["input"]["1"]["mute"], true);
        assert_eq!(s["channels"]["zone"]["5"]["level_db"], 0.0);
        assert_eq!(s["channels"]["input"]["1"]["pad"], true);
        assert_eq!(s["preset"]["current"], 500);
        assert_eq!(s["sends"]["input"]["1"]["zone"]["5"]["level_db"], -45.0);
        assert_eq!(s["channels"]["input"]["1"]["name"], "Mic");
        assert_eq!(s["channels"]["zone"]["1"]["colour"], "magenta");
        assert_eq!(s["zones"]["1"]["source"], 3);
        assert_eq!(s["zones"]["1"]["source_name"], "TV");
        assert_eq!(s["rooms"]["2"]["divided_from"]["3"], false);
    }

    #[test]
    fn the_source_list_reply_answers_get_source() {
        let (mut m, _) = connected(ahm(), &[0xFE]);
        let a = run(&mut m, 10, "get_source", json!({"zone": 2}));
        assert_eq!(sent(&a), [h("01 01 0F 08 01")]);
        // Two sources, the second selected: colours red, blue; "A", "BC".
        let a = feed(&mut m, 20, &h("00 08 01 02 01 01 04 41 00 42 43 00"));
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value { value: json!(2) })
        }));
        let s = state(&a);
        assert_eq!(s["zones"]["2"]["source_name"], "BC");
        assert_eq!(
            s["zones"]["2"]["sources"],
            json!([
                {"number": 1, "name": "A", "colour": "red"},
                {"number": 2, "name": "BC", "colour": "blue"}
            ])
        );
    }
}
