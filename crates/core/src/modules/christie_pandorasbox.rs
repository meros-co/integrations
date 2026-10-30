//! Christie Pandoras Box: Pandoras Box Automation over TCP 6211.
//!
//! Written from the protocol page and the command database of coolux's
//! Pandoras Box SDK generator (MIT); see the spec's sources. Every message is
//! `PBAU`, then a header of version 1, the domain (int), the length of what
//! follows (short), a connection ID (int, 0 over TCP), the protocol (byte, 0
//! over TCP) and a checksum (the sum of those header bytes modulo 255), then
//! the function's code (short) and its arguments. Integers are big-endian;
//! doubles are little-endian, as every vendor SDK writes and reads them;
//! narrow strings are a short count and ASCII, wide strings a short count and
//! UCS-2 big-endian, buffers an int length and their elements.
//!
//! The commands are the SDK's functions, from the generated table in
//! `pandorasbox_commands.rs`. A function that receives values is answered
//! with its code and the values, or with a negative code and an error number;
//! one at a time, so a reply is the answer to the request in flight. A
//! function that receives nothing is not answered, as the SDKs treat it: it is
//! sent in turn and reported unverified, and a command on a watched sequence
//! has that sequence read back.
//!
//! Nothing is pushed. The sequences of the `sequences` setting are read every
//! `poll_ms` (transport, time, and the cues around the playhead where the
//! master supports it), and the revision, sites and selection every 10 s.
//! Opened for commands only (`monitor` false), only the revision is read,
//! every 10 s, as the liveness check: the protocol gives no other sign the
//! master is there. Every answered request reports its round trip.

use std::collections::VecDeque;

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

use super::pandorasbox_commands::COMMANDS;

const SOCKET: Key = "pbau";
const REPLY_TIMER: Key = "reply";
const FAST_TIMER: Key = "poll-fast";
const SLOW_TIMER: Key = "poll-slow";
const RETRY_TIMER: Key = "retry";

pub(crate) const DEFAULT_PORT: u16 = 6211;

const REPLY_TIMEOUT: Millis = 3_000;
const MISSES: u32 = 3;
const SLOW_EVERY: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const HEADER: usize = 17;

/// A value type of the protocol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Ty {
    Byte,
    Short,
    Int,
    Double,
    Narrow,
    Wide,
    Bool,
    IntBuffer,
    ByteBuffer,
    /// An int carrying one of an enumeration's values.
    IntEnum(&'static [(&'static str, i32)]),
    /// A byte carrying one of an enumeration's values.
    ByteEnum(&'static [(&'static str, i32)]),
}

#[derive(Debug)]
pub(crate) struct Arg {
    pub name: &'static str,
    pub ty: Ty,
}

#[derive(Debug)]
pub(crate) struct Cmd {
    pub name: &'static str,
    pub code: i16,
    pub send: &'static [Arg],
    pub recv: &'static [Arg],
}

fn command(name: &str) -> Option<&'static Cmd> {
    COMMANDS.iter().find(|c| c.name == name)
}

fn command_of(code: i16) -> &'static Cmd {
    COMMANDS
        .iter()
        .find(|c| c.code == code)
        .expect("the table has the internal reads")
}

/// The header and body of one message.
pub(crate) fn frame(domain: i32, body: &[u8]) -> Vec<u8> {
    let mut header = vec![1u8];
    header.extend(domain.to_be_bytes());
    header.extend((body.len() as u16).to_be_bytes());
    header.extend(0i32.to_be_bytes());
    header.push(0);
    let checksum = (header.iter().map(|&b| u32::from(b)).sum::<u32>() % 255) as u8;
    let mut out = b"PBAU".to_vec();
    out.extend(header);
    out.push(checksum);
    out.extend_from_slice(body);
    out
}

