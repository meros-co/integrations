//! BSS Soundweb London over the London Direct Inject protocol, TCP 1023.
//!
//! From BSS's Soundweb London Interface Kit (revision 2.7, April 2013; see the
//! spec's sources):
//!
//! - A message is STX, a body, the XOR of the body's bytes, then ETX. STX,
//!   ETX, ACK, NAK and Escape inside the body or checksum are sent as Escape
//!   followed by the byte plus 0x80.
//! - A body is a command byte, then the state variable's address: a 16-bit
//!   node, an 8-bit virtual device, a 24-bit object and a 16-bit state
//!   variable, all big-endian, then 32 bits of data (a string for SET STRING).
//! - Over Ethernet nothing is acknowledged: a set is unverified. A
//!   subscription is answered by a SET (or SET%, or SET STRING) carrying the
//!   current value and then by one on every change, or periodically for a
//!   meter. A read is a subscription cancelled once its value has arrived.
//! - Addresses come from the London Architect design, so commands take them
//!   as parameters; the `subscriptions` setting lists what to keep current.
//!   Subscriptions are made again after every reconnection.
//!
//! Opened for commands only, it makes no subscription from the settings.
//! Subscriptions and reads the consumer asks for are still made. With any
//! subscription in place, one is renewed after 10 s of silence as the liveness
//! check (the device answers with the current value); with none, the protocol
//! offers no request the device must answer, and the TCP connection alone says
//! the device is there.

use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Millis, Module, OpenContext, Outcome, TcpInput,
};

const PORT: u16 = 1023;
const SOCKET: Key = "london";

const STX: u8 = 0x02;
const ETX: u8 = 0x03;
const ACK: u8 = 0x06;
const NAK: u8 = 0x15;
const ESC: u8 = 0x1B;

const SET_SV: u8 = 0x88;
const SUBSCRIBE_SV: u8 = 0x89;
const UNSUBSCRIBE_SV: u8 = 0x8A;
const VENUE_PRESET: u8 = 0x8B;
const PARAM_PRESET: u8 = 0x8C;
const SET_PERCENT: u8 = 0x8D;
const SUBSCRIBE_PERCENT: u8 = 0x8E;
const UNSUBSCRIBE_PERCENT: u8 = 0x8F;
const BUMP_PERCENT: u8 = 0x90;
const SET_STRING: u8 = 0x91;

/// How long a read or first subscription value may take.
const REPLY_TIMEOUT: Millis = 3_000;
/// Silence after which a subscription is renewed as the liveness check.
const IDLE_PROBE: Millis = 10_000;
/// Without an answer to that renewal, the connection is dead.
const PROBE_TIMEOUT: Millis = 5_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

const REPLY: Key = "reply";
const IDLE: Key = "idle";
const PROBE: Key = "probe";
const RETRY: Key = "retry";

/// A state variable's full address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Address {
    node: u16,
    vd: u8,
    object: u32,
    sv: u16,
}

impl Address {
    fn bytes(&self) -> Vec<u8> {
        let mut b = self.node.to_be_bytes().to_vec();
        b.push(self.vd);
        b.extend_from_slice(&self.object.to_be_bytes()[1..]);
        b.extend_from_slice(&self.sv.to_be_bytes());
        b
    }

    fn parse(b: &[u8]) -> Option<Address> {
        if b.len() < 8 {
            return None;
        }
        Some(Address {
            node: u16::from_be_bytes([b[0], b[1]]),
            vd: b[2],
            object: u32::from_be_bytes([0, b[3], b[4], b[5]]),
            sv: u16::from_be_bytes([b[6], b[7]]),
        })
    }

    fn patch(&self, leaf: Value) -> Value {
        json!({"nodes": {self.node.to_string(): {self.vd.to_string(): {
            self.object.to_string(): {self.sv.to_string(): leaf}}}}})
    }
}

