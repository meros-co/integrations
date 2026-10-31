//! MIDI 1.0 messages, as the core's RTP-MIDI sessions carry them.
//!
//! From the MIDI 1.0 Detailed Specification 4.2.1 (MIDI Manufacturers
//! Association, 1996), MIDI Time Code (MMA 4.2.1) and MIDI Machine Control
//! 1.0 (MMA RP-013):
//!
//! - A status byte (80-FF) starts a message. Channel messages (80-EF) carry
//!   one or two data bytes (00-7F); data bytes that follow without a new
//!   status reuse the last channel status (running status).
//! - System exclusive runs from F0 to F7. Any status byte other than a
//!   real-time one ends it too, and is taken as the end of the message.
//! - System common messages (F1-F7) cancel running status: F1 (MTC quarter
//!   frame) and F3 (song select) carry one data byte, F2 (song position) two,
//!   F6 (tune request) none. F4 and F5 are undefined; data bytes after them
//!   are skipped, as their length is not known.
//! - Real-time bytes (F8-FF) may appear anywhere, even inside another
//!   message, and change nothing else.
//!
//! Received bytes are parsed by [`Parser`]; messages are written whole, with
//! their status byte, by [`Message::encode`]. Channels are 0 to 15 here and
//! 1 to 16 wherever a person reads them.

use crate::{
    json,
    value::{Map, Value},
};

/// One MIDI message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Message {
    NoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    PolyPressure {
        channel: u8,
        note: u8,
        pressure: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    ChannelPressure {
        channel: u8,
        pressure: u8,
    },
    /// 0 to 16383, 8192 at the centre.
    PitchBend {
        channel: u8,
        value: u16,
    },
    /// The bytes between F0 and F7.
    SysEx(Vec<u8>),
    QuarterFrame {
        piece: u8,
        value: u8,
    },
    /// MIDI beats (sixteenth notes) since the start of the song.
    SongPosition(u16),
    SongSelect(u8),
    TuneRequest,
    /// An undefined system common status: F4 or F5.
    Undefined(u8),
    Clock,
    Start,
    Continue,
    Stop,
    ActiveSensing,
    Reset,
    /// An undefined real-time status: F9 or FD.
    RealTime(u8),
}

impl Message {
    /// The message's bytes, its status byte always written.
    pub(crate) fn encode(&self) -> Vec<u8> {
        match *self {
            Message::NoteOff {
                channel,
                note,
                velocity,
            } => vec![0x80 | channel, note, velocity],
            Message::NoteOn {
                channel,
                note,
                velocity,
            } => vec![0x90 | channel, note, velocity],
            Message::PolyPressure {
                channel,
                note,
                pressure,
            } => vec![0xA0 | channel, note, pressure],
            Message::ControlChange {
                channel,
                controller,
                value,
            } => vec![0xB0 | channel, controller, value],
            Message::ProgramChange { channel, program } => vec![0xC0 | channel, program],
            Message::ChannelPressure { channel, pressure } => vec![0xD0 | channel, pressure],
            Message::PitchBend { channel, value } => {
                vec![
                    0xE0 | channel,
                    (value & 0x7F) as u8,
                    (value >> 7) as u8 & 0x7F,
                ]
            }
            Message::SysEx(ref body) => {
                let mut out = Vec::with_capacity(body.len() + 2);
                out.push(0xF0);
                out.extend_from_slice(body);
                out.push(0xF7);
                out
            }
            Message::QuarterFrame { piece, value } => vec![0xF1, (piece & 7) << 4 | value & 0x0F],
            Message::SongPosition(beats) => {
                vec![0xF2, (beats & 0x7F) as u8, (beats >> 7) as u8 & 0x7F]
            }
            Message::SongSelect(song) => vec![0xF3, song],
            Message::TuneRequest => vec![0xF6],
            Message::Undefined(status) | Message::RealTime(status) => vec![status],
            Message::Clock => vec![0xF8],
            Message::Start => vec![0xFA],
            Message::Continue => vec![0xFB],
            Message::Stop => vec![0xFC],
            Message::ActiveSensing => vec![0xFE],
            Message::Reset => vec![0xFF],
        }
    }