fn bad(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

/// A function's code and arguments, from a command's parameters.
pub(crate) fn encode(cmd: &Cmd, params: &Params) -> Result<Vec<u8>, CommandError> {
    let mut out = cmd.code.to_be_bytes().to_vec();
    for arg in cmd.send {
        let v = params.get(arg.name);
        let int = |lo: i64, hi: i64| -> Result<i64, CommandError> {
            let n = v
                .and_then(Value::as_i64)
                .ok_or_else(|| bad(format!("'{}' must be an integer", arg.name)))?;
            if !(lo..=hi).contains(&n) {
                return Err(bad(format!("'{}' must be {lo} to {hi}", arg.name)));
            }
            Ok(n)
        };
        let text = || -> Result<&str, CommandError> {
            v.and_then(Value::as_str)
                .ok_or_else(|| bad(format!("'{}' must be text", arg.name)))
        };
        let enumerated = |table: &[(&str, i32)]| -> Result<i32, CommandError> {
            let name = text()?;
            table
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| *v)
                .ok_or_else(|| bad(format!("'{}' has no value '{name}'", arg.name)))
        };
        match arg.ty {
            Ty::Byte => out.push(int(0, 255)? as u8),
            Ty::Short => out.extend((int(i16::MIN.into(), i16::MAX.into())? as i16).to_be_bytes()),
            Ty::Int => out.extend((int(i32::MIN.into(), i32::MAX.into())? as i32).to_be_bytes()),
            Ty::Double => {
                let x = v
                    .and_then(Value::as_f64)
                    .ok_or_else(|| bad(format!("'{}' must be a number", arg.name)))?;
                out.extend(x.to_le_bytes());
            }
            Ty::Bool => {
                let b = match v {
                    None | Some(Value::Null) => false,
                    Some(v) => v
                        .as_bool()
                        .ok_or_else(|| bad(format!("'{}' must be true or false", arg.name)))?,
                };
                out.push(u8::from(b));
            }
            Ty::Narrow => {
                let s = text()?;
                if !s.is_ascii() || s.len() > i16::MAX as usize {
                    return Err(bad(format!(
                        "'{}' must be ASCII text of at most 32767 characters",
                        arg.name
                    )));
                }
                out.extend((s.len() as i16).to_be_bytes());
                out.extend(s.as_bytes());
            }
            Ty::Wide => {
                let units: Vec<u16> = text()?.encode_utf16().collect();
                if units.len() > i16::MAX as usize {
                    return Err(bad(format!("'{}' is too long", arg.name)));
                }
                out.extend((units.len() as i16).to_be_bytes());
                for u in units {
                    out.extend(u.to_be_bytes());
                }
            }
            Ty::ByteBuffer => {
                let hex = text()?;
                if hex.len() % 2 != 0 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(bad(format!(
                        "'{}' must be hexadecimal digits in pairs",
                        arg.name
                    )));
                }
                let bytes: Vec<u8> = (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect();
                out.extend((bytes.len() as i32).to_be_bytes());
                out.extend(bytes);
            }
            Ty::IntBuffer => {
                let items = v
                    .and_then(Value::as_array)
                    .ok_or_else(|| bad(format!("'{}' must be a list of integers", arg.name)))?;
                out.extend((items.len() as i32).to_be_bytes());
                for i in items {
                    let n = i
                        .as_i64()
                        .and_then(|n| i32::try_from(n).ok())
                        .ok_or_else(|| bad(format!("'{}' holds a value out of range", arg.name)))?;
                    out.extend(n.to_be_bytes());
                }
            }
            Ty::IntEnum(table) => out.extend(enumerated(table)?.to_be_bytes()),
            Ty::ByteEnum(table) => out.push(enumerated(table)? as u8),
        }
    }
    Ok(out)
}

