//! MIDI as Allen & Heath consoles carry it over TCP: a byte stream with no
//! framing of its own, so messages are delimited by MIDI's own rules.
//!
//! - A status byte (80-EF) starts a channel message; data bytes (00-7F) that
//!   follow without a new status reuse the last one (running status, which
//!   dLive and Avantis use: "9B, 00, 7F, 01, 7F, 02, 7F" is three Note Ons,
//!   dLive MIDI Over TCP/IP Protocol V2.0 p.1).
//! - System exclusive runs from F0 to F7. A status byte other than a
//!   real-time one ends it early, and the partial message is dropped.
//! - Real-time bytes (F8-FF) may appear anywhere, even inside SysEx, and
//!   change nothing else. Qu sends Active Sensing (FE) to show it is there.
//! - System common messages (F1-F6) cancel running status.
//!
//! On top of the byte level, [`Assembler`] turns controller sequences into the
//! units the consoles use: an NRPN (CC 63 parameter MSB, CC 62 LSB, CC 06
//! data, optionally CC 26 data LSB) and a scene recall (CC 00 / CC 20 bank,
//! then Program Change).

use serde_json::{Map, Value};

/// SysEx longer than this is dropped rather than grown without limit.
const MAX_SYSEX: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Midi {
    NoteOff {
        ch: u8,
        note: u8,
        velocity: u8,
    },
    NoteOn {
        ch: u8,
        note: u8,
        velocity: u8,
    },
    Cc {
        ch: u8,
        controller: u8,
        value: u8,
    },
    Program {
        ch: u8,
        program: u8,
    },
    PitchBend {
        ch: u8,
        first: u8,
        second: u8,
    },
    /// Poly or channel pressure: parsed so the stream stays aligned, unused.
    Pressure,
    /// The bytes between F0 and F7.
    SysEx(Vec<u8>),
    ActiveSensing,
    /// Any other real-time byte.
    RealTime(u8),
}

/// Bytes to messages, keeping a partial message between calls.
#[derive(Debug, Default)]
pub(crate) struct Parser {
    running: Option<u8>,
    data: Vec<u8>,
    sysex: Option<Vec<u8>>,
    /// Data bytes of a system common message still to skip.
    skip: u8,
}

fn data_len(status: u8) -> usize {
    match status & 0xF0 {
        0xC0 | 0xD0 => 1,
        _ => 2,
    }
}

impl Parser {
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<Midi> {
        let mut out = Vec::new();
        for &b in bytes {
            if b >= 0xF8 {
                out.push(if b == 0xFE {
                    Midi::ActiveSensing
                } else {
                    Midi::RealTime(b)
                });
                continue;
            }
            if let Some(sysex) = self.sysex.as_mut() {
                if b < 0x80 {
                    if sysex.len() < MAX_SYSEX {
                        sysex.push(b);
                    } else {
                        self.sysex = None;
                    }
                    continue;
                }
                let body = self.sysex.take().unwrap_or_default();
                if b == 0xF7 {
                    out.push(Midi::SysEx(body));
                    continue;
                }
                // Any other status ends the SysEx without its F7: dropped,
                // and the byte is handled as the status it is.
            }
            match b {
                0xF0 => {
                    self.sysex = Some(Vec::new());
                    self.running = None;
                    self.skip = 0;
                }
                0xF1..=0xF7 => {
                    self.running = None;
                    self.data.clear();
                    self.skip = match b {
                        0xF1 | 0xF3 => 1,
                        0xF2 => 2,
                        _ => 0,
                    };
                }
                0x80..=0xEF => {
                    self.running = Some(b);
                    self.data.clear();
                    self.skip = 0;
                }
                _ => {
                    if self.skip > 0 {
                        self.skip -= 1;
                        continue;
                    }
                    let Some(status) = self.running else {
                        continue;
                    };
                    self.data.push(b);
                    if self.data.len() == data_len(status) {
                        out.push(message(status, &self.data));
                        self.data.clear();
                    }
                }
            }
        }
        out
    }
}

fn message(status: u8, d: &[u8]) -> Midi {
    let ch = status & 0x0F;
    match status & 0xF0 {
        0x80 => Midi::NoteOff {
            ch,
            note: d[0],
            velocity: d[1],
        },
        0x90 => Midi::NoteOn {
            ch,
            note: d[0],
            velocity: d[1],
        },
        0xB0 => Midi::Cc {
            ch,
            controller: d[0],
            value: d[1],
        },
        0xC0 => Midi::Program { ch, program: d[0] },
        0xE0 => Midi::PitchBend {
            ch,
            first: d[0],
            second: d[1],
        },
        _ => Midi::Pressure,
    }
}