    /// The message as an event: its kind and its fields, channels from 1.
    pub(crate) fn describe(&self) -> (&'static str, Value) {
        let ch = |c: u8| c as u64 + 1;
        match self {
            Message::NoteOff {
                channel,
                note,
                velocity,
            } => (
                "note_off",
                json!({"channel": ch(*channel), "note": note, "velocity": velocity}),
            ),
            Message::NoteOn {
                channel,
                note,
                velocity,
            } => (
                "note_on",
                json!({"channel": ch(*channel), "note": note, "velocity": velocity}),
            ),
            Message::PolyPressure {
                channel,
                note,
                pressure,
            } => (
                "poly_pressure",
                json!({"channel": ch(*channel), "note": note, "pressure": pressure}),
            ),
            Message::ControlChange {
                channel,
                controller,
                value,
            } => (
                "control_change",
                json!({"channel": ch(*channel), "controller": controller, "value": value}),
            ),
            Message::ProgramChange { channel, program } => (
                "program_change",
                json!({"channel": ch(*channel), "program": program}),
            ),
            Message::ChannelPressure { channel, pressure } => (
                "channel_pressure",
                json!({"channel": ch(*channel), "pressure": pressure}),
            ),
            Message::PitchBend { channel, value } => (
                "pitch_bend",
                json!({"channel": ch(*channel), "value": *value as i64 - 8192}),
            ),
            Message::SysEx(_) => ("sysex", json!({"hex": to_hex(&self.encode())})),
            Message::QuarterFrame { piece, value } => {
                ("mtc_quarter_frame", json!({"piece": piece, "value": value}))
            }
            Message::SongPosition(beats) => ("song_position", json!({"beats": beats})),
            Message::SongSelect(song) => ("song_select", json!({"song": song})),
            Message::TuneRequest => ("tune_request", json!({})),
            Message::Undefined(status) | Message::RealTime(status) => {
                ("undefined", json!({"status": format!("{status:02x}")}))
            }
            Message::Clock => ("clock", json!({})),
            Message::Start => ("start", json!({})),
            Message::Continue => ("continue", json!({})),
            Message::Stop => ("stop", json!({})),
            Message::ActiveSensing => ("active_sensing", json!({})),
            Message::Reset => ("system_reset", json!({})),
        }
    }
}

fn real_time(b: u8) -> Message {
    match b {
        0xF8 => Message::Clock,
        0xFA => Message::Start,
        0xFB => Message::Continue,
        0xFC => Message::Stop,
        0xFE => Message::ActiveSensing,
        0xFF => Message::Reset,
        other => Message::RealTime(other),
    }
}

/// How many data bytes follow a channel or system common status.
fn data_len(status: u8) -> usize {
    match status {
        0x80..=0xBF | 0xE0..=0xEF | 0xF2 => 2,
        0xC0..=0xDF | 0xF1 | 0xF3 => 1,
        _ => 0,
    }
}

fn complete(status: u8, d: &[u8]) -> Message {
    let channel = status & 0x0F;
    match status & 0xF0 {
        0x80 => Message::NoteOff {
            channel,
            note: d[0],
            velocity: d[1],
        },
        0x90 => Message::NoteOn {
            channel,
            note: d[0],
            velocity: d[1],
        },
        0xA0 => Message::PolyPressure {
            channel,
            note: d[0],
            pressure: d[1],
        },
        0xB0 => Message::ControlChange {
            channel,
            controller: d[0],
            value: d[1],
        },
        0xC0 => Message::ProgramChange {
            channel,
            program: d[0],
        },
        0xD0 => Message::ChannelPressure {
            channel,
            pressure: d[0],
        },
        0xE0 => Message::PitchBend {
            channel,
            value: d[0] as u16 | (d[1] as u16) << 7,
        },
        _ => match status {
            0xF1 => Message::QuarterFrame {
                piece: d[0] >> 4 & 7,
                value: d[0] & 0x0F,
            },
            0xF2 => Message::SongPosition(d[0] as u16 | (d[1] as u16) << 7),
            0xF3 => Message::SongSelect(d[0]),
            _ => Message::TuneRequest,
        },
    }
}