/// The values a function receives, by argument name.
pub(crate) fn decode(recv: &[Arg], data: &[u8]) -> Option<Map<String, Value>> {
    let mut at = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let s = data.get(at..at + n)?;
        at += n;
        Some(s)
    };
    let mut out = Map::new();
    for arg in recv {
        let name_of = |table: &[(&str, i32)], v: i32| -> Value {
            table
                .iter()
                .find(|(_, x)| *x == v)
                .map_or(json!(v), |(n, _)| json!(n))
        };
        let value = match arg.ty {
            Ty::Byte => json!(take(1)?[0]),
            Ty::Short => json!(i16::from_be_bytes(take(2)?.try_into().ok()?)),
            Ty::Int => json!(i32::from_be_bytes(take(4)?.try_into().ok()?)),
            Ty::Double => json!(f64::from_le_bytes(take(8)?.try_into().ok()?)),
            Ty::Bool => json!(take(1)?[0] == 1),
            Ty::Narrow => {
                let n = u16::from_be_bytes(take(2)?.try_into().ok()?) as usize;
                json!(String::from_utf8_lossy(take(n)?))
            }
            Ty::Wide => {
                let n = u16::from_be_bytes(take(2)?.try_into().ok()?) as usize;
                let units: Vec<u16> = take(n * 2)?
                    .chunks(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                json!(String::from_utf16_lossy(&units))
            }
            Ty::ByteBuffer => {
                let n = usize::try_from(i32::from_be_bytes(take(4)?.try_into().ok()?)).ok()?;
                let hex: String = take(n)?.iter().map(|b| format!("{b:02x}")).collect();
                json!(hex)
            }
            Ty::IntBuffer => {
                let n = usize::try_from(i32::from_be_bytes(take(4)?.try_into().ok()?)).ok()?;
                let mut items = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    items.push(json!(i32::from_be_bytes(take(4)?.try_into().ok()?)));
                }
                Value::Array(items)
            }
            Ty::IntEnum(table) => name_of(table, i32::from_be_bytes(take(4)?.try_into().ok()?)),
            Ty::ByteEnum(table) => name_of(table, i32::from(take(1)?[0])),
        };
        out.insert(arg.name.to_string(), value);
    }
    Some(out)
}

/// A message read off the stream: the code and what follows it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reply {
    pub code: i16,
    pub data: Vec<u8>,
}

#[derive(Debug, Default)]
pub(crate) struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<Reply> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            match self.buf.windows(4).position(|w| w == b"PBAU") {
                Some(0) => {}
                Some(i) => {
                    self.buf.drain(..i);
                }
                None => {
                    // Keep a partial PBAU at the end.
                    let keep = self.buf.len().min(3);
                    self.buf.drain(..self.buf.len() - keep);
                    return out;
                }
            }
            if self.buf.len() < HEADER {
                return out;
            }
            let len = usize::from(u16::from_be_bytes([self.buf[9], self.buf[10]]));
            if self.buf.len() < HEADER + len {
                return out;
            }
            let body: Vec<u8> = self.buf[HEADER..HEADER + len].to_vec();
            self.buf.drain(..HEADER + len);
            if body.len() >= 2 {
                out.push(Reply {
                    code: i16::from_be_bytes([body[0], body[1]]),
                    data: body[2..].to_vec(),
                });
            }
        }
    }
}

/// What an internal read is for.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Read {
    Revision,
    SiteIds,
    SiteConnected(i32),
    SelectedCount,
    Transport(i32),
    Time(i32),
    CueInfo(i32),
}

impl Read {
    fn message(self) -> (i16, Vec<u8>) {
        let one = |code: i16, n: i32| {
            let mut b = code.to_be_bytes().to_vec();
            b.extend(n.to_be_bytes());
            (code, b)
        };
        match self {
            Read::Revision => (334, 334i16.to_be_bytes().to_vec()),
            Read::SiteIds => (317, 317i16.to_be_bytes().to_vec()),
            Read::SelectedCount => (81, 81i16.to_be_bytes().to_vec()),
            Read::SiteConnected(site) => one(109, site),
            Read::Transport(seq) => one(72, seq),
            Read::Time(seq) => one(73, seq),
            Read::CueInfo(seq) => one(295, seq),
        }
    }
}

#[derive(Debug)]
enum Job {
    Command {
        id: CommandId,
        cmd: &'static Cmd,
        body: Vec<u8>,
        /// A watched sequence the command acts on, read back afterwards.
        sequence: Option<i32>,
    },
    Read(Read),
}

fn timecode(m: &Map<String, Value>, h: &str, mi: &str, s: &str, f: &str) -> Option<String> {
    let n = |k: &str| m.get(k).and_then(Value::as_i64);
    Some(format!(
        "{:02}:{:02}:{:02}:{:02}",
        n(h)?,
        n(mi)?,
        n(s)?,
        n(f)?
    ))
}

pub(crate) struct PandorasBox {
    link: Link,
    monitor: bool,
    domain: i32,
    sequences: Vec<i32>,
    poll_ms: Millis,
    open: bool,
    closed: bool,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    /// Whether the master answers GetCurrentTimeCueInfo (newer versions).
    cue_info: bool,
    deframer: Deframer,
    queue: VecDeque<Job>,
    in_flight: Option<(Job, Millis)>,
}

