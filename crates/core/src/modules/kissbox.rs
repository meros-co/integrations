//! KissBox IO3CC and IO8CC I/O card frames with the TCP/UDP firmware, as
//! KissBox's "IO3CC and IO8CC - TCP/UDP Control Commands" (V2, 2023) and
//! the Kiss-Box Software User's Manual (rev. 1.5, §10.1) document them.
//!
//! - Commands are a header byte and data bytes: write one channel (A5h
//!   slot, channel, value), write all eight channels of a slot (A4h), read
//!   one channel (A2h, answered by A3h with the value) and read a slot (A0h,
//!   answered by A1h with eight values). Slots and channels are 0-based on
//!   the wire and 1-based here; digital cards take 0 or 1, analog cards 0 to
//!   255.
//! - Over UDP the frame answers whoever sends, and sends its answers and
//!   input change notifications to the destination port set in its Editor,
//!   as a broadcast: the core listens on `feedback_port` (shared with other
//!   programs on the host) and keeps what this frame's address sends. Over
//!   TCP the frame is a server that only answers reads.
//! - Every slot with a card (`slots`) is read every `poll_interval_ms`, and
//!   a slot written is read again, since writes are not acknowledged.
//!
//! Opened for commands only (`monitor` false), no slot is polled; one read
//! of the first slot each 10 s of silence stays as the liveness check (the
//! protocol has no other), and notifications the frame sends unasked still
//! reach state. Every read answered reports its round trip.

use std::net::{IpAddr, SocketAddr};

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext,
    Outcome, PollStamps, TcpInput,
};

const SOCKET: Key = "kissbox";
const POLL_TIMER: Key = "poll";
const REPLY_TIMER: Key = "reply";
const RETRY_TIMER: Key = "retry";
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const LIVENESS: Millis = 10_000;
const REPLY_TIMEOUT: Millis = 1_000;
const MISSES: u32 = 3;

pub(crate) const READ_ALL: u8 = 0xA0;
pub(crate) const ALL_STATUS: u8 = 0xA1;
pub(crate) const READ_ONE: u8 = 0xA2;
pub(crate) const ONE_STATUS: u8 = 0xA3;
pub(crate) const WRITE_ALL: u8 = 0xA4;
pub(crate) const WRITE_ONE: u8 = 0xA5;

/// A card in a slot, as the `slots` setting names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Card {
    None,
    DigitalInput,
    DigitalOutput,
    AnalogInput,
    AnalogOutput,
}

impl Card {
    fn parse(t: &str) -> Option<Card> {
        Some(match t.trim().to_ascii_lowercase().as_str() {
            "" | "none" | "-" => Card::None,
            "di" | "di8dc" => Card::DigitalInput,
            "do" | "do4pr" | "do8sk" => Card::DigitalOutput,
            "ai" | "ai8ra" => Card::AnalogInput,
            "ao" | "ao8da" => Card::AnalogOutput,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Card::None => "none",
            Card::DigitalInput => "digital_input",
            Card::DigitalOutput => "digital_output",
            Card::AnalogInput => "analog_input",
            Card::AnalogOutput => "analog_output",
        }
    }

    fn digital(self) -> bool {
        matches!(self, Card::DigitalInput | Card::DigitalOutput)
    }
}

/// What a request waits for: an answer about this slot (and channel).
#[derive(Debug, Clone, PartialEq)]
struct Request {
    bytes: Vec<u8>,
    slot: u8,
    channel: Option<u8>,
    command: Option<CommandId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Link {
    Udp,
    Tcp,
}

pub(crate) struct KissBox {
    to: SocketAddr,
    link: Link,
    listen: u16,
    bind: Option<IpAddr>,
    cards: Vec<Card>,
    monitor: bool,
    /// When each poll was last answered (`polled.<group>`, SPEC.md §8).
    stamps: PollStamps,
    poll_every: Millis,
    open: bool,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    buf: Vec<u8>,
    queue: std::collections::VecDeque<Request>,
    in_flight: Option<(Request, Millis)>,
    heard_at: Millis,
}

impl KissBox {
    pub(crate) fn new(ctx: OpenContext) -> Result<KissBox, String> {
        if ctx.host.is_unspecified() {
            return Err("give the KissBox's address as host".into());
        }
        let port = ctx.port.ok_or(
            "a KissBox has no standard port: give the UDP or TCP port set in the KissBox Editor",
        )?;
        let s = &ctx.settings;
        let slots = if ctx.model == "io3cc" { 3 } else { 8 };
        let text = s.get("slots").and_then(Value::as_str).unwrap_or("");
        let mut cards: Vec<Card> = Vec::new();
        // Positional: an empty item is an empty slot.
        let items: Vec<&str> = if text.trim().is_empty() {
            Vec::new()
        } else {
            text.split([',', ';']).collect()
        };
        for t in items {
            cards.push(
                Card::parse(t).ok_or(format!("slot card '{t}' is not di, do, ai, ao or none"))?,
            );
        }
        if cards.len() > slots {
            return Err(format!("the {} has {slots} slots", ctx.model));
        }
        cards.resize(slots, Card::None);
        let link = match s.get("link").and_then(Value::as_str) {
            Some("tcp") => Link::Tcp,
            _ => Link::Udp,
        };
        let bind = match s.get("bind_address").and_then(Value::as_str).map(str::trim) {
            None | Some("") => None,
            Some(a) => Some(
                a.parse::<IpAddr>()
                    .map_err(|_| format!("bind_address '{a}' is not an IP address"))?,
            ),
        };
        Ok(KissBox {
            to: SocketAddr::new(ctx.host, port),
            link,
            listen: s
                .get("feedback_port")
                .and_then(Value::as_u64)
                .map_or(port, |p| p as u16),
            bind,
            cards,
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            poll_every: s
                .get("poll_interval_ms")
                .and_then(Value::as_u64)
                .unwrap_or(1000),
            open: false,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            buf: Vec::new(),
            queue: Default::default(),
            in_flight: None,
            heard_at: 0,
        })
    }