/// Bytes to messages, keeping a partial message between calls.
#[derive(Debug, Default)]
pub(crate) struct Parser {
    /// The channel status data bytes are read against.
    running: Option<u8>,
    /// A system common status waiting for its data.
    common: Option<u8>,
    data: Vec<u8>,
    sysex: Option<Vec<u8>>,
    /// After F4 or F5: data bytes are skipped until the next status.
    skipping: bool,
}

impl Parser {
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<Message> {
        let mut out = Vec::new();
        for &b in bytes {
            if b >= 0xF8 {
                out.push(real_time(b));
                continue;
            }
            if b >= 0x80 {
                // Any other status ends a system exclusive message.
                if let Some(body) = self.sysex.take() {
                    out.push(Message::SysEx(body));
                    if b == 0xF7 {
                        continue;
                    }
                }
                self.data.clear();
                self.common = None;
                self.skipping = false;
                match b {
                    0x80..=0xEF => self.running = Some(b),
                    0xF0 => {
                        self.running = None;
                        self.sysex = Some(Vec::new());
                    }
                    0xF6 => {
                        self.running = None;
                        out.push(Message::TuneRequest);
                    }
                    0xF4 | 0xF5 => {
                        self.running = None;
                        self.skipping = true;
                        out.push(Message::Undefined(b));
                    }
                    // A lone F7 only cancels running status.
                    0xF7 => self.running = None,
                    _ => {
                        self.running = None;
                        self.common = Some(b);
                    }
                }
                continue;
            }
            if let Some(body) = self.sysex.as_mut() {
                body.push(b);
                continue;
            }
            if self.skipping {
                continue;
            }
            let Some(status) = self.common.or(self.running) else {
                continue;
            };
            self.data.push(b);
            if self.data.len() == data_len(status) {
                out.push(complete(status, &self.data));
                self.data.clear();
                // A system common message is not repeated by running status.
                self.common = None;
            }
        }
        out
    }
}

// --- Hex ---------------------------------------------------------------------

/// Bytes given as hex digit pairs, optionally separated by spaces, ':', ','
/// or '-', each group optionally prefixed 0x.
pub(crate) fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    let mut digits = String::new();
    for group in text.split([' ', ':', ',', '-']).filter(|g| !g.is_empty()) {
        let (group, prefixed) = match group
            .strip_prefix("0x")
            .or_else(|| group.strip_prefix("0X"))
        {
            Some(rest) => (rest, true),
            None => (group, false),
        };
        if group.is_empty() || !group.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("'{group}' is not hex"));
        }
        if group.len() % 2 == 1 {
            if !prefixed {
                return Err(format!("'{group}' has an odd number of hex digits"));
            }
            digits.push('0');
        }
        digits.push_str(group);
    }
    Ok((0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).expect("checked hex"))
        .collect())
}

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

// --- Time code ---------------------------------------------------------------

/// The four SMPTE frame rates MIDI Time Code, MIDI Machine Control and MIDI
/// Show Control code in two bits: 24, 25, 30 drop-frame and 30.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rate {
    Fps24,
    Fps25,
    Fps30Drop,
    Fps30,
}

impl Rate {
    pub(crate) const NAMES: [&'static str; 4] = ["24", "25", "30df", "30"];

    pub(crate) fn code(self) -> u8 {
        self as u8
    }

    pub(crate) fn from_code(code: u8) -> Rate {
        match code & 3 {
            0 => Rate::Fps24,
            1 => Rate::Fps25,
            2 => Rate::Fps30Drop,
            _ => Rate::Fps30,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        Rate::NAMES[self as usize]
    }

    pub(crate) fn from_name(name: &str) -> Option<Rate> {
        Rate::NAMES
            .iter()
            .position(|n| *n == name)
            .map(|i| Rate::from_code(i as u8))
    }

    pub(crate) fn fps(self) -> u32 {
        match self {
            Rate::Fps24 => 24,
            Rate::Fps25 => 25,
            Rate::Fps30Drop | Rate::Fps30 => 30,
        }
    }
}

/// A time code: hours, minutes, seconds and frames at a rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Timecode {
    pub(crate) hours: u8,
    pub(crate) minutes: u8,
    pub(crate) seconds: u8,
    pub(crate) frames: u8,
    pub(crate) rate: Rate,
}

const DAY_SECONDS: u32 = 24 * 3600;

impl Timecode {
    /// `None` when a field is out of range for the rate, or names a frame
    /// drop-frame time code skips.
    pub(crate) fn new(hours: u8, minutes: u8, seconds: u8, frames: u8, rate: Rate) -> Option<Self> {
        let valid = hours < 24
            && minutes < 60
            && seconds < 60
            && (frames as u32) < rate.fps()
            && !(rate == Rate::Fps30Drop
                && seconds == 0
                && frames < 2
                && !minutes.is_multiple_of(10));
        valid.then_some(Timecode {
            hours,
            minutes,
            seconds,
            frames,
            rate,
        })
    }

