//! OSC listener: receive Open Sound Control from any sender on a port, such as
//! the control surfaces TouchOSC and Lemur, and report every message.
//!
//! The codec is generic-osc's (OSC 1.0 and 1.1; see that module). The
//! difference is the direction: generic-osc reaches out to one device and
//! hears its replies, this listens and hears whoever sends.
//!
//! - Over UDP, one socket of its own on the port, hearing any source address;
//!   a datagram is one packet. Over TCP, any number of senders connect at once
//!   (up to the session's limit), each framed on its own: SLIP as in OSC 1.1,
//!   or an int32 byte count as in OSC 1.0.
//! - Every message is reported as an `Event::Message`, even when it repeats
//!   the last, so a button pressed twice is two events; state cannot do that,
//!   since an equal value is no change. A bundle is unpacked into one event
//!   per message, in order; time tags are not acted on.
//! - State is only a summary: the senders heard (`ip:port`), how many messages
//!   each sent and the address of its last, and the total.
//! - "Connected" means listening. A port that cannot be bound is reported
//!   disconnected and tried again after 1 s, doubling to 30 s.
//! - Opening for commands only changes nothing: listening is the whole
//!   purpose, and nothing is ever asked of a sender. There is no host: the
//!   integration is opened without one.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};

use serde_json::{json, Map, Value};

use super::generic_osc::{decode, encode, frame, parse_args, Deframer, Framing};
use super::generic_tcp_udp::{to_hex, RETRY_MAX, RETRY_MIN};
use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const SOCKET: Key = "osc";
const RETRY: Key = "retry";
/// Senders kept in state; the least recently heard is dropped first, since
/// every UDP source port is a sender of its own.
const MAX_SENDERS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Transport {
    Udp,
    Tcp(Framing),
}

pub(crate) struct OscListener {
    port: u16,
    address: Option<IpAddr>,
    transport: Transport,
    prefix: Option<String>,
    listening: bool,
    retry_after: Millis,
    /// Open TCP connections, each with its own framing state.
    clients: HashMap<SocketAddr, Deframer>,
    /// Senders in state, least recently heard first, with their counts.
    order: VecDeque<String>,
    counts: HashMap<String, u64>,
    total: u64,
}

impl OscListener {
    pub(crate) fn new(ctx: OpenContext) -> Result<OscListener, String> {
        let port = ctx
            .port
            .ok_or("OSC has no standard port: give the port to listen on")?;
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
        let address = match s.get("bind_address").and_then(Value::as_str) {
            None | Some("") => None,
            Some(a) => Some(
                a.trim()
                    .parse::<IpAddr>()
                    .map_err(|_| format!("bind_address '{a}' is not an IP address"))?,
            ),
        };
        let prefix = s
            .get("address_prefix")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
            .map(str::to_string);
        Ok(OscListener {
            port,
            address,
            transport,
            prefix,
            listening: false,
            retry_after: RETRY_MIN,
            clients: HashMap::new(),
            order: VecDeque::new(),
            counts: HashMap::new(),
            total: 0,
        })
    }

    fn listen(&mut self, cx: &mut Cx) {
        self.listening = true;
        // Before the socket: a port that cannot be bound is reported after
        // this, and must be the last word.
        cx.connection(Connection::Connected);
        match self.transport {
            Transport::Udp => cx.udp_open(
                SOCKET,
                Bind::Any {
                    address: self.address,
                    port: self.port,
                },
            ),
            Transport::Tcp(_) => cx.tcp_serve(SOCKET, self.address, self.port),
        }
    }

