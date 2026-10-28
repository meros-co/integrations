//! Samsung MDC (Multiple Display Control) for Samsung signage displays, over
//! TCP (RJ45, port 1515), its secured TLS mode, or a serial port (RS-232C).
//!
//! Wire format from Samsung's "Multiple Display Control Protocol" Ver. 15.0
//! (2020-11-06); section numbers below are that document's.
//!
//! - A request is 0xAA, the command, the display's ID, the data length, the
//!   data and a checksum: the sum of every byte after 0xAA, modulo 256
//!   (§1.2.1). A reply is 0xAA 0xFF, the ID, a length, 'A' (ACK) or 'N'
//!   (NAK), the command it answers, then its values (a NAK's one value is an
//!   error code). Commands with a sub-command carry it as the first data byte,
//!   and an ACK echoes it before the values.
//! - One request is outstanding at a time; its reply is the next frame for its
//!   command and ID. A reply is waited for 3 s; power on and off are sent up to
//!   three times 2 s apart until acknowledged (§2.1.11). Every command's layout
//!   comes from the generated table (`samsung_mdc_table.rs`), and a reply with
//!   several documented layouts is decoded by its length.
//! - Displays on an RS-232C chain share the link and are told apart by ID
//!   (§1.1): the `display_id` setting and `chain_ids` name them, and state is
//!   kept per display under `displays.<id>`. ID 0xFE reaches every display and
//!   none answers (§1.2.1), so such a command is unverified.
//! - The secured mode (newer firmware, not in Ver. 15.0): the display greets
//!   with `MDCSTART<<TLS>>`, a TLS handshake follows on the same connection,
//!   then the 4-digit PIN, answered `MDCAUTH<<PASS>>` or `MDCAUTH<<FAIL:...>>`.
//!   With the `pin` setting the module opens a TLS stream that reads the
//!   greeting first, then sends the PIN; a refused PIN is terminal.
//! - Nothing is pushed: status, the error status and power are read every
//!   `poll_interval_s`, everything else readable every `refresh_interval_s`
//!   and on connecting. A read answered NAK, or twice unanswered, is not
//!   polled again until reconnecting or `refresh`.
//! - A display that is off over RJ45 takes power on only by Wake-on-LAN
//!   (§2.1.11): set_power on while unreachable, and wake_on_lan, send the
//!   magic packet when `mac_address` is set; after a power on acknowledged
//!   over TCP the connection is reopened 10 s later, as §2.1.11 asks.
//!
//! Opened for commands only (`monitor` false), nothing is read on connecting
//! and nothing polled; a power read every 10 s while idle is the liveness
//! check, and what replies carry still reaches state.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde_json::{json, Map, Value};

use super::samsung_mdc_table::DEFS;
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, SerialPort, TcpInput, TlsTarget,
};

/// MDC over RJ45 (§1.2.2).
pub(crate) const DEFAULT_PORT: u16 = 1515;
const SOCKET: Key = "mdc";
const WOL_SOCKET: Key = "wol";

const REPLY_TIMER: Key = "reply";
const RETRY_TIMER: Key = "retry";
const POLL_TIMER: Key = "poll";
const REFRESH_TIMER: Key = "refresh";
const PROBE_TIMER: Key = "probe";
const AUTH_TIMER: Key = "auth";

const HEADER: u8 = 0xAA;
const REPLY: u8 = 0xFF;
const BROADCAST: u8 = 0xFE;
const POWER: u8 = 0x11;

const REPLY_TIMEOUT: Millis = 3_000;
/// §2.1.11: power commands are retried every 2 s, three times in all.
const POWER_TIMEOUT: Millis = 2_000;
const POWER_ATTEMPTS: u32 = 3;
/// §2.1.11: reconnect 10 s after a power on over RJ45.
const POWER_ON_RECONNECT: Millis = 10_000;
const PROBE_EVERY: Millis = 10_000;
const AUTH_TIMEOUT: Millis = 5_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// Unanswered requests in a row before the display is reported unreachable.
const SILENT_LIMIT: u32 = 3;
const GREETING: &[u8] = b"MDCSTART<<TLS>>";

// ── The command table's types ───────────────────────────────────────────────

/// How a field of a frame's data is laid out.
#[derive(Debug)]
pub(crate) enum Kind {
    U8 {
        min: u8,
        max: u8,
    },
    /// High byte first.
    U16 {
        min: u16,
        max: u16,
    },
    I8 {
        min: i8,
        max: i8,
    },
    Enum(&'static [(u8, &'static str)]),
    /// Read only: each named bit becomes a boolean.
    Flags(&'static [(u8, &'static str)]),
    /// The rest of the data as text.
    Text {
        max: usize,
    },
    /// Text of a fixed length, trailing NULs and spaces trimmed.
    Fixed(usize),
    /// A length byte, then that much text.
    PText {
        max: usize,
    },
    Ip4,
    /// The rest of the data, given and kept as hex.
    Hex {
        max: usize,
    },
    /// A constant byte: written as is, and a reply must hold it.
    Lit(u8),
    /// Bytes written as zero and ignored when read.
    Skip(usize),
    /// Repeated items: as many as a count byte says, or to the end.
    List {
        prefix: bool,
        item: &'static [Field],
    },
}

#[derive(Debug)]
pub(crate) struct Field {
    pub name: &'static str,
    pub kind: Kind,
}

/// When a readable setting is read without being asked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Poll {
    /// Every `poll_interval_s`.
    Fast,
    /// On connecting, then every `refresh_interval_s`.
    Slow,
    /// On connecting and on `refresh`: what does not change.
    Identity,
    /// Only when asked: reads that need a parameter.
    Never,
}

/// One MDC command (a command byte and maybe a sub-command), as the spec's
/// get and set commands use it.
#[derive(Debug)]
pub(crate) struct Def {
    pub key: &'static str,
    /// The state key replies write, under `displays.<id>`; `None` for an action.
    pub state: Option<&'static str>,
    pub cmd: u8,
    pub sub: Option<u8>,
    pub get: Option<&'static str>,
    pub get_args: &'static [Field],
    pub set: Option<&'static str>,
    pub set_fields: &'static [Field],
    /// The reply's layouts after the sub-command echo, told apart by length.
    pub ack: &'static [&'static [Field]],
    /// The reply to a set, when it differs from `ack`.
    pub set_ack: &'static [&'static [Field]],
    pub poll: Poll,
    /// Each value is its own state key (the combined status reads).
    pub flat: bool,
    /// The reply is one value, kept at the state key itself.
    pub single: bool,
}

// ── Framing ─────────────────────────────────────────────────────────────────

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

/// A request frame (§1.2.1).
pub(crate) fn frame(cmd: u8, id: u8, data: &[u8]) -> Vec<u8> {
    let mut out = vec![HEADER, cmd, id, data.len() as u8];
    out.extend_from_slice(data);
    out.push(checksum(&out[1..]));
    out
}

#[derive(Debug, Clone, PartialEq)]
struct Reply {
    id: u8,
    ack: bool,
    cmd: u8,
    /// The values after the answered command byte.
    data: Vec<u8>,
}