    /// Frames since 00:00:00:00, counting only frames the rate labels.
    fn to_frames(self) -> u32 {
        let fps = self.rate.fps();
        let seconds = self.hours as u32 * 3600 + self.minutes as u32 * 60 + self.seconds as u32;
        let mut n = seconds * fps + self.frames as u32;
        if self.rate == Rate::Fps30Drop {
            let minutes = self.hours as u32 * 60 + self.minutes as u32;
            n -= 2 * (minutes - minutes / 10);
        }
        n
    }

    fn from_frames(n: u32, rate: Rate) -> Timecode {
        let fps = rate.fps();
        let mut n = n;
        if rate == Rate::Fps30Drop {
            // 17982 labelled frames per ten minutes, 1798 per minute after
            // the first of each ten.
            let tens = n / 17982;
            let rest = n % 17982;
            let skipped = 18 * tens + if rest < 2 { 0 } else { 2 * ((rest - 2) / 1798) };
            n += skipped;
        }
        let seconds = (n / fps) % DAY_SECONDS;
        Timecode {
            hours: (seconds / 3600) as u8,
            minutes: (seconds / 60 % 60) as u8,
            seconds: (seconds % 60) as u8,
            frames: (n % fps) as u8,
            rate,
        }
    }

    /// The time `frames` labelled frames later (or earlier), across midnight.
    pub(crate) fn add_frames(self, frames: i64) -> Timecode {
        let day = Timecode {
            hours: 23,
            minutes: 59,
            seconds: 59,
            frames: (self.rate.fps() - 1) as u8,
            rate: self.rate,
        }
        .to_frames() as i64
            + 1;
        let n = (self.to_frames() as i64 + frames).rem_euclid(day);
        Timecode::from_frames(n as u32, self.rate)
    }

    /// `hh:mm:ss:ff`.
    pub(crate) fn text(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}:{:02}",
            self.hours, self.minutes, self.seconds, self.frames
        )
    }

    pub(crate) fn json(&self) -> Value {
        json!({
            "hours": self.hours,
            "minutes": self.minutes,
            "seconds": self.seconds,
            "frames": self.frames,
            "rate": self.rate.name(),
            "timecode": self.text(),
        })
    }

    /// The hours byte of MTC's full message and the Standard Time
    /// Specification: `0 tt hhhhh`.
    pub(crate) fn hours_byte(&self) -> u8 {
        self.rate.code() << 5 | self.hours
    }

    /// From the four bytes `hr mn sc fr`, ignoring the flag bits MMC and MSC
    /// add above each field.
    pub(crate) fn from_bytes(b: &[u8]) -> Option<Timecode> {
        let rate = Rate::from_code(b.first()? >> 5);
        Timecode::new(
            b[0] & 0x1F,
            b.get(1)? & 0x3F,
            b.get(2)? & 0x3F,
            b.get(3)? & 0x1F,
            rate,
        )
    }
}

/// MIDI Time Code from quarter frame messages: the time is complete after
/// eight in a row. Running forward (pieces 0 to 7) it is complete at piece 7,
/// two frames after the time the pieces carry, so two frames are added (MIDI
/// Time Code, "Quarter Frame Message Implementation"); running in reverse
/// (7 to 0) it is complete at piece 0, on the boundary of the frame it names.
#[derive(Debug, Default)]
pub(crate) struct MtcReader {
    nibbles: [u8; 8],
    /// The pieces received in this run of eight.
    seen: u8,
    last: Option<u8>,
}