    fn packet(&mut self, cx: &mut Cx, from: SocketAddr, packet: &[u8]) {
        cx.alive();
        let messages = decode(packet);
        if messages.is_empty() {
            cx.log(
                Level::Debug,
                format!(
                    "not an OSC packet from {from}: {}",
                    to_hex(&packet[..packet.len().min(64)])
                ),
            );
            return;
        }
        let mut last = None;
        let mut reported = 0;
        for m in messages {
            if let Some(prefix) = &self.prefix {
                if !m.address.starts_with(prefix.as_str()) {
                    continue;
                }
            }
            cx.message(m.address.clone(), Some(m.types), Value::Array(m.args), from);
            reported += 1;
            last = Some(m.address);
        }
        if let Some(last) = last {
            self.total += reported;
            let mut senders = Map::new();
            let key = from.to_string();
            if let Some(at) = self.order.iter().position(|k| *k == key) {
                self.order.remove(at);
            } else if self.order.len() >= MAX_SENDERS {
                if let Some(oldest) = self.order.pop_front() {
                    self.counts.remove(&oldest);
                    senders.insert(oldest, Value::Null);
                }
            }
            self.order.push_back(key.clone());
            let count = self.counts.entry(key.clone()).or_insert(0);
            *count += reported;
            senders.insert(key, json!({"messages": *count, "last_address": last}));
            cx.state(json!({"message_count": self.total, "senders": senders}));
        }
    }

    fn connected(&self, cx: &mut Cx, peer: SocketAddr, connected: bool) {
        cx.state(json!({
            "connections": self.clients.len(),
            "senders": {peer.to_string(): {"connected": connected}},
        }));
    }
}

impl Module for OscListener {
    fn start(&mut self, cx: &mut Cx) {
        let transport = match self.transport {
            Transport::Udp => "udp",
            Transport::Tcp(_) => "tcp",
        };
        cx.state(json!({"transport": transport, "port": self.port, "message_count": 0}));
        self.listen(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if name != "send" {
            cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            );
            return;
        }
        let to = params.get("to").and_then(Value::as_str).unwrap_or("");
        let Ok(to) = to.trim().parse::<SocketAddr>() else {
            cx.complete(
                id,
                Err(CommandError::InvalidParams {
                    message: format!("to '{to}' is not an ip:port"),
                }),
            );
            return;
        };
        let address = params.get("address").and_then(Value::as_str).unwrap_or("");
        let args = match parse_args(params.get("args")) {
            Ok(args) => args,
            Err(message) => {
                cx.complete(id, Err(CommandError::InvalidParams { message }));
                return;
            }
        };
        if !self.listening {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let packet = encode(address, &args);
        match self.transport {
            Transport::Udp => cx.udp_send(SOCKET, to, packet),
            Transport::Tcp(framing) => {
                if !self.clients.contains_key(&to) {
                    cx.complete(id, Err(CommandError::NotConnected));
                    return;
                }
                cx.tcp_client_send(SOCKET, to, frame(framing, &packet));
            }
        }
        // OSC has no acknowledgement.
        cx.complete(id, Ok(Outcome::Unverified));
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        self.packet(cx, from, data);
    }

    fn tcp_client(&mut self, cx: &mut Cx, _socket: Key, peer: SocketAddr, input: TcpInput) {
        let Transport::Tcp(framing) = self.transport else {
            return;
        };
        match input {
            TcpInput::Connected => {
                self.clients.insert(peer, Deframer::new(framing));
                self.connected(cx, peer, true);
            }
            TcpInput::Data(bytes) => {
                let Some(deframer) = self.clients.get_mut(&peer) else {
                    return;
                };
                match deframer.feed(&bytes) {
                    Ok(packets) => {
                        for packet in packets {
                            self.packet(cx, peer, &packet);
                        }
                    }
                    // Only this sender's connection is closed; the port and
                    // every other sender carry on.
                    Err(problem) => {
                        cx.log(
                            Level::Warning,
                            format!("closed {peer}: the stream is not framed as set: {problem}"),
                        );
                        cx.tcp_client_close(SOCKET, peer);
                        self.clients.remove(&peer);
                        self.connected(cx, peer, false);
                    }
                }
            }
            TcpInput::Closed { .. } => {
                self.clients.remove(&peer);
                self.connected(cx, peer, false);
            }
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        self.listening = false;
        self.clients.clear();
        cx.log(Level::Warning, format!("listening: {message}"));
        cx.connection(Connection::Disconnected {
            reason: message.to_string(),
        });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == RETRY {
            self.listen(cx);
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        match self.transport {
            Transport::Udp => cx.udp_close(SOCKET),
            Transport::Tcp(_) => cx.tcp_close(SOCKET),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::Ipv4Addr;

    fn listener(settings: Value) -> OscListener {
        OscListener::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: Some(9000),
            model: "osc".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: false,
        })
        .unwrap()
    }

    fn messages(actions: &[Action]) -> Vec<(String, Value, SocketAddr)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Message {
                    address,
                    args,
                    source,
                    ..
                } => Some((address.clone(), args.clone(), *source)),
                _ => None,
            })
            .collect()
    }