#[derive(Default)]
struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    /// Every whole frame received; bytes before a header, and frames whose
    /// checksum fails, are skipped.
    fn feed(&mut self, data: &[u8]) -> Vec<Reply> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            let Some(start) = self.buf.iter().position(|b| *b == HEADER) else {
                self.buf.clear();
                break;
            };
            self.buf.drain(..start);
            if self.buf.len() < 4 {
                break;
            }
            let total = 4 + self.buf[3] as usize + 1;
            if self.buf.len() < total {
                break;
            }
            let frame: Vec<u8> = self.buf.drain(..total).collect();
            if checksum(&frame[1..total - 1]) != frame[total - 1] {
                // Not a frame: look for the next header after this one.
                self.buf.splice(0..0, frame[1..].iter().copied());
                continue;
            }
            if frame[1] != REPLY || frame[3] < 2 {
                continue;
            }
            out.push(Reply {
                id: frame[2],
                ack: frame[4] == b'A',
                cmd: frame[5],
                data: frame[6..total - 1].to_vec(),
            });
        }
        out
    }
}

// ── Values ──────────────────────────────────────────────────────────────────

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches(['\0', ' '])
        .to_string()
}

/// Decode `fields` from `bytes` at `pos`; the new position, or `None` when
/// the bytes do not fit. A value the display reports as unsupported (0xFF
/// where the field's range is smaller) is left out.
fn decode_fields(
    fields: &[Field],
    bytes: &[u8],
    mut pos: usize,
    out: &mut Map<String, Value>,
) -> Option<usize> {
    for f in fields {
        let rest = bytes.get(pos..)?;
        let mut put = |v: Value| {
            out.insert(f.name.to_string(), v);
        };
        match &f.kind {
            Kind::U8 { max, .. } => {
                let b = *rest.first()?;
                if !(b == 0xFF && *max < 0xFF) {
                    put(json!(b));
                }
                pos += 1;
            }
            Kind::U16 { max, .. } => {
                let v = u16::from_be_bytes([*rest.first()?, *rest.get(1)?]);
                if !(v == 0xFFFF && *max < 0xFFFF) {
                    put(json!(v));
                }
                pos += 2;
            }
            Kind::I8 { .. } => {
                let b = *rest.first()?;
                if b != 0xFF {
                    put(json!(b as i8));
                }
                pos += 1;
            }
            Kind::Enum(table) => {
                let b = *rest.first()?;
                match table.iter().find(|(c, _)| *c == b) {
                    Some((_, v)) => put(json!(v)),
                    None if b == 0xFF => {}
                    None => put(json!(format!("0x{b:02x}"))),
                }
                pos += 1;
            }
            Kind::Flags(bits) => {
                let b = *rest.first()?;
                let flags: Map<String, Value> = bits
                    .iter()
                    .map(|(bit, name)| (name.to_string(), json!(b & (1 << bit) != 0)))
                    .collect();
                put(Value::Object(flags));
                pos += 1;
            }
            Kind::Text { .. } => {
                put(json!(text_of(rest)));
                pos = bytes.len();
            }
            Kind::Fixed(n) => {
                put(json!(text_of(rest.get(..*n)?)));
                pos += n;
            }
            Kind::PText { .. } => {
                let n = *rest.first()? as usize;
                put(json!(text_of(rest.get(1..1 + n)?)));
                pos += 1 + n;
            }
            Kind::Ip4 => {
                let o = rest.get(..4)?;
                put(json!(Ipv4Addr::new(o[0], o[1], o[2], o[3]).to_string()));
                pos += 4;
            }
            Kind::Hex { .. } => {
                put(json!(hex(rest)));
                pos = bytes.len();
            }
            Kind::Lit(v) => {
                if rest.first()? != v {
                    return None;
                }
                pos += 1;
            }
            Kind::Skip(n) => {
                rest.get(..*n)?;
                pos += n;
            }
            Kind::List { prefix, item } => {
                let mut items = Vec::new();
                let count = if *prefix {
                    pos += 1;
                    Some(*rest.first()? as usize)
                } else {
                    None
                };
                while count.map_or(pos < bytes.len(), |c| items.len() < c) {
                    let mut m = Map::new();
                    pos = decode_fields(item, bytes, pos, &mut m)?;
                    items.push(Value::Object(m));
                }
                put(Value::Array(items));
            }
        }
    }
    Some(pos)
}

/// The first layout that takes every byte of the reply.
fn decode(layouts: &[&[Field]], bytes: &[u8]) -> Option<Map<String, Value>> {
    layouts.iter().find_map(|layout| {
        let mut m = Map::new();
        (decode_fields(layout, bytes, 0, &mut m)? == bytes.len()).then_some(m)
    })
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

/// The data bytes for `fields` from a command's parameters.
fn encode(fields: &[Field], p: &Params, out: &mut Vec<u8>) -> Result<(), CommandError> {
    for f in fields {
        let int = || {
            p.get(f.name)
                .and_then(Value::as_i64)
                .ok_or_else(|| invalid(format!("'{}' is required", f.name)))
        };
        let text = || {
            p.get(f.name)
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(format!("'{}' is required", f.name)))
        };
        match &f.kind {
            Kind::U8 { min, max } => {
                let v = int()?;
                if v < i64::from(*min) || v > i64::from(*max) {
                    return Err(invalid(format!("'{}' {v} is out of range", f.name)));
                }
                out.push(v as u8);
            }
            Kind::U16 { min, max } => {
                let v = int()?;
                if v < i64::from(*min) || v > i64::from(*max) {
                    return Err(invalid(format!("'{}' {v} is out of range", f.name)));
                }
                out.extend_from_slice(&(v as u16).to_be_bytes());
            }
            Kind::I8 { min, max } => {
                let v = int()?;
                if v < i64::from(*min) || v > i64::from(*max) {
                    return Err(invalid(format!("'{}' {v} is out of range", f.name)));
                }
                out.push(v as i8 as u8);
            }
            Kind::Enum(table) => {
                let v = text()?;
                let code = table
                    .iter()
                    .find(|(_, name)| *name == v)
                    .map(|(c, _)| *c)
                    .ok_or_else(|| invalid(format!("'{}' has no value '{v}'", f.name)))?;
                out.push(code);
            }
            Kind::Text { max } => {
                let v = text()?;
                if v.len() > *max {
                    return Err(invalid(format!("'{}' is longer than {max} bytes", f.name)));
                }
                out.extend_from_slice(v.as_bytes());
            }
            Kind::PText { max } => {
                let v = text()?;
                if v.len() > *max {
                    return Err(invalid(format!("'{}' is longer than {max} bytes", f.name)));
                }
                out.push(v.len() as u8);
                out.extend_from_slice(v.as_bytes());
            }
            Kind::Ip4 => {
                let v: Ipv4Addr = text()?
                    .parse()
                    .map_err(|_| invalid(format!("'{}' is not an IPv4 address", f.name)))?;
                out.extend_from_slice(&v.octets());
            }
            Kind::Hex { max } => {
                let v = p.get(f.name).and_then(Value::as_str).unwrap_or("");
                let bytes =
                    unhex(v).ok_or_else(|| invalid(format!("'{}' is not hex bytes", f.name)))?;
                if bytes.len() > *max {
                    return Err(invalid(format!("'{}' is longer than {max} bytes", f.name)));
                }
                out.extend_from_slice(&bytes);
            }
            Kind::Lit(v) => out.push(*v),
            Kind::Skip(n) => out.extend(std::iter::repeat_n(0, *n)),
            Kind::Fixed(_) | Kind::Flags(_) | Kind::List { .. } => {
                return Err(invalid(format!("'{}' cannot be set", f.name)));
            }
        }
    }
    Ok(())
}

