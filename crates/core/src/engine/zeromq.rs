//! `telemetry.zeromq` (SPEC.md §8, "A ZeroMQ bus beside the transport"): a
//! SUB socket that subscribes to topic prefixes on the device's ZeroMQ
//! publisher and offers each message to the JSON rules, and optionally a PUB
//! socket that publishes the spec's messages each time it connects (requests
//! the device answers on the bus, such as a read of its whole state).
//!
//! Both are ZMTP 3.0 connections the core opens to the device's host
//! (`crate::zmtp`), opened only while the device is monitored, reopened
//! after 1 s doubling to 30 s when they close, and never deciding whether
//! the device is connected: the transport does. The bus carries no
//! credential, so a credential the transport's device refuses leaves it as
//! it is.

use std::collections::BTreeMap;

use crate::{json, value::Value};

use super::template::{no_escape, render};
use super::{listen_port, telemetry, SpecEngine, RECONNECT_MAX, RECONNECT_MIN};
use crate::catalog::Params;
use crate::module::{Cx, Key, Level, Millis, TcpInput};
use crate::zmtp::{self, Event, Reader, SocketType};

pub(super) const ZMQ_SUB: Key = "zmq-sub";
pub(super) const ZMQ_PUB: Key = "zmq-pub";
pub(super) const ZMQ_SUB_RETRY: Key = "zmq-sub-retry";
pub(super) const ZMQ_PUB_RETRY: Key = "zmq-pub-retry";
pub(super) const ZMQ_RENEW: Key = "zmq-renew";

/// What the spec declares, with ports resolved from the settings; the
/// topics and messages stay templates, rendered when sent.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ZmqSpec {
    sub_port: u16,
    topics: Vec<String>,
    pub_port: Option<u16>,
    send: Vec<Vec<String>>,
    every: Option<Millis>,
}

/// One of the two connections.
#[derive(Debug)]
pub(super) struct Link {
    reader: Option<Reader>,
    backoff: Millis,
}

impl Link {
    fn new() -> Link {
        Link {
            reader: None,
            backoff: RECONNECT_MIN,
        }
    }
}

#[derive(Debug)]
pub(super) struct Zmq {
    pub(super) spec: ZmqSpec,
    sub: Link,
    publ: Link,
}

impl Zmq {
    pub(super) fn new(spec: ZmqSpec) -> Zmq {
        Zmq {
            spec,
            sub: Link::new(),
            publ: Link::new(),
        }
    }
}

fn port_of(v: Option<&Value>, settings: &Params, at: &str) -> Result<Option<u16>, String> {
    listen_port(v, settings)
        .map_err(|_| format!("{at}.port must be a port number, 1 to 65535, or {{setting: name}}"))
}

/// Read `telemetry.zeromq`.
pub(super) fn parse(v: &Value, settings: &Params) -> Result<ZmqSpec, String> {
    let sub = v
        .get("subscribe")
        .ok_or("telemetry.zeromq needs subscribe")?;
    let sub_port = port_of(sub.get("port"), settings, "telemetry.zeromq.subscribe")?
        .ok_or("telemetry.zeromq.subscribe needs a port")?;
    let topics: Vec<String> = match sub.get("topics") {
        Some(Value::Array(a)) if !a.is_empty() => a
            .iter()
            .map(|t| {
                t.as_str()
                    .map(str::to_string)
                    .ok_or("telemetry.zeromq.subscribe.topics are strings")
            })
            .collect::<Result<_, _>>()?,
        _ => return Err("telemetry.zeromq.subscribe needs topics, a list of prefixes".into()),
    };
    let (pub_port, send, every) = match v.get("publish") {
        None => (None, Vec::new(), None),
        Some(p) => {
            let port = port_of(p.get("port"), settings, "telemetry.zeromq.publish")?
                .ok_or("telemetry.zeromq.publish needs a port")?;
            let send = match p.get("send") {
                Some(Value::Array(messages)) if !messages.is_empty() => messages
                    .iter()
                    .map(|m| match m {
                        Value::Array(frames) if !frames.is_empty() => frames
                            .iter()
                            .map(|f| {
                                f.as_str()
                                    .map(str::to_string)
                                    .ok_or("telemetry.zeromq.publish.send: each frame is a string")
                            })
                            .collect::<Result<Vec<_>, _>>(),
                        _ => Err("telemetry.zeromq.publish.send: each message is a list of frames"),
                    })
                    .collect::<Result<_, _>>()?,
                _ => return Err("telemetry.zeromq.publish needs send, a list of messages".into()),
            };
            let every = match p.get("every_ms") {
                None => None,
                Some(n) => Some(
                    n.as_u64()
                        .filter(|n| *n >= 1000)
                        .ok_or("telemetry.zeromq.publish.every_ms is at least 1000")?,
                ),
            };
            (Some(port), send, every)
        }
    };
    Ok(ZmqSpec {
        sub_port,
        topics,
        pub_port,
        send,
        every,
    })
}