impl PandorasBox {
    pub(crate) fn new(ctx: OpenContext) -> Result<PandorasBox, String> {
        let s = &ctx.settings;
        let link = Link::from_settings(&ctx, DEFAULT_PORT)?;
        let domain = s.get("domain").and_then(Value::as_i64).unwrap_or(0);
        let domain = i32::try_from(domain).map_err(|_| "domain is out of range".to_string())?;
        let sequences = s
            .get("sequences")
            .and_then(Value::as_str)
            .unwrap_or("1")
            .split(',')
            .filter(|p| !p.trim().is_empty())
            .map(|p| {
                p.trim()
                    .parse::<i32>()
                    .map_err(|_| format!("sequence '{p}' is not a number"))
            })
            .collect::<Result<Vec<i32>, String>>()?;
        let poll_ms = s
            .get("poll_ms")
            .and_then(Value::as_u64)
            .unwrap_or(500)
            .max(100);
        Ok(PandorasBox {
            link,
            monitor: ctx.monitor,
            domain,
            sequences,
            poll_ms,
            open: false,
            closed: false,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            cue_info: true,
            deframer: Deframer::default(),
            queue: VecDeque::new(),
            in_flight: None,
        })
    }

    fn read(&mut self, read: Read) {
        let queued = self
            .queue
            .iter()
            .any(|j| matches!(j, Job::Read(r) if *r == read))
            || matches!(&self.in_flight, Some((Job::Read(r), _)) if *r == read);
        if !queued {
            self.queue.push_back(Job::Read(read));
        }
    }

    fn sequence_reads(&mut self, seq: i32) {
        self.read(Read::Transport(seq));
        self.read(Read::Time(seq));
        if self.cue_info {
            self.read(Read::CueInfo(seq));
        }
    }

    fn fast_reads(&mut self) {
        for seq in self.sequences.clone() {
            self.sequence_reads(seq);
        }
    }

