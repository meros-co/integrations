//! Video disk servers over VDCP (the Video Disk Control Protocol, Louth's,
//! now Imagine Communications'): the `vdcp` integration.
//!
//! - A message is STX (02), a byte count (the bytes from CMD-1 to the last
//!   data byte), CMD-1 (a command type and a unit address), CMD-2, the data
//!   and a checksum: the two's complement of the low byte of the sum from
//!   CMD-1 on. Commands of types 0, 1, 2, 4 and 5 are answered ACK (04) or
//!   NAK (05 and an error byte); sense requests (type 3) are answered with
//!   a message whose CMD-2 has its top bit set. One message is in flight at a
//!   time.
//! - IDs are eight characters padded with spaces (types 0, 2, 3, 5), or with
//!   the `variable_ids` setting a length byte and the characters (types 8, A,
//!   B, D, the same commands).
//! - A communications port works on a signal port it opens (3X.01) and
//!   selects (2X.22): the `port` setting names it, and the core opens and
//!   selects it on connecting, and again after the server reports a reboot.
//! - The server pushes nothing: monitored, the port status (3X.05) is asked
//!   every 250 ms, the position (3X.06) while the port is active, the active
//!   ID every second and the system status every 10 s. When the port status
//!   says IDs were added or deleted, those lists are read and the ID list in
//!   state follows them; the whole list is read on connecting. A macro's
//!   completion return, which the server sends in place of a port status, is
//!   applied to state and acknowledged.
//!
//! Opened for commands only (`monitor` false), the port is still opened and
//! selected (commands need it); then only a port status request every 10 s
//! while idle is the liveness check.

use std::collections::VecDeque;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, SerialPort, TcpInput,
};

const SOCKET: Key = "vdcp";
const REPLY_TIMER: Key = "reply";
const FAST_TIMER: Key = "fast";
const SLOW_TIMER: Key = "slow";
const RETRY_TIMER: Key = "retry";

const REPLY_TIMEOUT: Millis = 1_000;
const FAST_EVERY: Millis = 250;
const SLOW_EVERY: Millis = 10_000;
const PROBE_EVERY: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

const STX: u8 = 0x02;
const ACK: u8 = 0x04;
const NAK: u8 = 0x05;

/// One message: CMD-1, CMD-2 and the data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Msg {
    pub cmd1: u8,
    pub cmd2: u8,
    pub data: Vec<u8>,
}

impl Msg {
    pub(crate) fn new(cmd1: u8, cmd2: u8, data: &[u8]) -> Msg {
        Msg {
            cmd1,
            cmd2,
            data: data.to_vec(),
        }
    }
}

/// The checksum: the two's complement of the low byte of the sum.
pub(crate) fn checksum(bytes: &[u8]) -> u8 {
    bytes
        .iter()
        .fold(0u8, |a, b| a.wrapping_add(*b))
        .wrapping_neg()
}

pub(crate) fn frame(msg: &Msg) -> Vec<u8> {
    let mut body = vec![msg.cmd1, msg.cmd2];
    body.extend_from_slice(&msg.data);
    let mut out = vec![STX, body.len() as u8];
    out.extend_from_slice(&body);
    out.push(checksum(&body));
    out
}

/// BCD time, frames first.
pub(crate) fn timecode(text: &str) -> Result<[u8; 4], CommandError> {
    let parts: Vec<&str> = text.split([':', ';', '.']).collect();
    let bad = || invalid(format!("'{text}' is not a timecode HH:MM:SS:FF"));
    if parts.len() != 4 {
        return Err(bad());
    }
    let mut v = [0u8; 4];
    for (i, (part, max)) in parts.iter().zip([99u8, 59, 59, 59]).enumerate() {
        let n: u8 = part.parse().map_err(|_| bad())?;
        if n > max || part.len() != 2 {
            return Err(bad());
        }
        v[i] = (n / 10) << 4 | (n % 10);
    }
    Ok([v[3], v[2], v[1], v[0]])
}

pub(crate) fn tc_text(d: &[u8]) -> String {
    let b = |i: usize, mask: u8| {
        let x = d.get(i).copied().unwrap_or(0) & mask;
        (x >> 4) * 10 + (x & 0x0F)
    };
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        b(3, 0xFF),
        b(2, 0x7F),
        b(1, 0x7F),
        b(0, 0x3F)
    )
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