/// What the consoles mean by a run of MIDI messages.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    /// Note On with a non-zero velocity. Velocity 0 and Note Off are ignored
    /// by every A&H mute protocol, so they produce nothing.
    Note {
        ch: u8,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        ch: u8,
        note: u8,
    },
    /// NRPN data entry MSB (CC 06) for the parameter last selected.
    Nrpn {
        ch: u8,
        msb: u8,
        lsb: u8,
        value: u8,
    },
    /// NRPN data entry LSB (CC 26), with the CC 06 value that preceded it.
    NrpnFine {
        ch: u8,
        msb: u8,
        lsb: u8,
        coarse: Option<u8>,
        fine: u8,
    },
    /// Program Change, with the bank selected on that channel before it.
    Program {
        ch: u8,
        bank_msb: Option<u8>,
        bank_lsb: Option<u8>,
        program: u8,
    },
    PitchBend {
        ch: u8,
        first: u8,
        second: u8,
    },
    /// Any other controller.
    Cc {
        ch: u8,
        controller: u8,
        value: u8,
    },
    SysEx(Vec<u8>),
    ActiveSensing,
}

#[derive(Debug, Default, Clone, Copy)]
struct ChannelState {
    nrpn_msb: Option<u8>,
    nrpn_lsb: Option<u8>,
    data_msb: Option<u8>,
    bank_msb: Option<u8>,
    bank_lsb: Option<u8>,
}

/// Per-channel controller state, turning [`Midi`] into [`Event`]s.
#[derive(Debug, Default)]
pub(crate) struct Assembler {
    channels: [ChannelState; 16],
}

impl Assembler {
    pub(crate) fn feed(&mut self, msg: Midi) -> Option<Event> {
        match msg {
            Midi::NoteOn { ch, note, velocity } if velocity > 0 => {
                Some(Event::Note { ch, note, velocity })
            }
            Midi::NoteOn { ch, note, .. } | Midi::NoteOff { ch, note, .. } => {
                Some(Event::NoteOff { ch, note })
            }
            Midi::Cc {
                ch,
                controller,
                value,
            } => {
                let c = &mut self.channels[ch as usize];
                match controller {
                    0x63 => {
                        c.nrpn_msb = Some(value);
                        c.data_msb = None;
                        None
                    }
                    0x62 => {
                        c.nrpn_lsb = Some(value);
                        c.data_msb = None;
                        None
                    }
                    0x06 => {
                        c.data_msb = Some(value);
                        Some(Event::Nrpn {
                            ch,
                            msb: c.nrpn_msb?,
                            lsb: c.nrpn_lsb?,
                            value,
                        })
                    }
                    0x26 => Some(Event::NrpnFine {
                        ch,
                        msb: c.nrpn_msb?,
                        lsb: c.nrpn_lsb?,
                        coarse: c.data_msb,
                        fine: value,
                    }),
                    0x00 => {
                        c.bank_msb = Some(value);
                        None
                    }
                    0x20 => {
                        c.bank_lsb = Some(value);
                        None
                    }
                    _ => Some(Event::Cc {
                        ch,
                        controller,
                        value,
                    }),
                }
            }
            Midi::Program { ch, program } => {
                let c = &self.channels[ch as usize];
                Some(Event::Program {
                    ch,
                    bank_msb: c.bank_msb,
                    bank_lsb: c.bank_lsb,
                    program,
                })
            }
            Midi::PitchBend { ch, first, second } => Some(Event::PitchBend { ch, first, second }),
            Midi::SysEx(body) => Some(Event::SysEx(body)),
            Midi::ActiveSensing => Some(Event::ActiveSensing),
            Midi::Pressure | Midi::RealTime(_) => None,
        }
    }
}

// Encoders. Status bytes are always written in full: running status is an
// option for the sender, and every receiver accepts full messages.

pub(crate) fn note_on(ch: u8, note: u8, velocity: u8) -> Vec<u8> {
    vec![0x90 | ch, note, velocity]
}

pub(crate) fn note_off(ch: u8, note: u8, velocity: u8) -> Vec<u8> {
    vec![0x80 | ch, note, velocity]
}

pub(crate) fn cc(ch: u8, controller: u8, value: u8) -> Vec<u8> {
    vec![0xB0 | ch, controller, value]
}

/// NRPN with a 7-bit value: `BN 63 MSB, BN 62 LSB, BN 06 value`.
pub(crate) fn nrpn(ch: u8, msb: u8, lsb: u8, value: u8) -> Vec<u8> {
    let mut v = cc(ch, 0x63, msb);
    v.extend(cc(ch, 0x62, lsb));
    v.extend(cc(ch, 0x06, value));
    v
}