impl MtcReader {
    /// The time, and whether it runs forward, when this piece completes it.
    pub(crate) fn quarter_frame(&mut self, piece: u8, value: u8) -> Option<(Timecode, bool)> {
        let piece = piece & 7;
        let forward = match self.last {
            Some(last) if piece == (last + 1) % 8 => Some(true),
            Some(last) if piece == (last + 7) % 8 => Some(false),
            _ => None,
        };
        self.last = Some(piece);
        self.nibbles[piece as usize] = value & 0x0F;
        let Some(forward) = forward else {
            // Out of sequence: a new run starts here.
            self.seen = 1 << piece;
            return None;
        };
        let start = if forward { 0 } else { 7 };
        if piece == start {
            self.seen = 1 << piece;
            return None;
        }
        self.seen |= 1 << piece;
        let end = if forward { 7 } else { 0 };
        if piece != end || self.seen != 0xFF {
            return None;
        }
        self.seen = 0;
        let n = &self.nibbles;
        let tc = Timecode::new(
            n[6] | (n[7] & 1) << 4,
            n[4] | (n[5] & 3) << 4,
            n[2] | (n[3] & 3) << 4,
            n[0] | (n[1] & 1) << 4,
            Rate::from_code(n[7] >> 1),
        )?;
        Some((if forward { tc.add_frames(2) } else { tc }, forward))
    }

    /// Forget a run in progress (a full message relocates the time).
    pub(crate) fn reset(&mut self) {
        self.seen = 0;
        self.last = None;
    }
}

/// The eight quarter frame messages for `tc`, piece 0 first.
#[cfg(any(test, feature = "generic-midi"))]
pub(crate) fn quarter_frames(tc: &Timecode) -> Vec<Message> {
    let values = [
        tc.frames & 0x0F,
        tc.frames >> 4,
        tc.seconds & 0x0F,
        tc.seconds >> 4,
        tc.minutes & 0x0F,
        tc.minutes >> 4,
        tc.hours & 0x0F,
        tc.hours >> 4 | tc.rate.code() << 1,
    ];
    values
        .iter()
        .enumerate()
        .map(|(piece, &value)| Message::QuarterFrame {
            piece: piece as u8,
            value,
        })
        .collect()
}

// --- Universal real-time system exclusive -------------------------------------

/// The universal real-time system exclusive header's id (F0 7F ...).
pub(crate) const REAL_TIME: u8 = 0x7F;
/// Sub-ID #1 of MIDI Time Code, MIDI Show Control, MMC commands and MMC
/// responses.
pub(crate) const SUB_MTC: u8 = 0x01;
pub(crate) const SUB_MSC: u8 = 0x02;
pub(crate) const SUB_MMC_COMMAND: u8 = 0x06;
pub(crate) const SUB_MMC_RESPONSE: u8 = 0x07;

/// A universal real-time system exclusive message: `F0 7F <device>
/// <sub-id> <data> F7`, as `(device, sub_id, data)`.
pub(crate) fn universal_real_time(body: &[u8]) -> Option<(u8, u8, &[u8])> {
    match body {
        [REAL_TIME, device, sub, data @ ..] => Some((*device, *sub, data)),
        _ => None,
    }
}

/// The MMC transport commands that carry no data (MMC 1.0, the command
/// index), by their command byte.
pub(crate) const MMC_COMMANDS: [(u8, &str); 13] = [
    (0x01, "stop"),
    (0x02, "play"),
    (0x03, "deferred_play"),
    (0x04, "fast_forward"),
    (0x05, "rewind"),
    (0x06, "record_strobe"),
    (0x07, "record_exit"),
    (0x08, "record_pause"),
    (0x09, "pause"),
    (0x0A, "eject"),
    (0x0B, "chase"),
    (0x0C, "command_error_reset"),
    (0x0D, "mmc_reset"),
];

