//! MIDI Show Control: cue commands to lighting, sound, video, machinery and
//! other show equipment, over an RTP-MIDI session or a MIDI port of the host
//! (`super::midi_link`), and the MSC the equipment sends.
//!
//! From MIDI Show Control 1.1.1 (MMA RP-002 and RP-014, two-phase commit):
//!
//! - A message is a universal real-time SysEx: `F0 7F <device_ID> 02
//!   <command_format> <command> <data> F7`, at most 128 bytes, one command
//!   per message. The device ID is the destination: 00-6F a device, 70-7E a
//!   group, 7F all-call. The command format names the kind of equipment
//!   (01 lighting ... 64 smoke pots, 7F all types).
//! - Cue numbers, lists and paths are ASCII digits and '.', in the order
//!   number, list, path, each after a 00 delimiter: a list needs its number
//!   and a path its list, except where a command takes a list or path alone.
//! - Times are the Standard Time Specification with subframes (`hr mn sc fr
//!   ff`, the rate in the hours byte), shared with MIDI Machine Control.
//! - SET carries a 14-bit control number and a 14-bit value, each LSB first,
//!   and optionally a time; FIRE a macro number 0-127.
//! - Two-phase commit (20-26): the controller sends STANDBY, GO_2PC and
//!   CANCEL; the controlled device answers STANDING_BY (with the most time
//!   the cue can take) or ABORT within 2 s of a STANDBY, COMPLETE or ABORT
//!   within 125 % of that time of a GO_2PC, and CANCELLED or ABORT within
//!   2 s of a CANCEL. Each carries a checksum and a sequence number (1 to
//!   16383, chosen by the controller and returned by the device), both
//!   14-bit LSB first; STANDBY and GO_2PC carry four cue data values d1-d4.
//!   A response that does not arrive in time, or whose checksum is wrong,
//!   counts as an ABORT ("timeout", "checksum error").
//!
//! Every MSC message received (from a console sending show commands, or the
//! two-phase commit answers) is reported as a message event and kept in
//! state: the last message, the last cue commanded per cue list, cue clock
//! and open list and path status, generic control values, ALL_OFF, and each
//! two-phase commit transaction by sequence number. MSC without two-phase
//! commit carries no answer, so those commands are `unverified`; STANDBY and
//! CANCEL return the device's answer.
//!
//! Opening for commands only changes nothing: the module asks the device
//! for nothing.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use crate::{
    json,
    value::{Map, Value},
};

use super::midi_link::{timecode, MidiLink};
use crate::catalog::Params;
use crate::midi::{self, Message};
use crate::module::{
    CommandError, CommandId, Cx, Key, Millis, Module, OpenContext, Outcome, SettingsUpdate,
    TcpInput,
};

/// The command formats (MSC 1.1.1 §4.1), by code.
pub(crate) const FORMATS: [(u8, &str); 57] = [
    (0x01, "lighting"),
    (0x02, "moving_lights"),
    (0x03, "color_changers"),
    (0x04, "strobes"),
    (0x05, "lasers"),
    (0x06, "chasers"),
    (0x10, "sound"),
    (0x11, "music"),
    (0x12, "cd_players"),
    (0x13, "eprom_playback"),
    (0x14, "audio_tape_machines"),
    (0x15, "intercoms"),
    (0x16, "amplifiers"),
    (0x17, "audio_effects_devices"),
    (0x18, "equalizers"),
    (0x20, "machinery"),
    (0x21, "rigging"),
    (0x22, "flys"),
    (0x23, "lifts"),
    (0x24, "turntables"),
    (0x25, "trusses"),
    (0x26, "robots"),
    (0x27, "animation"),
    (0x28, "floats"),
    (0x29, "breakaways"),
    (0x2A, "barges"),
    (0x30, "video"),
    (0x31, "video_tape_machines"),
    (0x32, "video_cassette_machines"),
    (0x33, "video_disc_players"),
    (0x34, "video_switchers"),
    (0x35, "video_effects"),
    (0x36, "video_character_generators"),
    (0x37, "video_still_stores"),
    (0x38, "video_monitors"),
    (0x40, "projection"),
    (0x41, "film_projectors"),
    (0x42, "slide_projectors"),
    (0x43, "video_projectors"),
    (0x44, "dissolvers"),
    (0x45, "shutter_controls"),
    (0x50, "process_control"),
    (0x51, "hydraulic_oil"),
    (0x52, "h2o"),
    (0x53, "co2"),
    (0x54, "compressed_air"),
    (0x55, "natural_gas"),
    (0x56, "fog"),
    (0x57, "smoke"),
    (0x58, "cracked_haze"),
    (0x60, "pyro"),
    (0x61, "fireworks"),
    (0x62, "explosions"),
    (0x63, "flame"),
    (0x64, "smoke_pots"),
    (0x7F, "all_types"),
    (0x00, "extension"),
];

/// The commands (MSC 1.1.1 §4.3-4.5), by code.
const COMMANDS: [(u8, &str); 33] = [
    (0x01, "go"),
    (0x02, "stop"),
    (0x03, "resume"),
    (0x04, "timed_go"),
    (0x05, "load"),
    (0x06, "set"),
    (0x07, "fire"),
    (0x08, "all_off"),
    (0x09, "restore"),
    (0x0A, "reset"),
    (0x0B, "go_off"),
    (0x10, "go_jam_clock"),
    (0x11, "standby_plus"),
    (0x12, "standby_minus"),
    (0x13, "sequence_plus"),
    (0x14, "sequence_minus"),
    (0x15, "start_clock"),
    (0x16, "stop_clock"),
    (0x17, "zero_clock"),
    (0x18, "set_clock"),
    (0x19, "mtc_chase_on"),
    (0x1A, "mtc_chase_off"),
    (0x1B, "open_cue_list"),
    (0x1C, "close_cue_list"),
    (0x1D, "open_cue_path"),
    (0x1E, "close_cue_path"),
    (0x20, "standby"),
    (0x21, "standing_by"),
    (0x22, "go_2pc"),
    (0x23, "complete"),
    (0x24, "cancel"),
    (0x25, "cancelled"),
    (0x26, "abort"),
];