    fn slow_reads(&mut self) {
        self.read(Read::Revision);
        if self.monitor {
            self.read(Read::SiteIds);
            self.read(Read::SelectedCount);
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        while self.in_flight.is_none() && self.open {
            let Some(job) = self.queue.pop_front() else {
                return;
            };
            let body = match &job {
                Job::Command { body, .. } => body.clone(),
                Job::Read(r) => r.message().1,
            };
            cx.tcp_send(SOCKET, frame(self.domain, &body));
            match job {
                // Not answered: sent in turn, and done.
                Job::Command {
                    id, cmd, sequence, ..
                } if cmd.recv.is_empty() => {
                    cx.complete(id, Ok(Outcome::Unverified));
                    if let Some(seq) = sequence {
                        self.sequence_reads(seq);
                    }
                }
                job => {
                    cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
                    self.in_flight = Some((job, cx.now()));
                }
            }
        }
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.misses = 0;
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
    }

    fn received(&mut self, cx: &mut Cx, reply: Reply) {
        let Some((job, _)) = &self.in_flight else {
            if reply.code < 0 {
                cx.log(
                    Level::Warning,
                    format!("Pandoras Box refused a command (code {})", reply.code),
                );
            }
            return;
        };
        let code = match job {
            Job::Command { cmd, .. } => cmd.code,
            Job::Read(r) => r.message().0,
        };
        // A positive code that is not the one asked for answers something
        // else (a function the SDKs do not wait for, answered after all).
        if reply.code != code && reply.code >= 0 {
            return;
        }
        let (job, sent_at) = self.in_flight.take().unwrap();
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.heard(cx);
        let error = (reply.code < 0).then(|| {
            let number = reply
                .data
                .get(..4)
                .map(|b| i32::from_be_bytes([b[0], b[1], b[2], b[3]]));
            CommandError::DeviceError {
                code: Some(number.unwrap_or(i32::from(reply.code)).to_string()),
                message: format!(
                    "Pandoras Box refused it (code {}, error {})",
                    reply.code,
                    number.map_or("none".into(), |n| n.to_string())
                ),
            }
        });
        match job {
            Job::Command { id, cmd, .. } => {
                let result = match error {
                    Some(e) => Err(e),
                    None => match decode(cmd.recv, &reply.data) {
                        Some(values) => {
                            self.apply_command(cx, cmd, &values);
                            Ok(Outcome::Value {
                                value: Value::Object(values),
                            })
                        }
                        None => Err(CommandError::DeviceError {
                            code: None,
                            message: "the answer is shorter than the function's values".into(),
                        }),
                    },
                };
                cx.complete(id, result);
            }
            Job::Read(read) => match error {
                Some(_) if matches!(read, Read::CueInfo(_)) => {
                    // An older master without GetCurrentTimeCueInfo.
                    self.cue_info = false;
                }
                Some(e) => cx.log(Level::Debug, format!("{read:?}: {e}")),
                None => {
                    if let Some(values) = decode(command_of(read.message().0).recv, &reply.data) {
                        self.apply_read(cx, read, &values);
                    }
                }
            },
        }
        self.pump(cx);
    }

    /// A command's answer that is also state.
    fn apply_command(&mut self, cx: &mut Cx, cmd: &Cmd, values: &Map<String, Value>) {
        let read = match cmd.name {
            "get_host_revision_number" => Some(Read::Revision),
            "get_site_ids" => Some(Read::SiteIds),
            "get_selected_device_count" => Some(Read::SelectedCount),
            _ => None,
        };
        if let Some(read) = read {
            self.apply_read(cx, read, values);
        }
    }

    fn apply_read(&mut self, cx: &mut Cx, read: Read, v: &Map<String, Value>) {
        let patch = match read {
            Read::Revision => json!({"device": {"revision": v.get("revision")}}),
            Read::SelectedCount => {
                json!({"device": {"selected_devices": v.get("selected_devices_count")}})
            }
            Read::SiteIds => {
                if self.monitor {
                    let sites: Vec<i32> = v
                        .get("site_ids")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(|s| s.as_i64())
                                .map(|s| s as i32)
                                .collect()
                        })
                        .unwrap_or_default();
                    for site in sites {
                        self.read(Read::SiteConnected(site));
                    }
                }
                return;
            }
            Read::SiteConnected(site) => {
                json!({"sites": {site.to_string(): {"connected": v.get("is_connected")}}})
            }
            Read::Transport(seq) => {
                json!({"sequences": {seq.to_string(): {"transport": v.get("transport_mode")}}})
            }
            Read::Time(seq) => match timecode(v, "hours", "minutes", "seconds", "frames") {
                Some(tc) => json!({"sequences": {seq.to_string(): {"timecode": tc}}}),
                None => return,
            },
            Read::CueInfo(seq) => {
                let cue = |id: &str, name: &str, h: &str, m: &str, s: &str, f: &str, mode: &str| {
                    json!({"id": v.get(id), "name": v.get(name),
                           "timecode": timecode(v, h, m, s, f), "mode": v.get(mode)})
                };
                json!({"sequences": {seq.to_string(): {
                    "timecode": timecode(v, "hours", "minutes", "seconds", "frames"),
                    "previous_cue": cue("previous_cue_id", "previous_cue_name", "hours_previous_cue",
                                        "minutes_previous_cue", "seconds_previous_cue", "frames_previous_cue",
                                        "previous_cue_mode"),
                    "next_cue": cue("next_cue_id", "next_cue_name", "hours_next_cue", "minutes_next_cue",
                                    "seconds_next_cue", "frames_next_cue", "next_cue_mode"),
                }}})
            }
        };
        cx.state(strip_nulls(patch));
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.open = false;
        self.closed = true;
        self.reachable = false;
        self.deframer = Deframer::default();
        let jobs = self
            .in_flight
            .take()
            .map(|(j, _)| j)
            .into_iter()
            .chain(self.queue.drain(..));
        for job in jobs {
            if let Job::Command { id, .. } = job {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        for key in [REPLY_TIMER, FAST_TIMER, SLOW_TIMER] {
            cx.cancel_timer(key);
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn open_link(&mut self, cx: &mut Cx) {
        self.closed = false;
        cx.connection(Connection::Connecting);
        self.link.open(cx, SOCKET);
    }
}

/// A patch never carries null: a value the answer did not give is left out.
fn strip_nulls(v: Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.into_iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k, strip_nulls(v)))
                .collect(),
        ),
        other => other,
    }
}