/// A frame as the rules see it: its JSON where it is JSON, its text where
/// it is UTF-8, otherwise `{"base64": ...}`.
fn frame_value(bytes: &[u8]) -> Value {
    if let Ok(v) = serde_json::from_slice::<Value>(bytes) {
        return v;
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Value::String(text.to_string()),
        Err(_) => {
            use base64::Engine as _;
            json!({ "base64": base64::engine::general_purpose::STANDARD.encode(bytes) })
        }
    }
}

/// A message as the JSON rules see it.
pub(crate) fn message_value(frames: &[Vec<u8>]) -> Value {
    json!({ "frames": frames.iter().map(|f| frame_value(f)).collect::<Vec<_>>() })
}

impl SpecEngine {
    /// Open the bus connections, while monitored.
    pub(super) fn open_zmq(&mut self, cx: &mut Cx) {
        if !self.monitor {
            return;
        }
        let Some(z) = &self.zmq else {
            return;
        };
        let host = self.host;
        let (sub_port, pub_port) = (z.spec.sub_port, z.spec.pub_port);
        self.zmq_open(cx, ZMQ_SUB, host, sub_port);
        if let Some(port) = pub_port {
            self.zmq_open(cx, ZMQ_PUB, host, port);
        }
    }

    fn zmq_open(&mut self, cx: &mut Cx, key: Key, host: std::net::IpAddr, port: u16) {
        if let Some(link) = self.zmq_link(key) {
            link.reader = None;
        }
        cx.tcp_open(key, std::net::SocketAddr::new(host, port));
    }

    fn zmq_link(&mut self, key: Key) -> Option<&mut Link> {
        let z = self.zmq.as_mut()?;
        match key {
            ZMQ_SUB => Some(&mut z.sub),
            ZMQ_PUB => Some(&mut z.publ),
            _ => None,
        }
    }

    pub(super) fn close_zmq(&mut self, cx: &mut Cx) {
        if let Some(z) = self.zmq.as_mut() {
            z.sub.reader = None;
            z.publ.reader = None;
            cx.tcp_close(ZMQ_SUB);
            if z.spec.pub_port.is_some() {
                cx.tcp_close(ZMQ_PUB);
            }
            cx.cancel_timer(ZMQ_SUB_RETRY);
            cx.cancel_timer(ZMQ_PUB_RETRY);
            cx.cancel_timer(ZMQ_RENEW);
        }
    }

    pub(super) fn zmq_retry(&mut self, cx: &mut Cx, key: Key) {
        if !self.monitor {
            return;
        }
        let host = self.host;
        let Some(z) = &self.zmq else {
            return;
        };
        let port = match key {
            ZMQ_SUB_RETRY => Some((ZMQ_SUB, z.spec.sub_port)),
            ZMQ_PUB_RETRY => z.spec.pub_port.map(|p| (ZMQ_PUB, p)),
            _ => None,
        };
        if let Some((socket, port)) = port {
            self.zmq_open(cx, socket, host, port);
        }
    }