/// The status codes independent of the command format (MSC 1.1.1 §6.7):
/// CANCELLED's dispositions and ABORT's reasons. "timeout" is the
/// controller's own and never sent.
const STATUSES: [(u16, &str); 23] = [
    (0x0000, "unknown or undefined error"),
    (0x8000, "checksum error"),
    (0x8004, "completing"),
    (0x8008, "paused"),
    (0x800C, "terminated"),
    (0x8010, "reversed"),
    (0x8020, "timeout"),
    (0x8024, "not standing by"),
    (0x8028, "manual override initiated"),
    (0x8030, "manual override in progress"),
    (0x8040, "deadman interlock not established"),
    (0x8044, "required safety interlock not established"),
    (0x8050, "unknown cue number"),
    (0x8054, "unknown cue list"),
    (0x8058, "unknown cue path"),
    (0x805C, "too many cues active"),
    (0x8060, "cue out of sequence"),
    (0x8064, "invalid d1 cue data value"),
    (0x8068, "invalid d2 cue data value"),
    (0x806C, "invalid d3 cue data value"),
    (0x8070, "invalid d4 cue data value"),
    (0x8090, "manual cueing of playback medium required"),
    (0x80A0, "power failure in controlled device subsystem"),
];

/// The meaning of a status code: the format-independent ones by name, the
/// others by the range they fall in.
fn status_text(code: u16) -> String {
    if let Some((_, t)) = STATUSES.iter().find(|(c, _)| *c == code) {
        return (*t).to_string();
    }
    match code {
        0x80B0 => "reading new show cues from disk".into(),
        0x0004..=0x0FFC => format!("{code:04x}: defined by the command format and manufacturer"),
        0x1000..=0x7FFC => format!("{code:04x}: defined by the command format"),
        _ => format!("{code:04x}"),
    }
}

/// How long a controlled device has to answer STANDBY and CANCEL.
const ANSWER_MS: Millis = 2000;
const TRANSACTIONS: Key = "msc-2pc";

fn format_code(name: &str) -> Option<u8> {
    FORMATS.iter().find(|(_, n)| *n == name).map(|(c, _)| *c)
}

fn format_name(code: u8) -> String {
    FORMATS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| (*n).to_string())
        .unwrap_or_else(|| format!("{code:02x}"))
}

fn command_code(name: &str) -> Option<u8> {
    COMMANDS.iter().find(|(_, n)| *n == name).map(|(c, _)| *c)
}

fn command_name(code: u8) -> String {
    COMMANDS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| (*n).to_string())
        .unwrap_or_else(|| format!("{code:02x}"))
}

/// The two-phase commit checksum (MSC 1.1.1 §6.5): the command format,
/// command and data, with the checksum's own bytes zeroed and a zero added
/// to make the length even, summed as 16-bit values (each pair's first byte
/// the low one, as every MSC 2PC field is sent LSB first), plus the device
/// ID, masked with 7F7F; sent as its low byte, then its high byte.
pub(crate) fn checksum(device: u8, format_command_data: &[u8]) -> [u8; 2] {
    let mut sum: u16 = 0;
    for pair in format_command_data.chunks(2) {
        let lo = pair[0] as u16;
        let hi = pair.get(1).copied().unwrap_or(0) as u16;
        sum = sum.wrapping_add(lo | hi << 8);
    }
    sum = sum.wrapping_add(device as u16) & 0x7F7F;
    [(sum & 0x7F) as u8, (sum >> 8) as u8 & 0x7F]
}

fn seven_bit_pair(b: &[u8]) -> Option<u16> {
    Some((*b.first()? as u16 & 0x7F) | (*b.get(1)? as u16 & 0x7F) << 7)
}

/// A cue field from text: digits and decimal points.
fn cue_text(params: &Params, name: &str) -> Result<Option<Vec<u8>>, CommandError> {
    let Some(v) = params.get(name).and_then(Value::as_str) else {
        return Ok(None);
    };
    let v = v.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if !v.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(invalid(format!(
            "{name} '{v}' is not a cue number: digits and '.' only"
        )));
    }
    Ok(Some(v.as_bytes().to_vec()))
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn opt_int(params: &Params, name: &str) -> Option<i64> {
    params.get(name).and_then(Value::as_i64)
}

/// What a command's cue fields carry.
#[derive(Clone, Copy, PartialEq)]
enum Cue {
    /// `<Q_number> 00 <Q_list> 00 <Q_path>`, the number optional or required.
    Number {
        required: bool,
    },
    /// `<Q_list>` alone, optional or required.
    List {
        required: bool,
    },
    /// `<Q_path>` alone, required.
    Path,
    None,
}