fn put(root: &mut Map<String, Value>, path: &[&str], value: Value) {
    match path {
        [] => {}
        [last] => {
            root.insert(last.to_string(), value);
        }
        [first, rest @ ..] => {
            let child = root
                .entry(first.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !child.is_object() {
                *child = Value::Object(Map::new());
            }
            if let Value::Object(m) = child {
                put(m, rest, value);
            }
        }
    }
}

// ── The module ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum What {
    Get { def: usize, index: Option<u8> },
    Set { def: usize },
    Raw,
}

#[derive(Debug, Clone)]
struct Job {
    command: Option<CommandId>,
    id: u8,
    cmd: u8,
    data: Vec<u8>,
    what: What,
    /// A read the module asked for itself.
    poll: bool,
    attempts: u32,
    max_attempts: u32,
    timeout: Millis,
    /// Set power on: reconnect after its ACK.
    power_on: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Connecting,
    /// TLS is up and the PIN sent; waiting for MDCAUTH.
    Auth,
    Ready,
    /// The PIN was refused, or a PIN is needed: nothing more until the
    /// settings change.
    Refused,
}

pub(crate) struct SamsungMdc {
    link: Link,
    host: IpAddr,
    pin: Option<String>,
    mac: Option<[u8; 6]>,
    wol_to: Ipv4Addr,
    displays: Vec<u8>,
    primary: u8,
    monitor: bool,
    poll_every: Millis,
    refresh_every: Millis,
    by_name: HashMap<&'static str, (usize, bool)>,
    phase: Phase,
    connected: bool,
    deframer: Deframer,
    text: Vec<u8>,
    commands: VecDeque<Job>,
    polls: VecDeque<Job>,
    in_flight: Option<Job>,
    sent_at: Millis,
    retry_after: Millis,
    unsupported: HashSet<(u8, usize)>,
    unanswered: HashMap<(u8, usize), u32>,
    silent: u32,
}

impl SamsungMdc {
    pub(crate) fn new(ctx: OpenContext) -> Result<SamsungMdc, String> {
        let s = &ctx.settings;
        let text = |k: &str| s.get(k).and_then(Value::as_str).unwrap_or("").trim();
        let num = |k: &str, d: u64| s.get(k).and_then(Value::as_u64).unwrap_or(d);
        let primary = num("display_id", 0) as u8;
        let mut displays = vec![primary];
        for id in text("chain_ids").split(',').filter(|t| !t.is_empty()) {
            let id: u8 = id
                .trim()
                .parse()
                .ok()
                .filter(|v| *v <= 253)
                .ok_or_else(|| format!("chain_ids: '{id}' is not an ID from 0 to 253"))?;
            if !displays.contains(&id) {
                displays.push(id);
            }
        }
        let mac = match text("mac_address") {
            "" => None,
            m => Some(
                super::wol::parse_mac(m)
                    .ok_or_else(|| format!("mac_address '{m}' is not a MAC address"))?,
            ),
        };
        let wol_to = text("wol_address").parse().unwrap_or(Ipv4Addr::BROADCAST);
        let pin = Some(text("pin").to_string()).filter(|p| !p.is_empty());
        let mut by_name = HashMap::new();
        for (i, d) in DEFS.iter().enumerate() {
            if let Some(g) = d.get {
                by_name.insert(g, (i, true));
            }
            if let Some(n) = d.set {
                by_name.insert(n, (i, false));
            }
        }
        Ok(SamsungMdc {
            link: Link::from_settings(&ctx, DEFAULT_PORT)?,
            host: ctx.host,
            pin,
            mac,
            wol_to,
            displays,
            primary,
            monitor: ctx.monitor,
            poll_every: num("poll_interval_s", 5) * 1000,
            refresh_every: num("refresh_interval_s", 300) * 1000,
            by_name,
            phase: Phase::Closed,
            connected: false,
            deframer: Deframer::default(),
            text: Vec::new(),
            commands: VecDeque::new(),
            polls: VecDeque::new(),
            in_flight: None,
            sent_at: 0,
            retry_after: RETRY_MIN,
            unsupported: HashSet::new(),
            unanswered: HashMap::new(),
            silent: 0,
        })
    }

    fn open(&mut self, cx: &mut Cx) {
        self.phase = Phase::Connecting;
        self.deframer = Deframer::default();
        self.text.clear();
        cx.connection(Connection::Connecting);
        match (&self.link, &self.pin) {
            (Link::Tcp(to), Some(_)) => cx.tcp_open_tls(
                SOCKET,
                TlsTarget {
                    to: *to,
                    server_name: to.ip().to_string(),
                    // The display's certificate is its own, signed by no
                    // public root; the library this follows checks none.
                    accept_invalid_certs: true,
                    preamble: Some(GREETING.to_vec()),
                    alpn: Vec::new(),
                },
            ),
            (link, _) => link.open(cx, SOCKET),
        }
    }

    fn is_serial(&self) -> bool {
        matches!(self.link, Link::Serial(SerialPort { .. }))
    }

    // ── Jobs ────────────────────────────────────────────────────────────────

    fn get_job(&self, def: usize, id: u8, p: &Params) -> Result<Job, CommandError> {
        let d = &DEFS[def];
        let mut data: Vec<u8> = d.sub.into_iter().collect();
        encode(d.get_args, p, &mut data)?;
        let index = d
            .get_args
            .iter()
            .find(|f| f.name == "index")
            .and_then(|f| p.get(f.name))
            .and_then(Value::as_u64)
            .map(|v| v as u8);
        Ok(self.job(id, d.cmd, data, What::Get { def, index }))
    }

    fn job(&self, id: u8, cmd: u8, data: Vec<u8>, what: What) -> Job {
        let power = cmd == POWER && !matches!(what, What::Get { .. });
        Job {
            command: None,
            id,
            cmd,
            data,
            what,
            poll: false,
            attempts: 0,
            max_attempts: if power { POWER_ATTEMPTS } else { 1 },
            timeout: if power { POWER_TIMEOUT } else { REPLY_TIMEOUT },
            power_on: false,
        }
    }

    /// Queue reads of every display's settings of these kinds, skipping
    /// those already queued and those a display refused.
    fn queue_polls(&mut self, kinds: &[Poll]) {
        for id in self.displays.clone() {
            for (i, d) in DEFS.iter().enumerate() {
                if d.get.is_none() || !d.get_args.is_empty() || !kinds.contains(&d.poll) {
                    continue;
                }
                if self.unsupported.contains(&(id, i)) {
                    continue;
                }
                let queued = |j: &Job| {
                    j.id == id
                        && j.what
                            == What::Get {
                                def: i,
                                index: None,
                            }
                };
                if self.polls.iter().any(queued) || self.in_flight.as_ref().is_some_and(queued) {
                    continue;
                }
                let Ok(mut job) = self.get_job(i, id, &Params::new()) else {
                    continue;
                };
                job.poll = true;
                self.polls.push_back(job);
            }
        }
    }