fn int(p: &Params, key: &str) -> Result<i64, CommandError> {
    p.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn flag(p: &Params, key: &str) -> Result<bool, CommandError> {
    p.get(key)
        .and_then(Value::as_bool)
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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

/// The port status bits, by status byte and bit (3X.05's Status 1-3).
const STATUS1: &[(u8, &str)] = &[
    (0, "idle"),
    (1, "cue_init"),
    (2, "play_record"),
    (3, "still"),
    (4, "jog"),
    (5, "variable_play"),
    (6, "busy"),
    (7, "cue_init_done"),
];
const STATUS2: &[(u8, &str)] = &[
    (0, "port_down"),
    (1, "ids_added"),
    (2, "ids_deleted"),
    (3, "ids_added_to_archive"),
    (4, "no_reference"),
    (5, "no_video_input"),
    (6, "no_audio_input"),
    (7, "audio_overload"),
];
const ERRORS: &[(usize, u8, &str)] = &[
    (0, 0, "system_error"),
    (0, 1, "illegal_value"),
    (0, 2, "invalid_port"),
    (0, 3, "wrong_port_type"),
    (0, 4, "command_queue_full"),
    (0, 5, "disk_full"),
    (0, 6, "command_while_busy"),
    (0, 7, "not_supported"),
    (1, 0, "invalid_id"),
    (1, 1, "id_not_found"),
    (1, 2, "id_already_exists"),
    (1, 3, "id_still_recording"),
    (1, 4, "id_still_playing"),
    (1, 5, "id_not_transferred_from_archive"),
    (1, 6, "id_not_transferred_to_archive"),
    (1, 7, "id_delete_protected"),
    (2, 0, "not_in_cue_init"),
    (2, 1, "cue_not_done"),
    (2, 2, "port_not_idle"),
    (2, 3, "port_playing_active"),
    (2, 4, "port_not_active"),
    (2, 5, "cue_or_operation_failed"),
    (2, 6, "network_error"),
    (2, 7, "system_rebooted"),
];

/// What a reply is read as.
#[derive(Debug, Clone, PartialEq)]
enum Read {
    Ack,
    OpenPort,
    PortStatus,
    Position,
    ActiveId,
    DeviceType,
    SystemStatus,
    /// An ID list read whole (3X.11, then NEXT), and which list.
    Ids(&'static str, Vec<String>),
    IdSize,
    IdRequest,
    Compression,
    MultiPortStatus,
    AsiProgram,
    ActiveMacros,
    MacroStatus,
    Last,
    Raw,
}

#[derive(Debug, Clone)]
struct Job {
    command: Option<CommandId>,
    msg: Msg,
    read: Read,
    then: Vec<(Msg, Read)>,
}

impl Job {
    fn ack(msg: Msg) -> Job {
        Job::read(msg, Read::Ack)
    }

    fn read(msg: Msg, read: Read) -> Job {
        Job {
            command: None,
            msg,
            read,
            then: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Connecting,
    Ready,
}

pub(crate) struct Vdcp {
    link: Link,
    monitor: bool,
    port: i8,
    lock: bool,
    variable_ids: bool,
    phase: Phase,
    connected: bool,
    buf: Vec<u8>,
    commands: VecDeque<Job>,
    polls: VecDeque<Job>,
    in_flight: Option<Job>,
    sent_at: Millis,
    retry_after: Millis,
    /// Whether the port was active at the last port status.
    active: bool,
    fast_ticks: u32,
}

impl Vdcp {
    pub(crate) fn new(ctx: OpenContext) -> Result<Vdcp, String> {
        let link = match ctx.settings.get("link").and_then(Value::as_str) {
            None | Some("serial") => Link::Serial(SerialPort::from_settings(&ctx.settings)?),
            Some("tcp") => {
                let port = ctx
                    .port
                    .ok_or("the server's TCP port has no standard number: give it when opening")?;
                if ctx.host.is_unspecified() {
                    return Err("a TCP link needs the server's host".into());
                }
                Link::Tcp(std::net::SocketAddr::new(ctx.host, port))
            }
            Some(other) => return Err(format!("link '{other}' is not serial or tcp")),
        };
        let port = ctx
            .settings
            .get("port")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .clamp(-127, 127) as i8;
        Ok(Vdcp {
            link,
            monitor: ctx.monitor,
            port,
            lock: ctx
                .settings
                .get("lock_port")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            variable_ids: ctx
                .settings
                .get("variable_ids")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            phase: Phase::Closed,
            connected: false,
            buf: Vec::new(),
            commands: VecDeque::new(),
            polls: VecDeque::new(),
            in_flight: None,
            sent_at: 0,
            retry_after: RETRY_MIN,
            active: false,
            fast_ticks: 0,
        })
    }

    /// An ID as the protocol carries it, and the major command type to use:
    /// eight characters padded with spaces, or a length byte and the
    /// characters with variable IDs (the type's high bit set).
    fn id(&self, id: &str) -> Result<Vec<u8>, CommandError> {
        if !id.chars().all(|c| (' '..='~').contains(&c)) || id.is_empty() {
            return Err(invalid(format!("'{id}' is not a printable ASCII ID")));
        }
        if self.variable_ids {
            if id.len() > 255 {
                return Err(invalid("an ID is at most 255 characters"));
            }
            let mut v = vec![id.len() as u8];
            v.extend_from_slice(id.as_bytes());
            Ok(v)
        } else {
            if id.len() > 8 {
                return Err(invalid(format!(
                    "'{id}' is longer than 8 characters; turn on variable_ids for longer IDs"
                )));
            }
            let mut v = id.as_bytes().to_vec();
            v.resize(8, b' ');
            Ok(v)
        }
    }

    /// CMD-1 for an ID-carrying command of a type (0, 2, 3, 5): the
    /// variable-ID type (8, A, B, D) when IDs are variable.
    fn id_type(&self, kind: u8) -> u8 {
        if self.variable_ids {
            (kind | 0x8) << 4
        } else {
            kind << 4
        }
    }

    /// IDs read from a list return's data: ten fixed IDs, or length-prefixed.
    fn ids_of(&self, data: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        if self.variable_ids {
            let mut d = data;
            while let Some((&n, rest)) = d.split_first() {
                let n = n as usize;
                if rest.len() < n {
                    break;
                }
                out.push(String::from_utf8_lossy(&rest[..n]).into_owned());
                d = &rest[n..];
            }
        } else {
            for chunk in data.chunks(8) {
                let s = String::from_utf8_lossy(chunk).trim_end().to_string();
                if !s.is_empty() {
                    out.push(s);
                }
            }
        }
        out
    }

    fn job(&self, name: &str, p: &Params) -> Result<Job, CommandError> {
        let ack = |cmd1: u8, cmd2: u8, data: &[u8]| Ok(Job::ack(Msg::new(cmd1, cmd2, data)));
        let read =
            |cmd1: u8, cmd2: u8, data: &[u8], r: Read| Ok(Job::read(Msg::new(cmd1, cmd2, data), r));
        let id = |key: &str| -> Result<Vec<u8>, CommandError> { self.id(text(p, key)?) };
        let tc = |key: &str| -> Result<[u8; 4], CommandError> { timecode(text(p, key)?) };
        let u16b = |key: &str| -> Result<[u8; 2], CommandError> {
            Ok((int(p, key)? as u16).to_be_bytes())
        };
        let t0 = self.id_type(0);
        let t2 = self.id_type(2);
        let t3 = self.id_type(3);
        let t5 = self.id_type(5);
        let concat = |parts: &[&[u8]]| -> Vec<u8> { parts.concat() };
        match name {
            "local_disable" => ack(0x00, 0x0C, &[]),
            "local_enable" => ack(0x00, 0x0D, &[]),
            "delete_from_archive" => ack(t0, 0x14, &id("id")?),
            "protect_id" => ack(t0, 0x15, &id("id")?),
            "unprotect_id" => ack(t0, 0x16, &id("id")?),
            "stop" => ack(0x10, 0x00, &[]),
            "play" => ack(0x10, 0x01, &[]),
            "play_prepared" => ack(0x10, 0x01, &[int(p, "handle")? as u8]),
            "record" => ack(0x10, 0x02, &[]),
            "freeze" => ack(0x10, 0x03, &[]),
            "still" => ack(0x10, 0x04, &[]),
            "step" => ack(0x10, 0x05, &[]),
            "continue" => ack(0x10, 0x06, &[]),
            "jog" => {
                let frames = int(p, "frames")?;
                if (-128..=127).contains(&frames) {
                    ack(0x10, 0x07, &[frames as i8 as u8])
                } else {
                    ack(0x10, 0x07, &(frames as i32).to_be_bytes())
                }
            }
            "variable_play" => {
                let speed = p
                    .get("speed")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| invalid("'speed' is required"))?;
                let v = (speed * 65536.0).round().clamp(-8_388_608.0, 8_388_607.0) as i32;
                let b = v.to_be_bytes();
                ack(0x10, 0x08, &b[1..])
            }
            "unfreeze" => ack(0x10, 0x09, &[]),
            "ee_mode" => ack(
                0x10,
                0x0A,
                &[pick(p, "mode", &[("off", 0), ("on", 1), ("auto", 2)])?],
            ),
            "at" => {
                let code = pick(
                    p,
                    "command",
                    &[
                        ("stop", 0x00),
                        ("play", 0x01),
                        ("record", 0x02),
                        ("freeze", 0x03),
                        ("still", 0x04),
                        ("step", 0x05),
                        ("continue", 0x06),
                        ("unfreeze", 0x09),
                    ],
                )?;
                ack(0x40, code, &tc("timecode")?)
            }
            "rename_id" => ack(t2, 0x1D, &concat(&[&id("id")?, &id("new_id")?])),
            "preset_standard_time" => {
                let mode = pick(p, "mode", &[("drop_frame", 0), ("non_drop", 1), ("pal", 2)])?;
                ack(0x20, 0x1E, &concat(&[&[mode], &tc("timecode")?]))
            }
            "new_copy" => ack(
                t2,
                0x1F,
                &concat(&[&id("id")?, &id("new_id")?, &tc("offset")?, &tc("duration")?]),
            ),
            "sort_mode" => ack(
                0x20,
                0x20,
                &[pick(p, "order", &[("alphabetic", 0), ("fifo", 1)])?],
            ),
            "close_port" => ack(0x20, 0x21, &[int(p, "port")? as i8 as u8]),
            "select_port" => ack(0x20, 0x22, &[int(p, "port")? as i8 as u8]),
            "record_init" => ack(t2, 0x23, &concat(&[&id("id")?, &tc("duration")?])),
            "play_cue" => ack(t2, 0x24, &id("id")?),
            "play_cue_prepared" => {
                ack(t2, 0x24, &concat(&[&id("id")?, &[int(p, "handle")? as u8]]))
            }
            "cue_with_data" => ack(
                t2,
                0x25,
                &concat(&[&id("id")?, &tc("start")?, &tc("duration")?]),
            ),
            "delete_id" => ack(t2, 0x26, &id("id")?),
            "get_from_archive" => ack(t2, 0x27, &id("id")?),
            "clear" => ack(0x20, 0x29, &[]),
            "send_to_archive" => ack(t2, 0x2A, &id("id")?),
            "set_signal_full" => ack(0x20, 0x2B, &[int(p, "percent")? as u8]),
            "record_init_with_data" => ack(
                t2,
                0x2C,
                &concat(&[&id("id")?, &tc("start")?, &tc("duration")?]),
            ),
            "select_logical_drive" => {
                let drive = int(p, "drive")? as u8 & 0x1F;
                let only = u8::from(flag(p, "only")?) << 7;
                let descending = u8::from(flag(p, "descending")?) << 6;
                ack(0x20, 0x2D, &[drive | only | descending])
            }
            "system_delete_id" => ack(t2, 0x2E, &id("id")?),
            "preset" => ack(0x20, 0x30, &[]),
            "set_video_compression_rate" => {
                ack(0x20, 0x31, &(int(p, "rate")? as u32).to_be_bytes())
            }
            "set_audio_sample_rate" => ack(0x20, 0x32, &[int(p, "value")? as u8]),
            "set_audio_compression_rate" => ack(0x20, 0x33, &[int(p, "value")? as u8]),
            "set_audio_in_level" => ack(0x20, 0x34, &u16b("value")?),
            "set_audio_out_level" => ack(0x20, 0x35, &u16b("value")?),
            "set_video_compression_parameters" => {
                let b = unhex(text(p, "parameters")?)
                    .filter(|b| b.len() == 4)
                    .ok_or_else(|| invalid("'parameters' is four bytes as hex"))?;
                ack(0x20, 0x37, &b)
            }
            "select_output" | "select_input" => {
                let code = if name == "select_output" { 0x38 } else { 0x39 };
                let mode = pick(
                    p,
                    "mode",
                    &[
                        ("off", 0x01),
                        ("composite", 0x02),
                        ("s_video", 0x04),
                        ("yuv", 0x08),
                        ("d1", 0x10),
                    ],
                )?;
                ack(0x20, code, &[mode])
            }
            "set_record_mode" => {
                let mut bits: u16 = u16::from(flag(p, "video")?);
                if let Some(list) = p.get("audio").and_then(Value::as_array) {
                    for ch in list {
                        let ch = ch
                            .as_u64()
                            .filter(|c| (1..=15).contains(c))
                            .ok_or_else(|| invalid("'audio' is a list of channels 1-15"))?;
                        bits |= 1 << ch;
                    }
                }
                ack(0x20, 0x3A, &[bits as u8, (bits >> 8) as u8])
            }
            "set_subcarrier" => ack(0x20, 0x41, &u16b("value")?),
            "set_h_position" => ack(0x20, 0x42, &u16b("value")?),
            "set_disk_preroll" => ack(
                0x20,
                0x43,
                &[int(p, "frames")? as u8, int(p, "seconds")? as u8],
            ),
            "copy_file_to" | "abort_copy_file_to" => {
                let code = if name == "copy_file_to" { 0x50 } else { 0x52 };
                let mut data = id("id")?;
                data.push(int(p, "source")? as u8);
                for d in destinations(p)? {
                    data.push(d);
                }
                ack(t2, code, &data)
            }
            "delete_file_from" => {
                let mut data = id("id")?;
                for d in destinations(p)? {
                    data.push(d);
                }
                ack(t2, 0x51, &data)
            }
            "set_asi_program" => ack(0x20, 0x67, &u16b("program")?),
            "open_port" => {
                let port = int(p, "port")? as i8 as u8;
                read(
                    0x30,
                    0x01,
                    &[port, u8::from(flag(p, "lock")?)],
                    Read::OpenPort,
                )
            }
            "get_last" => read(0x30, 0x03, &[], Read::Last),
            "get_port_status" => read(0x30, 0x05, &[0x1F], Read::PortStatus),
            "get_position" => read(
                0x30,
                0x06,
                &[pick(
                    p,
                    "kind",
                    &[("remaining", 0), ("timecode", 1), ("offset", 2)],
                )?],
                Read::Position,
            ),
            "get_active_id" => read(t3, 0x07, &[], Read::ActiveId),
            "get_device_type" => read(0x30, 0x08, &[], Read::DeviceType),
            "get_system_status" => read(0x30, 0x10, &[0x1F], Read::SystemStatus),
            "list_ids" => read(t3, 0x11, &[], Read::Ids("ids", Vec::new())),
            "get_id_size" => read(t3, 0x14, &id("id")?, Read::IdSize),
            "list_ids_added_to_archive" => {
                read(t3, 0x15, &[], Read::Ids("archive_added", Vec::new()))
            }
            "get_id_status" => read(t3, 0x16, &id("id")?, Read::IdRequest),
            "get_compression_settings" => read(0x30, 0x17, &[0x3F], Read::Compression),
            "list_ids_added" => read(t3, 0x18, &[], Read::Ids("added", Vec::new())),
            "list_ids_deleted" => read(t3, 0x19, &[], Read::Ids("deleted", Vec::new())),
            "get_multi_port_status" => read(
                0x30,
                0x25,
                &[int(p, "start")? as i8 as u8, int(p, "count")? as u8],
                Read::MultiPortStatus,
            ),
            "get_asi_program" => read(0x30, 0x67, &[], Read::AsiProgram),
            "abort_macro" => ack(0x50, 0x60, &u16b("macro")?),
            "get_active_macros" => read(0x50, 0x61, &[], Read::ActiveMacros),
            "get_macro_status" => read(
                0x50,
                0x62,
                &concat(&[&[0x1F], &u16b("macro")?]),
                Read::MacroStatus,
            ),
            "copy_file_macro" => {
                let mut data = u16b("macro")?.to_vec();
                data.extend(id("id")?);
                data.push(int(p, "source")? as u8);
                data.extend(destinations(p)?);
                ack(t5, 0x63, &data)
            }
            "get_from_archive_macro" => ack(t5, 0x64, &concat(&[&u16b("macro")?, &id("id")?])),
            "send_to_archive_macro" => ack(t5, 0x65, &concat(&[&u16b("macro")?, &id("id")?])),
            "prepare_to_play" => ack(
                t5,
                0x66,
                &concat(&[&u16b("macro")?, &id("id")?, &[int(p, "handle")? as u8]]),
            ),
            "prepare_to_play_with_data" => ack(
                t5,
                0x66,
                &concat(&[
                    &u16b("macro")?,
                    &id("id")?,
                    &[int(p, "handle")? as u8],
                    &tc("start")?,
                    &tc("duration")?,
                ]),
            ),
            "close_from_play" => ack(t5, 0x67, &concat(&[&u16b("macro")?, &id("id")?])),
            "send_raw" => {
                let bytes = unhex(text(p, "message")?)
                    .filter(|b| b.len() >= 2)
                    .ok_or_else(|| {
                        invalid("'message' is CMD-1, CMD-2 and the data as hex bytes")
                    })?;
                Ok(Job::read(
                    Msg::new(bytes[0], bytes[1], &bytes[2..]),
                    Read::Raw,
                ))
            }
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
        cx.tcp_send(SOCKET, frame(&job.msg));
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

    /// Open and select the signal port, ahead of everything.
    fn take_port(&mut self) {
        if self.port == 0 {
            return;
        }
        let port = self.port as u8;
        let select = Job::ack(Msg::new(0x20, 0x22, &[port]));
        let open = Job::read(
            Msg::new(0x30, 0x01, &[port, u8::from(self.lock)]),
            Read::OpenPort,
        );
        self.commands.push_front(select);
        self.commands.push_front(open);
    }

    fn open(&mut self, cx: &mut Cx) {
        self.phase = Phase::Connecting;
        self.buf.clear();
        cx.connection(Connection::Connecting);
        self.link.open(cx, SOCKET);
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

    /// One whole reply at the front of the buffer: an ACK, a NAK and its
    /// byte, or a message; `None` until it has all arrived.
    fn take_reply(&mut self) -> Option<Result<Reply, String>> {
        // Anything before a reply's first byte is noise.
        let start = self
            .buf
            .iter()
            .position(|b| matches!(*b, ACK | NAK | STX))?;
        self.buf.drain(..start);
        match self.buf[0] {
            ACK => {
                self.buf.drain(..1);
                Some(Ok(Reply::Ack))
            }
            NAK => {
                let bits = *self.buf.get(1)?;
                self.buf.drain(..2);
                Some(Ok(Reply::Nak(bits)))
            }
            _ => {
                let bc = *self.buf.get(1)? as usize;
                let total = 2 + bc + 1;
                if self.buf.len() < total {
                    return None;
                }
                let bytes: Vec<u8> = self.buf.drain(..total).collect();
                let body = &bytes[2..2 + bc];
                if checksum(body) != bytes[total - 1] || bc < 2 {
                    return Some(Err(format!("a reply with a bad checksum: {}", hex(&bytes))));
                }
                Some(Ok(Reply::Msg(Msg::new(body[0], body[1], &body[2..]))))
            }
        }
    }

    fn on_data(&mut self, cx: &mut Cx, data: &[u8]) {
        self.buf.extend_from_slice(data);
        loop {
            if self.in_flight.is_none() {
                self.buf.clear();
                return;
            }
            let Some(reply) = self.take_reply() else {
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
            self.answer(cx, job, reply);
            self.pump(cx);
        }
    }

    fn answer(&mut self, cx: &mut Cx, mut job: Job, reply: Result<Reply, String>) {
        let reply = match reply {
            Ok(r) => r,
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
        let msg = match reply {
            Reply::Nak(bits) => {
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
                return self.finish(
                    cx,
                    job,
                    Err(CommandError::DeviceError {
                        code: Some(format!("{bits:02x}")),
                        message: format!(
                            "the server refused the command (NAK): {}",
                            found.join(", ")
                        ),
                    }),
                );
            }
            Reply::Ack => {
                if job.read == Read::Raw {
                    return self.finish(cx, job, Ok(Outcome::Value { value: json!("04") }));
                }
                let then = std::mem::take(&mut job.then);
                for (m, r) in then.into_iter().rev() {
                    self.commands.push_front(Job::read(m, r));
                }
                return self.finish(cx, job, Ok(Outcome::Ack));
            }
            Reply::Msg(m) => m,
        };
        // A macro's completion return comes in place of a port status, and
        // is acknowledged.
        if msg.cmd2 == 0xE2 && job.read != Read::MacroStatus {
            self.macro_status(cx, &msg.data);
            cx.tcp_send(SOCKET, vec![ACK]);
            return self.finish(cx, job, Ok(Outcome::Ack));
        }
        if job.read == Read::Raw {
            let mut bytes = vec![msg.cmd1, msg.cmd2];
            bytes.extend_from_slice(&msg.data);
            return self.finish(
                cx,
                job,
                Ok(Outcome::Value {
                    value: json!(hex(&bytes)),
                }),
            );
        }
        let d = &msg.data;
        let value = match (job.read.clone(), msg.cmd2) {
            (Read::OpenPort, 0x81) => {
                let granted = d.first() == Some(&1);
                cx.state(json!({"port": {"number": self.port, "granted": granted}}));
                if !granted {
                    cx.log(
                        Level::Warning,
                        format!("VDCP: signal port {} denied", self.port),
                    );
                }
                json!(granted)
            }
            (Read::PortStatus, 0x85) => self.port_status(cx, d),
            (Read::Position, 0x86) => {
                let kind = match d.first() {
                    Some(0) => "remaining",
                    Some(1) => "timecode",
                    _ => "offset",
                };
                let t = tc_text(d.get(1..).unwrap_or(&[]));
                let mut m = Map::new();
                m.insert(kind.into(), json!(t));
                cx.state(json!({"position": m}));
                json!(t)
            }
            (Read::ActiveId, 0x87) => {
                let active = d.first() == Some(&1);
                let id = if active {
                    self.ids_of(d.get(1..).unwrap_or(&[]))
                        .into_iter()
                        .next()
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                cx.state(json!({"active_id": id}));
                json!({"active": active, "id": id})
            }
            (Read::DeviceType, 0x88) => {
                let t = String::from_utf8_lossy(d)
                    .trim_end_matches(['\0', ' '])
                    .to_string();
                cx.state(json!({"device_type": t}));
                json!(t)
            }
            (Read::SystemStatus, 0x90) => self.system_status(cx, d),
            (Read::Ids(which, mut acc), 0x91 | 0x82 | 0x95 | 0x98 | 0x99) => {
                let remaining = u16::from_be_bytes([
                    d.first().copied().unwrap_or(0),
                    d.get(1).copied().unwrap_or(0),
                ]);
                acc.extend(self.ids_of(d.get(2..).unwrap_or(&[])));
                if remaining > 0 && acc.len() < 1_000_000 {
                    job.read = Read::Ids(which, acc);
                    job.msg = Msg::new(self.id_type(3), 0x02, &[]);
                    self.commands.push_front(job);
                    return;
                }
                self.ids_listed(cx, which, &acc);
                json!(acc)
            }
            (Read::IdSize, 0x94) => json!(tc_text(d)),
            (Read::IdRequest, 0x96) => {
                let b1 = d.first().copied().unwrap_or(0);
                let b2 = d.get(1).copied().unwrap_or(0);
                let bit = |b: u8, n: u8| b & (1 << n) != 0;
                json!({
                    "in_disk": bit(b1, 0), "in_archive_transfer_list": bit(b1, 1), "in_archive": bit(b1, 2),
                    "delete_protected": bit(b1, 3), "in_offline_storage": bit(b1, 4),
                    "in_offline_transfer_list": bit(b1, 5), "transfer_in_progress": bit(b1, 6),
                    "query_pending": bit(b1, 7), "not_ready_to_play": bit(b2, 0),
                    "not_ready_to_copy": bit(b2, 1), "not_ready_to_archive": bit(b2, 2),
                    "video_type": d.get(2).copied().unwrap_or(0),
                })
            }
            (Read::Compression, 0x97) => {
                let v = json!({"bitmap": d.first().copied().unwrap_or(0), "data": hex(d.get(1..).unwrap_or(&[]))});
                cx.state(json!({"compression": {"data": hex(d.get(1..).unwrap_or(&[]))}}));
                v
            }
            (Read::MultiPortStatus, 0xA5) => json!({"data": hex(d)}),
            (Read::AsiProgram, 0xE7) => {
                let n = u16::from_be_bytes([
                    d.first().copied().unwrap_or(0),
                    d.get(1).copied().unwrap_or(0),
                ]);
                cx.state(json!({"asi_program": n}));
                json!(n)
            }
            (Read::ActiveMacros, 0xE1) => {
                let list: Vec<u16> = d
                    .chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                json!(list)
            }
            (Read::MacroStatus, 0xE2) => self.macro_status(cx, d),
            (Read::Last, _) => json!({"data": hex(d)}),
            _ => {
                let mut bytes = vec![msg.cmd1, msg.cmd2];
                bytes.extend_from_slice(d);
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

    /// Port status 1-5 (85): the flags, the selected port, media, errors and
    /// settings; added or deleted IDs are read, and a reboot opens the port
    /// again.
    fn port_status(&mut self, cx: &mut Cx, d: &[u8]) -> Value {
        let bitmap = d.first().copied().unwrap_or(0);
        let mut at = 1;
        let mut v = Map::new();
        if bitmap & 0x01 != 0 {
            let s1 = d.get(at).copied().unwrap_or(0);
            for (bit, name) in STATUS1 {
                v.insert((*name).into(), json!(s1 & (1 << bit) != 0));
            }
            v.insert(
                "port_id".into(),
                json!(d.get(at + 1).copied().unwrap_or(0) as i8),
            );
            self.active = s1 & 0b1011_1110 != 0;
            v.insert(
                "state".into(),
                json!(if s1 & 0x04 != 0 {
                    "play_record"
                } else if s1 & 0x08 != 0 {
                    "still"
                } else if s1 & 0x10 != 0 {
                    "jog"
                } else if s1 & 0x80 != 0 {
                    "cued"
                } else if s1 & 0x02 != 0 {
                    "cueing"
                } else if s1 & 0x01 != 0 {
                    "idle"
                } else {
                    "unknown"
                }),
            );
            at += 2;
        }
        let mut added = false;
        let mut deleted = false;
        let mut rebooted = false;
        if bitmap & 0x02 != 0 {
            let s2 = d.get(at).copied().unwrap_or(0);
            for (bit, name) in STATUS2 {
                v.insert((*name).into(), json!(s2 & (1 << bit) != 0));
            }
            added = s2 & 0x02 != 0;
            deleted = s2 & 0x04 != 0;
            at += if bitmap & 0x40 != 0 { 2 } else { 1 };
        }
        if bitmap & 0x04 != 0 {
            let mut errors = Map::new();
            for (byte, bit, name) in ERRORS {
                let b = d.get(at + byte).copied().unwrap_or(0);
                errors.insert((*name).into(), json!(b & (1 << bit) != 0));
            }
            rebooted = d.get(at + 2).is_some_and(|b| b & 0x80 != 0);
            v.insert("errors".into(), Value::Object(errors));
            at += if bitmap & 0x20 != 0 { 4 } else { 3 };
        }
        if bitmap & 0x08 != 0 {
            let s4 = d.get(at).copied().unwrap_or(0);
            let format = ["off", "composite", "s_video", "yuv", "d1"]
                .iter()
                .enumerate()
                .filter(|(i, _)| s4 & (1 << i) != 0)
                .map(|(_, n)| *n)
                .collect::<Vec<_>>()
                .join(",");
            v.insert("format".into(), json!(format));
        }
        let value = Value::Object(v);
        cx.state(json!({"port_status": value.clone()}));
        if self.monitor {
            if added {
                self.queue_poll(
                    Msg::new(self.id_type(3), 0x18, &[]),
                    Read::Ids("added", Vec::new()),
                );
            }
            if deleted {
                self.queue_poll(
                    Msg::new(self.id_type(3), 0x19, &[]),
                    Read::Ids("deleted", Vec::new()),
                );
            }
        }
        if rebooted {
            self.take_port();
        }
        value
    }

    fn system_status(&mut self, cx: &mut Cx, d: &[u8]) -> Value {
        let bitmap = d.first().copied().unwrap_or(0);
        let extended = bitmap & 0x40 != 0;
        let mut at = 1;
        let mut v = Map::new();
        let tc_long = |b: &[u8]| -> String {
            let base = tc_text(b);
            if b.len() >= 6 {
                let bcd = |x: u8| (x >> 4) as u32 * 10 + (x & 0x0F) as u32;
                let hours = bcd(b[3]) + bcd(b[4]) * 100 + bcd(b[5]) * 10_000;
                format!("{hours:02}{}", &base[2..])
            } else {
                base
            }
        };
        if bitmap & 0x01 != 0 {
            let n = if extended { 6 } else { 4 };
            v.insert(
                "time_remaining".into(),
                json!(tc_long(d.get(at..at + n).unwrap_or(&[]))),
            );
            v.insert(
                "largest_block".into(),
                json!(tc_long(d.get(at + n..at + 2 * n).unwrap_or(&[]))),
            );
            at += 2 * n;
        }
        if bitmap & 0x02 != 0 {
            let n = if extended { 4 } else { 2 };
            let count = d
                .get(at..at + n)
                .unwrap_or(&[])
                .iter()
                .fold(0u64, |a, b| a << 8 | *b as u64);
            v.insert("ids_stored".into(), json!(count));
            at += n;
        }
        if bitmap & 0x04 != 0 {
            let b = d.get(at).copied().unwrap_or(0);
            v.insert("disk_full".into(), json!(b & 0x01 != 0));
            v.insert("system_down".into(), json!(b & 0x02 != 0));
            v.insert("disk_down".into(), json!(b & 0x04 != 0));
            v.insert("remote_control_disabled".into(), json!(b & 0x08 != 0));
            at += 1;
        }
        if bitmap & 0x08 != 0 {
            let b = d.get(at).copied().unwrap_or(0);
            for (bit, name) in [
                "local_offline_storage_available",
                "system_offline_storage_available",
                "local_offline_storage_full",
                "system_offline_storage_full",
                "archive_available",
                "archive_full",
            ]
            .iter()
            .enumerate()
            {
                v.insert((*name).into(), json!(b & (1 << bit) != 0));
            }
            at += 1;
        }
        if bitmap & 0x10 != 0 {
            v.insert(
                "standard_time".into(),
                json!(tc_text(d.get(at..at + 4).unwrap_or(&[]))),
            );
            at += 4;
        }
        if bitmap & 0x20 != 0 {
            v.insert(
                "signal_full_percent".into(),
                json!(d.get(at).copied().unwrap_or(0)),
            );
        }
        let value = Value::Object(v);
        cx.state(json!({"system": value.clone()}));
        value
    }

    fn macro_status(&mut self, cx: &mut Cx, d: &[u8]) -> Value {
        let bitmap = d.first().copied().unwrap_or(0);
        let mut at = 1;
        let mut v = Map::new();
        let mut number = None;
        if bitmap & 0x01 != 0 {
            let n = u16::from_be_bytes([
                d.get(at).copied().unwrap_or(0),
                d.get(at + 1).copied().unwrap_or(0),
            ]);
            number = Some(n);
            v.insert("macro".into(), json!(n));
            at += 2;
        }
        if bitmap & 0x02 != 0 {
            let s = d.get(at).copied().unwrap_or(0);
            let state = [
                (0, "unknown"),
                (1, "pending"),
                (2, "in_progress"),
                (3, "complete"),
                (4, "failed"),
                (5, "aborted"),
            ]
            .iter()
            .rev()
            .find(|(bit, _)| s & (1 << bit) != 0)
            .map(|(_, n)| *n)
            .unwrap_or("none");
            v.insert("state".into(), json!(state));
            at += 1;
        }
        if bitmap & 0x04 != 0 {
            v.insert(
                "completion".into(),
                json!(hex(d.get(at..at + 4).unwrap_or(&[]))),
            );
            at += 4;
        }
        if bitmap & 0x08 != 0 {
            v.insert(
                "command".into(),
                json!(hex(d.get(at..at + 2).unwrap_or(&[]))),
            );
            at += 2;
        }
        if bitmap & 0x10 != 0 {
            v.insert(
                "id".into(),
                json!(self
                    .ids_of(d.get(at..).unwrap_or(&[]))
                    .into_iter()
                    .next()
                    .unwrap_or_default()),
            );
        }
        let value = Value::Object(v);
        if let Some(n) = number {
            let mut m = Map::new();
            m.insert(n.to_string(), value.clone());
            cx.state(json!({"macros": m}));
        }
        value
    }

    /// A list read whole: the ID list replaces `ids`; added and deleted IDs
    /// change it.
    fn ids_listed(&mut self, cx: &mut Cx, which: &str, list: &[String]) {
        match which {
            "ids" => {
                cx.state(json!({"ids": Value::Null}));
                let m: Map<String, Value> = list.iter().map(|i| (i.clone(), json!(true))).collect();
                cx.state(json!({"ids": m}));
            }
            "added" | "archive_added" => {
                let m: Map<String, Value> = list.iter().map(|i| (i.clone(), json!(true))).collect();
                if which == "added" && !m.is_empty() {
                    cx.state(json!({"ids": m}));
                }
                let mut last = Map::new();
                last.insert(format!("last_{which}"), json!(list));
                cx.state(Value::Object(last));
            }
            "deleted" => {
                let m: Map<String, Value> = list.iter().map(|i| (i.clone(), Value::Null)).collect();
                if !m.is_empty() {
                    cx.state(json!({"ids": m}));
                }
                cx.state(json!({"last_deleted": list}));
            }
            _ => {}
        }
    }

    fn finish(&mut self, cx: &mut Cx, job: Job, result: Result<Outcome, CommandError>) {
        match (job.command, result) {
            (Some(id), r) => cx.complete(id, r),
            (None, Err(e)) => cx.log(Level::Debug, format!("VDCP: {e}")),
            _ => {}
        }
    }

    fn fast_poll(&mut self) {
        self.fast_ticks = self.fast_ticks.wrapping_add(1);
        self.queue_poll(Msg::new(0x30, 0x05, &[0x0F]), Read::PortStatus);
        if self.active {
            self.queue_poll(Msg::new(0x30, 0x06, &[1]), Read::Position);
            self.queue_poll(Msg::new(0x30, 0x06, &[0]), Read::Position);
        }
        if self.fast_ticks.is_multiple_of(4) {
            self.queue_poll(Msg::new(self.id_type(3), 0x07, &[]), Read::ActiveId);
        }
    }

    fn slow_poll(&mut self) {
        self.queue_poll(Msg::new(0x30, 0x10, &[0x3F]), Read::SystemStatus);
    }
}

/// A reply: ACK, NAK and its error byte, or a message.
#[derive(Debug, Clone, PartialEq)]
enum Reply {
    Ack,
    Nak(u8),
    Msg(Msg),
}

fn destinations(p: &Params) -> Result<Vec<u8>, CommandError> {
    let list = p
        .get("destinations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("'destinations' is a list of disk system numbers"))?;
    list.iter()
        .map(|d| d.as_u64().filter(|d| *d <= 255).map(|d| d as u8))
        .collect::<Option<Vec<u8>>>()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("'destinations' is a list of disk system numbers 0-255"))
}

impl Module for Vdcp {
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
                self.phase = Phase::Ready;
                self.take_port();
                self.queue_poll(Msg::new(0x30, 0x08, &[]), Read::DeviceType);
                if self.monitor {
                    self.queue_poll(
                        Msg::new(self.id_type(3), 0x11, &[]),
                        Read::Ids("ids", Vec::new()),
                    );
                    self.fast_poll();
                    self.slow_poll();
                    cx.set_timer(FAST_TIMER, FAST_EVERY);
                    cx.set_timer(SLOW_TIMER, SLOW_EVERY);
                } else {
                    cx.set_timer(FAST_TIMER, PROBE_EVERY);
                }
                self.pump(cx);
            }
            TcpInput::Data(data) => {
                if self.phase == Phase::Ready {
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
                        self.queue_poll(Msg::new(0x30, 0x05, &[0x0F]), Read::PortStatus);
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
                if let Some(job) = self.in_flight.take() {
                    self.buf.clear();
                    if let Some(id) = job.command {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    if self.connected {
                        self.connected = false;
                        cx.connection(Connection::Disconnected {
                            reason: "the server did not answer".into(),
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

    fn module(monitor: bool, settings: Value) -> Vdcp {
        let s = Catalog::source_tree().devices["vdcp"].clone();
        let settings = validate(&s.settings, settings.as_object().unwrap()).unwrap();
        Vdcp::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: Some(5000),
            model: "generic".into(),
            channels: None,
            settings,
            monitor,
        })
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

    fn feed(m: &mut Vdcp, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut Vdcp, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    /// Opened for commands only on output port 1: open, select, device type.
    fn idle() -> Vdcp {
        let mut m = module(false, json!({"link": "tcp", "port": 1}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&cx.take()), [frame(&Msg::new(0x30, 0x01, &[1, 0]))]);
        let a = feed(&mut m, 0, &frame(&Msg::new(0x30, 0x81, &[1])));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(sent(&a), [frame(&Msg::new(0x20, 0x22, &[1]))]);
        let a = feed(&mut m, 0, &[ACK]);
        assert_eq!(sent(&a), [frame(&Msg::new(0x30, 0x08, &[]))]);
        feed(&mut m, 0, &frame(&Msg::new(0x30, 0x88, b"MAKER:DDR")));
        m
    }

    #[test]
    fn frames_follow_the_documented_examples() {
        // Play: 02 02 10 01 EF.
        assert_eq!(
            frame(&Msg::new(0x10, 0x01, &[])),
            [0x02, 0x02, 0x10, 0x01, 0xEF]
        );
        assert_eq!(timecode("00:07:35:22").unwrap(), [0x22, 0x35, 0x07, 0x00]);
    }

    #[test]
    fn ids_cue_and_play() {
        let mut m = idle();
        let a = run(
            &mut m,
            1,
            "cue_with_data",
            json!({"id": "Fred1", "start": "00:07:35:22", "duration": "00:06:30:00"}),
        );
        let mut data = b"Fred1   ".to_vec();
        data.extend([0x22, 0x35, 0x07, 0x00, 0x00, 0x30, 0x06, 0x00]);
        assert_eq!(sent(&a), [frame(&Msg::new(0x20, 0x25, &data))]);
        let a = feed(&mut m, 0, &[ACK]);
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        run(&mut m, 2, "play", json!({}));
        let a = feed(&mut m, 0, &[NAK, 0x01]);
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn the_id_list_is_read_whole() {
        let mut m = idle();
        let a = run(&mut m, 1, "list_ids", json!({}));
        assert_eq!(sent(&a), [frame(&Msg::new(0x30, 0x11, &[]))]);
        let mut first = vec![0, 1];
        first.extend(b"SPOT1   SPOT2   ");
        let a = feed(&mut m, 0, &frame(&Msg::new(0x30, 0x91, &first)));
        assert_eq!(sent(&a), [frame(&Msg::new(0x30, 0x02, &[]))]);
        let mut next = vec![0, 0];
        next.extend(b"SPOT3   ");
        let a = feed(&mut m, 0, &frame(&Msg::new(0x30, 0x82, &next)));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!(["SPOT1", "SPOT2", "SPOT3"])
            }))
        );
        assert!(a.contains(&Action::State(
            json!({"ids": {"SPOT1": true, "SPOT2": true, "SPOT3": true}})
        )));
    }

    #[test]
    fn port_status_becomes_state_and_reads_added_ids() {
        let mut m = module(true, json!({"link": "tcp"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        m.polls.clear();
        m.commands.clear();
        m.in_flight = Some(Job::read(Msg::new(0x30, 0x05, &[0x0F]), Read::PortStatus));
        // Status 1: play/record on port 1; status 2: IDs added.
        let a = feed(
            &mut m,
            0,
            &frame(&Msg::new(
                0x30,
                0x85,
                &[0x0F, 0x04, 0x01, 0x02, 0, 0, 0, 0x10],
            )),
        );
        let state = a.iter().find_map(|a| match a {
            Action::State(v) => Some(v.clone()),
            _ => None,
        });
        let state = state.unwrap();
        assert_eq!(state["port_status"]["state"], "play_record");
        assert_eq!(state["port_status"]["ids_added"], true);
        assert_eq!(state["port_status"]["format"], "d1");
        assert_eq!(sent(&a), [frame(&Msg::new(0x30, 0x18, &[]))]);
    }

    #[test]
    fn opened_for_commands_only_it_polls_nothing_but_liveness() {
        let mut m = idle();
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, FAST_TIMER);
        assert_eq!(sent(&cx.take()), [frame(&Msg::new(0x30, 0x05, &[0x0F]))]);
    }

    #[test]
    fn the_time_to_each_answer_is_reported() {
        let mut m = idle();
        let mut cx = Cx::new(1_000);
        m.command(&mut cx, 1, "stop", json!({}).as_object().unwrap());
        let a = feed(&mut m, 1_007, &[ACK]);
        assert!(a.contains(&Action::RoundTrip(7)));
    }

    #[test]
    fn variable_ids_use_the_length_byte_forms() {
        let mut m = module(
            false,
            json!({"link": "tcp", "port": 0, "variable_ids": true}),
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        feed(&mut m, 0, &frame(&Msg::new(0x30, 0x88, b"X")));
        let a = run(&mut m, 1, "play_cue", json!({"id": "LongSpotName"}));
        let mut data = vec![12];
        data.extend(b"LongSpotName");
        assert_eq!(sent(&a), [frame(&Msg::new(0xA0, 0x24, &data))]);
    }
}