/// NRPN with data MSB and LSB: `... BN 06 coarse, BN 26 fine`.
pub(crate) fn nrpn_fine(ch: u8, msb: u8, lsb: u8, coarse: u8, fine: u8) -> Vec<u8> {
    let mut v = nrpn(ch, msb, lsb, coarse);
    v.extend(cc(ch, 0x26, fine));
    v
}

/// NRPN data increment (CC 60) or decrement (CC 61) with a value.
pub(crate) fn nrpn_step(ch: u8, msb: u8, lsb: u8, controller: u8, value: u8) -> Vec<u8> {
    let mut v = cc(ch, 0x63, msb);
    v.extend(cc(ch, 0x62, lsb));
    v.extend(cc(ch, controller, value));
    v
}

/// Bank select MSB then Program Change.
pub(crate) fn bank_program(ch: u8, bank: u8, program: u8) -> Vec<u8> {
    let mut v = cc(ch, 0x00, bank);
    v.extend([0xC0 | ch, program]);
    v
}

/// `F0, header..., body..., F7`.
pub(crate) fn sysex(header: &[u8], body: &[u8]) -> Vec<u8> {
    let mut v = vec![0xF0];
    v.extend_from_slice(header);
    v.extend_from_slice(body);
    v.push(0xF7);
    v
}

/// A 14-bit value as (coarse, fine) 7-bit halves.
pub(crate) fn split14(v: u16) -> (u8, u8) {
    (((v >> 7) & 0x7F) as u8, (v & 0x7F) as u8)
}

pub(crate) fn join14(coarse: u8, fine: u8) -> u16 {
    ((coarse as u16 & 0x7F) << 7) | (fine as u16 & 0x7F)
}

/// Text for a name SysEx: printable ASCII only, since SysEx data bytes are
/// 7-bit and the documents' character tables list nothing else.
pub(crate) fn ascii_name(name: &str) -> Result<Vec<u8>, String> {
    if let Some(c) = name.chars().find(|c| !(' '..='~').contains(c)) {
        return Err(format!(
            "'{c}' cannot be sent: names are printable ASCII (20-7E)"
        ));
    }
    Ok(name.as_bytes().to_vec())
}

/// A name as received: up to the first NUL, other bytes as ASCII.
pub(crate) fn read_name(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect()
}

/// Hex text such as `"B0 63 00"` to bytes.
pub(crate) fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    let digits: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if digits.is_empty() || !digits.len().is_multiple_of(2) {
        return Err("give whole bytes in hex, such as 'B0 63 00'".into());
    }
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// Conversion between a documented unit (dB) and the value on the wire.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Law {
    /// The document's own table, ascending by dB, with values between its
    /// points interpolated in a straight line and rounded to the nearest step.
    Table(&'static [(f64, u16)]),
    /// `floor((dB - lo) / (hi - lo) * max)`, the dLive/Avantis and AHM
    /// formula `[(dB + 54) / 64] * 7F` written generally. Values at or below
    /// `floor_raw` (except 0) read as `lo`.
    Floor {
        lo: f64,
        hi: f64,
        max: u16,
        floor_raw: u16,
    },
    /// A range the document gives only by its ends: spaced evenly between
    /// them and rounded to the nearest step.
    Linear { lo: f64, hi: f64, max: u16 },
}

impl Law {
    /// The documented range in dB.
    pub(crate) fn range(&self) -> (f64, f64) {
        match *self {
            Law::Table(points) => (points[0].0, points[points.len() - 1].0),
            Law::Floor { lo, hi, .. } | Law::Linear { lo, hi, .. } => (lo, hi),
        }
    }

    /// dB to the wire value, or None outside the documented range.
    pub(crate) fn encode(&self, db: f64) -> Option<u16> {
        let (lo, hi) = self.range();
        if !db.is_finite() || db < lo - 1e-9 || db > hi + 1e-9 {
            return None;
        }
        match *self {
            Law::Table(points) => {
                for pair in points.windows(2) {
                    let ((d0, r0), (d1, r1)) = (pair[0], pair[1]);
                    if db <= d1 + 1e-9 {
                        let t = ((db - d0) / (d1 - d0)).clamp(0.0, 1.0);
                        let r = r0 as f64 + t * (r1 as f64 - r0 as f64);
                        return Some(r.round() as u16);
                    }
                }
                Some(points[points.len() - 1].1)
            }
            Law::Floor {
                lo,
                hi,
                max,
                floor_raw,
            } => {
                let raw = ((db - lo) / (hi - lo) * max as f64 + 1e-9).floor() as u16;
                Some(raw.max(floor_raw).min(max))
            }
            Law::Linear { lo, hi, max } => {
                Some((((db - lo) / (hi - lo)) * max as f64).round() as u16)
            }
        }
    }