/// The name of an MMC command byte, or its hex.
pub(crate) fn mmc_name(code: u8) -> String {
    MMC_COMMANDS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| (*n).to_string())
        .or_else(|| {
            [
                (0x40u8, "write"),
                (0x41, "masked_write"),
                (0x42, "read"),
                (0x43, "update"),
                (0x44, "locate"),
                (0x45, "variable_play"),
                (0x46, "search"),
                (0x47, "shuttle"),
                (0x48, "step"),
                (0x49, "assign_system_master"),
                (0x4A, "generator_command"),
                (0x4B, "mtc_command"),
                (0x4C, "move"),
                (0x4D, "add"),
                (0x4E, "subtract"),
                (0x4F, "drop_frame_adjust"),
                (0x50, "procedure"),
                (0x51, "event"),
                (0x52, "group"),
                (0x53, "command_segment"),
                (0x54, "deferred_variable_play"),
                (0x55, "record_strobe_variable"),
                (0x7C, "wait"),
                (0x7F, "resume"),
            ]
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, n)| (*n).to_string())
        })
        .unwrap_or_else(|| format!("{code:02x}"))
}

/// The commands (or response fields) of an MMC message's data: a byte from
/// 01 to 3F stands alone; one from 40 to 77 is followed by a byte count and
/// that many bytes (MMC 1.0, "Message Syntax"). 00 starts an extension
/// whose length cannot be known, so it ends the list, as does a count
/// running past the end.
pub(crate) fn mmc_items(mut data: &[u8]) -> Vec<(u8, &[u8])> {
    let mut out = Vec::new();
    while let Some((&code, rest)) = data.split_first() {
        match code {
            0x01..=0x3F | 0x78..=0x7F => {
                out.push((code, &rest[..0]));
                data = rest;
            }
            0x40..=0x77 => {
                let Some((&count, rest)) = rest.split_first() else {
                    break;
                };
                let count = count as usize;
                if rest.len() < count {
                    break;
                }
                out.push((code, &rest[..count]));
                data = &rest[count..];
            }
            _ => break,
        }
    }
    out
}

/// The fields of an MMC response's data: a name from 01 to 1F is followed
/// by a 5-byte time code, one from 20 to 3F by a 2-byte short time code
/// (frames and subframes of field `name - 20`), one from 40 to 77 by a byte
/// count and that many bytes, and 78 to 7F (handshakes) by nothing (MMC 1.0,
/// "Response Message Syntax"). What runs past the end, or an extension
/// (00), ends the list.
pub(crate) fn mmc_responses(mut data: &[u8]) -> Vec<(u8, &[u8])> {
    let mut out = Vec::new();
    while let Some((&code, rest)) = data.split_first() {
        let (len, skip) = match code {
            0x01..=0x1F => (5, 0),
            0x20..=0x3F => (2, 0),
            0x40..=0x77 => match rest.first() {
                Some(&count) => (count as usize, 1),
                None => break,
            },
            0x78..=0x7F => (0, 0),
            _ => break,
        };
        let Some(field) = rest.get(skip..skip + len) else {
            break;
        };
        out.push((code, field));
        data = &rest[skip + len..];
    }
    out
}

/// MMC's Standard Speed Specification: `sh sm sl`, a 17-bit number whose
/// binary point the shift count in `sh` moves, with a sign bit (MMC 1.0,
/// "Standard Speed"). Times play speed; negative is reverse.
pub(crate) fn mmc_speed(speed: f64) -> Option<[u8; 3]> {
    let reverse = speed < 0.0;
    let magnitude = speed.abs();
    for shift in 0u32..=7 {
        if magnitude >= f64::from(1u32 << (3 + shift)) {
            continue;
        }
        let n = (magnitude * f64::from(1u32 << (14 - shift))).round() as u32;
        if n >= 1 << 17 {
            continue;
        }
        let sh = (reverse as u8) << 6 | (shift as u8) << 3 | (n >> 14) as u8;
        return Some([sh, (n >> 7) as u8 & 0x7F, n as u8 & 0x7F]);
    }
    None
}

/// The speed a Standard Speed Specification codes.
pub(crate) fn mmc_speed_value(b: &[u8]) -> Option<f64> {
    let [sh, sm, sl] = <[u8; 3]>::try_from(b.get(..3)?).ok()?;
    let shift = (sh >> 3) & 7;
    let n = ((sh & 7) as u32) << 14 | (sm as u32 & 0x7F) << 7 | sl as u32 & 0x7F;
    let value = f64::from(n) / f64::from(1u32 << (14 - shift));
    Some(if sh & 0x40 != 0 { -value } else { value })
}

