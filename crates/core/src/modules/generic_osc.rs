//! Generic OSC: send any OSC message to a device the catalogue does not cover
//! yet, and keep what it sends back.
//!
//! Wire format from The Open Sound Control 1.0 Specification (Matt Wright,
//! CNMAT, 2002) and "Features and Future of Open Sound Control version 1.1
//! for NIME" (Freed and Schmeder, NIME 2009):
//!
//! - A message is an OSC-string address pattern beginning with `/`, an
//!   OSC-string type tag string beginning with `,`, then the arguments. An
//!   OSC-string is the characters, a NUL, and NULs to a multiple of four
//!   bytes. Numbers are big-endian: `i` int32 (two's complement), `f` float32
//!   (IEEE 754), `h` int64, `d` float64. A blob `b` is an int32 byte count, the
//!   bytes, and NULs to a multiple of four. `T`, `F`, `N` (nil) and `I`
//!   (impulse, OSC 1.0's "Infinitum") carry no bytes. 1.0 lists `h`, `d`, `T`,
//!   `F`, `N` and `I` as nonstandard types; 1.1 makes `T`, `F`, `N` and `I`
//!   required.
//! - A bundle is `#bundle`, an 8-byte time tag, then elements, each an int32
//!   size and a message or bundle (1.0, "OSC Bundles"). Received bundles are
//!   unpacked; their time tags are not acted on.
//! - Over UDP a datagram is one packet. Over TCP 1.0 sends each packet after
//!   an int32 byte count; 1.1 frames packets with SLIP (RFC 1055) with an END
//!   byte at both ends.
//!
//! Received messages also decode `c` (char), `S` (symbol), `t` (time tag),
//! `r` (RGBA), `m` (MIDI) and `[`/`]` arrays from 1.0's nonstandard list.
//! A type the module does not know ends that message's arguments, since its
//! size cannot be known.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use super::generic_tcp_udp::{parse_hex, to_hex, RETRY_MAX, RETRY_MIN};
use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const SOCKET: Key = "osc";
const RETRY: Key = "retry";
const WAIT: Key = "wait";
const DEFAULT_MAX_ADDRESSES: usize = 256;
/// A TCP stream holding more than this without a complete packet is not
/// framed as configured.
const MAX_PACKET: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Arg {
    Int(i32),
    Float(f32),
    Str(String),
    Blob(Vec<u8>),
    True,
    False,
    Nil,
    Impulse,
    Int64(i64),
    Double(f64),
}

impl Arg {
    fn tag(&self) -> char {
        match self {
            Arg::Int(_) => 'i',
            Arg::Float(_) => 'f',
            Arg::Str(_) => 's',
            Arg::Blob(_) => 'b',
            Arg::True => 'T',
            Arg::False => 'F',
            Arg::Nil => 'N',
            Arg::Impulse => 'I',
            Arg::Int64(_) => 'h',
            Arg::Double(_) => 'd',
        }
    }
}

fn pad(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn push_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    pad(out);
}

pub(super) fn encode(address: &str, args: &[Arg]) -> Vec<u8> {
    let mut out = Vec::new();
    push_str(&mut out, address);
    let tags: String = std::iter::once(',')
        .chain(args.iter().map(Arg::tag))
        .collect();
    push_str(&mut out, &tags);
    for arg in args {
        match arg {
            Arg::Int(n) => out.extend_from_slice(&n.to_be_bytes()),
            Arg::Float(f) => out.extend_from_slice(&f.to_be_bytes()),
            Arg::Str(s) => push_str(&mut out, s),
            Arg::Blob(b) => {
                out.extend_from_slice(&(b.len() as i32).to_be_bytes());
                out.extend_from_slice(b);
                pad(&mut out);
            }
            Arg::Int64(n) => out.extend_from_slice(&n.to_be_bytes()),
            Arg::Double(d) => out.extend_from_slice(&d.to_be_bytes()),
            Arg::True | Arg::False | Arg::Nil | Arg::Impulse => {}
        }
    }
    out
}