    /// The wire value to dB, or None where the document gives no dB for it
    /// (its "-inf", or below the first point of a table).
    pub(crate) fn decode(&self, raw: u16) -> Option<f64> {
        let db = match *self {
            Law::Table(points) => {
                let first = points[0].1;
                let last = points[points.len() - 1].1;
                if raw < first || raw > last {
                    return None;
                }
                let mut found = points[points.len() - 1].0;
                for pair in points.windows(2) {
                    let ((d0, r0), (d1, r1)) = (pair[0], pair[1]);
                    if raw <= r1 {
                        let t = (raw - r0) as f64 / (r1 - r0) as f64;
                        found = d0 + t * (d1 - d0);
                        break;
                    }
                }
                found
            }
            Law::Floor {
                lo,
                hi,
                max,
                floor_raw,
            } => {
                if raw == 0 || raw > max {
                    return None;
                }
                if raw <= floor_raw {
                    return Some(lo);
                }
                // A raw value covers a step of dB values. Report the whole dB
                // in it, or else the half, or else the tenth, so the
                // documents' table values (0 dB = 6B) read back exactly.
                let step = (hi - lo) / max as f64;
                let from = lo + raw as f64 * step;
                let to = if raw == max { hi + 1e-9 } else { from + step };
                [1.0, 0.5, 0.1]
                    .iter()
                    .map(|s| (from / s - 1e-9).ceil() * s)
                    .find(|v| *v < to)
                    .unwrap_or(from)
            }
            Law::Linear { lo, hi, max } => {
                if raw > max {
                    return None;
                }
                lo + raw as f64 * (hi - lo) / max as f64
            }
        };
        Some((db * 100.0).round() / 100.0)
    }
}

/// A state change: a dotted path and its new value.
pub(crate) type Update = (String, Value);