    /// The publisher's messages, rendered from the settings now.
    pub(super) fn zmq_publish(&mut self, cx: &mut Cx) {
        let Some(z) = &self.zmq else {
            return;
        };
        if z.publ.reader.is_none() {
            return;
        }
        let (send, every) = (z.spec.send.clone(), z.spec.every);
        let (empty, empty_specs) = (Params::new(), BTreeMap::new());
        let values = self.values(&empty, &empty_specs);
        for frames in send {
            let rendered: Result<Vec<Vec<u8>>, String> = frames
                .iter()
                .map(|f| render(f, &values, no_escape).map(String::into_bytes))
                .collect();
            match rendered {
                Ok(frames) => cx.tcp_send(ZMQ_PUB, zmtp::message(&frames)),
                Err(e) => cx.log(Level::Warning, format!("ZeroMQ message not published: {e}")),
            }
        }
        if let Some(every) = every {
            cx.set_timer(ZMQ_RENEW, every);
        }
    }

    pub(super) fn zmq_input(&mut self, cx: &mut Cx, key: Key, input: TcpInput) {
        let socket = if key == ZMQ_SUB {
            SocketType::Sub
        } else {
            SocketType::Pub
        };
        match input {
            TcpInput::Connected => {
                if let Some(link) = self.zmq_link(key) {
                    link.reader = Some(Reader::new(socket));
                }
                cx.tcp_send(key, zmtp::greeting());
                cx.tcp_send(key, zmtp::ready(socket));
            }
            TcpInput::Data(bytes) => {
                let events = match self.zmq_link(key).and_then(|l| l.reader.as_mut()) {
                    Some(reader) => reader.feed(&bytes),
                    None => return,
                };
                for event in events {
                    match event {
                        Event::Ready { .. } => {
                            if let Some(link) = self.zmq_link(key) {
                                link.backoff = RECONNECT_MIN;
                            }
                            if key == ZMQ_SUB {
                                self.zmq_subscribe(cx);
                            } else {
                                self.zmq_publish(cx);
                            }
                        }
                        Event::Message(frames) if key == ZMQ_SUB => {
                            let doc = message_value(&frames);
                            self.offer(cx, &telemetry::Inbound::Json(&doc));
                            self.last_heard = cx.now();
                            cx.alive();
                        }
                        // A publisher is sent subscriptions only.
                        Event::Message(_) => {}
                        Event::Reply(bytes) => cx.tcp_send(key, bytes),
                        Event::Failed(reason) => {
                            cx.log(
                                Level::Warning,
                                format!("ZeroMQ {}: {reason}", socket_name(key)),
                            );
                            cx.tcp_close(key);
                            self.zmq_lost(cx, key);
                            return;
                        }
                    }
                }
            }
            TcpInput::Closed { reason } => {
                cx.log(
                    Level::Debug,
                    format!("ZeroMQ {} closed: {reason}", socket_name(key)),
                );
                self.zmq_lost(cx, key);
            }
        }
    }

    fn zmq_subscribe(&mut self, cx: &mut Cx) {
        let Some(topics) = self.zmq.as_ref().map(|z| z.spec.topics.clone()) else {
            return;
        };
        let (empty, empty_specs) = (Params::new(), BTreeMap::new());
        let values = self.values(&empty, &empty_specs);
        for topic in topics {
            match render(&topic, &values, no_escape) {
                Ok(prefix) => cx.tcp_send(ZMQ_SUB, zmtp::subscribe(prefix.as_bytes())),
                Err(e) => cx.log(Level::Warning, format!("ZeroMQ topic not subscribed: {e}")),
            }
        }
    }

    fn zmq_lost(&mut self, cx: &mut Cx, key: Key) {
        let retry = if key == ZMQ_SUB {
            ZMQ_SUB_RETRY
        } else {
            cx.cancel_timer(ZMQ_RENEW);
            ZMQ_PUB_RETRY
        };
        let monitored = self.monitor;
        let Some(link) = self.zmq_link(key) else {
            return;
        };
        link.reader = None;
        if !monitored {
            return;
        }
        let wait = link.backoff;
        link.backoff = (link.backoff * 2).min(RECONNECT_MAX);
        cx.set_timer(retry, wait);
    }
}