fn osc_string(value: &Value, what: &str) -> Result<String, String> {
    let s = value
        .as_str()
        .ok_or_else(|| format!("{what} must be a string"))?;
    if s.contains('\0') {
        return Err(format!("{what} contains a NUL, which ends an OSC-string"));
    }
    Ok(s.to_string())
}

/// One argument from JSON: a bare value (an integer is `i`, any other number
/// `f`, a string `s`, true/false `T`/`F`, null `N`) or `{type, value}`.
fn parse_arg(v: &Value, i: usize) -> Result<Arg, String> {
    let what = format!("args[{i}]");
    let int32 = |n: &Value| {
        n.as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| format!("{what} must be a 32-bit integer; use type int64 for more"))
    };
    match v {
        Value::Number(n) if n.is_i64() || n.is_u64() => int32(v).map(Arg::Int),
        Value::Number(n) => Ok(Arg::Float(n.as_f64().unwrap_or_default() as f32)),
        Value::String(_) => osc_string(v, &what).map(Arg::Str),
        Value::Bool(true) => Ok(Arg::True),
        Value::Bool(false) => Ok(Arg::False),
        Value::Null => Ok(Arg::Nil),
        Value::Object(o) => {
            let kind = o
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{what} needs a 'type'"))?;
            let value = o.get("value").unwrap_or(&Value::Null);
            let number = || {
                value
                    .as_f64()
                    .ok_or_else(|| format!("{what} needs a numeric 'value'"))
            };
            match kind {
                "int" | "i" => int32(value).map(Arg::Int),
                "float" | "f" => number().map(|n| Arg::Float(n as f32)),
                "string" | "s" => osc_string(value, &what).map(Arg::Str),
                "blob" | "b" => {
                    let hex = value
                        .as_str()
                        .ok_or_else(|| format!("{what} needs a hex string 'value'"))?;
                    parse_hex(hex)
                        .map(Arg::Blob)
                        .map_err(|e| format!("{what}: {e}"))
                }
                "true" | "T" => Ok(Arg::True),
                "false" | "F" => Ok(Arg::False),
                "nil" | "N" => Ok(Arg::Nil),
                "impulse" | "I" => Ok(Arg::Impulse),
                "int64" | "h" => value
                    .as_i64()
                    .map(Arg::Int64)
                    .ok_or_else(|| format!("{what} needs an integer 'value'")),
                "double" | "d" => number().map(Arg::Double),
                other => Err(format!(
                    "{what} has unknown type '{other}': int, float, string, blob, true, false, \
                     nil, impulse, int64 or double"
                )),
            }
        }
        Value::Array(_) => Err(format!("{what}: nested arrays are not sent")),
    }
}

pub(super) fn parse_args(v: Option<&Value>) -> Result<Vec<Arg>, String> {
    match v {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(i, v)| parse_arg(v, i))
            .collect(),
        Some(_) => Err("args must be an array".into()),
    }
}

/// A received message: address, type tags without the leading comma, and the
/// arguments as JSON (a blob, RGBA or MIDI as hex, nil and impulse as null).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Received {
    pub(super) address: String,
    pub(super) types: String,
    pub(super) args: Vec<Value>,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn string(&mut self) -> Option<String> {
        let rest = self.data.get(self.pos..)?;
        let end = rest.iter().position(|b| *b == 0)?;
        let s = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.pos += (end + 4) & !3;
        Some(s)
    }

    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let slice = self.data.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(slice)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }
}

/// A float32 as JSON, through its shortest decimal form, so 0.1 reads as 0.1
/// and not 0.10000000149011612. NaN and infinities have no JSON form: null.
fn float_json(f: f32) -> Value {
    f.to_string()
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map_or(Value::Null, Value::Number)
}