/// Updates to a JSON merge patch.
pub(crate) fn patch(updates: &[Update]) -> Value {
    fn insert(map: &mut Map<String, Value>, parts: &[&str], value: &Value) {
        match parts {
            [] => {}
            [last] => {
                map.insert(last.to_string(), value.clone());
            }
            [first, rest @ ..] => {
                let child = map
                    .entry(first.to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                if !child.is_object() {
                    *child = Value::Object(Map::new());
                }
                if let Value::Object(m) = child {
                    insert(m, rest, value);
                }
            }
        }
    }
    let mut root = Map::new();
    for (path, value) in updates {
        let parts: Vec<&str> = path.split('.').collect();
        insert(&mut root, &parts, value);
    }
    Value::Object(root)
}

/// The level updates for `prefix`: the raw value always, and dB where the
/// law gives one (null otherwise, which removes a previous reading).
pub(crate) fn level_updates(prefix: &str, name: &str, law: &Law, raw: u16) -> Vec<Update> {
    vec![
        (format!("{prefix}.{name}_raw"), Value::from(raw)),
        (
            format!("{prefix}.{name}_db"),
            law.decode(raw).map(Value::from).unwrap_or(Value::Null),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_status_from_the_dlive_document() {
        // dLive V2.0 p.1: Mute on for Inputs 1, 2 and 3 on MIDI channel 12.
        let full = Parser::default().feed(&[0x9B, 0x00, 0x7F, 0x9B, 0x01, 0x7F, 0x9B, 0x02, 0x7F]);
        let short = Parser::default().feed(&[0x9B, 0x00, 0x7F, 0x01, 0x7F, 0x02, 0x7F]);
        assert_eq!(full, short);
        assert_eq!(
            short,
            (0..3)
                .map(|note| Midi::NoteOn {
                    ch: 11,
                    note,
                    velocity: 0x7F
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn messages_split_across_reads_and_realtime_inside_sysex() {
        let mut p = Parser::default();
        assert!(p.feed(&[0xF0, 0x00, 0x00]).is_empty());
        assert_eq!(p.feed(&[0x1A, 0xFE, 0x50]), vec![Midi::ActiveSensing]);
        assert_eq!(
            p.feed(&[0x10, 0xF7, 0xB0]),
            vec![Midi::SysEx(vec![0x00, 0x00, 0x1A, 0x50, 0x10])]
        );
        assert_eq!(
            p.feed(&[0x63, 0x05, 0x62]),
            vec![Midi::Cc {
                ch: 0,
                controller: 0x63,
                value: 0x05
            }]
        );
        assert_eq!(
            p.feed(&[0x17]),
            vec![Midi::Cc {
                ch: 0,
                controller: 0x62,
                value: 0x17
            }]
        );
    }

    #[test]
    fn a_sysex_cut_short_by_a_status_is_dropped() {
        let mut p = Parser::default();
        assert_eq!(
            p.feed(&[0xF0, 0x01, 0x02, 0x90, 0x10, 0x7F]),
            vec![Midi::NoteOn {
                ch: 0,
                note: 0x10,
                velocity: 0x7F
            }]
        );
        // System common cancels running status: the stray data is ignored.
        assert!(p.feed(&[0xF6, 0x10, 0x7F]).is_empty());
    }

    #[test]
    fn nrpn_and_bank_program_assembly() {
        let mut p = Parser::default();
        let mut a = Assembler::default();
        let mut events = |bytes: &[u8]| -> Vec<Event> {
            p.feed(bytes)
                .into_iter()
                .filter_map(|m| a.feed(m))
                .collect()
        };
        // SQ Issue 5 p.12: Ip40 to Aux5 -12 dB on channel 4, running status.
        assert_eq!(
            events(&[0xB3, 0x63, 0x44, 0x62, 0x1C, 0x06, 0x6B, 0x26, 0x06]),
            vec![
                Event::Nrpn {
                    ch: 3,
                    msb: 0x44,
                    lsb: 0x1C,
                    value: 0x6B
                },
                Event::NrpnFine {
                    ch: 3,
                    msb: 0x44,
                    lsb: 0x1C,
                    coarse: Some(0x6B),
                    fine: 0x06
                }
            ]
        );
        // SQ p.9: scene 156 on channel 3 is B2 00 01 C2 1B.
        assert_eq!(
            events(&[0xB2, 0x00, 0x01, 0xC2, 0x1B]),
            vec![Event::Program {
                ch: 2,
                bank_msb: Some(1),
                bank_lsb: None,
                program: 0x1B
            }]
        );
        // Note On velocity 0 is a release, not a mute.
        assert_eq!(
            events(&[0x90, 0x05, 0x00]),
            vec![Event::NoteOff { ch: 0, note: 5 }]
        );
    }

    #[test]
    fn encoders() {
        assert_eq!(
            nrpn(0x0B, 0x20, 0x17, 0x6B),
            [0xBB, 0x63, 0x20, 0xBB, 0x62, 0x17, 0xBB, 0x06, 0x6B]
        );
        assert_eq!(
            nrpn_fine(0, 0x40, 0x00, 0x76, 0x5C),
            [0xB0, 0x63, 0x40, 0xB0, 0x62, 0x00, 0xB0, 0x06, 0x76, 0xB0, 0x26, 0x5C]
        );
        assert_eq!(bank_program(0, 0, 6), [0xB0, 0x00, 0x00, 0xC0, 0x06]);
        assert_eq!(split14(join14(0x3B, 0x40)), (0x3B, 0x40));
        assert_eq!(join14(0x76, 0x5C), 15196);
        assert_eq!(parse_hex("b0 63 00").unwrap(), [0xB0, 0x63, 0x00]);
        assert!(parse_hex("B0 6").is_err());
        assert!(ascii_name("Vox 1").is_ok());
        assert!(ascii_name("Vóx").is_err());
        assert_eq!(read_name(b"Kick\0\0"), "Kick");
    }

    #[test]
    fn table_law_hits_every_point_and_interpolates_between() {
        static T: &[(f64, u16)] = &[(-10.0, 10), (0.0, 30), (10.0, 40)];
        let law = Law::Table(T);
        for &(db, raw) in T {
            assert_eq!(law.encode(db), Some(raw));
            assert_eq!(law.decode(raw), Some(db));
        }
        assert_eq!(law.encode(-5.0), Some(20));
        assert_eq!(law.decode(35), Some(5.0));
        assert_eq!(law.encode(10.5), None);
        assert_eq!(law.decode(5), None);
    }

    #[test]
    fn patch_builds_nested_objects() {
        let p = patch(&[
            ("channels.input.1.mute".into(), Value::Bool(true)),
            ("channels.input.1.level_raw".into(), Value::from(5)),
            ("scene.current".into(), Value::from(3)),
        ]);
        assert_eq!(
            p,
            serde_json::json!({"channels": {"input": {"1": {"mute": true, "level_raw": 5}}}, "scene": {"current": 3}})
        );
    }
}