    fn send_in_flight(&mut self, cx: &mut Cx) {
        let Some(job) = self.in_flight.as_mut() else {
            return;
        };
        job.attempts += 1;
        cx.tcp_send(SOCKET, frame(job.cmd, job.id, &job.data));
        self.sent_at = cx.now();
        if job.id == BROADCAST {
            // §1.2.1: displays addressed all at once do not acknowledge.
            let job = self.in_flight.take().expect("in flight");
            if let Some(id) = job.command {
                cx.complete(id, Ok(Outcome::Unverified));
            }
            self.pump(cx);
            return;
        }
        cx.set_timer(REPLY_TIMER, job.timeout);
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Ready || self.in_flight.is_some() {
            return;
        }
        let Some(job) = self.commands.pop_front().or_else(|| self.polls.pop_front()) else {
            return;
        };
        self.in_flight = Some(job);
        self.send_in_flight(cx);
    }

    fn finish(&mut self, cx: &mut Cx, result: Result<Outcome, CommandError>) {
        cx.cancel_timer(REPLY_TIMER);
        if let Some(job) = self.in_flight.take() {
            if let Some(id) = job.command {
                cx.complete(id, result);
            }
        }
        self.pump(cx);
    }

    // ── Connection ──────────────────────────────────────────────────────────

    fn set_connected(&mut self, cx: &mut Cx) {
        self.silent = 0;
        if !self.connected {
            self.connected = true;
            cx.connection(Connection::Connected);
        }
    }

    fn fail_all(&mut self, cx: &mut Cx, error: CommandError) {
        let fail = |cx: &mut Cx, job: Job| {
            if let Some(id) = job.command {
                cx.complete(id, Err(error.clone()));
            }
        };
        if let Some(job) = self.in_flight.take() {
            fail(cx, job);
        }
        for job in self.commands.drain(..) {
            fail(cx, job);
        }
        self.polls.clear();
        cx.cancel_timer(REPLY_TIMER);
    }

    /// The link is gone: fail what waits and open it again after `after`.
    fn lost(&mut self, cx: &mut Cx, reason: String, after: Millis) {
        self.fail_all(
            cx,
            CommandError::Transport {
                message: reason.clone(),
            },
        );
        if self.phase != Phase::Closed {
            cx.tcp_close(SOCKET);
        }
        self.phase = Phase::Closed;
        self.connected = false;
        cx.cancel_timer(AUTH_TIMER);
        cx.connection(Connection::Disconnected {
            reason: reason.clone(),
        });
        cx.log(Level::Debug, format!("MDC: {reason}"));
        cx.set_timer(RETRY_TIMER, after);
    }

    fn refuse(&mut self, cx: &mut Cx, reason: String) {
        self.fail_all(
            cx,
            CommandError::Auth {
                message: reason.clone(),
            },
        );
        cx.tcp_close(SOCKET);
        self.phase = Phase::Refused;
        self.connected = false;
        cx.cancel_timer(AUTH_TIMER);
        cx.connection(Connection::Unauthorized { reason });
    }

    fn ready(&mut self, cx: &mut Cx) {
        self.phase = Phase::Ready;
        self.retry_after = RETRY_MIN;
        self.unsupported.clear();
        self.unanswered.clear();
        if self.monitor {
            self.queue_polls(&[Poll::Fast, Poll::Identity, Poll::Slow]);
            if self.poll_every > 0 {
                cx.set_timer(POLL_TIMER, self.poll_every);
            }
            if self.refresh_every > 0 {
                cx.set_timer(REFRESH_TIMER, self.refresh_every);
            }
        } else {
            self.queue_probe();
        }
        cx.set_timer(PROBE_TIMER, PROBE_EVERY);
        self.pump(cx);
    }

    /// One power read of the main display: the liveness check.
    fn queue_probe(&mut self) {
        if let Some(def) = DEFS.iter().position(|d| d.cmd == POWER) {
            if let Ok(mut job) = self.get_job(def, self.primary, &Params::new()) {
                job.poll = true;
                self.polls.push_back(job);
            }
        }
    }

    // ── Wake-on-LAN ─────────────────────────────────────────────────────────

    fn wake(&mut self, cx: &mut Cx) -> bool {
        let Some(mac) = self.mac else {
            return false;
        };
        super::wol::send(
            cx,
            WOL_SOCKET,
            super::wol::magic_packet(mac, None),
            SocketAddr::new(IpAddr::V4(self.wol_to), 9),
            Some(self.host),
        );
        true
    }

    // ── Replies ─────────────────────────────────────────────────────────────

    fn apply(
        &self,
        cx: &mut Cx,
        def: &Def,
        id: u8,
        index: Option<u8>,
        values: &Map<String, Value>,
    ) {
        let Some(key) = def.state else {
            return;
        };
        let display = id.to_string();
        let mut root = Map::new();
        let key = match index {
            Some(i) => key.replace("{index}", &i.to_string()),
            None => key.to_string(),
        };
        let base: Vec<&str> = ["displays", display.as_str()]
            .into_iter()
            .chain(key.split('.'))
            .collect();
        if def.flat {
            for (name, v) in values {
                put(&mut root, &["displays", &display, name], v.clone());
            }
        } else if def.single {
            if let Some(v) = values.values().next() {
                put(&mut root, &base, v.clone());
            }
        } else {
            for (name, v) in values {
                let mut path = base.clone();
                path.push(name);
                put(&mut root, &path, v.clone());
            }
        }
        if !root.is_empty() {
            cx.state(Value::Object(root));
        }
    }

    fn on_reply(&mut self, cx: &mut Cx, reply: Reply) {
        let Some(job) = self.in_flight.as_ref() else {
            return;
        };
        if reply.cmd != job.cmd || reply.id != job.id {
            cx.log(
                Level::Debug,
                format!(
                    "MDC: ignored a reply to 0x{:02x} from ID {}",
                    reply.cmd, reply.id
                ),
            );
            return;
        }
        cx.round_trip(cx.now().saturating_sub(self.sent_at));
        let job = job.clone();
        if let What::Get { def, .. } | What::Set { def } = job.what {
            self.unanswered.remove(&(job.id, def));
        }
        if job.what == What::Raw {
            let value = json!({"ack": reply.ack, "command": reply.cmd, "data": hex(&reply.data)});
            self.finish(cx, Ok(Outcome::Value { value }));
            return;
        }
        if !reply.ack {
            let code = reply.data.first().map(|b| format!("0x{b:02x}"));
            if let (true, What::Get { def, .. }) = (job.poll, &job.what) {
                self.unsupported.insert((job.id, *def));
            }
            let message = format!(
                "the display refused 0x{:02x} (NAK{})",
                job.cmd,
                code.as_ref().map(|c| format!(" {c}")).unwrap_or_default()
            );
            self.finish(cx, Err(CommandError::DeviceError { code, message }));
            return;
        }
        let (def_index, index, is_set) = match job.what {
            What::Get { def, index } => (def, index, false),
            What::Set { def } => (def, None, true),
            What::Raw => unreachable!("handled above"),
        };
        let def = &DEFS[def_index];
        // An ACK of a command with a sub-command echoes it first.
        let body = match def.sub {
            Some(_) if !reply.data.is_empty() => &reply.data[1..],
            _ => &reply.data[..],
        };
        let layouts = if is_set && !def.set_ack.is_empty() {
            def.set_ack
        } else {
            def.ack
        };
        let decoded = decode(layouts, body);
        match &decoded {
            Some(values) => self.apply(cx, def, job.id, index, values),
            None if !layouts.is_empty() => cx.log(
                Level::Debug,
                format!(
                    "MDC: a reply to {} (0x{:02x}) of {} bytes matches no documented layout: {}",
                    def.key,
                    job.cmd,
                    body.len(),
                    hex(body)
                ),
            ),
            None => {}
        }
        if job.power_on && !self.is_serial() {
            // §2.1.11: reconnect 10 s after a power on over RJ45.
            if let Some(id) = job.command {
                cx.complete(id, Ok(Outcome::Ack));
            }
            self.in_flight = None;
            cx.cancel_timer(REPLY_TIMER);
            self.lost(
                cx,
                "reconnecting after power on, as the display asks".into(),
                POWER_ON_RECONNECT,
            );
            return;
        }
        let result = if is_set {
            Ok(Outcome::Ack)
        } else {
            let value = match decoded {
                Some(m) if def.single => {
                    m.into_iter().next().map(|(_, v)| v).unwrap_or(Value::Null)
                }
                Some(m) => Value::Object(m),
                None => json!({"raw": hex(body)}),
            };
            Ok(Outcome::Value { value })
        };
        self.finish(cx, result);
    }