fn decode_message(packet: &[u8]) -> Option<Received> {
    let mut r = Reader {
        data: packet,
        pos: 0,
    };
    let address = r.string()?;
    if !address.starts_with('/') {
        return None;
    }
    let tags = if r.pos < packet.len() {
        r.string().unwrap_or_default()
    } else {
        String::new()
    };
    let types = tags.strip_prefix(',').unwrap_or("").to_string();
    // Arrays nest: the innermost open one receives each value.
    let mut stack: Vec<Vec<Value>> = vec![Vec::new()];
    for tag in types.chars() {
        let value = match tag {
            'i' => r.array::<4>().map(|b| json!(i32::from_be_bytes(b))),
            'f' => r.array::<4>().map(|b| float_json(f32::from_be_bytes(b))),
            's' | 'S' => r.string().map(Value::String),
            'b' => r.array::<4>().and_then(|n| {
                let n = usize::try_from(i32::from_be_bytes(n)).ok()?;
                let bytes = to_hex(r.take(n)?);
                r.pos = (r.pos + 3) & !3;
                Some(Value::String(bytes))
            }),
            'h' => r.array::<8>().map(|b| json!(i64::from_be_bytes(b))),
            't' => r.array::<8>().map(|b| json!(u64::from_be_bytes(b))),
            'd' => r.array::<8>().map(|b| {
                serde_json::Number::from_f64(f64::from_be_bytes(b))
                    .map_or(Value::Null, Value::Number)
            }),
            'c' => r.array::<4>().map(|b| {
                char::from_u32(u32::from_be_bytes(b))
                    .map_or(Value::Null, |c| Value::String(c.to_string()))
            }),
            'r' | 'm' => r.array::<4>().map(|b| Value::String(to_hex(&b))),
            'T' => Some(Value::Bool(true)),
            'F' => Some(Value::Bool(false)),
            'N' | 'I' => Some(Value::Null),
            '[' => {
                stack.push(Vec::new());
                continue;
            }
            ']' if stack.len() > 1 => {
                let inner = stack.pop().expect("checked");
                Some(Value::Array(inner))
            }
            _ => None,
        };
        match value {
            Some(v) => stack.last_mut().expect("never empty").push(v),
            None => break,
        }
    }
    // An array left open by a truncated message keeps what it got.
    while stack.len() > 1 {
        let inner = stack.pop().expect("checked");
        stack
            .last_mut()
            .expect("never empty")
            .push(Value::Array(inner));
    }
    Some(Received {
        address,
        types,
        args: stack.pop().expect("never empty"),
    })
}

/// Every message in a packet: a plain message, or the contents of a bundle.
pub(super) fn decode(packet: &[u8]) -> Vec<Received> {
    let mut out = Vec::new();
    decode_into(packet, &mut out, 0);
    out
}

fn decode_into(packet: &[u8], out: &mut Vec<Received>, depth: u32) {
    if packet.starts_with(b"#bundle\0") {
        if depth > 8 {
            return;
        }
        let mut r = Reader {
            data: packet,
            pos: 16,
        };
        while r.pos < packet.len() {
            let Some(size) = r.array::<4>() else { return };
            let Ok(size) = usize::try_from(i32::from_be_bytes(size)) else {
                return;
            };
            let Some(element) = r.take(size) else { return };
            decode_into(element, out, depth + 1);
        }
        return;
    }
    out.extend(decode_message(packet));
}

const SLIP_END: u8 = 0xC0;
const SLIP_ESC: u8 = 0xDB;
const SLIP_ESC_END: u8 = 0xDC;
const SLIP_ESC_ESC: u8 = 0xDD;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Framing {
    /// RFC 1055, an END byte at both ends (OSC 1.1).
    Slip,
    /// An int32 byte count before each packet (OSC 1.0).
    LengthPrefixed,
}

pub(super) fn frame(framing: Framing, packet: &[u8]) -> Vec<u8> {
    match framing {
        Framing::Slip => {
            let mut out = vec![SLIP_END];
            for &b in packet {
                match b {
                    SLIP_END => out.extend_from_slice(&[SLIP_ESC, SLIP_ESC_END]),
                    SLIP_ESC => out.extend_from_slice(&[SLIP_ESC, SLIP_ESC_ESC]),
                    b => out.push(b),
                }
            }
            out.push(SLIP_END);
            out
        }
        Framing::LengthPrefixed => {
            let mut out = (packet.len() as u32).to_be_bytes().to_vec();
            out.extend_from_slice(packet);
            out
        }
    }
}