fn socket_name(key: Key) -> &'static str {
    if key == ZMQ_SUB {
        "subscriber"
    } else {
        "publisher"
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;

    use crate::json;

    use super::*;
    use crate::catalog::Catalog;
    use crate::module::{Action, Module, OpenContext, SettingsUpdate};

    /// A HyperDeck spec with a ZeroMQ bus beside its line transport.
    fn bus(settings: Value, monitor: bool, publish: bool) -> SpecEngine {
        let mut spec = Catalog::source_tree()
            .device("blackmagic-hyperdeck")
            .unwrap()
            .clone();
        spec.settings.insert(
            "bus_port".into(),
            crate::value::from_value(
                json!({"type": "int", "min": 1, "max": 65535, "default": 16201}),
            )
            .unwrap(),
        );
        spec.settings.insert(
            "me".into(),
            crate::value::from_value(json!({"type": "string", "default": "Client1"})).unwrap(),
        );
        spec.state.insert(
            "bus.*.value".into(),
            crate::value::from_value(json!({"type": "string", "description": "A value"})).unwrap(),
        );
        let telemetry = spec.telemetry.as_mut().unwrap();
        let mut zmq = json!({
            "subscribe": {"port": {"setting": "bus_port"},
                          "topics": ["{\"Type\":\"Update\"", "{\"Target\":\"{settings.me}\""]},
        });
        if publish {
            zmq["publish"] = json!({"port": 16202, "every_ms": 60000,
                "send": [["{\"Type\":\"Get\",\"RespondTo\":\"{settings.me}\"}", "{}"]]});
        }
        telemetry["zeromq"] = zmq;
        telemetry["updates"].as_array_mut().unwrap().push(json!({
            "json_match": {"$.frames[0].Type": "^Update$", "$.frames[0].Name": "^([A-Za-z]+)$"},
            "json": {"value": "$.frames[1].Value"},
            "state": {"bus.{1}.value": "{value}"}
        }));
        let settings =
            crate::catalog::validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        let model = spec.models[0].id.clone();
        SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model,
                channels: None,
                settings,
                monitor,
            },
        )
        .unwrap()
    }

    fn peer_ready(socket: &str) -> Vec<u8> {
        let mut data = vec![11u8];
        data.extend_from_slice(b"Socket-Type");
        data.extend_from_slice(&(socket.len() as u32).to_be_bytes());
        data.extend_from_slice(socket.as_bytes());
        let mut body = vec![5u8];
        body.extend_from_slice(b"READY");
        body.extend_from_slice(&data);
        let mut bytes = zmtp::greeting();
        bytes.extend([0x04, body.len() as u8]);
        bytes.extend(body);
        bytes
    }

    fn opened(actions: &[Action]) -> Vec<(Key, u16)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpOpen { socket, to } => Some((*socket, to.port())),
                _ => None,
            })
            .collect()
    }

    fn sent(actions: &[Action], key: Key) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { socket, data } if *socket == key => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_subscriber_greets_subscribes_and_offers_messages_to_the_rules() {
        let mut e = bus(json!({"bus_port": 16301}), true, false);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let a = cx.take();
        assert_eq!(opened(&a), [("device", 9993), (ZMQ_SUB, 16301)]);
        let mut cx = Cx::new(1);
        e.tcp(&mut cx, ZMQ_SUB, TcpInput::Connected);
        assert_eq!(
            sent(&cx.take(), ZMQ_SUB),
            [zmtp::greeting(), zmtp::ready(SocketType::Sub)]
        );
        let mut cx = Cx::new(2);
        e.tcp(&mut cx, ZMQ_SUB, TcpInput::Data(peer_ready("XPUB")));
        assert_eq!(
            sent(&cx.take(), ZMQ_SUB),
            [
                zmtp::subscribe(b"{\"Type\":\"Update\""),
                zmtp::subscribe(b"{\"Target\":\"Client1\"")
            ]
        );
        let mut cx = Cx::new(3);
        let m = zmtp::message(&[
            br#"{"Type":"Update","Target":"BROADCAST","Source":"Store","Name":"Gain"}"#.to_vec(),
            br#"{"Value":{"a":1}}"#.to_vec(),
        ]);
        e.tcp(&mut cx, ZMQ_SUB, TcpInput::Data(m));
        let a = cx.take();
        let state: Vec<_> = a
            .iter()
            .filter_map(|a| match a {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(state, [json!({"bus": {"Gain": {"value": "{\"a\":1}"}}})]);
        assert!(a.iter().any(|a| matches!(a, Action::Alive)));
    }

    #[test]
    fn the_publisher_sends_its_messages_on_connecting_and_at_its_interval() {
        let mut e = bus(json!({}), true, true);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        assert_eq!(
            opened(&cx.take()),
            [("device", 9993), (ZMQ_SUB, 16201), (ZMQ_PUB, 16202)]
        );
        let mut cx = Cx::new(1);
        e.tcp(&mut cx, ZMQ_PUB, TcpInput::Connected);
        e.tcp(&mut cx, ZMQ_PUB, TcpInput::Data(peer_ready("XSUB")));
        let a = cx.take();
        let get = zmtp::message(&[
            br#"{"Type":"Get","RespondTo":"Client1"}"#.to_vec(),
            b"{}".to_vec(),
        ]);
        assert_eq!(
            sent(&a, ZMQ_PUB),
            [zmtp::greeting(), zmtp::ready(SocketType::Pub), get.clone()]
        );
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::SetTimer { key, after: 60000 } if *key == ZMQ_RENEW)));
        let mut cx = Cx::new(60001);
        e.timer(&mut cx, ZMQ_RENEW);
        assert_eq!(sent(&cx.take(), ZMQ_PUB), [get]);
    }

    #[test]
    fn a_lost_or_refused_connection_is_opened_again_with_backoff() {
        let mut e = bus(json!({}), true, false);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        cx.take();
        let mut waits = Vec::new();
        for _ in 0..7 {
            let mut cx = Cx::new(1);
            e.tcp(
                &mut cx,
                ZMQ_SUB,
                TcpInput::Closed {
                    reason: "refused".into(),
                },
            );
            for a in cx.take() {
                if let Action::SetTimer { key, after } = a {
                    if key == ZMQ_SUB_RETRY {
                        waits.push(after);
                    }
                }
            }
        }
        assert_eq!(waits, [1000, 2000, 4000, 8000, 16000, 30000, 30000]);
        let mut cx = Cx::new(2);
        e.timer(&mut cx, ZMQ_SUB_RETRY);
        assert_eq!(opened(&cx.take()), [(ZMQ_SUB, 16201)]);
        // A peer that does not pair is closed and retried.
        let mut cx = Cx::new(3);
        e.tcp(&mut cx, ZMQ_SUB, TcpInput::Connected);
        e.tcp(&mut cx, ZMQ_SUB, TcpInput::Data(peer_ready("SUB")));
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::TcpClose { socket } if *socket == ZMQ_SUB)));
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::SetTimer { key, .. } if *key == ZMQ_SUB_RETRY)));
    }

    #[test]
    fn the_bus_is_not_opened_for_commands_only_and_moves_with_its_settings() {
        let mut e = bus(json!({}), false, true);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        assert_eq!(opened(&cx.take()), [("device", 9993)]);

        let mut e = bus(json!({}), true, false);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(1);
        let new = json!({"bus_port": 16401});
        let new = crate::catalog::validate(&e.spec.settings, new.as_object().unwrap()).unwrap();
        assert!(matches!(
            e.update_settings(&mut cx, &new),
            SettingsUpdate::Applied
        ));
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::TcpClose { socket } if *socket == ZMQ_SUB)));
        assert_eq!(opened(&a), [(ZMQ_SUB, 16401)]);
        let mut cx = Cx::new(2);
        e.stop(&mut cx);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::TcpClose { socket } if *socket == ZMQ_SUB)));
    }

    #[test]
    fn frames_that_are_not_json_are_text_or_base64() {
        assert_eq!(
            message_value(&[b"plain".to_vec(), vec![0xFF, 0x00], b"[1]".to_vec()]),
            json!({"frames": ["plain", {"base64": "/wA="}, [1]]})
        );
    }
}