    fn on_text(&mut self, cx: &mut Cx, data: &[u8]) {
        self.text.extend_from_slice(data);
        let text = String::from_utf8_lossy(&self.text).to_string();
        if self.phase == Phase::Auth {
            if text.contains("MDCAUTH<<PASS>>") {
                cx.cancel_timer(AUTH_TIMER);
                self.text.clear();
                self.set_connected(cx);
                self.ready(cx);
            } else if let Some(at) = text.find("MDCAUTH<<FAIL") {
                if text[at..].contains(">>") {
                    let end = at + text[at..].find(">>").unwrap_or(0) + 2;
                    self.refuse(
                        cx,
                        format!("the display refused the PIN ({})", &text[at..end]),
                    );
                }
            }
            return;
        }
        if text.starts_with("MDCSTART") || text.contains("MDCSTART<<TLS>>") {
            self.refuse(
                cx,
                "the display's secured protocol is on: set the pin setting to its PIN".into(),
            );
        }
    }

    fn on_data(&mut self, cx: &mut Cx, data: &[u8]) {
        cx.alive();
        if self.phase == Phase::Auth || (self.pin.is_none() && data.first() == Some(&b'M')) {
            self.on_text(cx, data);
            return;
        }
        let replies = self.deframer.feed(data);
        if !replies.is_empty() {
            self.set_connected(cx);
        }
        for reply in replies {
            self.on_reply(cx, reply);
            if self.phase != Phase::Ready {
                return;
            }
        }
    }

    fn command_job(&self, name: &str, p: &Params) -> Result<Job, CommandError> {
        let id = match p.get("display_id").and_then(Value::as_u64) {
            Some(v) if v <= 254 => v as u8,
            Some(v) => return Err(invalid(format!("display_id {v} is out of range"))),
            None => self.primary,
        };
        if name == "send_raw" {
            let cmd = p.get("command").and_then(Value::as_u64).unwrap_or(0) as u8;
            let data = unhex(p.get("data").and_then(Value::as_str).unwrap_or(""))
                .ok_or_else(|| invalid("'data' is not hex bytes"))?;
            if data.len() > 255 {
                return Err(invalid("'data' is longer than 255 bytes"));
            }
            return Ok(self.job(id, cmd, data, What::Raw));
        }
        let Some(&(def, is_get)) = self.by_name.get(name) else {
            return Err(CommandError::UnknownCommand {
                command: name.to_string(),
            });
        };
        if is_get {
            if id == BROADCAST {
                return Err(invalid("a read cannot address every display (254)"));
            }
            return self.get_job(def, id, p);
        }
        let d = &DEFS[def];
        let mut data: Vec<u8> = d.sub.into_iter().collect();
        encode(d.set_fields, p, &mut data)?;
        if data.len() > 255 {
            return Err(invalid("the command's data is longer than 255 bytes"));
        }
        let mut job = self.job(id, d.cmd, data, What::Set { def });
        job.power_on = d.cmd == POWER && p.get("power").and_then(Value::as_str) == Some("on");
        Ok(job)
    }
}