/// Packets from a TCP stream.
#[derive(Debug)]
pub(super) struct Deframer {
    framing: Framing,
    buf: Vec<u8>,
}

impl Deframer {
    pub(super) fn new(framing: Framing) -> Deframer {
        Deframer {
            framing,
            buf: Vec::new(),
        }
    }

    /// The packets completed, or `Err` when the stream cannot be framed.
    pub(super) fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        match self.framing {
            Framing::Slip => {
                while let Some(end) = self.buf.iter().position(|b| *b == SLIP_END) {
                    let raw: Vec<u8> = self.buf.drain(..=end).collect();
                    let mut packet = Vec::new();
                    let mut escaped = false;
                    for &b in &raw[..raw.len() - 1] {
                        match (escaped, b) {
                            (true, SLIP_ESC_END) => packet.push(SLIP_END),
                            (true, SLIP_ESC_ESC) => packet.push(SLIP_ESC),
                            (true, other) => packet.push(other),
                            (false, SLIP_ESC) => {
                                escaped = true;
                                continue;
                            }
                            (false, other) => packet.push(other),
                        }
                        escaped = false;
                    }
                    if !packet.is_empty() {
                        out.push(packet);
                    }
                }
                if self.buf.len() > MAX_PACKET {
                    self.buf.clear();
                    return Err("over 1 MiB without a SLIP END byte".into());
                }
            }
            Framing::LengthPrefixed => {
                while self.buf.len() >= 4 {
                    let len = u32::from_be_bytes(self.buf[..4].try_into().expect("4 bytes"));
                    let len = len as usize;
                    if len > MAX_PACKET {
                        self.buf.clear();
                        return Err(format!("a packet length of {len} bytes"));
                    }
                    if self.buf.len() < 4 + len {
                        break;
                    }
                    let packet: Vec<u8> = self.buf.drain(..4 + len).skip(4).collect();
                    out.push(packet);
                }
            }
        }
        Ok(out)
    }

    fn reset(&mut self) {
        self.buf.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Transport {
    Udp,
    Tcp(Framing),
}

#[derive(Debug)]
struct Query {
    id: CommandId,
    address: String,
    deadline: Millis,
}

pub(crate) struct GenericOsc {
    device: SocketAddr,
    transport: Transport,
    listen_port: Option<u16>,
    record: bool,
    max_addresses: usize,
    /// Addresses in state, least recently received first, with their counts.
    order: VecDeque<String>,
    counts: HashMap<String, u64>,
    deframer: Deframer,
    ready: bool,
    retry_after: Millis,
    queries: Vec<Query>,
}

impl GenericOsc {
    pub(crate) fn new(ctx: OpenContext) -> Result<GenericOsc, String> {
        let port = ctx
            .port
            .ok_or("OSC has no standard port: give the port the device listens on")?;
        let s = &ctx.settings;
        let framing = match s.get("tcp_framing").and_then(Value::as_str) {
            None | Some("slip") => Framing::Slip,
            Some("length-prefixed") => Framing::LengthPrefixed,
            Some(other) => return Err(format!("unknown tcp_framing '{other}'")),
        };
        let transport = match s.get("transport").and_then(Value::as_str) {
            None | Some("udp") => Transport::Udp,
            Some("tcp") => Transport::Tcp(framing),
            Some(other) => return Err(format!("unknown transport '{other}'")),
        };
        let listen_port = s
            .get("listen_port")
            .and_then(Value::as_u64)
            .map(|p| p as u16);
        if listen_port.is_some() && transport != Transport::Udp {
            return Err("listen_port applies to UDP only".into());
        }
        Ok(GenericOsc {
            device: SocketAddr::new(ctx.host, port),
            transport,
            listen_port,
            record: s
                .get("record_messages")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            max_addresses: s
                .get("max_addresses")
                .and_then(Value::as_u64)
                .map_or(DEFAULT_MAX_ADDRESSES, |n| n as usize),
            order: VecDeque::new(),
            counts: HashMap::new(),
            deframer: Deframer::new(framing),
            ready: false,
            retry_after: RETRY_MIN,
            queries: Vec::new(),
        })
    }

    fn open(&mut self, cx: &mut Cx) {
        match self.transport {
            Transport::Tcp(_) => {
                cx.connection(Connection::Connecting);
                cx.tcp_open(SOCKET, self.device);
            }
            Transport::Udp => {
                // With a fixed port, it both sends and receives, so a device
                // answering the sender and one sending to a configured port
                // are both heard.
                let bind = self.listen_port.map_or(Bind::Ephemeral, Bind::Shared);
                cx.udp_open(SOCKET, bind);
                self.ready = true;
                cx.connection(Connection::Unmonitored);
            }
        }
    }

    fn send(&mut self, cx: &mut Cx, packet: Vec<u8>) {
        match self.transport {
            Transport::Udp => cx.udp_send(SOCKET, self.device, packet),
            Transport::Tcp(framing) => cx.tcp_send(SOCKET, frame(framing, &packet)),
        }
    }

    fn arm(&self, cx: &mut Cx) {
        match self.queries.iter().map(|q| q.deadline).min() {
            Some(at) => cx.set_timer(WAIT, at.saturating_sub(cx.now()).max(1)),
            None => cx.cancel_timer(WAIT),
        }
    }

    fn packet(&mut self, cx: &mut Cx, packet: &[u8]) {
        let messages = decode(packet);
        if messages.is_empty() {
            cx.log(
                Level::Debug,
                format!(
                    "not an OSC packet: {}",
                    to_hex(&packet[..packet.len().min(64)])
                ),
            );
        }
        for message in messages {
            self.received(cx, message);
        }
    }

    fn received(&mut self, cx: &mut Cx, m: Received) {
        let record = json!({"address": m.address, "types": m.types, "args": m.args});
        if let Some(at) = self.queries.iter().position(|q| q.address == m.address) {
            let query = self.queries.remove(at);
            cx.complete(
                query.id,
                Ok(Outcome::Value {
                    value: record.clone(),
                }),
            );
            self.arm(cx);
        }
        if !self.record {
            return;
        }
        let mut messages = Map::new();
        if let Some(at) = self.order.iter().position(|a| *a == m.address) {
            self.order.remove(at);
        } else if self.order.len() >= self.max_addresses {
            if let Some(oldest) = self.order.pop_front() {
                self.counts.remove(&oldest);
                messages.insert(oldest, Value::Null);
            }
        }
        self.order.push_back(m.address.clone());
        let count = self.counts.entry(m.address.clone()).or_insert(0);
        *count += 1;
        messages.insert(
            m.address,
            json!({"args": m.args, "types": m.types, "count": *count}),
        );
        cx.state(json!({"messages": messages, "last_message": record}));
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.ready = false;
        for q in self.queries.drain(..) {
            cx.complete(
                q.id,
                Err(CommandError::Transport {
                    message: reason.clone(),
                }),
            );
        }
        cx.cancel_timer(WAIT);
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }
}

impl Module for GenericOsc {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if name == "clear_messages" {
            self.order.clear();
            self.counts.clear();
            cx.state(json!({"messages": null, "last_message": null}));
            cx.complete(id, Ok(Outcome::Ack));
            return;
        }
        if name != "send" && name != "query" {
            cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            );
            return;
        }
        let address = params.get("address").and_then(Value::as_str).unwrap_or("");
        let args = match parse_args(params.get("args")) {
            Ok(args) => args,
            Err(message) => {
                cx.complete(id, Err(CommandError::InvalidParams { message }));
                return;
            }
        };
        if !self.ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        self.send(cx, encode(address, &args));
        if name == "send" {
            // OSC has no acknowledgement.
            cx.complete(id, Ok(Outcome::Unverified));
            return;
        }
        let reply = params
            .get("reply_address")
            .and_then(Value::as_str)
            .unwrap_or(address);
        let timeout = params
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(1_000);
        self.queries.push(Query {
            id,
            address: reply.to_string(),
            deadline: cx.now() + timeout,
        });
        self.arm(cx);
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, _from: SocketAddr, data: &[u8]) {
        cx.alive();
        self.packet(cx, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        cx.log(Level::Warning, format!("UDP socket: {message}"));
        self.lost(cx, message.to_string());
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.ready = true;
                self.retry_after = RETRY_MIN;
                self.deframer.reset();
                cx.connection(Connection::Connected);
            }
            TcpInput::Data(bytes) => {
                cx.alive();
                match self.deframer.feed(&bytes) {
                    Ok(packets) => {
                        for packet in packets {
                            self.packet(cx, &packet);
                        }
                    }
                    Err(problem) => {
                        let reason = format!("the stream is not framed as set: {problem}");
                        cx.log(Level::Warning, reason.clone());
                        cx.tcp_close(SOCKET);
                        self.lost(cx, reason);
                    }
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.open(cx),
            WAIT => {
                let now = cx.now();
                let (due, keep): (Vec<Query>, Vec<Query>) =
                    self.queries.drain(..).partition(|q| q.deadline <= now);
                self.queries = keep;
                for q in due {
                    cx.complete(q.id, Err(CommandError::Timeout));
                }
                self.arm(cx);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        match self.transport {
            Transport::Tcp(_) => cx.tcp_close(SOCKET),
            Transport::Udp => cx.udp_close(SOCKET),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn hex(bytes: &[u8]) -> String {
        to_hex(bytes)
    }

    fn module(settings: Value) -> GenericOsc {
        GenericOsc::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 20)),
            port: Some(8000),
            model: "osc".into(),
            channels: None,
            settings: params(settings),
        })
        .unwrap()
    }

    fn completion(actions: &[Action], id: CommandId) -> Option<crate::module::CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    // OSC 1.0 specification, "Examples": "/oscillator/4/frequency" with the
    // float 440.0, and "/foo" with 1000, -1, "hello", 1.234, 5.678.
    #[test]
    fn encodes_the_specification_examples() {
        let bytes = encode("/oscillator/4/frequency", &[Arg::Float(440.0)]);
        assert_eq!(
            hex(&bytes),
            "2f6f7363696c6c61746f722f342f6672657175656e6379002c66000043dc0000"
        );
        let bytes = encode(
            "/foo",
            &[
                Arg::Int(1000),
                Arg::Int(-1),
                Arg::Str("hello".into()),
                Arg::Float(1.234),
                Arg::Float(5.678),
            ],
        );
        assert_eq!(
            hex(&bytes),
            "2f666f6f000000002c69697366660000000003e8ffffffff68656c6c6f0000003f9df3b640b5b22d"
        );
    }

    #[test]
    fn encodes_every_type() {
        let args = parse_args(Some(&json!([
            1, 0.5, "x", true, false, null,
            {"type": "blob", "value": "01 02 03"},
            {"type": "impulse"},
            {"type": "int64", "value": 5_000_000_000_i64},
            {"type": "double", "value": 0.25},
            {"type": "float", "value": 2},
            {"type": "i", "value": -3}
        ])))
        .unwrap();
        let bytes = encode("/t", &args);
        assert_eq!(
            hex(&bytes),
            concat!(
                "2f740000",                             // "/t"
                "2c6966735446 4e62 4968 6466 69000000", // ",ifsTFNbIhdfi"
            )
            .replace(' ', "")
                + concat!(
                    "00000001",
                    "3f000000",
                    "78000000", // 1, 0.5, "x"
                    "00000003",
                    "01020300",         // blob 010203
                    "000000012a05f200", // int64
                    "3fd0000000000000", // double 0.25
                    "40000000",
                    "fffffffd" // float 2, int -3
                )
        );
        let back = &decode(&bytes)[0];
        assert_eq!(back.types, "ifsTFNbIhdfi");
        assert_eq!(
            Value::Array(back.args.clone()),
            json!([
                1,
                0.5,
                "x",
                true,
                false,
                null,
                "010203",
                null,
                5_000_000_000_i64,
                0.25,
                2.0,
                -3
            ])
        );
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(parse_args(Some(&json!([5_000_000_000_i64]))).is_err());
        assert!(parse_args(Some(&json!(["a\u{0}b"]))).is_err());
        assert!(parse_args(Some(&json!([{"type": "blob", "value": "abc"}]))).is_err());
        assert!(parse_args(Some(&json!([{"type": "quaternion", "value": 1}]))).is_err());
        assert!(parse_args(Some(&json!([[1]]))).is_err());
        assert!(parse_args(Some(&json!({"a": 1}))).is_err());
        assert_eq!(parse_args(None).unwrap(), vec![]);
    }

    #[test]
    fn decodes_bundles_arrays_and_untagged_messages() {
        let a = encode("/a", &[Arg::Int(1)]);
        let b = encode("/b", &[Arg::Str("two".into())]);
        let mut bundle = b"#bundle\0".to_vec();
        bundle.extend_from_slice(&1u64.to_be_bytes());
        for m in [&a, &b] {
            bundle.extend_from_slice(&(m.len() as i32).to_be_bytes());
            bundle.extend_from_slice(m);
        }
        let got = decode(&bundle);
        assert_eq!(got.len(), 2);
        assert_eq!(got[1].address, "/b");
        assert_eq!(got[1].args, vec![json!("two")]);

        // ",i[if]" with 1, [2, 0.5].
        let mut m = Vec::new();
        push_str(&mut m, "/arr");
        push_str(&mut m, ",i[if]");
        for word in [1i32, 2] {
            m.extend_from_slice(&word.to_be_bytes());
        }
        m.extend_from_slice(&0.5f32.to_be_bytes());
        let got = &decode(&m)[0];
        assert_eq!(got.args, vec![json!(1), json!([2, 0.5])]);

        let mut m = Vec::new();
        push_str(&mut m, "/ping");
        assert_eq!(decode(&m)[0].args, Vec::<Value>::new());
        // Not OSC.
        assert!(decode(b"hello\0\0\0").is_empty());
        // Floats read as their shortest decimal form.
        let m = encode("/f", &[Arg::Float(0.1)]);
        assert_eq!(decode(&m)[0].args, vec![json!(0.1)]);
    }

    #[test]
    fn tcp_framings_round_trip() {
        let packet = vec![1, SLIP_END, 2, SLIP_ESC, 3];
        for framing in [Framing::Slip, Framing::LengthPrefixed] {
            let wire = frame(framing, &packet);
            let mut d = Deframer::new(framing);
            let (head, tail) = wire.split_at(3);
            assert!(d.feed(head).unwrap().is_empty());
            assert_eq!(d.feed(tail).unwrap(), vec![packet.clone()]);
        }
        assert_eq!(
            frame(Framing::Slip, &packet),
            vec![
                SLIP_END,
                1,
                SLIP_ESC,
                SLIP_ESC_END,
                2,
                SLIP_ESC,
                SLIP_ESC_ESC,
                3,
                SLIP_END
            ]
        );
        let mut d = Deframer::new(Framing::LengthPrefixed);
        assert!(d.feed(&u32::MAX.to_be_bytes()).is_err());
    }

    #[test]
    fn records_messages_and_answers_queries() {
        let mut m = module(json!({"max_addresses": 2}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Ephemeral
        }));
        assert!(a.contains(&Action::Connection(Connection::Unmonitored)));

        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "query",
            &params(json!({"address": "/ch/01/config/name", "timeout_ms": 300})),
        );
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(x, Action::UdpSend { data, .. }
            if *data == encode("/ch/01/config/name", &[]))));

        let from = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 20)), 8000);
        let mut state = json!({});
        let mut feed = |m: &mut GenericOsc, at: Millis, packet: Vec<u8>| {
            let mut cx = Cx::new(at);
            m.datagram(&mut cx, SOCKET, from, &packet);
            let a = cx.take();
            for x in &a {
                if let Action::State(p) = x {
                    merge_patch(&mut state, p);
                }
            }
            (a, state.clone())
        };
        let (a, _) = feed(&mut m, 20, encode("/other", &[Arg::Int(1)]));
        assert_eq!(completion(&a, 1), None);
        let (a, s) = feed(
            &mut m,
            30,
            encode("/ch/01/config/name", &[Arg::Str("Vox".into())]),
        );
        assert_eq!(
            completion(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!({"address": "/ch/01/config/name", "types": "s", "args": ["Vox"]})
            }))
        );
        assert_eq!(s["messages"]["/ch/01/config/name"]["args"], json!(["Vox"]));
        assert_eq!(s["last_message"]["address"], "/ch/01/config/name");

        // A third address evicts the least recently received.
        let (_, s) = feed(&mut m, 40, encode("/other", &[Arg::Int(2)]));
        assert_eq!(s["messages"]["/other"]["count"], 2);
        let (_, s) = feed(&mut m, 50, encode("/third", &[]));
        assert!(s["messages"].get("/ch/01/config/name").is_none());
        assert_eq!(s["messages"].as_object().unwrap().len(), 2);

        // An unanswered query times out.
        let mut cx = Cx::new(60);
        m.command(
            &mut cx,
            2,
            "query",
            &params(json!({"address": "/x", "reply_address": "/x/reply", "timeout_ms": 100})),
        );
        assert!(cx.take().contains(&Action::SetTimer {
            key: WAIT,
            after: 100
        }));
        let mut cx = Cx::new(160);
        m.timer(&mut cx, WAIT);
        assert_eq!(completion(&cx.take(), 2), Some(Err(CommandError::Timeout)));

        let mut cx = Cx::new(170);
        m.command(&mut cx, 3, "clear_messages", &Params::new());
        assert!(cx.take().contains(&Action::State(
            json!({"messages": null, "last_message": null})
        )));
    }

    #[test]
    fn tcp_sends_framed_and_reconnects() {
        let mut m = module(json!({"transport": "tcp", "tcp_framing": "length-prefixed"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx
            .take()
            .iter()
            .any(|x| matches!(x, Action::TcpOpen { .. })));
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "send", &params(json!({"address": "/go"})));
        assert_eq!(
            completion(&cx.take(), 1),
            Some(Err(CommandError::NotConnected))
        );
        let mut cx = Cx::new(5);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        m.command(
            &mut cx,
            2,
            "send",
            &params(json!({"address": "/go", "args": [1]})),
        );
        let a = cx.take();
        let packet = encode("/go", &[Arg::Int(1)]);
        assert!(a.contains(&Action::TcpSend {
            socket: SOCKET,
            data: frame(Framing::LengthPrefixed, &packet)
        }));
        assert_eq!(completion(&a, 2), Some(Ok(Outcome::Unverified)));

        let mut cx = Cx::new(10);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(frame(Framing::LengthPrefixed, &encode("/eos/out/x", &[]))),
        );
        assert!(cx.take().iter().any(|x| matches!(x, Action::State(p)
            if p["last_message"]["address"] == "/eos/out/x")));

        let mut cx = Cx::new(20);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "eof".into(),
            },
        );
        assert!(cx.take().contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
    }

    #[test]
    fn listen_port_is_for_udp() {
        let m = module(json!({"listen_port": 9000}));
        assert_eq!(m.listen_port, Some(9000));
        let tcp = GenericOsc::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: Some(1),
            model: "osc".into(),
            channels: None,
            settings: params(json!({"transport": "tcp", "listen_port": 9000})),
        });
        assert!(tcp.is_err());
    }
}