    fn sender(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)), port)
    }

    #[test]
    fn listens_on_any_address_and_reports_each_message() {
        let mut l = listener(json!({}));
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Any {
                address: None,
                port: 9000
            }
        }));
        assert!(a.contains(&Action::Connection(Connection::Connected)));

        let press = encode("/1/push1", &[super::super::generic_osc::Arg::Float(1.0)]);
        let mut cx = Cx::new(1);
        l.datagram(&mut cx, SOCKET, sender(5000), &press);
        l.datagram(&mut cx, SOCKET, sender(5000), &press);
        let a = cx.take();
        let got = messages(&a);
        assert_eq!(got.len(), 2, "a repeat is reported again");
        assert_eq!(got[0], ("/1/push1".into(), json!([1.0]), sender(5000)));
        let last = a
            .iter()
            .rev()
            .find_map(|a| match a {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            last,
            json!({"message_count": 2, "senders": {"10.0.0.9:5000":
                   {"messages": 2, "last_address": "/1/push1"}}})
        );
    }

    #[test]
    fn the_prefix_filters_and_garbage_is_only_logged() {
        let mut l = listener(json!({"address_prefix": "/1/"}));
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(1);
        l.datagram(&mut cx, SOCKET, sender(1), &encode("/2/fader", &[]));
        l.datagram(&mut cx, SOCKET, sender(1), b"junk");
        l.datagram(&mut cx, SOCKET, sender(1), &encode("/1/fader", &[]));
        let a = cx.take();
        assert_eq!(messages(&a).len(), 1);
        assert!(a.iter().any(|a| matches!(
            a,
            Action::Log {
                level: Level::Debug,
                ..
            }
        )));
    }

    #[test]
    fn senders_beyond_the_limit_are_dropped_from_state() {
        let mut l = listener(json!({}));
        let packet = encode("/x", &[]);
        let mut cx = Cx::new(0);
        for port in 1..=(MAX_SENDERS as u16 + 1) {
            l.datagram(&mut cx, SOCKET, sender(port), &packet);
        }
        let last = cx
            .take()
            .into_iter()
            .rev()
            .find_map(|a| match a {
                Action::State(p) => Some(p),
                _ => None,
            })
            .unwrap();
        assert_eq!(last["senders"]["10.0.0.9:1"], Value::Null);
        assert_eq!(l.order.len(), MAX_SENDERS);
    }

    #[test]
    fn a_port_that_cannot_be_bound_is_retried() {
        let mut l = listener(json!({"transport": "tcp"}));
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        assert!(cx.take().contains(&Action::TcpServe {
            socket: SOCKET,
            address: None,
            port: 9000
        }));
        let mut cx = Cx::new(1);
        l.socket_error(&mut cx, SOCKET, "in use");
        let a = cx.take();
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        let params = json!({"to": "10.0.0.9:9001", "address": "/x"})
            .as_object()
            .unwrap()
            .clone();
        let mut cx = Cx::new(2);
        l.command(&mut cx, 1, "send", &params);
        assert_eq!(
            cx.take(),
            [Action::Complete {
                id: 1,
                result: Err(CommandError::NotConnected)
            }]
        );
    }

    #[test]
    fn a_bad_bind_address_is_refused() {
        let ctx = OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: Some(9000),
            model: "osc".into(),
            channels: None,
            settings: json!({"bind_address": "nowhere"})
                .as_object()
                .unwrap()
                .clone(),
            monitor: true,
        };
        assert!(OscListener::new(ctx).is_err());
    }
}