impl Module for PandorasBox {
    fn start(&mut self, cx: &mut Cx) {
        self.open_link(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let Some(cmd) = command(name) else {
            cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            );
            return;
        };
        match encode(cmd, params) {
            Ok(body) => {
                let sequence = params
                    .get("sequence_id")
                    .and_then(Value::as_i64)
                    .map(|s| s as i32)
                    .filter(|s| self.monitor && self.sequences.contains(s))
                    .filter(|_| cmd.recv.is_empty());
                let job = Job::Command {
                    id,
                    cmd,
                    body,
                    sequence,
                };
                // Commands go ahead of background reads.
                match self.queue.iter().position(|j| matches!(j, Job::Read(_))) {
                    Some(at) => self.queue.insert(at, job),
                    None => self.queue.push_back(job),
                }
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.open = true;
                self.retry_after = RETRY_MIN;
                self.deframer = Deframer::default();
                self.slow_reads();
                if self.monitor {
                    self.fast_reads();
                    cx.set_timer(FAST_TIMER, self.poll_ms);
                }
                cx.set_timer(SLOW_TIMER, SLOW_EVERY);
                self.pump(cx);
            }
            TcpInput::Data(bytes) => {
                for reply in self.deframer.feed(&bytes) {
                    self.received(cx, reply);
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => self.open_link(cx),
            REPLY_TIMER => {
                if let Some((job, _)) = self.in_flight.take() {
                    match job {
                        Job::Command { id, .. } => cx.complete(id, Err(CommandError::Timeout)),
                        Job::Read(Read::CueInfo(_)) => self.cue_info = false,
                        Job::Read(_) => {}
                    }
                    self.misses += 1;
                    if self.misses >= MISSES && self.reachable {
                        self.reachable = false;
                        cx.log(
                            Level::Warning,
                            "Pandoras Box answered none of three requests",
                        );
                        cx.connection(Connection::Disconnected {
                            reason: "no reply from Pandoras Box".into(),
                        });
                    }
                }
                self.pump(cx);
            }
            FAST_TIMER if self.open && self.monitor => {
                self.fast_reads();
                self.pump(cx);
                cx.set_timer(FAST_TIMER, self.poll_ms);
            }
            SLOW_TIMER if self.open => {
                self.slow_reads();
                self.pump(cx);
                cx.set_timer(SLOW_TIMER, SLOW_EVERY);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::super::pandorasbox_commands::TRANSPORT_MODE;
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn module(monitor: bool, settings: Value) -> PandorasBox {
        let spec = Catalog::source_tree().devices["christie-pandorasbox"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        PandorasBox::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7)),
            host_name: None,
            port: None,
            model: "pandoras-box".into(),
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

    fn codes(actions: &[Action]) -> Vec<i16> {
        sent(actions)
            .iter()
            .map(|m| i16::from_be_bytes([m[HEADER], m[HEADER + 1]]))
            .collect()
    }

    fn state(actions: &[Action]) -> Value {
        let mut merged = json!({});
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(&mut merged, p);
            }
        }
        merged
    }

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn reply(code: i16, data: &[u8]) -> Vec<u8> {
        let mut body = code.to_be_bytes().to_vec();
        body.extend_from_slice(data);
        frame(0, &body)
    }

    fn opened(m: &mut PandorasBox) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    fn feed(m: &mut PandorasBox, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut PandorasBox, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    #[test]
    fn the_header_matches_the_sdk() {
        // The SDK's header for SetSequenceTransportMode(1, Play): PBAU, 01,
        // domain 0, length 10, connection 0, protocol 0, checksum 11.
        let cmd = command("set_sequence_transport_mode").unwrap();
        let body = encode(
            cmd,
            json!({"sequence_id": 1, "transport_mode": "play"})
                .as_object()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            frame(0, &body),
            [
                b"PBAU".as_slice(),
                &[1, 0, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 11],
                &[0, 3, 0, 0, 0, 1, 0, 0, 0, 1],
            ]
            .concat()
        );
        // The checksum counts the domain too.
        assert_eq!(frame(2, &body)[16], 13);
    }

    #[test]
    fn arguments_are_encoded_by_type() {
        let cmd = command("set_param_double").unwrap();
        let body = encode(
            cmd,
            json!({"site_id": 1, "device_id": 2, "parameter_name": "Opacity", "parameter_value": 0.5,
                   "do_silent": false, "do_direct": true})
            .as_object()
            .unwrap(),
        )
        .unwrap();
        let mut want = 84i16.to_be_bytes().to_vec();
        want.extend(1i32.to_be_bytes());
        want.extend(2i32.to_be_bytes());
        want.extend(7i16.to_be_bytes());
        want.extend(b"Opacity");
        want.extend(0.5f64.to_le_bytes());
        want.extend([0, 1]);
        assert_eq!(body, want);
        // Wide strings are UCS-2 big-endian; narrow ones must be ASCII.
        let wide = encode(
            command("create_project_folder").unwrap(),
            json!({"folder_name": "Ä1"}).as_object().unwrap(),
        )
        .unwrap();
        assert_eq!(wide, [0, 83, 0, 2, 0, 0xC4, 0, b'1']);
        assert!(encode(
            command("set_text").unwrap(),
            json!({"dmx_folder_id": 1, "dmx_file_id": 1, "text": "café"})
                .as_object()
                .unwrap(),
        )
        .is_err());
    }

    #[test]
    fn answers_are_decoded_and_replies_matched() {
        let mut m = module(true, json!({"sequences": "1"}));
        let a = opened(&mut m);
        // The revision first, then the selection, then sequence 1.
        assert_eq!(codes(&a), [334]);
        let a = feed(&mut m, 40, &reply(334, &12086i32.to_be_bytes()));
        assert_eq!(state(&a), json!({"device": {"revision": 12086}}));
        assert!(a.contains(&Action::RoundTrip(40)));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(codes(&a), [317]);
        let mut ids = 1i32.to_be_bytes().to_vec();
        ids.extend(2i32.to_be_bytes());
        let a = feed(
            &mut m,
            50,
            &reply(317, &[2i32.to_be_bytes().as_slice(), &ids].concat()),
        );
        assert_eq!(codes(&a), [81]);
        let a = feed(&mut m, 60, &reply(81, &0i32.to_be_bytes()));
        assert_eq!(codes(&a), [72]);
        let a = feed(&mut m, 70, &reply(72, &1i32.to_be_bytes()));
        assert_eq!(
            state(&a),
            json!({"sequences": {"1": {"transport": "play"}}})
        );
        let t: Vec<u8> = [0i32, 1, 20, 5]
            .iter()
            .flat_map(|n| n.to_be_bytes())
            .collect();
        let a = feed(&mut m, 80, &reply(73, &t));
        assert_eq!(
            state(&a),
            json!({"sequences": {"1": {"timecode": "00:01:20:05"}}})
        );
        // An older master refuses the cue information: it is not asked again.
        let a = feed(&mut m, 90, &reply(-295, &5i32.to_be_bytes()));
        assert_eq!(codes(&a), [109]);
        assert!(!m.cue_info);
        let a = feed(&mut m, 95, &reply(109, &[1]));
        assert_eq!(state(&a), json!({"sites": {"1": {"connected": true}}}));
    }

    #[test]
    fn a_command_with_values_waits_and_others_go_unverified() {
        let mut m = module(true, json!({"sequences": "1"}));
        opened(&mut m);
        feed(&mut m, 10, &reply(334, &1i32.to_be_bytes()));
        // A command goes ahead of the queued reads, behind the one in flight.
        let a = run(
            &mut m,
            7,
            "get_sequence_transport_mode",
            json!({"sequence_id": 3}),
        );
        assert!(sent(&a).is_empty());
        let a = feed(&mut m, 20, &reply(317, &0i32.to_be_bytes()));
        assert_eq!(codes(&a), [72]);
        let a = feed(&mut m, 30, &reply(72, &2i32.to_be_bytes()));
        assert_eq!(
            completed(&a, 7),
            Some(Ok(Outcome::Value {
                value: json!({"transport_mode": "stop"})
            }))
        );
        // Fire-and-forget: sent at its turn and reported unverified, with a
        // read-back of the watched sequence it acts on.
        let mut m = module(true, json!({"sequences": "1"}));
        opened(&mut m);
        feed(&mut m, 5, &reply(334, &1i32.to_be_bytes()));
        feed(&mut m, 6, &reply(317, &0i32.to_be_bytes()));
        feed(&mut m, 7, &reply(81, &0i32.to_be_bytes()));
        feed(&mut m, 8, &reply(72, &1i32.to_be_bytes()));
        let t: Vec<u8> = [0i32; 4].iter().flat_map(|n| n.to_be_bytes()).collect();
        feed(&mut m, 9, &reply(73, &t));
        let a = feed(&mut m, 10, &reply(-295, &[0, 0, 0, 1]));
        assert!(codes(&a).is_empty());
        let a = run(
            &mut m,
            9,
            "set_sequence_transport_mode",
            json!({"sequence_id": 1, "transport_mode": "stop"}),
        );
        assert_eq!(codes(&a), [3, 72]);
        assert_eq!(completed(&a, 9), Some(Ok(Outcome::Unverified)));
    }

    #[test]
    fn a_negative_code_fails_the_command() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        feed(&mut m, 10, &reply(334, &1i32.to_be_bytes()));
        let a = run(
            &mut m,
            4,
            "get_cue_name",
            json!({"sequence_id": 1, "cue_id": 9}),
        );
        assert_eq!(codes(&a), [277]);
        let a = feed(&mut m, 20, &reply(-1, &(-17i32).to_be_bytes()));
        assert!(matches!(
            completed(&a, 4),
            Some(Err(CommandError::DeviceError { code: Some(c), .. })) if c == "-17"
        ));
    }

    #[test]
    fn commands_only_reads_nothing_but_the_revision() {
        let mut m = module(false, json!({"sequences": "1,2"}));
        let a = opened(&mut m);
        assert_eq!(codes(&a), [334]);
        let a = feed(&mut m, 25, &reply(334, &7i32.to_be_bytes()));
        assert!(sent(&a).is_empty());
        assert!(a.contains(&Action::RoundTrip(25)));
        // The liveness read repeats; no sequence or site is read.
        let mut cx = Cx::new(10_000);
        m.timer(&mut cx, SLOW_TIMER);
        assert_eq!(codes(&cx.take()), [334]);
        let mut cx = Cx::new(10_000);
        m.timer(&mut cx, FAST_TIMER);
        assert!(sent(&cx.take()).is_empty());
        // Commands still work, after the read in flight.
        feed(&mut m, 10_020, &reply(334, &7i32.to_be_bytes()));
        let a = run(&mut m, 1, "save_project", json!({}));
        assert_eq!(codes(&a), [62]);
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Unverified)));
    }