fn cue_data(params: &Params, cue: Cue) -> Result<Vec<u8>, CommandError> {
    let mut out = Vec::new();
    match cue {
        Cue::Number { required } => {
            let number = cue_text(params, "cue")?;
            let list = cue_text(params, "list")?;
            let path = cue_text(params, "path")?;
            if required && number.is_none() {
                return Err(invalid("cue is required"));
            }
            if list.is_some() && number.is_none() {
                return Err(invalid("a cue list needs its cue number"));
            }
            if path.is_some() && list.is_none() {
                return Err(invalid("a cue path needs its cue list"));
            }
            if let Some(n) = number {
                out.extend(n);
            }
            if let Some(l) = list {
                out.push(0);
                out.extend(l);
            }
            if let Some(p) = path {
                out.push(0);
                out.extend(p);
            }
        }
        Cue::List { required } => match cue_text(params, "list")? {
            Some(l) => out.extend(l),
            None if required => return Err(invalid("list is required")),
            None => {}
        },
        Cue::Path => match cue_text(params, "path")? {
            Some(p) => out.extend(p),
            None => return Err(invalid("path is required")),
        },
        Cue::None => {}
    }
    Ok(out)
}

/// The cue fields of received data, split at the 00 delimiters.
fn cue_fields(data: &[u8]) -> Vec<String> {
    data.split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect()
}