impl Module for SamsungMdc {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match name {
            "wake_on_lan" => {
                let result = if self.wake(cx) {
                    Ok(Outcome::Unverified)
                } else {
                    Err(invalid("set the mac_address setting to wake the display"))
                };
                cx.complete(id, result);
                return;
            }
            "refresh" => {
                if self.phase != Phase::Ready {
                    cx.complete(id, Err(CommandError::NotConnected));
                    return;
                }
                self.unsupported.clear();
                self.unanswered.clear();
                self.queue_polls(&[Poll::Fast, Poll::Identity, Poll::Slow]);
                cx.complete(id, Ok(Outcome::Ack));
                self.pump(cx);
                return;
            }
            _ => {}
        }
        let job = match self.command_job(name, params) {
            Ok(job) => job,
            Err(e) => {
                cx.complete(id, Err(e));
                return;
            }
        };
        if matches!(self.phase, Phase::Closed | Phase::Refused) {
            // A display that is off over RJ45 takes power on only by
            // Wake-on-LAN (§2.1.11).
            if job.power_on && !self.is_serial() && self.wake(cx) {
                cx.complete(id, Ok(Outcome::Unverified));
            } else if self.phase == Phase::Refused {
                cx.complete(
                    id,
                    Err(CommandError::Auth {
                        message: "the display refused the connection's PIN".into(),
                    }),
                );
            } else {
                cx.complete(id, Err(CommandError::NotConnected));
            }
            return;
        }
        let mut job = job;
        job.command = Some(id);
        self.commands.push_back(job);
        self.pump(cx);
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                if self.phase != Phase::Connecting {
                    return;
                }
                match &self.pin {
                    Some(pin) if !self.is_serial() => {
                        self.phase = Phase::Auth;
                        cx.tcp_send(SOCKET, pin.as_bytes().to_vec());
                        cx.set_timer(AUTH_TIMER, AUTH_TIMEOUT);
                    }
                    _ => {
                        if self.is_serial() {
                            // A serial port gives no sign of the display:
                            // only its replies show it is there.
                            self.ready(cx);
                        } else {
                            self.set_connected(cx);
                            self.ready(cx);
                        }
                    }
                }
            }
            TcpInput::Data(data) => {
                if matches!(self.phase, Phase::Ready | Phase::Auth) {
                    self.on_data(cx, &data);
                }
            }
            TcpInput::Closed { reason } => {
                if matches!(self.phase, Phase::Closed | Phase::Refused) {
                    return;
                }
                let reason = if reason.contains("MDCSTART") {
                    format!("{reason} (is the display's secured protocol off? then clear the pin)")
                } else {
                    reason
                };
                self.phase = Phase::Closed;
                let after = self.retry_after;
                self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
                self.lost(cx, reason, after);
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
            AUTH_TIMER => {
                if self.phase == Phase::Auth {
                    self.phase = Phase::Connecting;
                    let after = self.retry_after;
                    self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
                    self.lost(cx, "no answer to the PIN".into(), after);
                }
            }
            POLL_TIMER => {
                if self.phase == Phase::Ready && self.monitor {
                    self.queue_polls(&[Poll::Fast]);
                    self.pump(cx);
                    cx.set_timer(POLL_TIMER, self.poll_every);
                }
            }
            REFRESH_TIMER => {
                if self.phase == Phase::Ready && self.monitor {
                    self.queue_polls(&[Poll::Slow]);
                    self.pump(cx);
                    cx.set_timer(REFRESH_TIMER, self.refresh_every);
                }
            }
            PROBE_TIMER => {
                if self.phase == Phase::Ready {
                    let idle = self.in_flight.is_none()
                        && self.commands.is_empty()
                        && self.polls.is_empty();
                    if idle && cx.now().saturating_sub(self.sent_at) >= PROBE_EVERY {
                        self.queue_probe();
                        self.pump(cx);
                    }
                    cx.set_timer(PROBE_TIMER, PROBE_EVERY);
                }
            }
            REPLY_TIMER => {
                let Some(job) = self.in_flight.as_ref() else {
                    return;
                };
                if job.attempts < job.max_attempts {
                    self.send_in_flight(cx);
                    return;
                }
                let job = job.clone();
                if let (true, What::Get { def, .. }) = (job.poll, &job.what) {
                    let n = self.unanswered.entry((job.id, *def)).or_insert(0);
                    *n += 1;
                    if *n >= 2 {
                        self.unsupported.insert((job.id, *def));
                    }
                }
                self.silent += 1;
                if self.silent >= SILENT_LIMIT && self.connected {
                    self.connected = false;
                    cx.connection(Connection::Disconnected {
                        reason: format!(
                            "no answer from display {} to {SILENT_LIMIT} requests",
                            job.id
                        ),
                    });
                }
                self.finish(cx, Err(CommandError::Timeout));
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if !matches!(self.phase, Phase::Closed | Phase::Refused) {
            cx.tcp_close(SOCKET);
        }
        self.phase = Phase::Closed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog, DeviceSpec};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;

    fn spec() -> DeviceSpec {
        Catalog::source_tree().devices["samsung-mdc"].clone()
    }

    fn module(monitor: bool, settings: Value) -> SamsungMdc {
        let spec = spec();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        SamsungMdc::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)),
            host_name: None,
            port: None,
            model: "lcd-signage".into(),
            channels: None,
            settings,
            monitor,
        })
        .unwrap()
    }

    fn unhex_ws(s: &str) -> Vec<u8> {
        unhex(&s.split_whitespace().collect::<String>()).unwrap()
    }

    /// A reply frame as a display sends it.
    fn reply(id: u8, ack: bool, cmd: u8, values: &[u8]) -> Vec<u8> {
        let mut data = vec![if ack { b'A' } else { b'N' }, cmd];
        data.extend_from_slice(values);
        let mut f = vec![HEADER, REPLY, id, data.len() as u8];
        f.extend_from_slice(&data);
        f.push(checksum(&f[1..]));
        f
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

    fn connected(m: &mut SamsungMdc) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    fn feed_at(m: &mut SamsungMdc, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run_at(m: &mut SamsungMdc, now: Millis, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn timer(m: &mut SamsungMdc, key: Key) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.timer(&mut cx, key);
        cx.take()
    }

    /// Opened for commands only and past its first liveness read.
    fn idle(settings: Value) -> SamsungMdc {
        let mut m = module(false, settings);
        connected(&mut m);
        let id = m.primary;
        feed_at(&mut m, 0, &reply(id, true, 0x11, &[1]));
        assert!(m.in_flight.is_none());
        m
    }

    #[test]
    fn frames_match_the_documented_example() {
        // §1.2.1: power on to ID 0xFE, checksum 0x11+0xFE+0x01+0x01 = 0x111 -> 0x11.
        assert_eq!(frame(0x11, 0xFE, &[1]), unhex_ws("AA 11 FE 01 01 11"));
        // Power get to ID 0.
        assert_eq!(frame(0x11, 0, &[]), unhex_ws("AA 11 00 00 11"));
        let mut d = Deframer::default();
        let mut bytes = vec![0x00, 0x42];
        bytes.extend(reply(1, true, 0x12, &[25]));
        bytes.extend(&reply(1, false, 0x14, &[3])[..4]);
        let got = d.feed(&bytes);
        assert_eq!(
            got,
            [Reply {
                id: 1,
                ack: true,
                cmd: 0x12,
                data: vec![25]
            }]
        );
        // The rest of a frame split across reads.
        let got = d.feed(&reply(1, false, 0x14, &[3])[4..]);
        assert_eq!(
            got,
            [Reply {
                id: 1,
                ack: false,
                cmd: 0x14,
                data: vec![3]
            }]
        );
        // A corrupt checksum is skipped.
        let mut bad = reply(1, true, 0x12, &[25]);
        *bad.last_mut().unwrap() ^= 1;
        assert!(d.feed(&bad).is_empty());
    }

    #[test]
    fn the_table_matches_the_spec() {
        let spec = spec();
        for d in DEFS {
            for (name, fields) in [(d.get, d.get_args), (d.set, d.set_fields)] {
                let Some(name) = name else { continue };
                let cmd = spec
                    .commands
                    .get(name)
                    .unwrap_or_else(|| panic!("{name} is not in the spec"));
                for f in fields
                    .iter()
                    .filter(|f| !matches!(f.kind, Kind::Lit(_) | Kind::Skip(_)))
                {
                    assert!(
                        cmd.params.contains_key(f.name),
                        "{name}: no param {}",
                        f.name
                    );
                }
            }
        }
        for name in spec.commands.keys() {
            let native = ["wake_on_lan", "refresh", "send_raw"].contains(&name.as_str());
            assert!(
                native
                    || DEFS
                        .iter()
                        .any(|d| d.get == Some(name) || d.set == Some(name)),
                "{name} has no table entry"
            );
        }
    }

    #[test]
    fn sets_are_the_documented_bytes_and_read_back_into_state() {
        let mut m = idle(json!({"display_id": 1}));
        let a = run_at(&mut m, 0, 1, "set_volume", json!({"volume": 25}));
        assert_eq!(sent(&a), [unhex_ws("AA 12 01 01 19 2D")]);
        let a = feed_at(&mut m, 5, &reply(1, true, 0x12, &[25]));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(state(&a)["displays"]["1"]["volume"], json!(25));

        let a = run_at(&mut m, 0, 2, "set_input", json!({"input": "hdmi2"}));
        assert_eq!(sent(&a), [frame(0x14, 1, &[0x23])]);
        let a = feed_at(&mut m, 0, &reply(1, true, 0x14, &[0x23]));
        assert_eq!(state(&a)["displays"]["1"]["input"], json!("hdmi2"));

        // A sub-command goes first and its echo is skipped.
        let a = run_at(
            &mut m,
            0,
            3,
            "set_menu_orientation",
            json!({"orientation": "portrait"}),
        );
        assert_eq!(sent(&a), [frame(0xC8, 1, &[0x81, 0x01])]);
        let a = feed_at(&mut m, 0, &reply(1, true, 0xC8, &[0x81, 0x01]));
        assert_eq!(
            state(&a)["displays"]["1"]["menu_orientation"],
            json!("portrait")
        );

        // A multi-value command, high byte first.
        let a = run_at(
            &mut m,
            0,
            4,
            "set_custom_pip",
            json!({"x": 10, "y": 20, "width": 512, "height": 288}),
        );
        assert_eq!(sent(&a), [frame(0xB7, 1, &[0, 10, 0, 20, 2, 0, 1, 32])]);
        let a = feed_at(
            &mut m,
            0,
            &reply(1, true, 0xB7, &[0, 10, 0, 20, 2, 0, 1, 32]),
        );
        assert_eq!(
            state(&a)["displays"]["1"]["custom_pip"],
            json!({"x": 10, "y": 20, "width": 512, "height": 288})
        );

        // An address and a string.
        let a = run_at(
            &mut m,
            0,
            5,
            "set_network",
            json!({"ip_address": "192.168.0.10", "subnet_mask": "255.255.255.0", "gateway": "192.168.0.1", "dns_server": "8.8.8.8"}),
        );
        assert_eq!(
            sent(&a),
            [frame(
                0x1B,
                1,
                &[0x82, 192, 168, 0, 10, 255, 255, 255, 0, 192, 168, 0, 1, 8, 8, 8, 8]
            )]
        );
        feed_at(
            &mut m,
            0,
            &reply(
                1,
                true,
                0x1B,
                &[
                    0x82, 192, 168, 0, 10, 255, 255, 255, 0, 192, 168, 0, 1, 8, 8, 8, 8,
                ],
            ),
        );
        let a = run_at(&mut m, 0, 6, "set_device_name", json!({"name": "Lobby"}));
        assert_eq!(sent(&a), [frame(0x67, 1, b"Lobby")]);
    }

    #[test]
    fn reads_decode_each_documented_layout() {
        let mut m = idle(json!({}));
        // Status (§2.1.00) fills the single-value keys.
        let a = run_at(&mut m, 0, 1, "get_status", json!({}));
        assert_eq!(sent(&a), [unhex_ws("AA 00 00 00 00")]);
        let a = feed_at(
            &mut m,
            0,
            &reply(0, true, 0x00, &[1, 30, 0, 0x21, 0x01, 0, 0]),
        );
        let s = &state(&a)["displays"]["0"];
        assert_eq!(s["power"], json!("on"));
        assert_eq!(s["volume"], json!(30));
        assert_eq!(s["mute"], json!("off"));
        assert_eq!(s["input"], json!("hdmi1"));
        assert_eq!(s["picture_size"], json!("16_9"));
        // A display without audio reports 0xFF: no value.
        let a = run_at(&mut m, 0, 2, "get_status", json!({}));
        assert!(!sent(&a).is_empty());
        let a = feed_at(
            &mut m,
            0,
            &reply(0, true, 0x00, &[1, 0xFF, 0xFF, 0x21, 0x01, 0, 0]),
        );
        assert!(state(&a)["displays"]["0"].get("volume").is_none());

        // Display status: signed temperature.
        run_at(&mut m, 0, 3, "get_display_status", json!({}));
        let a = feed_at(&mut m, 0, &reply(0, true, 0x0D, &[0, 0, 2, 1, 0xF6, 0]));
        let s = &state(&a)["displays"]["0"]["display_status"];
        assert_eq!(s["temperature_c"], json!(-10));
        assert_eq!(s["sync"], json!("no_sync"));

        // Timers: the 13-value and the 15-value layouts.
        run_at(&mut m, 0, 4, "get_timer1", json!({}));
        let a = feed_at(
            &mut m,
            0,
            &reply(
                0,
                true,
                0xA4,
                &[7, 30, 1, 1, 11, 0, 0, 1, 2, 0, 20, 0x21, 0],
            ),
        );
        let t = &state(&a)["displays"]["0"]["timer1"];
        assert_eq!(t["on_hour"], json!(7));
        assert_eq!(t["repeat"], json!("mon_fri"));
        assert_eq!(t["input"], json!("hdmi1"));
        run_at(&mut m, 0, 5, "get_timer1", json!({}));
        let a = feed_at(
            &mut m,
            0,
            &reply(
                0,
                true,
                0xA4,
                &[7, 30, 1, 1, 5, 0x3E, 11, 0, 0, 1, 1, 0, 20, 0x21, 0],
            ),
        );
        let t = &state(&a)["displays"]["0"]["timer1"];
        assert_eq!(t["on_repeat"], json!("manual"));
        assert_eq!(t["on_days"], json!(0x3E));
        assert_eq!(t["off_repeat"], json!("everyday"));

        // A text reply, and a list with a count.
        run_at(&mut m, 0, 6, "get_model_name", json!({}));
        let a = feed_at(&mut m, 0, &reply(0, true, 0x8A, b"QM55B"));
        assert_eq!(state(&a)["displays"]["0"]["model_name"], json!("QM55B"));
        assert_eq!(
            completed(&a, 6),
            Some(Ok(Outcome::Value {
                value: json!("QM55B")
            }))
        );
        run_at(&mut m, 0, 7, "get_software_versions", json!({}));
        let a = feed_at(
            &mut m,
            0,
            &reply(
                0,
                true,
                0x1B,
                &[0xA4, 2, 0, 3, b'1', b'.', b'2', 1, 1, b'9'],
            ),
        );
        assert_eq!(
            state(&a)["displays"]["0"]["software_versions"],
            json!([{"component": "cabinet_main_fw", "version": "1.2"}, {"component": "cabinet_fpga_fw", "version": "9"}])
        );
        // A code outside the table is kept as hex.
        run_at(&mut m, 0, 8, "get_input", json!({}));
        let a = feed_at(&mut m, 0, &reply(0, true, 0x14, &[0x77]));
        assert_eq!(state(&a)["displays"]["0"]["input"], json!("0x77"));
    }

    #[test]
    fn a_nak_fails_the_command_and_stops_polling_that_read() {
        let mut m = module(true, json!({}));
        connected(&mut m);
        // Answer every read until a fast poll is all that is left, refusing
        // the light sensor.
        let mut refused = false;
        while let Some(job) = m.in_flight.clone() {
            let def = match job.what {
                What::Get { def, .. } => &DEFS[def],
                _ => unreachable!(),
            };
            if def.key == "light_sensor" {
                feed_at(&mut m, 0, &reply(0, false, 0x50, &[0x01]));
                refused = true;
            } else {
                feed_at(&mut m, 0, &reply(0, false, job.cmd, &[0]));
            }
        }
        assert!(refused);
        let light = DEFS.iter().position(|d| d.key == "light_sensor").unwrap();
        assert!(m.unsupported.contains(&(0, light)));
        // Refreshing reads it again.
        let a = run_at(&mut m, 0, 1, "refresh", json!({}));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert!(!m.unsupported.contains(&(0, light)));
        // A command's NAK is a device error carrying the code.
        let mut m = idle(json!({}));
        run_at(&mut m, 0, 2, "set_volume", json!({"volume": 5}));
        let a = feed_at(&mut m, 0, &reply(0, false, 0x12, &[0x03]));
        match completed(&a, 2) {
            Some(Err(CommandError::DeviceError { code, .. })) => {
                assert_eq!(code.as_deref(), Some("0x03"))
            }
            other => panic!("expected a device error, got {other:?}"),
        }
    }

    #[test]
    fn opened_for_commands_only_it_reads_only_power_as_a_liveness_check() {
        let mut m = module(false, json!({"chain_ids": "2,3"}));
        let a = connected(&mut m);
        assert_eq!(sent(&a), [frame(0x11, 0, &[])]);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let a = feed_at(&mut m, 0, &reply(0, true, 0x11, &[0]));
        assert!(sent(&a).is_empty());
        assert_eq!(state(&a)["displays"]["0"]["power"], json!("off"));
        // Every 10 s while idle, the same read.
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, PROBE_TIMER);
        assert_eq!(sent(&cx.take()), [frame(0x11, 0, &[])]);
        feed_at(&mut m, 20_010, &reply(0, true, 0x11, &[0]));
        // No poll timer was armed, and commands still work on any display.
        assert!(timer(&mut m, POLL_TIMER).is_empty());
        let a = run_at(
            &mut m,
            0,
            1,
            "set_mute",
            json!({"display_id": 3, "mute": "on"}),
        );
        assert_eq!(sent(&a), [frame(0x13, 3, &[1])]);
    }

    #[test]
    fn monitored_it_reads_every_display_on_the_chain() {
        let mut m = module(true, json!({"display_id": 1, "chain_ids": "2"}));
        connected(&mut m);
        let mut ids = HashSet::new();
        let mut reads = 0;
        while let Some(job) = m.in_flight.clone() {
            ids.insert(job.id);
            reads += 1;
            let answer = match job.cmd {
                0x00 => reply(job.id, true, 0x00, &[1, 30, 0, 0x21, 1, 0, 0]),
                0x0D => reply(job.id, true, 0x0D, &[0, 0, 2, 0, 30, 0]),
                cmd => reply(job.id, false, cmd, &[0]),
            };
            feed_at(&mut m, 0, &answer);
        }
        assert_eq!(ids, HashSet::from([1, 2]));
        assert!(reads > 100, "{reads}");
        // The fast poll then reads status and the error status of both.
        let a = timer(&mut m, POLL_TIMER);
        assert_eq!(sent(&a), [frame(0x00, 1, &[])]);
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let mut m = idle(json!({}));
        run_at(&mut m, 1_000, 1, "get_volume", json!({}));
        let a = feed_at(&mut m, 1_040, &reply(0, true, 0x12, &[10]));
        assert!(a.contains(&Action::RoundTrip(40)));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(10) }))
        );
    }

    #[test]
    fn power_is_retried_and_a_display_that_is_off_is_woken() {
        let mut m = idle(json!({"mac_address": "00:11:22:33:44:55", "wol_address": "192.0.2.255"}));
        let a = run_at(&mut m, 0, 1, "set_power", json!({"power": "off"}));
        assert_eq!(sent(&a), [frame(0x11, 0, &[0])]);
        // Unanswered: sent again twice, 2 s apart, then a timeout.
        assert_eq!(sent(&timer(&mut m, REPLY_TIMER)), [frame(0x11, 0, &[0])]);
        assert_eq!(sent(&timer(&mut m, REPLY_TIMER)), [frame(0x11, 0, &[0])]);
        let a = timer(&mut m, REPLY_TIMER);
        assert_eq!(completed(&a, 1), Some(Err(CommandError::Timeout)));
        // Power on acknowledged over TCP: the connection is reopened in 10 s.
        run_at(&mut m, 0, 2, "set_power", json!({"power": "on"}));
        let a = feed_at(&mut m, 0, &reply(0, true, 0x11, &[1]));
        assert_eq!(completed(&a, 2), Some(Ok(Outcome::Ack)));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY_TIMER,
            after: POWER_ON_RECONNECT
        }));
        // While it cannot be reached, power on is Wake-on-LAN.
        let a = run_at(&mut m, 0, 3, "set_power", json!({"power": "on"}));
        assert_eq!(completed(&a, 3), Some(Ok(Outcome::Unverified)));
        let packets: Vec<_> = a
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { to, data, .. } => Some((*to, data.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].0, "192.0.2.255:9".parse().unwrap());
        assert_eq!(packets[1].0, "192.0.2.10:9".parse().unwrap());
        assert_eq!(&packets[0].1[..6], &[0xFF; 6]);
        assert_eq!(&packets[0].1[6..12], &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(packets[0].1.len(), 102);
    }

    #[test]
    fn broadcast_sets_are_unverified_and_raw_commands_return_bytes() {
        let mut m = idle(json!({}));
        let a = run_at(
            &mut m,
            0,
            1,
            "set_mute",
            json!({"display_id": 254, "mute": "on"}),
        );
        assert_eq!(sent(&a), [frame(0x13, 0xFE, &[1])]);
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Unverified)));
        let a = run_at(&mut m, 0, 2, "get_volume", json!({"display_id": 254}));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run_at(
            &mut m,
            0,
            3,
            "send_raw",
            json!({"command": 0x21, "data": "56"}),
        );
        assert_eq!(sent(&a), [frame(0x21, 0, &[0x56])]);
        let a = feed_at(&mut m, 0, &reply(0, true, 0x21, &[0x56, 0x01]));
        assert_eq!(
            completed(&a, 3),
            Some(Ok(Outcome::Value {
                value: json!({"ack": true, "command": 0x21, "data": "5601"})
            }))
        );
    }

    #[test]
    fn the_secured_mode_sends_the_pin_and_a_refusal_is_terminal() {
        let mut m = module(false, json!({"pin": "0000"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        let target = a
            .iter()
            .find_map(|a| match a {
                Action::TcpOpenTls { target, .. } => Some(target.clone()),
                _ => None,
            })
            .expect("a TLS stream");
        assert_eq!(target.preamble.as_deref(), Some(GREETING));
        assert_eq!(target.to, "192.0.2.10:1515".parse().unwrap());
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&cx.take()), [b"0000".to_vec()]);
        let a = feed_at(&mut m, 0, b"MDCAUTH<<PASS>>");
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(sent(&a), [frame(0x11, 0, &[])]);

        let mut m = module(false, json!({"pin": "1234"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        let a = feed_at(&mut m, 0, b"MDCAUTH<<FAIL:0x");
        assert!(a.is_empty() || !a.iter().any(|a| matches!(a, Action::Connection(_))));
        let a = feed_at(&mut m, 0, b"01>>");
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a.iter().any(|a| matches!(
            a,
            Action::SetTimer {
                key: RETRY_TIMER,
                ..
            }
        )));
        let a = run_at(&mut m, 0, 1, "get_power", json!({}));
        assert!(matches!(
            completed(&a, 1),
            Some(Err(CommandError::Auth { .. }))
        ));

        // Without a PIN, a display that greets for TLS is refused too.
        let mut m = module(false, json!({}));
        connected(&mut m);
        let a = feed_at(&mut m, 0, b"MDCSTART<<TLS>>");
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
    }

    #[test]
    fn a_serial_link_opens_the_port_and_waits_for_a_reply() {
        let mut m = module(false, json!({"link": "serial", "serial_port": "COM5"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(a, Action::SerialOpen { port, .. } if port.path == "COM5" && port.baud_rate == 9600)));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = cx.take();
        assert_eq!(sent(&a), [frame(0x11, 0, &[])]);
        assert!(!a.contains(&Action::Connection(Connection::Connected)));
        let a = feed_at(&mut m, 0, &reply(0, true, 0x11, &[1]));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
    }
}