    #[test]
    fn the_deframer_splits_and_resynchronises() {
        let mut d = Deframer::default();
        let one = reply(334, &1i32.to_be_bytes());
        let two = reply(81, &3i32.to_be_bytes());
        let mut stream = b"xx".to_vec();
        stream.extend(&one);
        stream.extend(&two[..5]);
        assert_eq!(
            d.feed(&stream),
            [Reply {
                code: 334,
                data: 1i32.to_be_bytes().to_vec()
            }]
        );
        assert_eq!(
            d.feed(&two[5..]),
            [Reply {
                code: 81,
                data: 3i32.to_be_bytes().to_vec()
            }]
        );
    }

    #[test]
    fn every_command_of_the_spec_is_in_the_table() {
        let spec = Catalog::source_tree().devices["christie-pandorasbox"].clone();
        for (name, c) in &spec.commands {
            let cmd = command(name).unwrap_or_else(|| panic!("{name} is not in the table"));
            let params: Vec<&str> = cmd.send.iter().map(|a| a.name).collect();
            let declared: Vec<&str> = c.params.keys().map(String::as_str).collect();
            let mut a = params.clone();
            let mut b = declared.clone();
            a.sort_unstable();
            b.sort_unstable();
            assert_eq!(a, b, "{name}");
        }
        assert_eq!(spec.commands.len(), COMMANDS.len());
        // The enumerations in use.
        assert!(TRANSPORT_MODE.contains(&("pause", 3)));
    }
}