    fn connect(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        match self.link {
            Link::Tcp => cx.tcp_open(SOCKET, self.to),
            Link::Udp => {
                cx.udp_open(
                    SOCKET,
                    Bind::Network {
                        address: self.bind,
                        port: self.listen,
                    },
                );
                self.opened(cx);
            }
        }
    }

    fn opened(&mut self, cx: &mut Cx) {
        self.open = true;
        self.retry_after = RETRY_MIN;
        self.heard_at = cx.now();
        if self.monitor {
            self.sweep();
        } else {
            self.queue.push_back(self.liveness());
        }
        cx.set_timer(POLL_TIMER, self.poll_interval());
        self.pump(cx);
    }

    fn poll_interval(&self) -> Millis {
        if self.monitor && self.poll_every > 0 {
            self.poll_every
        } else {
            LIVENESS
        }
    }

    fn liveness(&self) -> Request {
        let slot = self
            .cards
            .iter()
            .position(|c| *c != Card::None)
            .unwrap_or(0) as u8;
        Request {
            bytes: vec![READ_ALL, slot],
            slot,
            channel: None,
            command: None,
        }
    }

    fn sweep(&mut self) {
        for (slot, card) in self.cards.clone().iter().enumerate() {
            if *card != Card::None {
                self.read_slot(slot as u8);
            }
        }
    }

    fn read_slot(&mut self, slot: u8) {
        let bytes = vec![READ_ALL, slot];
        let waiting = self.queue.iter().any(|r| r.bytes == bytes)
            || self
                .in_flight
                .as_ref()
                .is_some_and(|(r, _)| r.bytes == bytes);
        if !waiting {
            self.queue.push_back(Request {
                bytes,
                slot,
                channel: None,
                command: None,
            });
        }
    }