/// The Standard Time Specification with subframes (`hr mn sc fr ff`) that
/// MMC and MSC share: the rate in the hours byte, the colour frame, reserved
/// and sign bits clear, and `ff` hundredths of a frame.
pub(crate) fn standard_time(tc: &Timecode, subframes: u8) -> [u8; 5] {
    [
        tc.hours_byte(),
        tc.minutes,
        tc.seconds,
        tc.frames,
        subframes.min(99),
    ]
}

/// A Standard Time Specification read back: the time, its `ff` or `st`
/// byte, and the flags above each field.
pub(crate) fn read_standard_time(b: &[u8]) -> Option<Value> {
    let tc = Timecode::from_bytes(b.get(..4)?)?;
    let last = *b.get(4)?;
    let mut out = match tc.json() {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    out.insert("negative".into(), json!(b[3] & 0x40 != 0));
    out.insert("color_frame".into(), json!(b[1] & 0x40 != 0));
    if b[3] & 0x20 == 0 {
        out.insert("subframes".into(), json!(last));
    } else {
        out.insert(
            "status".into(),
            json!({
                "estimated": last & 0x40 != 0,
                "invalid": last & 0x20 != 0,
                "video_field_1": last & 0x10 != 0,
            }),
        );
    }
    Some(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_status_real_time_and_sysex_are_parsed() {
        let mut p = Parser::default();
        // Three note ons with running status, a clock inside the second.
        let got = p.feed(&[0x92, 60, 100, 61, 0xF8, 101, 62, 0]);
        assert_eq!(
            got,
            vec![
                Message::NoteOn {
                    channel: 2,
                    note: 60,
                    velocity: 100
                },
                Message::Clock,
                Message::NoteOn {
                    channel: 2,
                    note: 61,
                    velocity: 101
                },
                Message::NoteOn {
                    channel: 2,
                    note: 62,
                    velocity: 0
                },
            ]
        );
        // SysEx across calls, with a real-time byte inside it.
        assert!(p.feed(&[0xF0, 0x7E, 0x7F]).is_empty());
        assert_eq!(
            p.feed(&[0xFE, 0x06, 0x01, 0xF7]),
            vec![
                Message::ActiveSensing,
                Message::SysEx(vec![0x7E, 0x7F, 0x06, 0x01])
            ]
        );
        // SysEx cancels running status: a data byte after it is stray.
        assert!(p.feed(&[0x40]).is_empty());
        // A status ends a SysEx with no F7.
        assert_eq!(
            p.feed(&[0xF0, 0x01, 0xC3, 5]),
            vec![
                Message::SysEx(vec![0x01]),
                Message::ProgramChange {
                    channel: 3,
                    program: 5
                }
            ]
        );
        // System common: quarter frame, song position, tune request.
        assert_eq!(
            p.feed(&[0xF1, 0x23, 0xF2, 0x10, 0x01, 0xF6, 0xE0, 0x00, 0x40]),
            vec![
                Message::QuarterFrame { piece: 2, value: 3 },
                Message::SongPosition(0x90),
                Message::TuneRequest,
                Message::PitchBend {
                    channel: 0,
                    value: 8192
                },
            ]
        );
        // F4's data bytes are skipped.
        assert_eq!(
            p.feed(&[0xF4, 1, 2, 3, 0xFA]),
            vec![Message::Undefined(0xF4), Message::Start]
        );
    }

    #[test]
    fn messages_encode_whole() {
        for m in [
            Message::PitchBend {
                channel: 15,
                value: 16383,
            },
            Message::SongPosition(300),
            Message::QuarterFrame { piece: 7, value: 6 },
            Message::SysEx(vec![1, 2, 3]),
            Message::ControlChange {
                channel: 1,
                controller: 7,
                value: 100,
            },
        ] {
            let mut p = Parser::default();
            assert_eq!(p.feed(&m.encode()), vec![m]);
        }
        let mut p = Parser::default();
        assert_eq!(
            p.feed(&[0x90, 1, 2, 3])
                .iter()
                .map(Message::encode)
                .collect::<Vec<_>>(),
            vec![vec![0x90, 1, 2]],
            "a partial message waits"
        );
        assert_eq!(
            p.feed(&[4]).iter().map(Message::encode).collect::<Vec<_>>(),
            vec![vec![0x90, 3, 4]]
        );
    }

    #[test]
    fn hex_is_read_leniently_and_written_spaced() {
        assert_eq!(parse_hex("F0 7f,0x1:02-03").unwrap(), [0xF0, 0x7F, 1, 2, 3]);
        assert!(parse_hex("F").is_err());
        assert!(parse_hex("zz").is_err());
        assert_eq!(to_hex(&[0xF0, 1]), "f0 01");
    }

    #[test]
    fn drop_frame_time_code_skips_its_labels() {
        let tc = Timecode::new(0, 0, 59, 29, Rate::Fps30Drop).unwrap();
        assert_eq!(tc.add_frames(1).text(), "00:01:00:02");
        let tc = Timecode::new(0, 9, 59, 29, Rate::Fps30Drop).unwrap();
        assert_eq!(tc.add_frames(1).text(), "00:10:00:00");
        assert!(Timecode::new(0, 1, 0, 0, Rate::Fps30Drop).is_none());
        let tc = Timecode::new(23, 59, 59, 24, Rate::Fps25).unwrap();
        assert_eq!(tc.add_frames(1).text(), "00:00:00:00");
        assert_eq!(tc.add_frames(-25).text(), "23:59:58:24");
        let tc = Timecode::new(0, 0, 0, 0, Rate::Fps30Drop).unwrap();
        assert_eq!(tc.add_frames(-1).text(), "23:59:59:29");
    }

    #[test]
    fn quarter_frames_assemble_forward_and_in_reverse() {
        // MIDI Time Code's worked example: 01:37:52:16 at 30 fps.
        let tc = Timecode::new(1, 37, 52, 16, Rate::Fps30).unwrap();
        let pieces = quarter_frames(&tc);
        let bytes: Vec<u8> = pieces.iter().flat_map(Message::encode).collect();
        assert_eq!(
            bytes,
            [
                0xF1, 0x00, 0xF1, 0x11, 0xF1, 0x24, 0xF1, 0x33, 0xF1, 0x45, 0xF1, 0x52, 0xF1, 0x61,
                0xF1, 0x76
            ]
        );
        let mut r = MtcReader::default();
        let mut got = None;
        // Joining mid-run: the first full run of eight completes it.
        for m in pieces[5..].iter().chain(pieces.iter()) {
            if let Message::QuarterFrame { piece, value } = m {
                got = r.quarter_frame(*piece, *value).or(got);
            }
        }
        assert_eq!(got, Some((tc.add_frames(2), true)));
        let mut r = MtcReader::default();
        let mut got = None;
        for m in pieces.iter().rev() {
            if let Message::QuarterFrame { piece, value } = m {
                got = r.quarter_frame(*piece, *value).or(got);
            }
        }
        assert_eq!(got, Some((tc, false)));
    }

    #[test]
    fn mmc_speed_and_items_round_trip() {
        assert_eq!(mmc_speed(1.0), Some([0x01, 0x00, 0x00]));
        assert_eq!(mmc_speed(-0.5), Some([0x40, 0x40, 0x00]));
        assert_eq!(mmc_speed_value(&mmc_speed(10.0).unwrap()), Some(10.0));
        assert_eq!(mmc_speed_value(&mmc_speed(-100.25).unwrap()), Some(-100.25));
        assert_eq!(mmc_speed(2000.0), None);
        let data = [0x01, 0x44, 0x06, 0x01, 0x21, 0, 0, 0, 0, 0x02];
        assert_eq!(
            mmc_items(&data),
            vec![
                (0x01, &[][..]),
                (0x44, &[0x01, 0x21, 0, 0, 0, 0][..]),
                (0x02, &[][..])
            ]
        );
        assert_eq!(mmc_items(&[0x44, 0x06, 0x01]), vec![]);
        let response = [
            0x01, 0x61, 1, 2, 3, 0, 0x21, 4, 5, 0x48, 0x03, 2, 0x7F, 1, 0x7C,
        ];
        assert_eq!(
            mmc_responses(&response),
            vec![
                (0x01, &[0x61, 1, 2, 3, 0][..]),
                (0x21, &[4, 5][..]),
                (0x48, &[2, 0x7F, 1][..]),
                (0x7C, &[][..])
            ]
        );
    }
}
