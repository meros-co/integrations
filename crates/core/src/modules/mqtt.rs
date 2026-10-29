//! A generic MQTT client for one broker, MQTT 3.1.1 (OASIS Standard, 2014)
//! or MQTT 5.0 (OASIS Standard, 2019), over TCP, TLS or a WebSocket.
//!
//! - On connecting the client sends CONNECT (client id, credentials, keep
//!   alive, clean session or clean start, the will, and with version 5 the
//!   session expiry) and waits for CONNACK. A refusal of the credential
//!   (3.1.1 return codes 4 and 5; version 5 reason codes 86h, 87h, 8Ah and
//!   8Ch) is terminal: the device is unauthorized and not retried until its
//!   settings change. Other refusals and lost connections are retried after
//!   1 s, doubling to 30 s.
//! - The `subscriptions` setting's filters are subscribed after each
//!   CONNACK. Every message received is kept under `topics.<topic>` (its
//!   payload as text, or base64 when it is not UTF-8, its QoS, the retain
//!   flag, and with version 5 its content type) and reported as a message
//!   event, since a repeated payload changes no state. QoS 1 messages are
//!   acknowledged with PUBACK, QoS 2 with PUBREC and, on PUBREL, PUBCOMP,
//!   and delivered once.
//! - `publish` sends at QoS 0 (unverified), 1 (answered by PUBACK) or 2
//!   (PUBREC, PUBREL, PUBCOMP); `subscribe` and `unsubscribe` are answered
//!   by SUBACK and UNSUBACK, and a filter subscribed by command is
//!   subscribed again after a reconnection.
//! - PINGREQ goes every three quarters of the keep alive (the server's, when
//!   a version 5 CONNACK gives one); a PINGRESP not back by the next one
//!   drops the connection. Every answer to a request reports the round trip.
//!
//! Opened for commands only (`monitor` false), the `subscriptions` setting
//! is not subscribed; the connection, its keep alive pings (which the
//! protocol requires) and commands work as usual, and messages the broker
//! delivers anyway (a persistent session's subscriptions) still reach
//! state.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};

use base64::Engine as _;
use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput, TlsTarget, WsInput, WsRequest,
};

pub(crate) const PORT: u16 = 1883;
pub(crate) const TLS_PORT: u16 = 8883;
const SOCKET: Key = "mqtt";
const PING_TIMER: Key = "ping";
const CONNACK_TIMER: Key = "connack";
const RETRY_TIMER: Key = "retry";
const ACK_TIMER: Key = "ack";
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const CONNACK_TIMEOUT: Millis = 10_000;

const CONNECT: u8 = 1;
const CONNACK: u8 = 2;
const PUBLISH: u8 = 3;
const PUBACK: u8 = 4;
const PUBREC: u8 = 5;
const PUBREL: u8 = 6;
const PUBCOMP: u8 = 7;
const SUBSCRIBE: u8 = 8;
const SUBACK: u8 = 9;
const UNSUBSCRIBE: u8 = 10;
const UNSUBACK: u8 = 11;
const PINGREQ: u8 = 12;
const PINGRESP: u8 = 13;
const DISCONNECT: u8 = 14;
const AUTH: u8 = 15;

/// Version 5 property identifiers (MQTT 5.0 §2.2.2.2).
mod prop {
    pub const PAYLOAD_FORMAT: u8 = 0x01;
    pub const MESSAGE_EXPIRY: u8 = 0x02;
    pub const CONTENT_TYPE: u8 = 0x03;
    pub const RESPONSE_TOPIC: u8 = 0x08;
    pub const CORRELATION_DATA: u8 = 0x09;
    pub const SUBSCRIPTION_ID: u8 = 0x0B;
    pub const SESSION_EXPIRY: u8 = 0x11;
    pub const ASSIGNED_CLIENT_ID: u8 = 0x12;
    pub const SERVER_KEEP_ALIVE: u8 = 0x13;
    pub const AUTH_METHOD: u8 = 0x15;
    pub const AUTH_DATA: u8 = 0x16;
    pub const REQUEST_PROBLEM: u8 = 0x17;
    pub const WILL_DELAY: u8 = 0x18;
    pub const REQUEST_RESPONSE: u8 = 0x19;
    pub const RESPONSE_INFO: u8 = 0x1A;
    pub const SERVER_REFERENCE: u8 = 0x1C;
    pub const REASON_STRING: u8 = 0x1F;
    pub const RECEIVE_MAXIMUM: u8 = 0x21;
    pub const TOPIC_ALIAS_MAXIMUM: u8 = 0x22;
    pub const TOPIC_ALIAS: u8 = 0x23;
    pub const MAXIMUM_QOS: u8 = 0x24;
    pub const RETAIN_AVAILABLE: u8 = 0x25;
    pub const USER_PROPERTY: u8 = 0x26;
    pub const MAXIMUM_PACKET_SIZE: u8 = 0x27;
    pub const WILDCARD_AVAILABLE: u8 = 0x28;
    pub const SUB_ID_AVAILABLE: u8 = 0x29;
    pub const SHARED_AVAILABLE: u8 = 0x2A;
}

/// A version 5 property value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Prop {
    Int(u32),
    Text(String),
    Bytes(Vec<u8>),
    Pair(String, String),
}

/// The variable byte integer (MQTT 3.1.1 §2.2.3, 5.0 §1.5.5).
pub(crate) fn varint(mut n: usize, out: &mut Vec<u8>) {
    loop {
        let mut byte = (n % 128) as u8;
        n /= 128;
        if n > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if n == 0 {
            break;
        }
    }
}

/// A variable byte integer at the start of `data`: its value and length.
fn read_varint(data: &[u8]) -> Option<(usize, usize)> {
    let mut value = 0usize;
    for (i, &b) in data.iter().enumerate().take(4) {
        value += usize::from(b & 0x7F) << (7 * i);
        if b & 0x80 == 0 {
            return Some((value, i + 1));
        }
    }
    None
}

fn string(s: &str, out: &mut Vec<u8>) {
    binary(s.as_bytes(), out);
}

fn binary(b: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&(b.len() as u16).to_be_bytes());
    out.extend_from_slice(b);
}

/// A control packet: the fixed header and the rest.
pub(crate) fn packet(kind: u8, flags: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![(kind << 4) | (flags & 0x0F)];
    varint(body.len(), &mut out);
    out.extend_from_slice(body);
    out
}

/// A version 5 property list, its length first.
fn properties(props: &[(u8, Prop)], out: &mut Vec<u8>) {
    let mut body = Vec::new();
    for (id, value) in props {
        body.push(*id);
        match value {
            Prop::Int(n) => match *id {
                prop::PAYLOAD_FORMAT
                | prop::REQUEST_PROBLEM
                | prop::REQUEST_RESPONSE
                | prop::MAXIMUM_QOS
                | prop::RETAIN_AVAILABLE
                | prop::WILDCARD_AVAILABLE
                | prop::SUB_ID_AVAILABLE
                | prop::SHARED_AVAILABLE => body.push(*n as u8),
                prop::SERVER_KEEP_ALIVE
                | prop::RECEIVE_MAXIMUM
                | prop::TOPIC_ALIAS_MAXIMUM
                | prop::TOPIC_ALIAS => body.extend_from_slice(&(*n as u16).to_be_bytes()),
                prop::SUBSCRIPTION_ID => varint(*n as usize, &mut body),
                _ => body.extend_from_slice(&n.to_be_bytes()),
            },
            Prop::Text(s) => string(s, &mut body),
            Prop::Bytes(b) => binary(b, &mut body),
            Prop::Pair(k, v) => {
                string(k, &mut body);
                string(v, &mut body);
            }
        }
    }
    varint(body.len(), out);
    out.extend(body);
}