    fn send(&self, cx: &mut Cx, bytes: Vec<u8>) {
        match self.link {
            Link::Tcp => cx.tcp_send(SOCKET, bytes),
            Link::Udp => cx.udp_send(SOCKET, self.to, bytes),
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || !self.open {
            return;
        }
        let Some(r) = self.queue.pop_front() else {
            return;
        };
        self.send(cx, r.bytes.clone());
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        self.in_flight = Some((r, cx.now()));
    }

    fn enqueue(&mut self, cx: &mut Cx, r: Request) {
        let at = self.queue.iter().position(|q| q.command.is_none());
        match at {
            Some(at) => self.queue.insert(at, r),
            None => self.queue.push_back(r),
        }
        self.pump(cx);
    }

    fn values(&self, slot: u8, first: u8, values: &[u8]) -> Value {
        let card = self
            .cards
            .get(usize::from(slot))
            .copied()
            .unwrap_or(Card::None);
        let mut channels = Map::new();
        for (i, v) in values.iter().enumerate() {
            let mut e = json!({"value": v});
            if card.digital() {
                e["on"] = json!(*v != 0);
            }
            channels.insert((usize::from(first) + i + 1).to_string(), e);
        }
        json!({"slots": {(slot + 1).to_string(): {"channels": channels}}})
    }

    /// One message from the frame: its answers, and notifications.
    fn message(&mut self, cx: &mut Cx, data: &[u8]) -> usize {
        let Some(&head) = data.first() else {
            return 0;
        };
        let (len, slot, channel, values): (usize, u8, Option<u8>, Vec<u8>) = match head {
            ALL_STATUS if data.len() >= 10 => (10, data[1], None, data[2..10].to_vec()),
            ONE_STATUS if data.len() >= 4 => (4, data[1], Some(data[2]), vec![data[3]]),
            ALL_STATUS | ONE_STATUS => return usize::MAX,
            _ => return 1,
        };
        self.misses = 0;
        self.heard_at = cx.now();
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
        if usize::from(slot) < self.cards.len() && channel.is_none_or(|c| c < 8) {
            cx.state(self.values(slot, channel.unwrap_or(0), &values));
        }
        let answers = self.in_flight.as_ref().is_some_and(|(r, _)| {
            r.slot == slot
                && match (r.bytes[0], channel) {
                    (READ_ALL, None) => true,
                    (READ_ONE, Some(c)) => r.channel == Some(c),
                    _ => false,
                }
        });
        if answers {
            let (r, sent_at) = self.in_flight.take().expect("in flight");
            cx.cancel_timer(REPLY_TIMER);
            cx.round_trip(cx.now().saturating_sub(sent_at));
            if r.command.is_none() && self.monitor && self.poll_every > 0 {
                self.stamps.answered(cx, "status");
            }
            if let Some(id) = r.command {
                let value = match channel {
                    Some(_) => json!(values[0]),
                    None => json!(values),
                };
                cx.complete(id, Ok(Outcome::Value { value }));
            }
            self.pump(cx);
        }
        len
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.open = false;
        self.reachable = false;
        self.buf.clear();
        for r in self
            .in_flight
            .take()
            .map(|(r, _)| r)
            .into_iter()
            .chain(self.queue.drain(..))
        {
            if let Some(id) = r.command {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        cx.cancel_timer(POLL_TIMER);
        cx.cancel_timer(REPLY_TIMER);
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn slot_param(&self, params: &Params) -> Option<u8> {
        params
            .get("slot")
            .and_then(Value::as_u64)
            .filter(|s| (1..=self.cards.len() as u64).contains(s))
            .map(|s| (s - 1) as u8)
    }
}

impl Module for KissBox {
    fn start(&mut self, cx: &mut Cx) {
        let mut slots = Map::new();
        for (i, c) in self.cards.iter().enumerate() {
            slots.insert((i + 1).to_string(), json!({"card": c.name()}));
        }
        cx.state(json!({ "slots": slots }));
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.open {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        if name == "refresh" {
            self.sweep();
            self.pump(cx);
            cx.complete(id, Ok(Outcome::Ack));
            return;
        }
        let Some(slot) = self.slot_param(params) else {
            cx.complete(
                id,
                Err(CommandError::InvalidParams {
                    message: format!("slot is 1 to {}", self.cards.len()),
                }),
            );
            return;
        };
        let channel = params
            .get("channel")
            .and_then(Value::as_u64)
            .map(|c| (c.clamp(1, 8) - 1) as u8);
        let card = self.cards[usize::from(slot)];
        let fail = |cx: &mut Cx, m: String| {
            cx.complete(id, Err(CommandError::InvalidParams { message: m }))
        };
        match name {
            "read_channel" | "read_slot" => {
                let (bytes, channel) = match (name, channel) {
                    ("read_channel", Some(c)) => (vec![READ_ONE, slot, c], Some(c)),
                    _ => (vec![READ_ALL, slot], None),
                };
                self.enqueue(
                    cx,
                    Request {
                        bytes,
                        slot,
                        channel,
                        command: Some(id),
                    },
                );
            }
            "write_channel" | "set_output" | "write_slot" => {
                let bytes = match name {
                    "write_slot" => {
                        let values: Option<Vec<u8>> = params
                            .get("values")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .split([',', ' ', ';'])
                            .filter(|t| !t.trim().is_empty())
                            .map(|t| t.trim().parse::<u8>().ok())
                            .collect();
                        match values {
                            Some(v) if v.len() == 8 => {
                                if card.digital() && v.iter().any(|&x| x > 1) {
                                    return fail(cx, "a digital card takes 0 or 1".into());
                                }
                                [vec![WRITE_ALL, slot], v].concat()
                            }
                            _ => return fail(cx, "values is eight numbers, 0 to 255".into()),
                        }
                    }
                    _ => {
                        let value = if name == "set_output" {
                            u8::from(
                                params
                                    .get("active")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                            )
                        } else {
                            params
                                .get("value")
                                .and_then(Value::as_u64)
                                .unwrap_or(0)
                                .min(255) as u8
                        };
                        if card.digital() && value > 1 {
                            return fail(cx, "a digital card takes 0 or 1".into());
                        }
                        vec![WRITE_ONE, slot, channel.unwrap_or(0), value]
                    }
                };
                self.send(cx, bytes);
                cx.complete(id, Ok(Outcome::Unverified));
                // Writes are not answered: read the slot back.
                if self.monitor {
                    self.read_slot(slot);
                    self.pump(cx);
                }
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        // The frame broadcasts: keep only what this frame's address sends.
        if from.ip() != self.to.ip() {
            return;
        }
        let mut at = 0;
        while at < data.len() {
            match self.message(cx, &data[at..]) {
                0 | usize::MAX => break,
                n => at += n,
            }
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        cx.log(Level::Warning, format!("KissBox UDP port: {message}"));
        self.lost(cx, message.to_string());
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.buf.clear();
                self.opened(cx);
            }
            TcpInput::Data(bytes) => {
                self.buf.extend_from_slice(&bytes);
                loop {
                    let data = std::mem::take(&mut self.buf);
                    match self.message(cx, &data) {
                        0 => break,
                        usize::MAX => {
                            self.buf = data;
                            break;
                        }
                        n => self.buf = data[n.min(data.len())..].to_vec(),
                    }
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER if !self.open => self.connect(cx),
            REPLY_TIMER => {
                if let Some((r, _)) = self.in_flight.take() {
                    if let Some(id) = r.command {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES && self.reachable {
                        self.reachable = false;
                        cx.connection(Connection::Disconnected {
                            reason: "no answer from the KissBox".into(),
                        });
                    }
                }
                self.pump(cx);
            }
            POLL_TIMER if self.open => {
                if self.monitor && self.poll_every > 0 {
                    self.sweep();
                } else if self.in_flight.is_none()
                    && self.queue.is_empty()
                    && cx.now().saturating_sub(self.heard_at) >= LIVENESS
                {
                    self.queue.push_back(self.liveness());
                }
                self.pump(cx);
                cx.set_timer(POLL_TIMER, self.poll_interval());
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        match self.link {
            Link::Tcp => cx.tcp_close(SOCKET),
            Link::Udp => cx.udp_close(SOCKET),
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

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 253));

    fn module(monitor: bool, model: &str, settings: Value) -> KissBox {
        let spec = Catalog::source_tree().devices["kissbox-io"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        KissBox::new(OpenContext {
            host: HOST,
            host_name: None,
            port: Some(9812),
            model: model.into(),
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
                Action::UdpSend { data, .. } | Action::TcpSend { data, .. } => Some(data.clone()),
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

    fn opened(m: &mut KissBox) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take()
    }

    fn from(m: &mut KissBox, now: Millis, ip: IpAddr, data: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.datagram(&mut cx, SOCKET, SocketAddr::new(ip, 9812), data);
        cx.take()
    }

    fn run(m: &mut KissBox, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    #[test]
    fn it_listens_on_the_notification_port_and_reads_each_card() {
        let mut m = module(
            true,
            "io8cc",
            json!({"slots": "di, do, none, ai", "feedback_port": 7000}),
        );
        let a = opened(&mut m);
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Network {
                address: None,
                port: 7000
            }
        }));
        assert_eq!(state(&a)["slots"]["3"]["card"], "none");
        // Slots 1, 2 and 4 are read, one at a time.
        assert_eq!(sent(&a), [vec![READ_ALL, 0]]);
        let a = from(&mut m, 4, HOST, &[ALL_STATUS, 0, 1, 0, 1, 0, 1, 0, 1, 0]);
        assert!(a.contains(&Action::RoundTrip(4)));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let ch = &state(&a)["slots"]["1"]["channels"];
        assert_eq!(ch["1"], json!({"value": 1, "on": true}));
        assert_eq!(ch["2"], json!({"value": 0, "on": false}));
        assert_eq!(sent(&a), [vec![READ_ALL, 1]]);
        from(&mut m, 5, HOST, &[ALL_STATUS, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
        // An analog card's values have no on/off.
        let a = from(
            &mut m,
            6,
            HOST,
            &[ALL_STATUS, 3, 255, 127, 63, 25, 0, 63, 127, 255],
        );
        assert_eq!(
            state(&a)["slots"]["4"]["channels"]["2"],
            json!({"value": 127})
        );
        // Another frame's broadcast is not this one's.
        let other = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 254));
        assert!(from(&mut m, 7, other, &[ONE_STATUS, 0, 0, 0]).is_empty());
        // A notification of one input.
        let a = from(&mut m, 8, HOST, &[ONE_STATUS, 0, 2, 1]);
        assert_eq!(state(&a)["slots"]["1"]["channels"]["3"]["on"], true);
    }

    #[test]
    fn writes_are_unverified_and_read_back() {
        let mut m = module(true, "io3cc", json!({"slots": "do"}));
        opened(&mut m);
        from(&mut m, 1, HOST, &[ALL_STATUS, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        // The manual's example: set the 4th channel of the first slot on.
        let a = run(
            &mut m,
            1,
            "write_channel",
            json!({"slot": 1, "channel": 4, "value": 1}),
        );
        assert_eq!(sent(&a), [vec![WRITE_ONE, 0, 3, 1], vec![READ_ALL, 0]]);
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Unverified)));
        from(&mut m, 2, HOST, &[ALL_STATUS, 0, 0, 0, 0, 1, 0, 0, 0, 0]);
        let a = run(
            &mut m,
            2,
            "write_slot",
            json!({"slot": 1, "values": "1,0,1,0,1,0,1,0"}),
        );
        assert_eq!(sent(&a)[0], [WRITE_ALL, 0, 1, 0, 1, 0, 1, 0, 1, 0]);
        // A digital card takes 0 or 1 only.
        let a = run(
            &mut m,
            3,
            "write_channel",
            json!({"slot": 1, "channel": 1, "value": 9}),
        );
        assert!(matches!(
            completed(&a, 3),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        // The IO3CC has three slots.
        let a = run(&mut m, 4, "read_slot", json!({"slot": 4}));
        assert!(matches!(
            completed(&a, 4),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
    }

    #[test]
    fn reads_are_answered_by_their_status_message() {
        let mut m = module(false, "io8cc", json!({"slots": ",,ai"}));
        let a = opened(&mut m);
        // Commands only: one read of the first card's slot, the liveness check.
        assert_eq!(sent(&a), [vec![READ_ALL, 2]]);
        from(&mut m, 1, HOST, &[ALL_STATUS, 2, 0, 0, 0, 0, 0, 0, 0, 0]);
        let a = run(&mut m, 1, "read_channel", json!({"slot": 3, "channel": 7}));
        assert_eq!(sent(&a), [vec![READ_ONE, 2, 6]]);
        // Another channel's status is not the answer.
        let a = from(&mut m, 2, HOST, &[ONE_STATUS, 2, 1, 9]);
        assert_eq!(completed(&a, 1), None);
        let a = from(&mut m, 3, HOST, &[ONE_STATUS, 2, 6, 0x7F]);
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(127) }))
        );
    }

    #[test]
    fn tcp_needs_its_port_and_frames_split_across_reads() {
        let spec = Catalog::source_tree().devices["kissbox-io"].clone();
        let settings = validate(
            &spec.settings,
            json!({"link": "tcp", "slots": "di"}).as_object().unwrap(),
        )
        .unwrap();
        let ctx = OpenContext {
            host: HOST,
            host_name: None,
            port: None,
            model: "io3cc".into(),
            channels: None,
            settings,
            monitor: true,
        };
        assert!(KissBox::new(ctx.clone()).is_err());
        let mut m = KissBox::new(OpenContext {
            port: Some(9813),
            ..ctx
        })
        .unwrap();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::TcpOpen { to, .. } if to.port() == 9813)));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&cx.take()), [vec![READ_ALL, 0]]);
        let mut cx = Cx::new(1);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(vec![ALL_STATUS, 0, 1, 1]));
        assert!(state(&cx.take()).get("slots").is_none());
        let mut cx = Cx::new(2);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(vec![0, 0, 0, 0, 0, 1]));
        let s = state(&cx.take());
        assert_eq!(s["slots"]["1"]["channels"]["2"]["on"], true);
        assert_eq!(s["slots"]["1"]["channels"]["8"]["on"], true);
    }
}