/// One framed message with its checksum and escapes.
pub(crate) fn frame(body: &[u8]) -> Vec<u8> {
    let mut out = vec![STX];
    let mut checksum = 0u8;
    let put = |b: u8, out: &mut Vec<u8>| {
        if matches!(b, STX | ETX | ACK | NAK | ESC) {
            out.push(ESC);
            out.push(b + 0x80);
        } else {
            out.push(b);
        }
    };
    for &b in body {
        checksum ^= b;
        put(b, &mut out);
    }
    put(checksum, &mut out);
    out.push(ETX);
    out
}

/// Splits the byte stream into checked message bodies; ACK and NAK bytes
/// between messages are dropped.
#[derive(Default)]
struct Reader {
    body: Vec<u8>,
    inside: bool,
    escape: bool,
}

impl Reader {
    fn feed(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for &b in data {
            match b {
                STX => {
                    self.body.clear();
                    self.inside = true;
                    self.escape = false;
                }
                ETX if self.inside => {
                    self.inside = false;
                    if let Some((&sum, body)) = self.body.split_last() {
                        if body.iter().fold(0u8, |a, &x| a ^ x) == sum {
                            out.push(body.to_vec());
                        }
                    }
                    self.body.clear();
                }
                ESC if self.inside => self.escape = true,
                _ if self.inside => {
                    let b = if self.escape { b.wrapping_sub(0x80) } else { b };
                    self.escape = false;
                    if self.body.len() < 4096 {
                        self.body.push(b);
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// How a state variable's 32-bit data maps to an operator's value
/// (Interface Kit, Appendix A).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Encoding {
    Raw,
    Scalar,
    Gain,
    Delay,
    Log,
    Percent,
}

impl Encoding {
    fn of(name: &str) -> Encoding {
        match name {
            "scalar" => Encoding::Scalar,
            "gain_db" => Encoding::Gain,
            "delay_ms" => Encoding::Delay,
            "frequency_or_speed" => Encoding::Log,
            "percent_value" => Encoding::Percent,
            _ => Encoding::Raw,
        }
    }

    fn encode(self, v: f64) -> Option<i32> {
        let raw = match self {
            Encoding::Raw => v,
            Encoding::Scalar => v * 10_000.0,
            Encoding::Gain => gain_to_raw(v)?,
            Encoding::Delay => v * 96.0,
            Encoding::Log => {
                if v <= 0.0 {
                    return None;
                }
                v.log10() * 1_000_000.0
            }
            Encoding::Percent => v * 100.0,
        };
        let raw = raw.round();
        (raw >= i32::MIN as f64 && raw <= i32::MAX as f64).then_some(raw as i32)
    }

    fn decode(self, raw: i32) -> Option<f64> {
        let r = raw as f64;
        Some(match self {
            Encoding::Raw => return None,
            Encoding::Scalar => r / 10_000.0,
            Encoding::Gain => raw_to_gain(raw),
            Encoding::Delay => r / 96.0,
            Encoding::Log => 10f64.powf(r / 1_000_000.0),
            Encoding::Percent => r / 100.0,
        })
    }
}

/// The fader law: linear from +10 to -10 dB, logarithmic below.
fn gain_to_raw(db: f64) -> Option<f64> {
    if db >= -10.0 {
        Some(db * 10_000.0)
    } else if db < 0.0 {
        Some(-(db / 10.0).abs().log10() * 200_000.0 - 100_000.0)
    } else {
        None
    }
}

fn raw_to_gain(raw: i32) -> f64 {
    if raw >= -100_000 {
        raw as f64 / 10_000.0
    } else {
        -10.0 * 10f64.powf(((raw as f64) + 100_000.0).abs() / 200_000.0)
    }
}

fn percent_to_raw(p: f64) -> i32 {
    (p * 65_536.0).round() as i32
}

/// One subscription this session keeps.
#[derive(Debug, Clone)]
struct Subscription {
    percent: bool,
    rate_ms: u32,
    encoding: Encoding,
}

/// A command waiting for the first value at an address.
#[derive(Debug)]
struct Waiting {
    id: CommandId,
    address: Address,
    percent: bool,
    /// A read: cancel the subscription once the value is in.
    read: bool,
    encoding: Encoding,
    sent_at: Millis,
    deadline: Millis,
}

pub(crate) struct London {
    device: SocketAddr,
    standing: Vec<(Address, Subscription)>,
    reader: Reader,
    socket_open: bool,
    connected: bool,
    subscriptions: BTreeMap<(Address, bool), Subscription>,
    waiting: VecDeque<Waiting>,
    /// When the liveness renewal went, while unanswered.
    probe_at: Option<Millis>,
    retry_after: Millis,
}

impl London {
    pub(crate) fn new(ctx: OpenContext) -> London {
        let standing = ctx
            .settings
            .get("subscriptions")
            .and_then(Value::as_str)
            .map(parse_standing)
            .unwrap_or_default();
        London {
            device: SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)),
            standing: if ctx.monitor { standing } else { Vec::new() },
            reader: Reader::default(),
            socket_open: false,
            connected: false,
            subscriptions: BTreeMap::new(),
            waiting: VecDeque::new(),
            probe_at: None,
            retry_after: RETRY_MIN,
        }
    }

    fn send(&self, cx: &mut Cx, body: &[u8]) {
        cx.tcp_send(SOCKET, frame(body));
    }

    fn body(command: u8, address: &Address, data: i32) -> Vec<u8> {
        let mut b = vec![command];
        b.extend(address.bytes());
        b.extend_from_slice(&data.to_be_bytes());
        b
    }

    fn subscribe(&self, cx: &mut Cx, address: &Address, sub: &Subscription) {
        let command = if sub.percent {
            SUBSCRIBE_PERCENT
        } else {
            SUBSCRIBE_SV
        };
        self.send(cx, &London::body(command, address, sub.rate_ms as i32));
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        for w in self.waiting.drain(..) {
            cx.complete(
                w.id,
                Err(CommandError::Transport {
                    message: reason.clone(),
                }),
            );
        }
        for key in [REPLY, IDLE, PROBE] {
            cx.cancel_timer(key);
        }
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.connected = false;
        self.probe_at = None;
        self.reader = Reader::default();
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn arm_reply(&self, cx: &mut Cx) {
        match self.waiting.iter().map(|w| w.deadline).min() {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    fn inbound(&mut self, cx: &mut Cx, body: &[u8]) {
        let Some((&command, rest)) = body.split_first() else {
            return;
        };
        let Some(address) = Address::parse(rest) else {
            return;
        };
        let data = &rest[8..];
        let (percent, leaf, raw) = match command {
            SET_SV | SET_PERCENT if data.len() >= 4 => {
                let raw = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                if command == SET_PERCENT {
                    let p = raw as f64 / 65_536.0;
                    (true, json!({"percent": p}), Value::from(p))
                } else {
                    let encoding = self
                        .subscriptions
                        .get(&(address, false))
                        .map(|s| s.encoding)
                        .or_else(|| {
                            self.waiting
                                .iter()
                                .find(|w| w.address == address && !w.percent)
                                .map(|w| w.encoding)
                        })
                        .unwrap_or(Encoding::Raw);
                    let mut leaf = json!({"raw": raw});
                    let mut out = Value::from(raw);
                    if let Some(v) = encoding.decode(raw) {
                        leaf["value"] = json!(v);
                        out = json!(v);
                    }
                    (false, leaf, out)
                }
            }
            SET_STRING if data.len() >= 2 => {
                let len = u16::from_be_bytes([data[0], data[1]]) as usize;
                let bytes = &data[2..(2 + len).min(data.len())];
                let text: String = String::from_utf8_lossy(bytes)
                    .trim_end_matches('\0')
                    .to_string();
                (false, json!({"text": text}), Value::from(text))
            }
            _ => return,
        };
        cx.state(address.patch(leaf));
        if let Some(sent) = self.probe_at.take() {
            cx.round_trip(cx.now().saturating_sub(sent));
            cx.cancel_timer(PROBE);
        }
        while let Some(i) = self
            .waiting
            .iter()
            .position(|w| w.address == address && w.percent == percent)
        {
            let w = self.waiting.remove(i).unwrap();
            cx.round_trip(cx.now().saturating_sub(w.sent_at));
            if w.read && !self.subscriptions.contains_key(&(address, percent)) {
                let command = if percent {
                    UNSUBSCRIBE_PERCENT
                } else {
                    UNSUBSCRIBE_SV
                };
                self.send(cx, &London::body(command, &address, 0));
            }
            cx.complete(w.id, Ok(Outcome::Value { value: raw.clone() }));
        }
        self.arm_reply(cx);
    }

    fn wait(
        &mut self,
        cx: &mut Cx,
        id: CommandId,
        address: Address,
        percent: bool,
        read: bool,
        encoding: Encoding,
    ) {
        self.waiting.push_back(Waiting {
            id,
            address,
            percent,
            read,
            encoding,
            sent_at: cx.now(),
            deadline: cx.now() + REPLY_TIMEOUT,
        });
        self.arm_reply(cx);
    }
}

/// The `subscriptions` setting: entries separated by `;`, commas or line
/// breaks, each `node/vd/object/sv`, numbers decimal or 0x hex, optionally
/// followed by `@<ms>` (the meter rate), `%` (as a percentage) and
/// `:<encoding>` (scalar, gain_db, delay_ms, frequency_or_speed,
/// percent_value) for the decoded value.
fn parse_standing(text: &str) -> Vec<(Address, Subscription)> {
    let num = |s: &str| -> Option<u64> {
        let s = s.trim();
        match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            Some(h) => u64::from_str_radix(h, 16).ok(),
            None => s.parse().ok(),
        }
    };
    let mut out = Vec::new();
    for entry in text.split([';', ',', '\n']) {
        let mut entry = entry.trim().to_string();
        if entry.is_empty() {
            continue;
        }
        let mut encoding = Encoding::Raw;
        if let Some(at) = entry.find(':') {
            encoding = Encoding::of(entry[at + 1..].trim());
            entry.truncate(at);
        }
        let percent = entry.ends_with('%');
        let entry = entry.trim_end_matches('%');
        let (addr, rate) = match entry.split_once('@') {
            Some((a, r)) => (a, num(r).unwrap_or(0)),
            None => (entry, 0),
        };
        let parts: Vec<&str> = addr.split('/').collect();
        let [node, vd, object, sv] = parts.as_slice() else {
            continue;
        };
        let (Some(node), Some(vd), Some(object), Some(sv)) =
            (num(node), num(vd), num(object), num(sv))
        else {
            continue;
        };
        if node > 0xFFFF || vd > 0xFF || object > 0xFF_FFFF || sv > 0xFFFF {
            continue;
        }
        out.push((
            Address {
                node: node as u16,
                vd: vd as u8,
                object: object as u32,
                sv: sv as u16,
            },
            Subscription {
                percent,
                rate_ms: rate.min(u32::MAX as u64) as u32,
                encoding,
            },
        ));
    }
    out
}

impl Module for London {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, p: &Params) {
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let int = |k: &str| p.get(k).and_then(Value::as_i64).unwrap_or(0);
        let float = |k: &str| p.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let address = Address {
            node: int("node") as u16,
            vd: int("virtual_device") as u8,
            object: int("object") as u32,
            sv: int("state_variable") as u16,
        };
        let encoding = Encoding::of(p.get("encoding").and_then(Value::as_str).unwrap_or("raw"));
        match name {
            "set_sv" => {
                let Some(raw) = encoding.encode(float("value")) else {
                    cx.complete(
                        id,
                        Err(CommandError::InvalidParams {
                            message: "the value cannot be encoded that way".into(),
                        }),
                    );
                    return;
                };
                self.send(cx, &London::body(SET_SV, &address, raw));
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "set_sv_percent" | "bump_sv_percent" => {
                let command = if name == "set_sv_percent" {
                    SET_PERCENT
                } else {
                    BUMP_PERCENT
                };
                self.send(
                    cx,
                    &London::body(command, &address, percent_to_raw(float("percent"))),
                );
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "set_string_sv" => {
                let text = p.get("text").and_then(Value::as_str).unwrap_or("");
                let mut body = vec![SET_STRING];
                body.extend(address.bytes());
                body.extend_from_slice(&((text.len() + 1) as u16).to_be_bytes());
                body.extend_from_slice(text.as_bytes());
                body.push(0);
                self.send(cx, &body);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "recall_venue_preset" | "recall_parameter_preset" => {
                let command = if name == "recall_venue_preset" {
                    VENUE_PRESET
                } else {
                    PARAM_PRESET
                };
                let mut body = vec![command];
                body.extend_from_slice(&(int("preset") as u32).to_be_bytes());
                self.send(cx, &body);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "subscribe" | "subscribe_percent" => {
                let percent = name == "subscribe_percent";
                let sub = Subscription {
                    percent,
                    rate_ms: int("rate_ms").max(0) as u32,
                    encoding,
                };
                self.subscribe(cx, &address, &sub);
                self.subscriptions.insert((address, percent), sub);
                self.wait(cx, id, address, percent, false, encoding);
            }
            "unsubscribe" | "unsubscribe_percent" => {
                let percent = name == "unsubscribe_percent";
                let command = if percent {
                    UNSUBSCRIBE_PERCENT
                } else {
                    UNSUBSCRIBE_SV
                };
                self.subscriptions.remove(&(address, percent));
                self.send(cx, &London::body(command, &address, 0));
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "get_sv" | "get_sv_percent" => {
                let percent = name == "get_sv_percent";
                let sub = Subscription {
                    percent,
                    rate_ms: 0,
                    encoding,
                };
                self.subscribe(cx, &address, &sub);
                self.wait(cx, id, address, percent, true, encoding);
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
            TcpInput::Connected => {
                self.socket_open = true;
                self.connected = true;
                self.retry_after = RETRY_MIN;
                cx.connection(Connection::Connected);
                for (address, sub) in self.standing.clone() {
                    self.subscriptions.insert((address, sub.percent), sub);
                }
                // Made again: they died with the last connection.
                for ((address, _), sub) in self.subscriptions.clone() {
                    self.subscribe(cx, &address, &sub);
                }
                cx.set_timer(IDLE, IDLE_PROBE);
            }
            TcpInput::Data(data) => {
                cx.alive();
                cx.set_timer(IDLE, IDLE_PROBE);
                for body in self.reader.feed(&data) {
                    self.inbound(cx, &body);
                }
            }
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => {
                cx.connection(Connection::Connecting);
                cx.tcp_open(SOCKET, self.device);
            }
            REPLY => {
                let now = cx.now();
                while let Some(i) = self.waiting.iter().position(|w| w.deadline <= now) {
                    let w = self.waiting.remove(i).unwrap();
                    cx.complete(w.id, Err(CommandError::Timeout));
                }
                self.arm_reply(cx);
            }
            IDLE => {
                // The liveness check: renewing a subscription makes the
                // device send the current value.
                if let Some(((address, _), sub)) = self.subscriptions.iter().next() {
                    let (address, sub) = (*address, sub.clone());
                    self.subscribe(cx, &address, &sub);
                    self.probe_at = Some(cx.now());
                    cx.set_timer(PROBE, PROBE_TIMEOUT);
                }
                cx.set_timer(IDLE, IDLE_PROBE);
            }
            PROBE if self.probe_at.is_some() => {
                self.lost(cx, "no answer to a subscription renewal".into());
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.socket_open {
            for (address, percent) in self.subscriptions.keys() {
                let command = if *percent {
                    UNSUBSCRIBE_PERCENT
                } else {
                    UNSUBSCRIBE_SV
                };
                self.send(cx, &London::body(command, address, 0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn london(settings: Value, monitor: bool) -> London {
        London::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 50)),
            port: None,
            model: "blu-100".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        })
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

    fn connected(settings: Value, monitor: bool) -> (London, Vec<Action>) {
        let mut m = london(settings, monitor);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        (m, cx.take())
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    const ADDR: Address = Address {
        node: 0x010F,
        vd: 3,
        object: 0x100,
        sv: 0,
    };

    #[test]
    fn framing_follows_the_interface_kits_example() {
        // Set the gain of object 0x000100 on node 0x010F to 0 dB (p.11): the
        // virtual device byte 0x03 is escaped, the checksum is 0x84.
        let wire = frame(&London::body(SET_SV, &ADDR, 0));
        assert_eq!(
            wire,
            [
                0x02, 0x88, 0x01, 0x0F, 0x1B, 0x83, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x84, 0x03
            ]
        );
        let mut r = Reader::default();
        let (a, b) = wire.split_at(5);
        assert!(r.feed(a).is_empty());
        assert_eq!(r.feed(b), vec![London::body(SET_SV, &ADDR, 0)]);
        // A bad checksum is dropped; ACK bytes between messages are ignored.
        let mut bad = wire.clone();
        bad[15] = 0x85;
        assert!(r.feed(&bad).is_empty());
        assert_eq!(r.feed(&[ACK]), Vec::<Vec<u8>>::new());
    }

    #[test]
    fn scaling_laws_follow_appendix_a() {
        assert_eq!(Encoding::Gain.encode(0.0), Some(0));
        assert_eq!(Encoding::Gain.encode(-10.0), Some(-100_000));
        assert_eq!(Encoding::Gain.encode(-100.0), Some(-300_000));
        assert_eq!(Encoding::Gain.encode(10.0), Some(100_000));
        assert!((raw_to_gain(-300_000) + 100.0).abs() < 1e-9);
        assert_eq!(Encoding::Delay.encode(10.0), Some(960));
        assert_eq!(Encoding::Log.encode(1000.0), Some(3_000_000));
        assert_eq!(Encoding::Scalar.encode(-2.5), Some(-25_000));
        assert_eq!(percent_to_raw(12.5), 819_200);
        assert_eq!(percent_to_raw(-10.0), -655_360);
    }

    #[test]
    fn commands_are_framed_and_sets_unverified() {
        let (mut m, _) = connected(json!({}), true);
        let mut cx = Cx::new(10);
        let addr =
            json!({"node": 0x010F, "virtual_device": 3, "object": 0x100, "state_variable": 1});
        let mut p = addr.as_object().unwrap().clone();
        p.insert("value".into(), json!(1.0));
        m.command(&mut cx, 1, "set_sv", &p);
        let mut p = addr.as_object().unwrap().clone();
        p.insert("percent".into(), json!(50.0));
        m.command(&mut cx, 2, "set_sv_percent", &p);
        m.command(
            &mut cx,
            3,
            "recall_venue_preset",
            &params(json!({"preset": 6})),
        );
        let mut p = addr.as_object().unwrap().clone();
        p.insert("text".into(), json!("Hi"));
        m.command(&mut cx, 4, "set_string_sv", &p);
        let a = cx.take();
        let s = sent(&a);
        let mute = London::body(SET_SV, &Address { sv: 1, ..ADDR }, 1);
        assert_eq!(s[0], frame(&mute));
        assert_eq!(
            s[1],
            frame(&London::body(
                SET_PERCENT,
                &Address { sv: 1, ..ADDR },
                3_276_800
            ))
        );
        assert_eq!(s[2], frame(&[VENUE_PRESET, 0, 0, 0, 6]));
        let mut body = vec![SET_STRING];
        body.extend(Address { sv: 1, ..ADDR }.bytes());
        body.extend_from_slice(&[0, 3, b'H', b'i', 0]);
        assert_eq!(s[3], frame(&body));
        assert!(
            a.iter()
                .filter(|x| matches!(
                    x,
                    Action::Complete {
                        result: Ok(Outcome::Unverified),
                        ..
                    }
                ))
                .count()
                == 4
        );
    }

    #[test]
    fn a_read_subscribes_takes_the_value_and_unsubscribes() {
        let (mut m, _) = connected(json!({}), true);
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "get_sv",
            &params(json!({"node": 0x010F, "virtual_device": 3, "object": 0x100, "state_variable": 0, "encoding": "gain_db"})),
        );
        assert_eq!(
            sent(&cx.take()),
            vec![frame(&London::body(SUBSCRIBE_SV, &ADDR, 0))]
        );
        let mut cx = Cx::new(25);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(frame(&London::body(SET_SV, &ADDR, -100_000))),
        );
        let a = cx.take();
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value {
                value: json!(-10.0)
            })
        }));
        assert!(a.contains(&Action::RoundTrip(15)));
        assert_eq!(
            state(&a),
            json!({"nodes": {"271": {"3": {"256": {"0": {"raw": -100_000, "value": -10.0}}}}}})
        );
        assert_eq!(
            sent(&a),
            vec![frame(&London::body(UNSUBSCRIBE_SV, &ADDR, 0))]
        );
    }

    #[test]
    fn standing_subscriptions_are_made_on_every_connection() {
        let settings =
            json!({"subscriptions": "0x010F/3/0x000100/0:gain_db; 271/3/256/1%; 5/3/2/6@100"});
        let (mut m, a) = connected(settings.clone(), true);
        assert_eq!(
            sent(&a),
            vec![
                frame(&London::body(
                    SUBSCRIBE_SV,
                    &Address {
                        node: 5,
                        vd: 3,
                        object: 2,
                        sv: 6
                    },
                    100
                )),
                frame(&London::body(SUBSCRIBE_SV, &ADDR, 0)),
                frame(&London::body(
                    SUBSCRIBE_PERCENT,
                    &Address { sv: 1, ..ADDR },
                    0
                )),
            ]
        );
        // Pushed values become state, decoded where the setting says how.
        let mut cx = Cx::new(10);
        let mut data = frame(&London::body(SET_SV, &ADDR, 60_000));
        data.extend(frame(&London::body(
            SET_PERCENT,
            &Address { sv: 1, ..ADDR },
            3_276_800,
        )));
        m.tcp(&mut cx, SOCKET, TcpInput::Data(data));
        let s = state(&cx.take());
        let obj = &s["nodes"]["271"]["3"]["256"];
        assert_eq!(obj["0"], json!({"raw": 60_000, "value": 6.0}));
        assert_eq!(obj["1"], json!({"percent": 50.0}));

        // Reconnected: subscribed again.
        let mut cx = Cx::new(20);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        m.timer(&mut cx, RETRY);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&cx.take()).len(), 3);

        // Closing unsubscribes.
        let mut cx = Cx::new(30);
        m.stop(&mut cx);
        assert_eq!(sent(&cx.take()).len(), 3);

        // For commands only, none of them.
        let (_, a) = connected(settings, false);
        assert!(sent(&a).is_empty());
        assert!(a.contains(&Action::Connection(Connection::Connected)));
    }

    #[test]
    fn the_liveness_check_renews_a_subscription() {
        let (mut m, _) = connected(json!({"subscriptions": "271/3/256/0"}), true);
        let mut cx = Cx::new(10_000);
        m.timer(&mut cx, IDLE);
        assert_eq!(
            sent(&cx.take()),
            vec![frame(&London::body(SUBSCRIBE_SV, &ADDR, 0))]
        );
        let mut cx = Cx::new(10_020);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(frame(&London::body(SET_SV, &ADDR, 1))),
        );
        assert!(cx.take().contains(&Action::RoundTrip(20)));
        // Unanswered, the connection is dropped.
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, IDLE);
        let mut cx = Cx::new(25_000);
        m.timer(&mut cx, PROBE);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
    }

    #[test]
    fn an_unanswered_read_times_out() {
        let (mut m, _) = connected(json!({}), true);
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            7,
            "get_sv_percent",
            &params(json!({"node": 1, "virtual_device": 3, "object": 1, "state_variable": 0})),
        );
        let mut cx = Cx::new(REPLY_TIMEOUT);
        m.timer(&mut cx, REPLY);
        assert!(cx.take().contains(&Action::Complete {
            id: 7,
            result: Err(CommandError::Timeout)
        }));
    }
}