/// A version 5 property list at the start of `data`, and the bytes it took.
fn read_properties(data: &[u8]) -> Option<(Vec<(u8, Prop)>, usize)> {
    let (len, head) = read_varint(data)?;
    let mut list = data.get(head..head + len)?;
    let mut out = Vec::new();
    let text = |d: &[u8]| -> Option<(String, usize)> {
        let n = usize::from(u16::from_be_bytes([*d.first()?, *d.get(1)?]));
        Some((
            String::from_utf8_lossy(d.get(2..2 + n)?).into_owned(),
            2 + n,
        ))
    };
    while let Some(&id) = list.first() {
        let rest = &list[1..];
        let (value, used) = match id {
            prop::PAYLOAD_FORMAT
            | prop::REQUEST_PROBLEM
            | prop::REQUEST_RESPONSE
            | prop::MAXIMUM_QOS
            | prop::RETAIN_AVAILABLE
            | prop::WILDCARD_AVAILABLE
            | prop::SUB_ID_AVAILABLE
            | prop::SHARED_AVAILABLE => (Prop::Int(u32::from(*rest.first()?)), 1),
            prop::SERVER_KEEP_ALIVE
            | prop::RECEIVE_MAXIMUM
            | prop::TOPIC_ALIAS_MAXIMUM
            | prop::TOPIC_ALIAS => (
                Prop::Int(u32::from(u16::from_be_bytes([
                    *rest.first()?,
                    *rest.get(1)?,
                ]))),
                2,
            ),
            prop::MESSAGE_EXPIRY
            | prop::SESSION_EXPIRY
            | prop::WILL_DELAY
            | prop::MAXIMUM_PACKET_SIZE => (
                Prop::Int(u32::from_be_bytes(rest.get(..4)?.try_into().ok()?)),
                4,
            ),
            prop::SUBSCRIPTION_ID => {
                let (v, n) = read_varint(rest)?;
                (Prop::Int(v as u32), n)
            }
            prop::CONTENT_TYPE
            | prop::RESPONSE_TOPIC
            | prop::ASSIGNED_CLIENT_ID
            | prop::AUTH_METHOD
            | prop::RESPONSE_INFO
            | prop::SERVER_REFERENCE
            | prop::REASON_STRING => {
                let (s, n) = text(rest)?;
                (Prop::Text(s), n)
            }
            prop::CORRELATION_DATA | prop::AUTH_DATA => {
                let n = usize::from(u16::from_be_bytes([*rest.first()?, *rest.get(1)?]));
                (Prop::Bytes(rest.get(2..2 + n)?.to_vec()), 2 + n)
            }
            prop::USER_PROPERTY => {
                let (k, a) = text(rest)?;
                let (v, b) = text(rest.get(a..)?)?;
                (Prop::Pair(k, v), a + b)
            }
            _ => return None,
        };
        out.push((id, value));
        list = rest.get(used..)?;
    }
    Some((out, head + len))
}

/// Splits the byte stream into (type and flags byte, body).
#[derive(Debug, Default)]
struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    /// `Err` when a length is malformed and the stream must be dropped.
    fn feed(&mut self, bytes: &[u8]) -> Result<Vec<(u8, Vec<u8>)>, ()> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while self.buf.len() >= 2 {
            let Some((len, n)) = read_varint(&self.buf[1..]) else {
                if self.buf.len() >= 5 {
                    return Err(());
                }
                break;
            };
            if self.buf.len() < 1 + n + len {
                break;
            }
            let head = self.buf[0];
            let body = self.buf[1 + n..1 + n + len].to_vec();
            self.buf.drain(..1 + n + len);
            out.push((head, body));
        }
        Ok(out)
    }
}

/// What a reason code means (MQTT 5.0 §2.4; 3.1.1's CONNACK return codes
/// and the SUBACK failure 80h map onto it).
pub(crate) fn reason_text(code: u8) -> &'static str {
    match code {
        0x00 => "success",
        0x01 => "granted QoS 1",
        0x02 => "granted QoS 2",
        0x04 => "disconnect with will message",
        0x10 => "no matching subscribers",
        0x11 => "no subscription existed",
        0x18 => "continue authentication",
        0x19 => "re-authenticate",
        0x80 => "unspecified error",
        0x81 => "malformed packet",
        0x82 => "protocol error",
        0x83 => "implementation specific error",
        0x84 => "unsupported protocol version",
        0x85 => "client identifier not valid",
        0x86 => "bad user name or password",
        0x87 => "not authorized",
        0x88 => "server unavailable",
        0x89 => "server busy",
        0x8A => "banned",
        0x8B => "server shutting down",
        0x8C => "bad authentication method",
        0x8D => "keep alive timeout",
        0x8E => "session taken over",
        0x8F => "topic filter invalid",
        0x90 => "topic name invalid",
        0x91 => "packet identifier in use",
        0x92 => "packet identifier not found",
        0x93 => "receive maximum exceeded",
        0x94 => "topic alias invalid",
        0x95 => "packet too large",
        0x96 => "message rate too high",
        0x97 => "quota exceeded",
        0x98 => "administrative action",
        0x99 => "payload format invalid",
        0x9A => "retain not supported",
        0x9B => "QoS not supported",
        0x9C => "use another server",
        0x9D => "server moved",
        0x9E => "shared subscriptions not supported",
        0x9F => "connection rate exceeded",
        0xA0 => "maximum connect time",
        0xA1 => "subscription identifiers not supported",
        0xA2 => "wildcard subscriptions not supported",
        _ => "unknown reason",
    }
}

/// A 3.1.1 CONNACK return code as the version 5 reason code meaning the
/// same (MQTT 3.1.1 §3.2.2.3).
fn connack_311(code: u8) -> u8 {
    match code {
        0 => 0x00,
        1 => 0x84,
        2 => 0x85,
        3 => 0x88,
        4 => 0x86,
        5 => 0x87,
        _ => 0x80,
    }
}

/// How the module reaches the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transport {
    Tcp,
    Tls,
    Ws,
    Wss,
}

/// A subscription: its filter and options byte (QoS in bits 0-1; version
/// 5's no local, retain as published and retain handling above).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Subscription {
    pub filter: String,
    pub options: u8,
}