fn insert_cue(args: &mut Map<String, Value>, fields: &[String], names: &[&str]) {
    for (name, value) in names.iter().zip(fields) {
        if !value.is_empty() {
            args.insert((*name).into(), json!(value));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Standby,
    Go,
    Cancel,
}

#[derive(Debug)]
struct Pending {
    phase: Phase,
    /// The command waiting for the answer (STANDBY, CANCEL).
    command: Option<CommandId>,
    deadline: Option<Millis>,
    device: u8,
    cue: Value,
}

pub(crate) struct MidiShowControl {
    link: MidiLink,
    device_id: u8,
    command_format: u8,
    sequence: u16,
    pending: BTreeMap<u16, Pending>,
    /// The longest time each standing-by cue may take, from STANDING_BY,
    /// keyed by device and cue.
    max_times: BTreeMap<(u8, String), Millis>,
}

impl MidiShowControl {
    pub(crate) fn new(ctx: OpenContext) -> Result<MidiShowControl, String> {
        let link = MidiLink::from_context(&ctx)?;
        let device_id = ctx
            .settings
            .get("device_id")
            .and_then(Value::as_u64)
            .unwrap_or(127)
            .min(127) as u8;
        let format = ctx
            .settings
            .get("command_format")
            .and_then(Value::as_str)
            .unwrap_or("all_types");
        let command_format =
            format_code(format).ok_or_else(|| format!("'{format}' is not a command format"))?;
        Ok(MidiShowControl {
            link,
            device_id,
            command_format,
            sequence: 0,
            pending: BTreeMap::new(),
            max_times: BTreeMap::new(),
        })
    }

    fn next_sequence(&mut self) -> u16 {
        self.sequence = self.sequence % 16383 + 1;
        self.sequence
    }

    /// The message: `7F <device> 02 <format> <command> <data>` between F0
    /// and F7, refused past MSC's 128 bytes.
    fn message(device: u8, format: u8, command: u8, data: &[u8]) -> Result<Message, CommandError> {
        let mut body = vec![midi::REAL_TIME, device, midi::SUB_MSC, format, command];
        body.extend_from_slice(data);
        if body.len() + 2 > 128 {
            return Err(invalid(format!(
                "the message would be {} bytes; MSC allows 128",
                body.len() + 2
            )));
        }
        Ok(Message::SysEx(body))
    }

    fn addressing(&self, params: &Params) -> Result<(u8, u8), CommandError> {
        let device = match opt_int(params, "device_id") {
            Some(d) if (0..=127).contains(&d) => d as u8,
            Some(d) => return Err(invalid(format!("device_id {d} is not 0 to 127"))),
            None => self.device_id,
        };
        let format = match params.get("command_format").and_then(Value::as_str) {
            Some(name) => format_code(name)
                .ok_or_else(|| invalid(format!("'{name}' is not a command format")))?,
            None => self.command_format,
        };
        Ok((device, format))
    }

    fn time(params: &Params) -> Result<[u8; 5], CommandError> {
        let tc = timecode(params)?;
        let sub = opt_int(params, "subframes").unwrap_or(0).clamp(0, 99) as u8;
        Ok(midi::standard_time(&tc, sub))
    }

    fn has_time(params: &Params) -> bool {
        ["hours", "minutes", "seconds", "frames"]
            .iter()
            .any(|k| params.contains_key(*k))
    }

    /// A two-phase commit message: checksum, sequence number, then `rest`.
    fn two_phase(
        &mut self,
        device: u8,
        format: u8,
        command: u8,
        rest: &[u8],
    ) -> Result<(Message, u16), CommandError> {
        let seq = self.next_sequence();
        let mut data = vec![0, 0, (seq & 0x7F) as u8, (seq >> 7) as u8 & 0x7F];
        data.extend_from_slice(rest);
        let mut summed = vec![format, command];
        summed.extend_from_slice(&data);
        let [lo, hi] = checksum(device, &summed);
        data[0] = lo;
        data[1] = hi;
        Ok((
            MidiShowControl::message(device, format, command, &data)?,
            seq,
        ))
    }

    fn cue_json(params: &Params) -> Value {
        let mut out = Map::new();
        for k in ["cue", "list", "path"] {
            if let Some(v) = params
                .get(k)
                .and_then(Value::as_str)
                .filter(|v| !v.trim().is_empty())
            {
                out.insert(k.into(), json!(v.trim()));
            }
        }
        Value::Object(out)
    }

    fn cue_key(cue: &Value) -> String {
        format!(
            "{}/{}/{}",
            cue["cue"].as_str().unwrap_or(""),
            cue["list"].as_str().unwrap_or(""),
            cue["path"].as_str().unwrap_or("")
        )
    }

    fn arm(&self, cx: &mut Cx) {
        match self.pending.values().filter_map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(TRANSACTIONS, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(TRANSACTIONS),
        }
    }

    fn command(
        &mut self,
        cx: &mut Cx,
        id: CommandId,
        name: &str,
        params: &Params,
    ) -> Result<Option<Outcome>, CommandError> {
        let (device, format) = self.addressing(params)?;
        let cue_kind = match name {
            "go" | "stop" | "resume" | "go_off" | "go_jam_clock" => Cue::Number { required: false },
            "load" => Cue::Number { required: true },
            "standby_plus" | "standby_minus" | "sequence_plus" | "sequence_minus"
            | "start_clock" | "stop_clock" | "zero_clock" | "mtc_chase_on" | "mtc_chase_off" => {
                Cue::List { required: false }
            }
            "open_cue_list" | "close_cue_list" => Cue::List { required: true },
            "open_cue_path" | "close_cue_path" => Cue::Path,
            _ => Cue::None,
        };
        let code = command_code(name);
        let data: Vec<u8> = match name {
            "timed_go" => {
                let mut d = MidiShowControl::time(params)?.to_vec();
                d.extend(cue_data(params, Cue::Number { required: false })?);
                d
            }
            "set_clock" => {
                let mut d = MidiShowControl::time(params)?.to_vec();
                d.extend(cue_data(params, Cue::List { required: false })?);
                d
            }
            "set" => {
                let control = opt_int(params, "control").unwrap_or(0).clamp(0, 16383) as u16;
                let value = opt_int(params, "value").unwrap_or(0).clamp(0, 16383) as u16;
                let mut d = vec![
                    (control & 0x7F) as u8,
                    (control >> 7) as u8,
                    (value & 0x7F) as u8,
                    (value >> 7) as u8,
                ];
                if MidiShowControl::has_time(params) {
                    d.extend(MidiShowControl::time(params)?);
                }
                d
            }
            "fire" => vec![opt_int(params, "macro").unwrap_or(0).clamp(0, 127) as u8],
            "standby" | "go_2pc" | "cancel" => {
                let cue = cue_data(params, Cue::Number { required: true })?;
                let mut rest = Vec::new();
                if name != "cancel" {
                    for d in ["d1", "d2", "d3", "d4"] {
                        rest.push(opt_int(params, d).unwrap_or(0).clamp(0, 127) as u8);
                    }
                }
                rest.extend(cue);
                let (message, seq) = self.two_phase(device, format, code.unwrap_or(0), &rest)?;
                self.link.send(cx, &[message])?;
                let cue = MidiShowControl::cue_json(params);
                let (phase, status, deadline, waits) = match name {
                    "standby" => (Phase::Standby, "standby", Some(cx.now() + ANSWER_MS), true),
                    "cancel" => (Phase::Cancel, "cancel", Some(cx.now() + ANSWER_MS), true),
                    _ => {
                        let max = self
                            .max_times
                            .get(&(device, MidiShowControl::cue_key(&cue)));
                        (Phase::Go, "go", max.map(|m| cx.now() + m + m / 4), false)
                    }
                };
                self.pending.insert(
                    seq,
                    Pending {
                        phase,
                        command: waits.then_some(id),
                        deadline,
                        device,
                        cue: cue.clone(),
                    },
                );
                let mut t = cue.as_object().cloned().unwrap_or_default();
                t.insert("status".into(), json!(status));
                t.insert("device_id".into(), json!(device));
                t.insert("command_format".into(), json!(format_name(format)));
                cx.state(json!({"transactions": {seq.to_string(): t}}));
                self.arm(cx);
                // STANDBY and CANCEL are answered when the device answers.
                return Ok(if waits {
                    None
                } else {
                    Some(Outcome::Value {
                        value: json!({"sequence": seq}),
                    })
                });
            }
            "raw" => {
                let command = opt_int(params, "command").unwrap_or(0).clamp(0, 127) as u8;
                let hex = params.get("hex").and_then(Value::as_str).unwrap_or("");
                let data = midi::parse_hex(hex).map_err(invalid)?;
                if let Some(b) = data.iter().find(|b| **b >= 0x80) {
                    return Err(invalid(format!("{b:02x} is not a data byte (00 to 7F)")));
                }
                let message = MidiShowControl::message(device, format, command, &data)?;
                self.link.send(cx, &[message])?;
                return Ok(Some(Outcome::Unverified));
            }
            _ => cue_data(params, cue_kind)?,
        };
        let code = code.ok_or_else(|| CommandError::UnknownCommand {
            command: name.into(),
        })?;
        let message = MidiShowControl::message(device, format, code, &data)?;
        self.link.send(cx, &[message])?;
        Ok(Some(Outcome::Unverified))
    }

    fn receive(&mut self, cx: &mut Cx, messages: Vec<Message>) {
        let source = self.link.source();
        for m in messages {
            if let Message::SysEx(body) = m {
                if let Some((device, midi::SUB_MSC, data)) = midi::universal_real_time(&body) {
                    self.msc(cx, device, data, source);
                }
            }
        }
    }

    /// One received MSC message.
    fn msc(&mut self, cx: &mut Cx, device: u8, data: &[u8], source: SocketAddr) {
        let (Some(&format), Some(&command)) = (data.first(), data.get(1)) else {
            return;
        };
        let rest = &data[2..];
        let name = command_name(command);
        let mut args = Map::new();
        args.insert("device_id".into(), json!(device));
        args.insert("command_format".into(), json!(format_name(format)));
        args.insert("command".into(), json!(name));
        let mut state = Map::new();
        let list_key = |fields: &[String], at: usize| -> String {
            fields
                .get(at)
                .filter(|l| !l.is_empty())
                .cloned()
                .unwrap_or_else(|| "default".into())
        };
        match command {
            0x01..=0x03 | 0x05 | 0x0B | 0x10 | 0x04 => {
                let (time, cues) = if command == 0x04 && rest.len() >= 5 {
                    (midi::read_standard_time(&rest[..5]), &rest[5..])
                } else {
                    (None, rest)
                };
                let fields = cue_fields(cues);
                insert_cue(&mut args, &fields, &["cue", "list", "path"]);
                if let Some(t) = time {
                    args.insert("time".into(), t);
                }
                // "": the message named no cue or path (a null would be a removal).
                let mut entry = Map::new();
                entry.insert("command".into(), json!(name));
                entry.insert(
                    "cue".into(),
                    fields
                        .first()
                        .filter(|c| !c.is_empty())
                        .map_or(json!(""), |c| json!(c)),
                );
                entry.insert(
                    "path".into(),
                    fields
                        .get(2)
                        .filter(|c| !c.is_empty())
                        .map_or(json!(""), |c| json!(c)),
                );
                state.insert("cue_lists".into(), json!({list_key(&fields, 1): entry}));
            }
            0x06 => {
                let control = seven_bit_pair(rest);
                let value = rest.get(2..).and_then(seven_bit_pair);
                if let (Some(c), Some(v)) = (control, value) {
                    args.insert("control".into(), json!(c));
                    args.insert("value".into(), json!(v));
                    state.insert("controls".into(), json!({c.to_string(): v}));
                }
                if let Some(t) = rest.get(4..9).and_then(midi::read_standard_time) {
                    args.insert("time".into(), t);
                }
            }
            0x07 => {
                if let Some(m) = rest.first() {
                    args.insert("macro".into(), json!(m));
                    state.insert("last_macro".into(), json!(m));
                }
            }
            0x08 => {
                state.insert("all_off".into(), json!(true));
            }
            0x09 | 0x0A => {
                state.insert("all_off".into(), json!(false));
            }
            0x11..=0x1A => {
                let (time, cues) = if command == 0x18 && rest.len() >= 5 {
                    (midi::read_standard_time(&rest[..5]), &rest[5..])
                } else {
                    (None, rest)
                };
                let fields = cue_fields(cues);
                insert_cue(&mut args, &fields, &["list"]);
                let key = fields
                    .first()
                    .filter(|l| !l.is_empty())
                    .cloned()
                    .unwrap_or_else(|| "all".into());
                let clock = match command {
                    0x15 => Some(json!({"running": true})),
                    0x16 => Some(json!({"running": false})),
                    0x17 => Some(json!({"time": "00:00:00:00"})),
                    0x18 => time.as_ref().map(|t| json!({"time": t["timecode"]})),
                    0x19 => Some(json!({"chasing": true})),
                    0x1A => Some(json!({"chasing": false})),
                    _ => None,
                };
                if let Some(t) = time {
                    args.insert("time".into(), t);
                }
                if let Some(clock) = clock {
                    state.insert("clocks".into(), json!({key: clock}));
                }
            }
            0x1B | 0x1C => {
                let fields = cue_fields(rest);
                insert_cue(&mut args, &fields, &["list"]);
                if let Some(l) = fields.first().filter(|l| !l.is_empty()) {
                    state.insert("open_lists".into(), json!({l: command == 0x1B}));
                }
            }
            0x1D | 0x1E => {
                let fields = cue_fields(rest);
                insert_cue(&mut args, &fields, &["path"]);
                if let Some(p) = fields.first().filter(|p| !p.is_empty()) {
                    state.insert("open_paths".into(), json!({p: command == 0x1D}));
                }
            }
            0x20..=0x26 => self.two_phase_answer(cx, device, command, data, &mut args),
            _ => {
                args.insert("hex".into(), json!(midi::to_hex(rest)));
            }
        }
        let args = Value::Object(args);
        state.insert("last".into(), args.clone());
        cx.state(Value::Object(state));
        cx.alive();
        cx.message(format!("msc/{name}"), None, args, source);
    }

    /// A two-phase commit message from a device: the answer to one of ours.
    fn two_phase_answer(
        &mut self,
        cx: &mut Cx,
        device: u8,
        command: u8,
        data: &[u8],
        args: &mut Map<String, Value>,
    ) {
        let rest = &data[2..];
        let Some(seq) = rest.get(2..).and_then(seven_bit_pair) else {
            return;
        };
        args.insert("sequence".into(), json!(seq));
        let mut zeroed = data.to_vec();
        zeroed[2] = 0;
        zeroed[3] = 0;
        let good = checksum(device, &zeroed) == [rest[0], rest[1] & 0x7F];
        // ABORT and CANCELLED carry a status before the sequence number.
        let (status, after) = match command {
            0x25 | 0x26 => {
                let status = rest.get(2..).and_then(|s| {
                    Some((*s.first()? as u16 & 0x7F) * 4 + (*s.get(1)? as u16 & 0x7F) * 512)
                });
                (status, 6)
            }
            _ => (None, 4),
        };
        let seq = if matches!(command, 0x25 | 0x26) {
            match rest.get(4..).and_then(seven_bit_pair) {
                Some(s) => s,
                None => return,
            }
        } else {
            seq
        };
        args.insert("sequence".into(), json!(seq));
        let tail = rest.get(after..).unwrap_or_default();
        let mut max_time = None;
        let cues = if command == 0x21 && tail.len() >= 5 {
            max_time = midi::read_standard_time(&tail[..5]);
            &tail[5..]
        } else {
            tail
        };
        insert_cue(args, &cue_fields(cues), &["cue", "list", "path"]);
        if let Some(t) = &max_time {
            args.insert("max_time".into(), t.clone());
        }
        if let Some(s) = status {
            args.insert("status_code".into(), json!(format!("{s:04x}")));
            args.insert("status".into(), json!(status_text(s)));
        }
        if !good {
            args.insert("checksum_error".into(), json!(true));
        }
        if !matches!(command, 0x21 | 0x23 | 0x25 | 0x26) {
            // Another controller's STANDBY, GO_2PC or CANCEL: reported only.
            return;
        }
        // Matched by sequence number alone: the device ID of an answer is
        // its destination, the controller's, not the device's own (§2.2).
        let Some(pending) = self.pending.get(&seq) else {
            return;
        };
        let mut t = Map::new();
        // A response with a bad checksum counts as an ABORT (§6.5).
        let (outcome, status_word): (Result<Value, CommandError>, &str) = if !good {
            (
                Err(CommandError::DeviceError {
                    code: Some("8000".into()),
                    message: "checksum error in the device's answer".into(),
                }),
                "abort",
            )
        } else {
            match command {
                0x21 => {
                    let mut v = json!({"sequence": seq});
                    if let Some(m) = &max_time {
                        v["max_time"] = m.clone();
                        if let Some(ms) = max_time_ms(m) {
                            let key = (pending.device, MidiShowControl::cue_key(&pending.cue));
                            self.max_times.insert(key, ms);
                        }
                    }
                    (Ok(v), "standing_by")
                }
                0x23 => (Ok(json!({"sequence": seq})), "complete"),
                0x25 => {
                    let s = status.unwrap_or(0);
                    (
                        Ok(
                            json!({"sequence": seq, "status_code": format!("{s:04x}"), "status": status_text(s)}),
                        ),
                        "cancelled",
                    )
                }
                _ => {
                    let s = status.unwrap_or(0);
                    (
                        Err(CommandError::DeviceError {
                            code: Some(format!("{s:04x}")),
                            message: status_text(s),
                        }),
                        "abort",
                    )
                }
            }
        };
        t.insert("status".into(), json!(status_word));
        if let Some(m) = max_time {
            t.insert("max_time".into(), m);
        }
        if let Some(s) = status {
            t.insert("status_code".into(), json!(format!("{s:04x}")));
            t.insert("status_text".into(), json!(status_text(s)));
        }
        let Some(pending) = self.pending.remove(&seq) else {
            return;
        };
        if let Some(id) = pending.command {
            cx.complete(id, outcome.map(|value| Outcome::Value { value }));
        }
        cx.state(json!({"transactions": {seq.to_string(): t}}));
        self.arm(cx);
    }

    /// Answers not given in time count as an ABORT with "timeout".
    fn deadlines(&mut self, cx: &mut Cx) {
        let now = cx.now();
        let late: Vec<u16> = self
            .pending
            .iter()
            .filter(|(_, p)| p.deadline.is_some_and(|d| d <= now))
            .map(|(s, _)| *s)
            .collect();
        for seq in late {
            if let Some(p) = self.pending.remove(&seq) {
                if let Some(id) = p.command {
                    cx.complete(id, Err(CommandError::Timeout));
                }
                let what = match p.phase {
                    Phase::Standby => "STANDING_BY",
                    Phase::Go => "COMPLETE",
                    Phase::Cancel => "CANCELLED",
                };
                cx.state(json!({"transactions": {seq.to_string(): {
                    "status": "timeout", "status_code": "8020", "status_text": "timeout",
                }}}));
                cx.message(
                    "msc/timeout",
                    None,
                    json!({"sequence": seq, "waited_for": what, "device_id": p.device}),
                    self.link.source(),
                );
            }
        }
        self.arm(cx);
    }
}

/// A Standard Time as milliseconds.
fn max_time_ms(t: &Value) -> Option<Millis> {
    let fps = match t["rate"].as_str()? {
        "24" => 24.0,
        "25" => 25.0,
        _ => 30.0,
    };
    let seconds =
        t["hours"].as_u64()? * 3600 + t["minutes"].as_u64()? * 60 + t["seconds"].as_u64()?;
    let frames = t["frames"].as_u64()? as f64 + t["subframes"].as_u64().unwrap_or(0) as f64 / 100.0;
    Some(seconds * 1000 + (frames * 1000.0 / fps) as u64)
}

impl Module for MidiShowControl {
    fn start(&mut self, cx: &mut Cx) {
        cx.state(json!({"link": self.link.kind()}));
        self.link.start(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match MidiShowControl::command(self, cx, id, name, params) {
            Ok(Some(outcome)) => cx.complete(id, Ok(outcome)),
            Ok(None) => {}
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        let messages = self.link.datagram(cx, socket, from, data);
        self.receive(cx, messages);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        self.link.socket_error(cx, socket, message);
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        let messages = self.link.tcp(cx, socket, input);
        self.receive(cx, messages);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == TRANSACTIONS {
            self.deadlines(cx);
            return;
        }
        self.link.timer(cx, key);
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.link.stop(cx);
    }

    fn update_settings(&mut self, _cx: &mut Cx, _settings: &Params) -> SettingsUpdate {
        SettingsUpdate::Restart
    }

    fn reconnect(&mut self, cx: &mut Cx) -> bool {
        self.link.reconnect(cx);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;
    use crate::module::{Action, CommandResult, Connection};

    fn module(settings: Value) -> MidiShowControl {
        let mut s = json!({"link": "port", "midi_input": "MSC", "midi_output": "MSC"});
        for (k, v) in settings.as_object().unwrap() {
            s[k] = v.clone();
        }
        let ctx = OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: None,
            model: "generic".into(),
            channels: None,
            settings: s.as_object().unwrap().clone(),
            monitor: true,
        };
        let mut m = MidiShowControl::new(ctx).unwrap();
        let mut cx = Cx::new(0);
        Module::start(&mut m, &mut cx);
        let mut cx = Cx::new(1);
        Module::tcp(&mut m, &mut cx, "midi-port", TcpInput::Connected);
        assert!(cx
            .take()
            .contains(&Action::Connection(Connection::Connected)));
        m
    }

    fn run(
        m: &mut MidiShowControl,
        at: Millis,
        name: &str,
        params: Value,
    ) -> (Vec<u8>, Option<CommandResult>) {
        let mut cx = Cx::new(at);
        Module::command(m, &mut cx, 7, name, params.as_object().unwrap());
        let mut sent = Vec::new();
        let mut result = None;
        for a in cx.take() {
            match a {
                Action::TcpSend { data, .. } => sent.extend(data),
                Action::Complete { result: r, .. } => result = Some(r),
                _ => {}
            }
        }
        (sent, result)
    }

    fn wire(m: &mut MidiShowControl, name: &str, params: Value) -> Vec<u8> {
        let (sent, result) = run(m, 10, name, params);
        assert_eq!(result, Some(Ok(Outcome::Unverified)), "{name}");
        sent
    }

    #[test]
    fn commands_follow_the_msc_format() {
        let mut m = module(json!({"device_id": 1, "command_format": "lighting"}));
        // MSC 1.1.1 §3.1's example: cue 235.6 list 36.6 path 59.
        assert_eq!(
            wire(
                &mut m,
                "go",
                json!({"cue": "235.6", "list": "36.6", "path": "59"})
            ),
            [
                0xF0, 0x7F, 0x01, 0x02, 0x01, 0x01, 0x32, 0x33, 0x35, 0x2E, 0x36, 0x00, 0x33, 0x36,
                0x2E, 0x36, 0x00, 0x35, 0x39, 0xF7
            ]
        );
        assert_eq!(
            wire(&mut m, "go", json!({})),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x01, 0xF7]
        );
        assert_eq!(
            wire(
                &mut m,
                "stop",
                json!({"device_id": 127, "command_format": "sound"})
            ),
            [0xF0, 0x7F, 0x7F, 0x02, 0x10, 0x02, 0xF7]
        );
        assert_eq!(
            wire(
                &mut m,
                "timed_go",
                json!({"hours": 0, "minutes": 0, "seconds": 5, "frames": 0, "rate": "30", "cue": "1"})
            ),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x04, 0x60, 0, 5, 0, 0, 0x31, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "set", json!({"control": 510, "value": 16383})),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x06, 0x7E, 0x03, 0x7F, 0x7F, 0xF7]
        );
        assert_eq!(
            wire(
                &mut m,
                "set",
                json!({"control": 1, "value": 2, "seconds": 3, "rate": "25"})
            ),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x06, 1, 0, 2, 0, 0x20, 0, 3, 0, 0, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "fire", json!({"macro": 12})),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x07, 12, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "all_off", json!({})),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x08, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "reset", json!({})),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x0A, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "standby_plus", json!({"list": "2"})),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x11, 0x32, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "set_clock", json!({"hours": 1, "rate": "24"})),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x18, 0x01, 0, 0, 0, 0, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "open_cue_path", json!({"path": "4"})),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x1D, 0x34, 0xF7]
        );
        assert_eq!(
            wire(&mut m, "raw", json!({"command": 0x55, "hex": "01 02"})),
            [0xF0, 0x7F, 0x01, 0x02, 0x01, 0x55, 1, 2, 0xF7]
        );
        // Refusals: a list without its cue, a missing required cue, letters,
        // and more than 128 bytes.
        for (name, p) in [
            ("go", json!({"list": "1"})),
            ("load", json!({})),
            ("go", json!({"cue": "1a"})),
            ("open_cue_list", json!({})),
            ("go", json!({"cue": "1".repeat(130)})),
        ] {
            let (sent, r) = run(&mut m, 10, name, p);
            assert!(sent.is_empty());
            assert!(
                matches!(r, Some(Err(CommandError::InvalidParams { .. }))),
                "{name}"
            );
        }
    }

    fn feed(m: &mut MidiShowControl, at: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(at);
        Module::tcp(m, &mut cx, "midi-port", TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    /// A device's two-phase commit answer, checksummed.
    fn answer(device: u8, format: u8, command: u8, rest: &[u8]) -> Vec<u8> {
        let mut data = vec![format, command, 0, 0];
        data.extend_from_slice(rest);
        let [lo, hi] = checksum(device, &data);
        data[2] = lo;
        data[3] = hi;
        let mut out = vec![0xF0, 0x7F, device, 0x02];
        out.extend(data);
        out.push(0xF7);
        out
    }

    #[test]
    fn two_phase_commit_runs_to_completion() {
        let mut m = module(json!({"device_id": 3, "command_format": "turntables"}));
        let (sent, r) = run(&mut m, 100, "standby", json!({"cue": "34", "d1": 1}));
        assert_eq!(r, None, "STANDBY waits for the answer");
        // F0 7F 03 02 24 20 cc cc 01 00 d1..d4 '3' '4' F7.
        assert_eq!(&sent[..6], &[0xF0, 0x7F, 0x03, 0x02, 0x24, 0x20]);
        assert_eq!(&sent[8..16], &[0x01, 0x00, 1, 0, 0, 0, 0x33, 0x34]);
        let mut summed = vec![0x24, 0x20, 0, 0];
        summed.extend_from_slice(&sent[8..16]);
        assert_eq!(&sent[6..8], &checksum(3, &summed));
        // STANDING_BY for sequence 1, at most 10 s.
        let a = feed(
            &mut m,
            200,
            &answer(3, 0x24, 0x21, &[1, 0, 0x60, 0, 10, 0, 0, 0x33, 0x34]),
        );
        let result = a.iter().find_map(|a| match a {
            Action::Complete { result, .. } => Some(result.clone()),
            _ => None,
        });
        let value = match result {
            Some(Ok(Outcome::Value { value })) => value,
            other => panic!("{other:?}"),
        };
        assert_eq!(value["sequence"], 1);
        assert_eq!(value["max_time"]["timecode"], "00:00:10:00");
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Message { address, .. } if address == "msc/standing_by")));
        // GO_2PC: returns its sequence at once and expects COMPLETE within
        // 12.5 s.
        let (_, r) = run(&mut m, 300, "go_2pc", json!({"cue": "34", "d1": 1}));
        assert_eq!(
            r,
            Some(Ok(Outcome::Value {
                value: json!({"sequence": 2})
            }))
        );
        let a = feed(&mut m, 5000, &answer(3, 0x24, 0x23, &[2, 0]));
        assert!(a.iter().any(
            |a| matches!(a, Action::State(s) if s["transactions"]["2"]["status"] == "complete")
        ));
        // A CANCEL the device answers with an ABORT (checksum error).
        let (_, r) = run(&mut m, 6000, "cancel", json!({"cue": "35"}));
        assert_eq!(r, None);
        let a = feed(&mut m, 6100, &answer(3, 0x24, 0x26, &[0x00, 0x40, 3, 0]));
        let result = a.iter().find_map(|a| match a {
            Action::Complete { result, .. } => Some(result.clone()),
            _ => None,
        });
        assert_eq!(
            result,
            Some(Err(CommandError::DeviceError {
                code: Some("8000".into()),
                message: "checksum error".into()
            }))
        );
    }

    #[test]
    fn unanswered_and_corrupt_answers_count_as_aborts() {
        let mut m = module(json!({}));
        let (_, r) = run(&mut m, 0, "standby", json!({"cue": "1"}));
        assert_eq!(r, None);
        let mut cx = Cx::new(2000);
        Module::timer(&mut m, &mut cx, TRANSACTIONS);
        let a = cx.take();
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Err(CommandError::Timeout)
        }));
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Message { address, .. } if address == "msc/timeout")));
        // A STANDING_BY whose checksum is wrong.
        let (_, _) = run(&mut m, 3000, "standby", json!({"cue": "2"}));
        let mut bad = answer(0x7F, 0x7F, 0x21, &[2, 0]);
        bad[6] ^= 0x01;
        let a = feed(&mut m, 3100, &bad);
        assert!(a.iter().any(|a| matches!(a, Action::Complete { result: Err(CommandError::DeviceError { code: Some(c), .. }), .. } if c == "8000")));
    }

    #[test]
    fn received_msc_is_kept_and_reported() {
        let mut m = module(json!({}));
        let a = feed(
            &mut m,
            10,
            &[
                0xF0, 0x7F, 0x01, 0x02, 0x01, 0x01, 0x31, 0x30, 0x00, 0x32, 0xF7,
            ],
        );
        let state = a.iter().find_map(|a| match a {
            Action::State(s) if s.get("cue_lists").is_some() => Some(s.clone()),
            _ => None,
        });
        let state = state.unwrap();
        assert_eq!(state["cue_lists"]["2"]["cue"], "10");
        assert_eq!(state["last"]["command"], "go");
        assert_eq!(state["last"]["command_format"], "lighting");
        let event = a.iter().find_map(|a| match a {
            Action::Message { address, args, .. } => Some((address.clone(), args.clone())),
            _ => None,
        });
        assert_eq!(
            event,
            Some((
                "msc/go".into(),
                json!({"device_id": 1, "command_format": "lighting", "command": "go", "cue": "10", "list": "2"})
            ))
        );
        let a = feed(
            &mut m,
            20,
            &[
                0xF0, 0x7F, 0x7F, 0x02, 0x10, 0x15, 0x33, 0xF7, 0xF0, 0x7F, 0x7F, 0x02, 0x7F, 0x08,
                0xF7,
            ],
        );
        let mut clocks = None;
        let mut all_off = None;
        for a in &a {
            if let Action::State(s) = a {
                if s.get("clocks").is_some() {
                    clocks = Some(s["clocks"].clone());
                }
                if s.get("all_off").is_some() {
                    all_off = Some(s["all_off"].clone());
                }
            }
        }
        assert_eq!(clocks, Some(json!({"3": {"running": true}})));
        assert_eq!(all_off, Some(json!(true)));
    }
}