/// The `subscriptions` setting: filters separated by commas or line
/// breaks, each optionally `@qos`.
pub(crate) fn parse_subscriptions(text: &str) -> Result<Vec<Subscription>, String> {
    let mut out = Vec::new();
    for item in text
        .split([',', '\n', '\r'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let (filter, qos) = match item.rsplit_once('@') {
            Some((f, q)) if matches!(q.trim(), "0" | "1" | "2") => {
                (f.trim(), q.trim().parse::<u8>().unwrap_or(0))
            }
            _ => (item, 0),
        };
        check_filter(filter)?;
        out.push(Subscription {
            filter: filter.to_string(),
            options: qos,
        });
    }
    Ok(out)
}

/// A topic filter's wildcards are whole levels, and `#` is last
/// (MQTT 3.1.1 §4.7.1).
fn check_filter(filter: &str) -> Result<(), String> {
    if filter.is_empty() {
        return Err("a topic filter is not empty".into());
    }
    let levels: Vec<&str> = filter.split('/').collect();
    for (i, level) in levels.iter().enumerate() {
        if level.contains('#') && (*level != "#" || i + 1 != levels.len()) {
            return Err(format!("'{filter}': # is a whole level, and the last"));
        }
        if level.contains('+') && *level != "+" {
            return Err(format!("'{filter}': + is a whole level"));
        }
    }
    Ok(())
}

fn check_topic(topic: &str) -> Result<(), String> {
    if topic.is_empty() {
        return Err("the topic is not empty".into());
    }
    if topic.contains(['#', '+']) {
        return Err("a topic name holds no wildcards".into());
    }
    Ok(())
}

/// What a packet identifier waits for.
#[derive(Debug, Clone, PartialEq)]
enum Waiting {
    PubAck,
    PubRec,
    PubComp,
    SubAck(Vec<String>),
    UnsubAck(Vec<String>),
}

#[derive(Debug, Clone)]
struct Pending {
    waiting: Waiting,
    command: Option<CommandId>,
    sent_at: Millis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Closed,
    Opening,
    /// CONNECT sent, waiting for CONNACK.
    Connecting,
    Ready,
    /// The credential was refused: nothing until the settings change.
    Refused,
}

pub(crate) struct Mqtt {
    host: IpAddr,
    host_name: Option<String>,
    port: u16,
    transport: Transport,
    ws_path: String,
    accept_invalid_certs: bool,
    v5: bool,
    client_id: String,
    username: String,
    password: String,
    clean: bool,
    session_expiry: u32,
    keepalive: u16,
    timeout: Millis,
    will: Option<(String, Vec<u8>, u8, bool)>,
    configured: Vec<Subscription>,
    /// Filters subscribed by command, subscribed again on reconnecting.
    added: Vec<Subscription>,
    monitor: bool,
    phase: Phase,
    deframer: Deframer,
    next_id: u16,
    pending: HashMap<u16, Pending>,
    /// QoS 2 messages received and not yet released, by packet identifier.
    inbound: HashSet<u16>,
    ping_sent: Option<Millis>,
    retry_after: Millis,
    /// The server's keep alive, when a version 5 CONNACK gives one.
    server_keepalive: Option<u16>,
}

fn text(s: &Params, key: &str) -> String {
    s.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

impl Mqtt {
    pub(crate) fn new(ctx: OpenContext) -> Result<Mqtt, String> {
        if ctx.host.is_unspecified() && ctx.host_name.is_none() {
            return Err("give the broker's address as host".into());
        }
        let s = &ctx.settings;
        let transport = match s.get("transport").and_then(Value::as_str) {
            Some("tls") => Transport::Tls,
            Some("ws") => Transport::Ws,
            Some("wss") => Transport::Wss,
            _ => Transport::Tcp,
        };
        let default_port = match transport {
            Transport::Tcp => PORT,
            Transport::Tls => TLS_PORT,
            Transport::Ws => s.get("ws_port").and_then(Value::as_u64).unwrap_or(80) as u16,
            Transport::Wss => s.get("wss_port").and_then(Value::as_u64).unwrap_or(443) as u16,
        };
        let mut ws_path = text(s, "ws_path");
        if !ws_path.starts_with('/') {
            ws_path.insert(0, '/');
        }
        let int = |k: &str, d: u64| s.get(k).and_then(Value::as_u64).unwrap_or(d);
        let will_topic = text(s, "will_topic");
        let will = if will_topic.is_empty() {
            None
        } else {
            check_topic(&will_topic)?;
            Some((
                will_topic,
                text(s, "will_payload").into_bytes(),
                int("will_qos", 0).min(2) as u8,
                s.get("will_retain")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ))
        };
        let v5 = s.get("version").and_then(Value::as_str) == Some("5");
        Ok(Mqtt {
            host: ctx.host,
            host_name: ctx.host_name.clone(),
            port: ctx.port.unwrap_or(default_port),
            transport,
            ws_path,
            accept_invalid_certs: s
                .get("accept_invalid_certs")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            v5,
            client_id: text(s, "client_id"),
            username: text(s, "username"),
            password: text(s, "password"),
            clean: s
                .get("clean_start")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            session_expiry: int("session_expiry_s", 0).min(u64::from(u32::MAX)) as u32,
            keepalive: int("keepalive_s", 30).min(65535) as u16,
            timeout: int("timeout_ms", 10_000).max(1),
            will,
            configured: parse_subscriptions(&text(s, "subscriptions"))?,
            added: Vec::new(),
            monitor: ctx.monitor,
            phase: Phase::Closed,
            deframer: Deframer::default(),
            next_id: 0,
            pending: HashMap::new(),
            inbound: HashSet::new(),
            ping_sent: None,
            retry_after: RETRY_MIN,
            server_keepalive: None,
        })
    }

    fn host_text(&self) -> String {
        match (&self.host_name, self.host) {
            (Some(n), _) => n.clone(),
            (None, IpAddr::V6(a)) => format!("[{a}]"),
            (None, a) => a.to_string(),
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        self.phase = Phase::Opening;
        self.deframer = Deframer::default();
        cx.connection(Connection::Connecting);
        let to = SocketAddr::new(self.host, self.port);
        let server_name = self
            .host_name
            .clone()
            .unwrap_or_else(|| self.host.to_string());
        match self.transport {
            Transport::Tcp => cx.tcp_open(SOCKET, to),
            Transport::Tls => cx.tcp_open_tls(
                SOCKET,
                TlsTarget {
                    to,
                    server_name,
                    accept_invalid_certs: self.accept_invalid_certs,
                    preamble: None,
                    alpn: Vec::new(),
                },
            ),
            Transport::Ws | Transport::Wss => {
                let scheme = if self.transport == Transport::Ws {
                    "ws"
                } else {
                    "wss"
                };
                cx.ws_open(
                    SOCKET,
                    WsRequest {
                        url: format!(
                            "{scheme}://{}:{}{}",
                            self.host_text(),
                            self.port,
                            self.ws_path
                        ),
                        headers: vec![("Sec-WebSocket-Protocol".into(), "mqtt".into())],
                        accept_invalid_certs: self.accept_invalid_certs,
                    },
                );
            }
        }
    }

    fn send(&self, cx: &mut Cx, bytes: Vec<u8>) {
        match self.transport {
            Transport::Tcp | Transport::Tls => cx.tcp_send(SOCKET, bytes),
            Transport::Ws | Transport::Wss => cx.ws_send_binary(SOCKET, bytes),
        }
    }

    fn close_socket(&self, cx: &mut Cx) {
        match self.transport {
            Transport::Tcp | Transport::Tls => cx.tcp_close(SOCKET),
            Transport::Ws | Transport::Wss => cx.ws_close(SOCKET),
        }
    }

    /// CONNECT (3.1.1 §3.1, 5.0 §3.1).
    pub(crate) fn connect_packet(&self) -> Vec<u8> {
        let mut body = Vec::new();
        string("MQTT", &mut body);
        body.push(if self.v5 { 5 } else { 4 });
        let mut flags = 0u8;
        if !self.username.is_empty() {
            flags |= 0x80;
        }
        if !self.password.is_empty() {
            flags |= 0x40;
        }
        if let Some((_, _, qos, retain)) = &self.will {
            flags |= 0x04 | (qos << 3);
            if *retain {
                flags |= 0x20;
            }
        }
        // 3.1.1 refuses an empty client id without a clean session.
        if self.clean || self.client_id.is_empty() {
            flags |= 0x02;
        }
        body.push(flags);
        body.extend_from_slice(&self.keepalive.to_be_bytes());
        if self.v5 {
            let mut props = Vec::new();
            if self.session_expiry > 0 {
                props.push((prop::SESSION_EXPIRY, Prop::Int(self.session_expiry)));
            }
            properties(&props, &mut body);
        }
        string(&self.client_id, &mut body);
        if let Some((topic, payload, _, _)) = &self.will {
            if self.v5 {
                properties(&[], &mut body);
            }
            string(topic, &mut body);
            binary(payload, &mut body);
        }
        if !self.username.is_empty() {
            string(&self.username, &mut body);
        }
        if !self.password.is_empty() {
            binary(self.password.as_bytes(), &mut body);
        }
        packet(CONNECT, 0, &body)
    }

    fn packet_id(&mut self) -> u16 {
        loop {
            self.next_id = self.next_id.wrapping_add(1);
            if self.next_id != 0 && !self.pending.contains_key(&self.next_id) {
                return self.next_id;
            }
        }
    }

    fn subscribe(&mut self, cx: &mut Cx, subs: &[Subscription], command: Option<CommandId>) {
        if subs.is_empty() {
            return;
        }
        let id = self.packet_id();
        let mut body = id.to_be_bytes().to_vec();
        if self.v5 {
            properties(&[], &mut body);
        }
        for s in subs {
            string(&s.filter, &mut body);
            body.push(if self.v5 { s.options } else { s.options & 3 });
        }
        self.send(cx, packet(SUBSCRIBE, 0x02, &body));
        self.wait(
            cx,
            id,
            Waiting::SubAck(subs.iter().map(|s| s.filter.clone()).collect()),
            command,
        );
    }

    fn wait(&mut self, cx: &mut Cx, id: u16, waiting: Waiting, command: Option<CommandId>) {
        self.pending.insert(
            id,
            Pending {
                waiting,
                command,
                sent_at: cx.now(),
            },
        );
        cx.set_timer(ACK_TIMER, self.timeout);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if self.phase == Phase::Refused {
            return;
        }
        self.phase = Phase::Closed;
        self.ping_sent = None;
        self.inbound.clear();
        for (_, p) in self.pending.drain() {
            if let Some(id) = p.command {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        for t in [PING_TIMER, CONNACK_TIMER, ACK_TIMER] {
            cx.cancel_timer(t);
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn ping_every(&self) -> Option<Millis> {
        let k = self.server_keepalive.unwrap_or(self.keepalive);
        (k > 0).then(|| (Millis::from(k) * 1000 * 3 / 4).max(1))
    }

    fn received(&mut self, cx: &mut Cx, head: u8, body: Vec<u8>) {
        cx.alive();
        let kind = head >> 4;
        match kind {
            CONNACK => self.connack(cx, &body),
            PUBLISH => self.publish_in(cx, head, &body),
            PUBACK | PUBREC | PUBCOMP | SUBACK | UNSUBACK => self.ack(cx, kind, &body),
            PUBREL => {
                let id = u16::from_be_bytes([
                    body.first().copied().unwrap_or(0),
                    body.get(1).copied().unwrap_or(0),
                ]);
                self.inbound.remove(&id);
                self.send(cx, packet(PUBCOMP, 0, &id.to_be_bytes()));
            }
            PINGRESP => {
                if let Some(at) = self.ping_sent.take() {
                    cx.round_trip(cx.now().saturating_sub(at));
                }
            }
            DISCONNECT => {
                let code = body.first().copied().unwrap_or(0);
                let mut reason = format!("the broker disconnected: {}", reason_text(code));
                if let Some((props, _)) = body.get(1..).and_then(read_properties) {
                    for (id, v) in props {
                        match (id, v) {
                            (prop::REASON_STRING, Prop::Text(t)) => {
                                reason.push_str(&format!(" ({t})"));
                            }
                            (prop::SERVER_REFERENCE, Prop::Text(t)) => {
                                cx.state(json!({"server": {"server_reference": t}}));
                            }
                            _ => {}
                        }
                    }
                }
                cx.log(Level::Warning, reason.clone());
                self.close_socket(cx);
                self.lost(cx, reason);
            }
            AUTH => {
                cx.log(
                    Level::Warning,
                    "the broker asked for enhanced authentication (AUTH), which this client does not offer",
                );
            }
            _ => {}
        }
    }

    fn connack(&mut self, cx: &mut Cx, body: &[u8]) {
        cx.cancel_timer(CONNACK_TIMER);
        let session_present = body.first().is_some_and(|f| f & 1 == 1);
        let raw = body.get(1).copied().unwrap_or(0x80);
        let code = if self.v5 { raw } else { connack_311(raw) };
        let mut server = Map::new();
        let mut assigned = None;
        let mut reason_string = None;
        if self.v5 {
            if let Some((props, _)) = body.get(2..).and_then(read_properties) {
                for (id, v) in props {
                    let name = match id {
                        prop::SESSION_EXPIRY => "session_expiry",
                        prop::RECEIVE_MAXIMUM => "receive_maximum",
                        prop::MAXIMUM_QOS => "maximum_qos",
                        prop::RETAIN_AVAILABLE => "retain_available",
                        prop::MAXIMUM_PACKET_SIZE => "maximum_packet_size",
                        prop::TOPIC_ALIAS_MAXIMUM => "topic_alias_maximum",
                        prop::WILDCARD_AVAILABLE => "wildcard_subscription_available",
                        prop::SUB_ID_AVAILABLE => "subscription_identifiers_available",
                        prop::SHARED_AVAILABLE => "shared_subscription_available",
                        prop::SERVER_KEEP_ALIVE => "server_keep_alive",
                        prop::RESPONSE_INFO => "response_information",
                        prop::SERVER_REFERENCE => "server_reference",
                        prop::ASSIGNED_CLIENT_ID => {
                            if let Prop::Text(t) = v {
                                assigned = Some(t);
                            }
                            continue;
                        }
                        prop::REASON_STRING => {
                            if let Prop::Text(t) = v {
                                reason_string = Some(t);
                            }
                            continue;
                        }
                        _ => continue,
                    };
                    let value = match (name, v) {
                        (
                            "retain_available"
                            | "wildcard_subscription_available"
                            | "subscription_identifiers_available"
                            | "shared_subscription_available",
                            Prop::Int(n),
                        ) => json!(n == 1),
                        (_, Prop::Int(n)) => json!(n),
                        (_, Prop::Text(t)) => json!(t),
                        _ => continue,
                    };
                    if name == "server_keep_alive" {
                        self.server_keepalive = value.as_u64().map(|k| k as u16);
                    }
                    server.insert(name.into(), value);
                }
            }
        }
        if code >= 0x80 {
            let mut reason = format!("the broker refused the connection: {}", reason_text(code));
            if let Some(t) = reason_string {
                reason.push_str(&format!(" ({t})"));
            }
            self.close_socket(cx);
            if matches!(code, 0x86 | 0x87 | 0x8A | 0x8C) {
                self.phase = Phase::Refused;
                for (_, p) in self.pending.drain() {
                    if let Some(id) = p.command {
                        cx.complete(
                            id,
                            Err(CommandError::Auth {
                                message: reason.clone(),
                            }),
                        );
                    }
                }
                cx.connection(Connection::Unauthorized { reason });
            } else {
                self.lost(cx, reason);
            }
            return;
        }
        self.phase = Phase::Ready;
        self.retry_after = RETRY_MIN;
        let client_id = assigned.unwrap_or_else(|| self.client_id.clone());
        let mut patch = json!({
            "session": {"present": session_present, "client_id": client_id},
            "version": if self.v5 { "5" } else { "3.1.1" },
        });
        if !server.is_empty() {
            patch["server"] = Value::Object(server);
        }
        cx.state(patch);
        cx.connection(Connection::Connected);
        if let Some(every) = self.ping_every() {
            cx.set_timer(PING_TIMER, every);
        }
        let mut subs = if self.monitor {
            self.configured.clone()
        } else {
            Vec::new()
        };
        for s in &self.added {
            if !subs.iter().any(|c| c.filter == s.filter) {
                subs.push(s.clone());
            }
        }
        self.subscribe(cx, &subs, None);
    }

    fn publish_in(&mut self, cx: &mut Cx, head: u8, body: &[u8]) {
        let qos = (head >> 1) & 3;
        let retain = head & 1 == 1;
        let Some(len) = body
            .get(..2)
            .map(|b| usize::from(u16::from_be_bytes([b[0], b[1]])))
        else {
            return;
        };
        let Some(topic) = body.get(2..2 + len) else {
            return;
        };
        let topic = String::from_utf8_lossy(topic).into_owned();
        let mut at = 2 + len;
        let mut id = None;
        if qos > 0 {
            let Some(b) = body.get(at..at + 2) else {
                return;
            };
            id = Some(u16::from_be_bytes([b[0], b[1]]));
            at += 2;
        }
        let mut props = Vec::new();
        if self.v5 {
            let Some((p, used)) = body.get(at..).and_then(read_properties) else {
                return;
            };
            props = p;
            at += used;
        }
        let payload = body.get(at..).unwrap_or(&[]);
        // Acknowledge first; a QoS 2 message is delivered once, until PUBREL.
        let duplicate = match (qos, id) {
            (1, Some(id)) => {
                self.send(cx, packet(PUBACK, 0, &id.to_be_bytes()));
                false
            }
            (2, Some(id)) => {
                self.send(cx, packet(PUBREC, 0, &id.to_be_bytes()));
                !self.inbound.insert(id)
            }
            _ => false,
        };
        if duplicate || topic.is_empty() {
            return;
        }
        let (text, encoding) = match std::str::from_utf8(payload) {
            Ok(t) => (t.to_string(), "text"),
            Err(_) => (
                base64::engine::general_purpose::STANDARD.encode(payload),
                "base64",
            ),
        };
        let mut entry = json!({
            "payload": text,
            "encoding": encoding,
            "qos": qos,
            "retained": retain,
            "received_at": cx.unix_millis(),
        });
        let mut args = json!({"payload": text, "encoding": encoding, "qos": qos, "retain": retain});
        let mut user = Vec::new();
        for (pid, v) in props {
            match (pid, v) {
                (prop::CONTENT_TYPE, Prop::Text(t)) => {
                    entry["content_type"] = json!(t);
                    args["content_type"] = json!(t);
                }
                (prop::RESPONSE_TOPIC, Prop::Text(t)) => {
                    entry["response_topic"] = json!(t);
                    args["response_topic"] = json!(t);
                }
                (prop::CORRELATION_DATA, Prop::Bytes(b)) => {
                    let c = base64::engine::general_purpose::STANDARD.encode(b);
                    entry["correlation_data"] = json!(c);
                    args["correlation_data"] = json!(c);
                }
                (prop::MESSAGE_EXPIRY, Prop::Int(n)) => args["message_expiry_s"] = json!(n),
                (prop::PAYLOAD_FORMAT, Prop::Int(n)) => args["payload_format"] = json!(n),
                (prop::SUBSCRIPTION_ID, Prop::Int(n)) => args["subscription_id"] = json!(n),
                (prop::USER_PROPERTY, Prop::Pair(k, v)) => user.push(json!([k, v])),
                _ => {}
            }
        }
        if !user.is_empty() {
            entry["user_properties"] = json!(user);
            args["user_properties"] = json!(user);
        }
        cx.state(json!({"topics": { topic.clone(): entry }}));
        let source = SocketAddr::new(self.host, self.port);
        cx.message(topic, None, args, source);
    }

    fn ack(&mut self, cx: &mut Cx, kind: u8, body: &[u8]) {
        let Some(b) = body.get(..2) else {
            return;
        };
        let id = u16::from_be_bytes([b[0], b[1]]);
        let Some(p) = self.pending.remove(&id) else {
            return;
        };
        let expected = match p.waiting {
            Waiting::PubAck => PUBACK,
            Waiting::PubRec => PUBREC,
            Waiting::PubComp => PUBCOMP,
            Waiting::SubAck(_) => SUBACK,
            Waiting::UnsubAck(_) => UNSUBACK,
        };
        if kind != expected {
            self.pending.insert(id, p);
            return;
        }
        cx.round_trip(cx.now().saturating_sub(p.sent_at));
        // PUBACK, PUBREC and PUBCOMP carry a reason code only in version 5,
        // and only when it is not success (5.0 §3.4.2.1).
        let reason = if self.v5 {
            body.get(2).copied().unwrap_or(0)
        } else {
            0
        };
        let fail = |cx: &mut Cx, code: u8| {
            if let Some(cid) = p.command {
                cx.complete(
                    cid,
                    Err(CommandError::DeviceError {
                        code: Some(format!("{code:#04x}")),
                        message: reason_text(code).into(),
                    }),
                );
            }
        };
        match p.waiting {
            Waiting::PubAck | Waiting::PubComp => {
                if reason >= 0x80 {
                    fail(cx, reason);
                } else if let Some(cid) = p.command {
                    cx.complete(cid, Ok(Outcome::Ack));
                }
            }
            Waiting::PubRec => {
                if reason >= 0x80 {
                    fail(cx, reason);
                    return;
                }
                self.send(cx, packet(PUBREL, 0x02, &id.to_be_bytes()));
                self.wait(cx, id, Waiting::PubComp, p.command);
            }
            Waiting::SubAck(filters) | Waiting::UnsubAck(filters) => {
                let codes = if self.v5 {
                    match body.get(2..).and_then(read_properties) {
                        Some((_, used)) => body.get(2 + used..).unwrap_or(&[]).to_vec(),
                        None => Vec::new(),
                    }
                } else {
                    body.get(2..).unwrap_or(&[]).to_vec()
                };
                let subscribing = kind == SUBACK;
                let mut subs = Map::new();
                let mut results = Vec::new();
                for (i, filter) in filters.iter().enumerate() {
                    // A 3.1.1 UNSUBACK has no codes: success.
                    let code = codes.get(i).copied().unwrap_or(0);
                    if subscribing {
                        if code < 0x80 {
                            subs.insert(filter.clone(), json!({"qos": code, "error": Value::Null}));
                            results.push(json!({"filter": filter, "qos": code}));
                        } else {
                            subs.insert(
                                filter.clone(),
                                json!({"qos": Value::Null, "error": reason_text(code)}),
                            );
                            results.push(json!({"filter": filter, "error": reason_text(code)}));
                        }
                    } else {
                        subs.insert(filter.clone(), Value::Null);
                        results.push(json!({"filter": filter, "result": reason_text(code)}));
                    }
                }
                cx.state(json!({ "subscriptions": subs }));
                if let Some(cid) = p.command {
                    let first = codes.first().copied().unwrap_or(0);
                    if subscribing && first >= 0x80 {
                        fail(cx, first);
                    } else if subscribing {
                        cx.complete(
                            cid,
                            Ok(Outcome::Value {
                                value: json!(first),
                            }),
                        );
                    } else if first >= 0x80 {
                        fail(cx, first);
                    } else {
                        cx.complete(cid, Ok(Outcome::Ack));
                    }
                }
            }
        }
    }

    fn invalid(cx: &mut Cx, id: CommandId, message: impl Into<String>) {
        cx.complete(
            id,
            Err(CommandError::InvalidParams {
                message: message.into(),
            }),
        );
    }

    fn publish(&mut self, cx: &mut Cx, id: CommandId, params: &Params, clear: bool) {
        let topic = text(params, "topic");
        if let Err(e) = check_topic(&topic) {
            return Mqtt::invalid(cx, id, e);
        }
        let qos = params
            .get("qos")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(2) as u8;
        let retain = clear
            || params
                .get("retain")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        let payload = if clear {
            Vec::new()
        } else {
            let raw = text(params, "payload");
            match params.get("encoding").and_then(Value::as_str) {
                Some("hex") => {
                    let digits: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
                    if !digits.len().is_multiple_of(2) {
                        return Mqtt::invalid(cx, id, "a hex payload has an even number of digits");
                    }
                    match (0..digits.len())
                        .step_by(2)
                        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16))
                        .collect::<Result<Vec<u8>, _>>()
                    {
                        Ok(b) => b,
                        Err(_) => return Mqtt::invalid(cx, id, "the payload is not hex"),
                    }
                }
                Some("base64") => {
                    match base64::engine::general_purpose::STANDARD.decode(raw.trim()) {
                        Ok(b) => b,
                        Err(_) => return Mqtt::invalid(cx, id, "the payload is not base64"),
                    }
                }
                _ => raw.into_bytes(),
            }
        };
        let mut props = Vec::new();
        let content_type = text(params, "content_type");
        let response_topic = text(params, "response_topic");
        let correlation = text(params, "correlation_data");
        let expiry = params
            .get("message_expiry_s")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let user = text(params, "user_properties");
        if !self.v5
            && (!content_type.is_empty()
                || !response_topic.is_empty()
                || !correlation.is_empty()
                || expiry > 0
                || !user.is_empty())
        {
            return Mqtt::invalid(
                cx,
                id,
                "content type, response topic, correlation data, expiry and user properties need MQTT version 5",
            );
        }
        if self.v5 {
            if expiry > 0 {
                props.push((
                    prop::MESSAGE_EXPIRY,
                    Prop::Int(expiry.min(u64::from(u32::MAX)) as u32),
                ));
            }
            if !content_type.is_empty() {
                props.push((prop::CONTENT_TYPE, Prop::Text(content_type)));
            }
            if !response_topic.is_empty() {
                if let Err(e) = check_topic(&response_topic) {
                    return Mqtt::invalid(cx, id, format!("response topic: {e}"));
                }
                props.push((prop::RESPONSE_TOPIC, Prop::Text(response_topic)));
            }
            if !correlation.is_empty() {
                props.push((
                    prop::CORRELATION_DATA,
                    Prop::Bytes(correlation.into_bytes()),
                ));
            }
            for pair in user
                .split([';', '\n'])
                .map(str::trim)
                .filter(|p| !p.is_empty())
            {
                let Some((k, v)) = pair.split_once('=') else {
                    return Mqtt::invalid(
                        cx,
                        id,
                        "user properties are name=value pairs separated by ;",
                    );
                };
                props.push((
                    prop::USER_PROPERTY,
                    Prop::Pair(k.trim().into(), v.trim().into()),
                ));
            }
        }
        let mut body = Vec::new();
        string(&topic, &mut body);
        let pid = (qos > 0).then(|| self.packet_id());
        if let Some(pid) = pid {
            body.extend_from_slice(&pid.to_be_bytes());
        }
        if self.v5 {
            properties(&props, &mut body);
        }
        body.extend_from_slice(&payload);
        self.send(cx, packet(PUBLISH, (qos << 1) | u8::from(retain), &body));
        match pid {
            None => cx.complete(id, Ok(Outcome::Unverified)),
            Some(pid) => {
                let waiting = if qos == 1 {
                    Waiting::PubAck
                } else {
                    Waiting::PubRec
                };
                self.wait(cx, pid, waiting, Some(id));
            }
        }
    }
}

impl Module for Mqtt {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.phase == Phase::Refused {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: "the broker refused the credential".into(),
                }),
            );
            return;
        }
        if self.phase != Phase::Ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match name {
            "publish" => self.publish(cx, id, params, false),
            "clear_retained" => self.publish(cx, id, params, true),
            "subscribe" => {
                let filter = text(params, "filter");
                if let Err(e) = check_filter(&filter) {
                    return Mqtt::invalid(cx, id, e);
                }
                let qos = params
                    .get("qos")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .min(2) as u8;
                let mut options = qos;
                let flag = |k: &str| params.get(k).and_then(Value::as_bool).unwrap_or(false);
                let handling = match params.get("retain_handling").and_then(Value::as_str) {
                    Some("if_new") => 1u8,
                    Some("never") => 2,
                    _ => 0,
                };
                if self.v5 {
                    options |= (u8::from(flag("no_local")) << 2)
                        | (u8::from(flag("retain_as_published")) << 3)
                        | (handling << 4);
                } else if flag("no_local") || flag("retain_as_published") || handling != 0 {
                    return Mqtt::invalid(cx, id, "subscription options need MQTT version 5");
                }
                let sub = Subscription {
                    filter: filter.clone(),
                    options,
                };
                self.added.retain(|s| s.filter != filter);
                self.added.push(sub.clone());
                self.subscribe(cx, &[sub], Some(id));
            }
            "unsubscribe" => {
                let filter = text(params, "filter");
                if let Err(e) = check_filter(&filter) {
                    return Mqtt::invalid(cx, id, e);
                }
                self.added.retain(|s| s.filter != filter);
                let pid = self.packet_id();
                let mut body = pid.to_be_bytes().to_vec();
                if self.v5 {
                    properties(&[], &mut body);
                }
                string(&filter, &mut body);
                self.send(cx, packet(UNSUBSCRIBE, 0x02, &body));
                self.wait(cx, pid, Waiting::UnsubAck(vec![filter]), Some(id));
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => self.opened(cx),
            TcpInput::Data(bytes) => self.data(cx, &bytes),
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn ws(&mut self, cx: &mut Cx, _socket: Key, input: WsInput) {
        match input {
            WsInput::Opened => self.opened(cx),
            WsInput::Binary(bytes) => self.data(cx, &bytes),
            WsInput::Closed { reason, .. } => self.lost(cx, reason),
            _ => {}
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER if self.phase == Phase::Closed => self.open(cx),
            CONNACK_TIMER if self.phase == Phase::Connecting => {
                self.close_socket(cx);
                self.lost(cx, "no CONNACK from the broker".into());
            }
            PING_TIMER if self.phase == Phase::Ready => {
                if self.ping_sent.is_some() {
                    self.close_socket(cx);
                    self.lost(cx, "no PINGRESP from the broker".into());
                    return;
                }
                self.ping_sent = Some(cx.now());
                self.send(cx, packet(PINGREQ, 0, &[]));
                if let Some(every) = self.ping_every() {
                    cx.set_timer(PING_TIMER, every);
                }
            }
            ACK_TIMER => {
                let now = cx.now();
                let late: Vec<u16> = self
                    .pending
                    .iter()
                    .filter(|(_, p)| now.saturating_sub(p.sent_at) >= self.timeout)
                    .map(|(id, _)| *id)
                    .collect();
                for pid in late {
                    if let Some(p) = self.pending.remove(&pid) {
                        if let Some(id) = p.command {
                            cx.complete(id, Err(CommandError::Timeout));
                        }
                    }
                }
                if let Some(oldest) = self.pending.values().map(|p| p.sent_at).min() {
                    cx.set_timer(
                        ACK_TIMER,
                        (oldest + self.timeout).saturating_sub(now).max(1),
                    );
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.phase == Phase::Ready {
            // A normal disconnection: the will is not published.
            let body: &[u8] = if self.v5 { &[0, 0] } else { &[] };
            self.send(cx, packet(DISCONNECT, 0, body));
        }
        self.close_socket(cx);
    }
}

impl Mqtt {
    fn opened(&mut self, cx: &mut Cx) {
        self.phase = Phase::Connecting;
        self.deframer = Deframer::default();
        let connect = self.connect_packet();
        self.send(cx, connect);
        cx.set_timer(CONNACK_TIMER, CONNACK_TIMEOUT);
    }

    fn data(&mut self, cx: &mut Cx, bytes: &[u8]) {
        match self.deframer.feed(bytes) {
            Ok(packets) => {
                for (head, body) in packets {
                    self.received(cx, head, body);
                    if matches!(self.phase, Phase::Closed | Phase::Refused) {
                        return;
                    }
                }
            }
            Err(()) => {
                self.close_socket(cx);
                self.lost(cx, "a malformed packet from the broker".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::Ipv4Addr;

    fn module(monitor: bool, settings: Value) -> Mqtt {
        let spec = Catalog::source_tree().devices["generic-mqtt"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        Mqtt::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 9)),
            host_name: None,
            port: None,
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
                Action::TcpSend { data, .. } | Action::WsSendBinary { data, .. } => {
                    Some(data.clone())
                }
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

    fn feed(m: &mut Mqtt, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut Mqtt, now: Millis, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    /// Open, connect and accept: what the CONNACK led to.
    fn ready(m: &mut Mqtt, connack: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        feed(m, 0, connack)
    }

    #[test]
    fn connect_packets_follow_both_versions() {
        // 3.1.1, empty client id, clean session, keep alive 30.
        let m = module(true, json!({}));
        assert_eq!(
            m.connect_packet(),
            [0x10, 12, 0, 4, b'M', b'Q', b'T', b'T', 4, 0x02, 0, 30, 0, 0]
        );
        // User name, password and a retained QoS 1 will.
        let m = module(
            true,
            json!({"client_id": "c", "username": "u", "password": "p", "will_topic": "w",
                   "will_payload": "x", "will_qos": 1, "will_retain": true, "keepalive_s": 60}),
        );
        assert_eq!(
            m.connect_packet(),
            [
                0x10, 25, 0, 4, b'M', b'Q', b'T', b'T', 4, 0xEE, 0, 60, 0, 1, b'c', 0, 1, b'w', 0,
                1, b'x', 0, 1, b'u', 0, 1, b'p'
            ]
        );
        // Version 5 with a session expiry of 3600 s (property 11h).
        let m = module(
            true,
            json!({"version": "5", "client_id": "c", "clean_start": false, "session_expiry_s": 3600}),
        );
        assert_eq!(
            m.connect_packet(),
            [
                0x10, 19, 0, 4, b'M', b'Q', b'T', b'T', 5, 0x00, 0, 30, 5, 0x11, 0, 0, 0x0E, 0x10,
                0, 1, b'c'
            ]
        );
    }

    #[test]
    fn the_remaining_length_is_a_variable_byte_integer() {
        // The specification's boundaries (3.1.1 Table 2.4).
        for (n, bytes) in [
            (0usize, vec![0x00]),
            (127, vec![0x7F]),
            (128, vec![0x80, 0x01]),
            (16_383, vec![0xFF, 0x7F]),
            (16_384, vec![0x80, 0x80, 0x01]),
            (268_435_455, vec![0xFF, 0xFF, 0xFF, 0x7F]),
        ] {
            let mut out = Vec::new();
            varint(n, &mut out);
            assert_eq!(out, bytes);
            assert_eq!(read_varint(&bytes), Some((n, bytes.len())));
        }
        let mut d = Deframer::default();
        let p = packet(PUBLISH, 0, &[0, 1, b't', b'x']);
        assert!(d.feed(&p[..3]).unwrap().is_empty());
        assert_eq!(d.feed(&p[3..]).unwrap(), [(0x30, vec![0, 1, b't', b'x'])]);
        assert!(d.feed(&[0x30, 0xFF, 0xFF, 0xFF, 0xFF]).is_err());
    }

    #[test]
    fn subscriptions_follow_the_connack_and_messages_reach_state() {
        let mut m = module(
            true,
            json!({"subscriptions": "studio/+/tally@1, sensors/#"}),
        );
        let a = ready(&mut m, &[0x20, 2, 0, 0]);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(
            state(&a),
            json!({"session": {"present": false, "client_id": ""}, "version": "3.1.1"})
        );
        let mut body = vec![0, 1];
        body.extend([0, 14]);
        body.extend(b"studio/+/tally");
        body.push(1);
        body.extend([0, 9]);
        body.extend(b"sensors/#");
        body.push(0);
        assert_eq!(sent(&a), [packet(SUBSCRIBE, 2, &body)]);
        let a = feed(&mut m, 4, &[0x90, 4, 0, 1, 1, 0x80]);
        assert!(a.contains(&Action::RoundTrip(4)));
        assert_eq!(
            state(&a),
            json!({"subscriptions": {"studio/+/tally": {"qos": 1}, "sensors/#": {"error": "unspecified error"}}})
        );

        // A QoS 1 message is acknowledged and kept.
        let mut body = vec![0, 14];
        body.extend(b"studio/1/tally");
        body.extend([0, 7]);
        body.extend(b"red");
        let a = feed(&mut m, 5, &packet(PUBLISH, 0x03, &body));
        assert_eq!(sent(&a), [packet(PUBACK, 0, &[0, 7])]);
        let topic = &state(&a)["topics"]["studio/1/tally"];
        assert_eq!(topic["payload"], "red");
        assert_eq!(topic["qos"], 1);
        assert_eq!(topic["retained"], true);
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Message { address, .. } if address == "studio/1/tally")));

        // A QoS 2 message is delivered once, then released.
        let mut body = vec![0, 1, b'b', 0, 9];
        body.extend([0xFF, 0xFE]);
        let a = feed(&mut m, 6, &packet(PUBLISH, 0x04, &body));
        assert_eq!(sent(&a), [packet(PUBREC, 0, &[0, 9])]);
        assert_eq!(
            state(&a)["topics"]["b"],
            json!({"payload": "//4=", "encoding": "base64", "qos": 2, "retained": false, "received_at": 6})
        );
        let a = feed(&mut m, 7, &packet(PUBLISH, 0x0C, &body));
        assert_eq!(state(&a), json!({}));
        let a = feed(&mut m, 8, &packet(PUBREL, 2, &[0, 9]));
        assert_eq!(sent(&a), [packet(PUBCOMP, 0, &[0, 9])]);
    }

    #[test]
    fn publishing_waits_for_its_acknowledgements() {
        let mut m = module(false, json!({"subscriptions": "a/#"}));
        let a = ready(&mut m, &[0x20, 2, 1, 0]);
        // Commands only: the subscriptions setting is not subscribed.
        assert!(sent(&a).is_empty());
        assert_eq!(state(&a)["session"]["present"], true);

        let a = run(
            &mut m,
            1,
            1,
            "publish",
            json!({"topic": "a/b", "payload": "hi"}),
        );
        assert_eq!(
            sent(&a),
            [packet(PUBLISH, 0, &[0, 3, b'a', b'/', b'b', b'h', b'i'])]
        );
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Unverified)));

        let a = run(
            &mut m,
            10,
            2,
            "publish",
            json!({"topic": "a/b", "payload": "0aff", "encoding": "hex", "qos": 1, "retain": true}),
        );
        assert_eq!(
            sent(&a),
            [packet(
                PUBLISH,
                0x03,
                &[0, 3, b'a', b'/', b'b', 0, 1, 0x0A, 0xFF]
            )]
        );
        let a = feed(&mut m, 13, &packet(PUBACK, 0, &[0, 1]));
        assert_eq!(completed(&a, 2), Some(Ok(Outcome::Ack)));
        assert!(a.contains(&Action::RoundTrip(3)));

        run(&mut m, 20, 3, "publish", json!({"topic": "a/b", "qos": 2}));
        let a = feed(&mut m, 21, &packet(PUBREC, 0, &[0, 2]));
        assert_eq!(sent(&a), [packet(PUBREL, 2, &[0, 2])]);
        assert_eq!(completed(&a, 3), None);
        let a = feed(&mut m, 22, &packet(PUBCOMP, 0, &[0, 2]));
        assert_eq!(completed(&a, 3), Some(Ok(Outcome::Ack)));

        // Version 5 properties need version 5.
        let a = run(
            &mut m,
            30,
            4,
            "publish",
            json!({"topic": "a", "content_type": "x"}),
        );
        assert!(matches!(
            completed(&a, 4),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(&mut m, 30, 5, "publish", json!({"topic": "a/#"}));
        assert!(matches!(
            completed(&a, 5),
            Some(Err(CommandError::InvalidParams { .. }))
        ));

        // A subscription by command, answered with the granted QoS.
        let a = run(
            &mut m,
            40,
            6,
            "subscribe",
            json!({"filter": "x/+", "qos": 2}),
        );
        assert_eq!(
            sent(&a),
            [packet(SUBSCRIBE, 2, &[0, 3, 0, 3, b'x', b'/', b'+', 2])]
        );
        let a = feed(&mut m, 41, &packet(SUBACK, 0, &[0, 3, 1]));
        assert_eq!(
            completed(&a, 6),
            Some(Ok(Outcome::Value { value: json!(1) }))
        );
    }

    #[test]
    fn version_5_reads_properties_and_reason_codes() {
        let mut m = module(true, json!({"version": "5", "subscriptions": "t"}));
        // CONNACK: session present, success, properties: maximum QoS 1,
        // assigned client id "auto", server keep alive 20.
        let mut props = vec![0x24, 1, 0x12, 0, 4];
        props.extend(b"auto");
        props.extend([0x13, 0, 20]);
        let mut body = vec![1, 0, props.len() as u8];
        body.extend(&props);
        let a = ready(&mut m, &packet(CONNACK, 0, &body));
        let s = state(&a);
        assert_eq!(s["session"]["client_id"], "auto");
        assert_eq!(s["server"]["maximum_qos"], 1);
        assert_eq!(s["server"]["server_keep_alive"], 20);
        // Pings every three quarters of the server's keep alive.
        assert!(a.contains(&Action::SetTimer {
            key: PING_TIMER,
            after: 15_000
        }));
        // SUBSCRIBE with an empty property list.
        assert_eq!(sent(&a), [packet(SUBSCRIBE, 2, &[0, 1, 0, 0, 1, b't', 0])]);

        // A message with a content type and a user property.
        let mut props = vec![0x03, 0, 4];
        props.extend(b"text");
        props.extend([0x26, 0, 1, b'k', 0, 1, b'v']);
        let mut body = vec![0, 1, b't', props.len() as u8];
        body.extend(&props);
        body.extend(b"on");
        let a = feed(&mut m, 1, &packet(PUBLISH, 0, &body));
        let t = &state(&a)["topics"]["t"];
        assert_eq!(t["payload"], "on");
        assert_eq!(t["content_type"], "text");
        assert_eq!(t["user_properties"], json!([["k", "v"]]));

        // A PUBACK with reason 87h fails the publish.
        run(
            &mut m,
            2,
            1,
            "publish",
            json!({"topic": "t", "qos": 1, "content_type": "text/plain"}),
        );
        let a = feed(&mut m, 3, &packet(PUBACK, 0, &[0, 2, 0x87, 0]));
        assert_eq!(
            completed(&a, 1),
            Some(Err(CommandError::DeviceError {
                code: Some("0x87".into()),
                message: "not authorized".into()
            }))
        );
    }

    #[test]
    fn a_refused_credential_is_terminal() {
        let mut m = module(true, json!({"username": "u", "password": "bad"}));
        let a = ready(&mut m, &[0x20, 2, 0, 4]);
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
        let a = run(&mut m, 1, 1, "publish", json!({"topic": "t"}));
        assert!(matches!(
            completed(&a, 1),
            Some(Err(CommandError::Auth { .. }))
        ));

        // Server unavailable is retried.
        let mut m = module(true, json!({}));
        let a = ready(&mut m, &[0x20, 2, 0, 3]);
        assert!(a.contains(&Action::SetTimer {
            key: RETRY_TIMER,
            after: RETRY_MIN
        }));
    }

    #[test]
    fn keep_alive_pings_and_drops_a_silent_broker() {
        let mut m = module(true, json!({"keepalive_s": 4}));
        let a = ready(&mut m, &[0x20, 2, 0, 0]);
        assert!(a.contains(&Action::SetTimer {
            key: PING_TIMER,
            after: 3_000
        }));
        let mut cx = Cx::new(3_000);
        m.timer(&mut cx, PING_TIMER);
        assert_eq!(sent(&cx.take()), [vec![0xC0, 0]]);
        let a = feed(&mut m, 3_005, &[0xD0, 0]);
        assert!(a.contains(&Action::RoundTrip(5)));
        let mut cx = Cx::new(6_000);
        m.timer(&mut cx, PING_TIMER);
        cx.take();
        // No PINGRESP by the next ping: the connection is dropped.
        let mut cx = Cx::new(9_000);
        m.timer(&mut cx, PING_TIMER);
        let a = cx.take();
        assert!(a.contains(&Action::TcpClose { socket: SOCKET }));
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
    }

    #[test]
    fn websockets_carry_the_packets_in_binary_messages() {
        let mut m = module(true, json!({"transport": "wss", "ws_path": "/mqtt"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(a, Action::WsOpen { request, .. }
            if request.url == "wss://192.0.2.9:443/mqtt"
            && request.headers.contains(&("Sec-WebSocket-Protocol".into(), "mqtt".into())))));
        let mut cx = Cx::new(0);
        m.ws(&mut cx, SOCKET, WsInput::Opened);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::WsSendBinary { data, .. } if data[0] == 0x10)));
        let mut cx = Cx::new(1);
        m.ws(&mut cx, SOCKET, WsInput::Binary(vec![0x20, 2, 0, 0]));
        assert!(cx
            .take()
            .contains(&Action::Connection(Connection::Connected)));
    }
}
