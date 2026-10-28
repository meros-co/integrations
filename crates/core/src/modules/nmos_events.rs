//! An NMOS Node's IS-07 event transports, added to the spec-driven
//! `nmos-node`.
//!
//! Commands, and the Events API's states read over REST, are the spec's
//! (`specs/nmos-node.yaml`), run by the spec engine. This adds what the format
//! cannot express: following each IS-07 Sender to where it publishes, and
//! listening there (AMWA IS-07 v1.0.1, docs "Transport - Websocket" and
//! "Transport - MQTT"; IS-05 v1.1 for the Senders' transport parameters).
//!
//! - Every 30 seconds the Node API's Senders are listed
//!   (`/x-nmos/node/<node_version>/senders`), and for each whose transport is
//!   `urn:x-nmos:transport:websocket` or `urn:x-nmos:transport:mqtt` its
//!   active transport parameters are read from the Connection API
//!   (`/x-nmos/connection/<connection_version>/single/senders/<id>/active`).
//!   A Sender that is not enabled (`master_enable` false) is not followed.
//! - WebSocket: the Senders sharing a `connection_uri` share one connection,
//!   on which a `subscription` command names every `ext_is_07_source_id`
//!   (sent again whenever the list changes); a `health` command goes every 5
//!   seconds, and a connection silent for 12 seconds is reopened. A bearer
//!   token goes on the opening where the Sender says `connection_authorization`
//!   is true.
//! - MQTT: the Senders sharing a broker (`destination_host`,
//!   `destination_port`, and `broker_protocol` `mqtt` or `secure-mqtt` over
//!   TLS) share one MQTT 3.1.1 connection, subscribed at QoS 0 to every
//!   `broker_topic` and `connection_status_broker_topic`, with the
//!   `mqtt_username` and `mqtt_password` settings where `broker_authorization`
//!   is true. A host name is resolved off the session's task. A broker named
//!   `auto` (found by the Sender itself) cannot be followed and is logged.
//!
//! What arrives is offered to the spec's telemetry rules as JSON: each
//! `state`, `reboot` and `shutdown` message as it is (they name their
//! source), and each MQTT connection status as `{"message_type":
//! "connection_status", "active": <bool>, "sender_id": <id>}`, once for each
//! Sender whose status topic it came on. Health answers keep a websocket
//! alive and are not offered. Each connection that closes is opened again
//! after a delay that starts at one second and doubles to thirty.
//!
//! A device opened for commands only runs without this module (it only keeps
//! state current), so it opens no event connection.

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use serde_json::{json, Value};

use super::nmos_mqtt as mqtt;
use crate::catalog::Params;
use crate::engine::SpecEngine;
use crate::module::{
    CommandId, Cx, FileInput, HttpRequest, HttpResponse, Key, Level, Millis, Module, OpenContext,
    RequestId, SseInput, TcpInput, TlsTarget, WsInput, WsRequest,
};

/// Request ids at and above this are this module's; the engine's count up
/// from 1 and never reach it.
const OWN_IDS: RequestId = 1 << 48;
const SCAN: Key = "nmos-ev-scan";
const TICK: Key = "nmos-ev-tick";
const SCAN_EVERY: Millis = 30_000;
const TICK_EVERY: Millis = 1_000;
const REQUEST_TIMEOUT: Millis = 5_000;
const HEALTH_EVERY: Millis = 5_000;
/// IS-07: two missed health exchanges and two seconds.
const SILENT: Millis = 12_000;
const MQTT_KEEP_ALIVE_S: u16 = 60;
const FIRST_RETRY: Millis = 1_000;
const MAX_RETRY: Millis = 30_000;

/// Connection slots: keys are static, so each kind has a fixed set.
const WS_KEYS: [Key; 16] = [
    "nmos-ev-ws-0",
    "nmos-ev-ws-1",
    "nmos-ev-ws-2",
    "nmos-ev-ws-3",
    "nmos-ev-ws-4",
    "nmos-ev-ws-5",
    "nmos-ev-ws-6",
    "nmos-ev-ws-7",
    "nmos-ev-ws-8",
    "nmos-ev-ws-9",
    "nmos-ev-ws-10",
    "nmos-ev-ws-11",
    "nmos-ev-ws-12",
    "nmos-ev-ws-13",
    "nmos-ev-ws-14",
    "nmos-ev-ws-15",
];
const MQTT_KEYS: [Key; 16] = [
    "nmos-ev-mqtt-0",
    "nmos-ev-mqtt-1",
    "nmos-ev-mqtt-2",
    "nmos-ev-mqtt-3",
    "nmos-ev-mqtt-4",
    "nmos-ev-mqtt-5",
    "nmos-ev-mqtt-6",
    "nmos-ev-mqtt-7",
    "nmos-ev-mqtt-8",
    "nmos-ev-mqtt-9",
    "nmos-ev-mqtt-10",
    "nmos-ev-mqtt-11",
    "nmos-ev-mqtt-12",
    "nmos-ev-mqtt-13",
    "nmos-ev-mqtt-14",
    "nmos-ev-mqtt-15",
];

/// Where one IS-07 Sender publishes.
#[derive(Debug, Clone, PartialEq)]
enum Found {
    Ws {
        uri: String,
        source: String,
        auth: bool,
    },
    Mqtt {
        broker: Broker,
        sender: String,
        topic: Option<String>,
        status_topic: Option<String>,
        auth: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Broker {
    host: String,
    port: u16,
    secure: bool,
}

/// A Sender's active transport parameters (IS-05), read into where it
/// publishes; `None` for a Sender not enabled or not followable, with why.
fn found(sender: &str, ws: bool, active: &Value) -> Result<Option<Found>, String> {
    if active.get("master_enable").and_then(Value::as_bool) == Some(false) {
        return Ok(None);
    }
    let params = active
        .get("transport_params")
        .and_then(|p| p.get(0))
        .ok_or("no transport_params")?;
    let text = |k: &str| {
        params
            .get(k)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let flag = |k: &str| params.get(k).and_then(Value::as_bool) == Some(true);
    if ws {
        let uri = text("connection_uri").ok_or("no connection_uri")?;
        if !(uri.starts_with("ws://") || uri.starts_with("wss://")) {
            return Err(format!("connection_uri {uri} is not a websocket address"));
        }
        let source = text("ext_is_07_source_id").ok_or("no ext_is_07_source_id")?;
        return Ok(Some(Found::Ws {
            uri,
            source,
            auth: flag("connection_authorization"),
        }));
    }
    let host = text("destination_host").ok_or("no destination_host")?;
    if host == "auto" {
        return Err("its broker is auto (chosen by the Sender), which cannot be followed".into());
    }
    let secure = text("broker_protocol").as_deref() == Some("secure-mqtt");
    let port = match params.get("destination_port") {
        Some(Value::Number(n)) => n
            .as_u64()
            .filter(|p| (1..=65535).contains(p))
            .ok_or("destination_port is not a port")? as u16,
        // `auto`: the protocol's own port (IANA: 1883, 8883 over TLS).
        _ if secure => 8883,
        _ => 1883,
    };
    Ok(Some(Found::Mqtt {
        broker: Broker { host, port, secure },
        sender: sender.to_string(),
        topic: text("broker_topic"),
        status_topic: text("connection_status_broker_topic"),
        auth: flag("broker_authorization"),
    }))
}

/// A Sender listed by the Node API that publishes over IS-07: its id and
/// whether over a websocket (else MQTT).
fn event_sender(sender: &Value) -> Option<(String, bool)> {
    let id = sender.get("id")?.as_str()?.to_string();
    match sender.get("transport")?.as_str()? {
        t if t.starts_with("urn:x-nmos:transport:websocket") => Some((id, true)),
        t if t.starts_with("urn:x-nmos:transport:mqtt") => Some((id, false)),
        _ => None,
    }
}

/// IS-07's timestamp, `<seconds>:<nanoseconds>`.
fn timestamp(unix_millis: Millis) -> String {
    format!(
        "{}:{}",
        unix_millis / 1000,
        (unix_millis % 1000) * 1_000_000
    )
}

#[derive(Debug, Default)]
enum Link {
    /// Waiting for its retry, or not yet opened.
    #[default]
    Idle,
    Resolving,
    Opening,
    Open,
}

#[derive(Debug)]
struct WsServer {
    uri: String,
    sources: BTreeSet<String>,
    auth: bool,
    link: Link,
    /// The source list the server was last sent.
    subscribed: Option<BTreeSet<String>>,
    last_heard: Millis,
    last_health: Millis,
    retry_at: Option<Millis>,
    retry: Millis,
}

#[derive(Debug)]
struct MqttBroker {
    broker: Broker,
    topics: BTreeSet<String>,
    /// Connection status topic to the Senders that report on it.
    status: BTreeMap<String, BTreeSet<String>>,
    auth: bool,
    link: Link,
    connected: bool,
    subscribed: BTreeSet<String>,
    reader: mqtt::Reader,
    last_sent: Millis,
    retry_at: Option<Millis>,
    retry: Millis,
    address: Option<SocketAddr>,
}

/// One scan of the Node's Senders.
#[derive(Debug, Default)]
struct Scan {
    list: Option<RequestId>,
    /// Requests for each event Sender's active parameters.
    reading: BTreeMap<RequestId, (String, bool)>,
    found: Vec<Found>,
}

pub(crate) struct NmosEvents {
    engine: SpecEngine,
    /// `scheme://host:port` of the Node API and of the Connection API.
    node: String,
    connection: String,
    node_version: String,
    connection_version: String,
    token: Option<String>,
    mqtt_login: Option<(String, String)>,
    client_id: String,
    scan: Option<Scan>,
    servers: Vec<Option<WsServer>>,
    brokers: Vec<Option<MqttBroker>>,
    next_id: RequestId,
    next_packet: u16,
    stopped: bool,
}

impl NmosEvents {
    pub(crate) fn new(engine: SpecEngine, ctx: &OpenContext) -> NmosEvents {
        let setting = |name: &str| {
            ctx.settings
                .get(name)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from)
        };
        let scheme = setting("scheme").unwrap_or_else(|| "http".into());
        let host = ctx.host_name.clone().unwrap_or_else(|| match ctx.host {
            IpAddr::V6(v6) => format!("[{v6}]"),
            v4 => v4.to_string(),
        });
        let default_port = if scheme == "https" { 443 } else { 80 };
        let node_port = ctx.port.unwrap_or(default_port);
        let connection_port = ctx
            .settings
            .get("connection_port")
            .and_then(Value::as_u64)
            .map_or(node_port, |p| p as u16);
        NmosEvents {
            engine,
            node: format!("{scheme}://{host}:{node_port}"),
            connection: format!("{scheme}://{host}:{connection_port}"),
            node_version: setting("node_version").unwrap_or_else(|| "v1.3".into()),
            connection_version: setting("connection_version").unwrap_or_else(|| "v1.1".into()),
            token: setting("token"),
            mqtt_login: setting("mqtt_username")
                .map(|u| (u, setting("mqtt_password").unwrap_or_default())),
            client_id: format!("meros-{host}-{node_port}"),
            scan: None,
            servers: WS_KEYS.iter().map(|_| None).collect(),
            brokers: MQTT_KEYS.iter().map(|_| None).collect(),
            next_id: OWN_IDS,
            next_packet: 1,
            stopped: false,
        }
    }

    fn get(&mut self, cx: &mut Cx, url: String) -> RequestId {
        let id = self.next_id;
        self.next_id += 1;
        let headers = self
            .token
            .iter()
            .map(|t| ("Authorization".to_string(), format!("Bearer {t}")))
            .collect();
        cx.http(
            id,
            HttpRequest {
                method: "GET",
                url,
                headers,
                body: None,
                body_stream: None,
                timeout: Some(REQUEST_TIMEOUT),
                accept_invalid_certs: true,
                digest: None,
            },
        );
        id
    }

    fn start_scan(&mut self, cx: &mut Cx) {
        let url = format!("{}/x-nmos/node/{}/senders/", self.node, self.node_version);
        let id = self.get(cx, url);
        self.scan = Some(Scan {
            list: Some(id),
            ..Scan::default()
        });
        cx.set_timer(SCAN, SCAN_EVERY);
    }

    fn scanned(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let Some(scan) = self.scan.as_mut() else {
            return;
        };
        let body = match result {
            Ok(r) if (200..300).contains(&r.status) => {
                serde_json::from_slice::<Value>(&r.body).ok()
            }
            Ok(r) => {
                cx.log(Level::Debug, format!("IS-07 senders: HTTP {}", r.status));
                None
            }
            Err(e) => {
                cx.log(Level::Debug, format!("IS-07 senders: {e}"));
                None
            }
        };
        if scan.list == Some(id) {
            scan.list = None;
            let Some(Value::Array(senders)) = body else {
                // Nothing known this round: what is open stays open.
                self.scan = None;
                return;
            };
            let wanted: Vec<(String, bool)> = senders.iter().filter_map(event_sender).collect();
            for (sender, ws) in wanted {
                let url = format!(
                    "{}/x-nmos/connection/{}/single/senders/{}/active",
                    self.connection, self.connection_version, sender
                );
                let rid = self.get(cx, url);
                if let Some(scan) = self.scan.as_mut() {
                    scan.reading.insert(rid, (sender, ws));
                }
            }
        } else if let Some((sender, ws)) = scan.reading.remove(&id) {
            match body.as_ref().map(|b| found(&sender, ws, b)) {
                Some(Ok(Some(f))) => scan.found.push(f),
                Some(Ok(None)) | None => {}
                Some(Err(why)) => cx.log(
                    Level::Info,
                    format!("IS-07 sender {sender} is not followed: {why}"),
                ),
            }
        }
        let done = self
            .scan
            .as_ref()
            .is_some_and(|s| s.list.is_none() && s.reading.is_empty());
        if done {
            let found = self.scan.take().map(|s| s.found).unwrap_or_default();
            self.reconcile(cx, found);
        }
    }

    /// Open, change and close connections to match what the Senders say.
    fn reconcile(&mut self, cx: &mut Cx, found: Vec<Found>) {
        let mut ws: BTreeMap<String, (BTreeSet<String>, bool)> = BTreeMap::new();
        let mut mq: BTreeMap<Broker, (BTreeSet<String>, BTreeMap<String, BTreeSet<String>>, bool)> =
            BTreeMap::new();
        for f in found {
            match f {
                Found::Ws { uri, source, auth } => {
                    let e = ws.entry(uri).or_default();
                    e.0.insert(source);
                    e.1 |= auth;
                }
                Found::Mqtt {
                    broker,
                    sender,
                    topic,
                    status_topic,
                    auth,
                } => {
                    let e = mq.entry(broker).or_default();
                    e.0.extend(topic);
                    if let Some(t) = status_topic {
                        e.0.insert(t.clone());
                        e.1.entry(t).or_default().insert(sender);
                    }
                    e.2 |= auth;
                }
            }
        }
        // Websockets: gone ones closed, kept ones resubscribed, new ones opened.
        for (i, slot) in self.servers.iter_mut().enumerate() {
            let Some(s) = slot else { continue };
            match ws.remove(&s.uri) {
                None => {
                    if matches!(s.link, Link::Open | Link::Opening) {
                        cx.ws_close(WS_KEYS[i]);
                    }
                    *slot = None;
                }
                Some((sources, auth)) => {
                    s.sources = sources;
                    s.auth = auth;
                }
            }
        }
        for (uri, (sources, auth)) in ws {
            match self.servers.iter().position(Option::is_none) {
                Some(i) => {
                    self.servers[i] = Some(WsServer {
                        uri,
                        sources,
                        auth,
                        link: Link::Idle,
                        subscribed: None,
                        last_heard: 0,
                        last_health: 0,
                        retry_at: Some(0),
                        retry: FIRST_RETRY,
                    })
                }
                None => cx.log(
                    Level::Warning,
                    format!(
                        "IS-07: more than {} websocket servers; {uri} is not followed",
                        WS_KEYS.len()
                    ),
                ),
            }
        }
        for i in 0..self.servers.len() {
            self.resubscribe_ws(cx, i);
        }
        // Brokers likewise.
        for (i, slot) in self.brokers.iter_mut().enumerate() {
            let Some(b) = slot else { continue };
            match mq.remove(&b.broker) {
                None => {
                    if b.connected {
                        cx.tcp_send(MQTT_KEYS[i], mqtt::disconnect());
                    }
                    if !matches!(b.link, Link::Idle | Link::Resolving) {
                        cx.tcp_close(MQTT_KEYS[i]);
                    }
                    *slot = None;
                }
                Some((topics, status, auth)) => {
                    b.topics = topics;
                    b.status = status;
                    b.auth = auth;
                }
            }
        }
        for (broker, (topics, status, auth)) in mq {
            match self.brokers.iter().position(Option::is_none) {
                Some(i) => {
                    self.brokers[i] = Some(MqttBroker {
                        broker,
                        topics,
                        status,
                        auth,
                        link: Link::Idle,
                        connected: false,
                        subscribed: BTreeSet::new(),
                        reader: mqtt::Reader::default(),
                        last_sent: 0,
                        retry_at: Some(0),
                        retry: FIRST_RETRY,
                        address: None,
                    })
                }
                None => cx.log(
                    Level::Warning,
                    format!(
                        "IS-07: more than {} MQTT brokers; {}:{} is not followed",
                        MQTT_KEYS.len(),
                        broker.host,
                        broker.port
                    ),
                ),
            }
        }
        for i in 0..self.brokers.len() {
            self.resubscribe_mqtt(cx, i);
        }
        self.tick(cx);
    }

    /// Send a websocket's subscription when its list changed.
    fn resubscribe_ws(&mut self, cx: &mut Cx, i: usize) {
        let Some(s) = self.servers[i].as_mut() else {
            return;
        };
        if !matches!(s.link, Link::Open) || s.subscribed.as_ref() == Some(&s.sources) {
            return;
        }
        let command =
            json!({"command": "subscription", "sources": s.sources.iter().collect::<Vec<_>>()});
        cx.ws_send(WS_KEYS[i], command.to_string());
        s.subscribed = Some(s.sources.clone());
    }

    /// Subscribe and unsubscribe a broker's topics to match what is wanted.
    fn resubscribe_mqtt(&mut self, cx: &mut Cx, i: usize) {
        let Some(b) = self.brokers[i].as_mut() else {
            return;
        };
        if !b.connected {
            return;
        }
        let add: Vec<String> = b.topics.difference(&b.subscribed).cloned().collect();
        let drop: Vec<String> = b.subscribed.difference(&b.topics).cloned().collect();
        if !add.is_empty() {
            cx.tcp_send(MQTT_KEYS[i], mqtt::subscribe(self.next_packet, &add));
            self.next_packet = self.next_packet.wrapping_add(1).max(1);
        }
        if !drop.is_empty() {
            cx.tcp_send(MQTT_KEYS[i], mqtt::unsubscribe(self.next_packet, &drop));
            self.next_packet = self.next_packet.wrapping_add(1).max(1);
        }
        b.subscribed = b.topics.clone();
        b.last_sent = cx.now();
    }

    /// Once a second while anything is followed: open what is due, send
    /// health and pings, and reopen what has gone silent.
    fn tick(&mut self, cx: &mut Cx) {
        let now = cx.now();
        let mut any = false;
        for i in 0..self.servers.len() {
            let token = self.token.clone();
            let Some(s) = self.servers[i].as_mut() else {
                continue;
            };
            any = true;
            match s.link {
                Link::Idle if s.retry_at.is_some_and(|t| t <= now) => {
                    s.retry_at = None;
                    s.link = Link::Opening;
                    s.subscribed = None;
                    let headers = match (&token, s.auth) {
                        (Some(t), true) => {
                            vec![("Authorization".to_string(), format!("Bearer {t}"))]
                        }
                        _ => Vec::new(),
                    };
                    cx.ws_open(
                        WS_KEYS[i],
                        WsRequest {
                            url: s.uri.clone(),
                            headers,
                            accept_invalid_certs: true,
                        },
                    );
                }
                Link::Open if now.saturating_sub(s.last_heard) >= SILENT => {
                    cx.log(
                        Level::Info,
                        format!("IS-07 websocket {} silent; reopening", s.uri),
                    );
                    cx.ws_close(WS_KEYS[i]);
                    s.link = Link::Idle;
                    s.retry_at = Some(now + s.retry);
                    s.retry = (s.retry * 2).min(MAX_RETRY);
                }
                Link::Open if now.saturating_sub(s.last_health) >= HEALTH_EVERY => {
                    let health =
                        json!({"command": "health", "timestamp": timestamp(cx.unix_millis())});
                    cx.ws_send(WS_KEYS[i], health.to_string());
                    s.last_health = now;
                }
                _ => {}
            }
        }
        for i in 0..self.brokers.len() {
            let Some(b) = self.brokers[i].as_mut() else {
                continue;
            };
            any = true;
            match b.link {
                Link::Idle if b.retry_at.is_some_and(|t| t <= now) => {
                    b.retry_at = None;
                    match b.broker.host.parse::<IpAddr>() {
                        Ok(ip) => {
                            b.address = Some(SocketAddr::new(ip, b.broker.port));
                            b.link = Link::Opening;
                            open_broker(cx, i, b);
                        }
                        Err(_) => {
                            // A host name: resolved off the session's task.
                            b.link = Link::Resolving;
                            let target = format!("{}:{}", b.broker.host, b.broker.port);
                            cx.compute(MQTT_KEYS[i], move || {
                                let addr = target
                                    .to_socket_addrs()
                                    .map_err(|e| e.to_string())?
                                    .next()
                                    .ok_or("no address")?;
                                Ok(addr.to_string().into_bytes())
                            });
                        }
                    }
                }
                Link::Open
                    if b.connected
                        && now.saturating_sub(b.last_sent)
                            >= Millis::from(MQTT_KEEP_ALIVE_S) * 500 =>
                {
                    cx.tcp_send(MQTT_KEYS[i], mqtt::pingreq());
                    b.last_sent = now;
                }
                _ => {}
            }
        }
        if any {
            cx.set_timer(TICK, TICK_EVERY);
        }
    }

    fn ws_input(&mut self, cx: &mut Cx, i: usize, input: WsInput) {
        let now = cx.now();
        let Some(s) = self.servers[i].as_mut() else {
            return;
        };
        match input {
            WsInput::Opened => {
                s.link = Link::Open;
                s.retry = FIRST_RETRY;
                s.last_heard = now;
                s.last_health = now;
                self.resubscribe_ws(cx, i);
            }
            WsInput::Text(text) => {
                s.last_heard = now;
                let Ok(doc) = serde_json::from_str::<Value>(&text) else {
                    return;
                };
                cx.alive();
                // A health answer only keeps the connection.
                if doc.get("message_type").and_then(Value::as_str) != Some("health") {
                    self.engine.apply_message(cx, &text);
                }
            }
            WsInput::Closed { reason, .. } => {
                cx.log(
                    Level::Debug,
                    format!(
                        "IS-07 websocket {} closed: {reason}; reopening in {} ms",
                        s.uri, s.retry
                    ),
                );
                s.link = Link::Idle;
                s.retry_at = Some(now + s.retry);
                s.retry = (s.retry * 2).min(MAX_RETRY);
            }
            _ => s.last_heard = now,
        }
    }

    fn broker_lost(&mut self, cx: &mut Cx, i: usize, why: &str) {
        let now = cx.now();
        let Some(b) = self.brokers[i].as_mut() else {
            return;
        };
        cx.log(
            Level::Debug,
            format!(
                "IS-07 MQTT broker {}:{}: {why}; reconnecting in {} ms",
                b.broker.host, b.broker.port, b.retry
            ),
        );
        if matches!(b.link, Link::Open | Link::Opening) {
            cx.tcp_close(MQTT_KEYS[i]);
        }
        b.link = Link::Idle;
        b.connected = false;
        b.subscribed.clear();
        b.reader = mqtt::Reader::default();
        b.retry_at = Some(now + b.retry);
        b.retry = (b.retry * 2).min(MAX_RETRY);
    }

    fn mqtt_input(&mut self, cx: &mut Cx, i: usize, input: TcpInput) {
        let login = self.mqtt_login.clone();
        let client_id = self.client_id.clone();
        let Some(b) = self.brokers[i].as_mut() else {
            return;
        };
        match input {
            TcpInput::Connected => {
                b.link = Link::Open;
                let login = match (&login, b.auth) {
                    (Some((u, p)), true) => Some((u.as_str(), p.as_str())),
                    _ => None,
                };
                cx.tcp_send(
                    MQTT_KEYS[i],
                    mqtt::connect(&client_id, MQTT_KEEP_ALIVE_S, login),
                );
                b.last_sent = cx.now();
            }
            TcpInput::Data(data) => {
                let (packets, answers) = match b.reader.push(&data) {
                    Ok(x) => x,
                    Err(e) => return self.broker_lost(cx, i, &e),
                };
                for a in answers {
                    cx.tcp_send(MQTT_KEYS[i], a);
                }
                for p in packets {
                    match p {
                        mqtt::Packet::ConnAck { code: 0 } => {
                            if let Some(b) = self.brokers[i].as_mut() {
                                b.connected = true;
                                b.retry = FIRST_RETRY;
                            }
                            self.resubscribe_mqtt(cx, i);
                        }
                        mqtt::Packet::ConnAck { code } => {
                            return self.broker_lost(cx, i, &format!("connection refused ({code})"))
                        }
                        mqtt::Packet::Publish { topic, payload } => {
                            cx.alive();
                            self.published(cx, i, &topic, &payload);
                        }
                        _ => {}
                    }
                }
            }
            TcpInput::Closed { reason } => self.broker_lost(cx, i, &reason),
        }
    }

    /// A message a broker relayed: a connection status names its Senders,
    /// anything else is offered as it is.
    fn published(&mut self, cx: &mut Cx, i: usize, topic: &str, payload: &[u8]) {
        let Ok(doc) = serde_json::from_slice::<Value>(payload) else {
            return;
        };
        let senders = self.brokers[i]
            .as_ref()
            .and_then(|b| b.status.get(topic).cloned());
        match senders {
            Some(senders)
                if doc.get("message_type").and_then(Value::as_str) == Some("connection_status") =>
            {
                let active = doc.get("active").cloned().unwrap_or(Value::Null);
                for sender in senders {
                    let message = json!({"message_type": "connection_status", "active": active, "sender_id": sender});
                    self.engine.apply_message(cx, &message.to_string());
                }
            }
            _ => self.engine.apply_message(cx, &doc.to_string()),
        }
    }

    fn resolved(&mut self, cx: &mut Cx, i: usize, result: Result<Vec<u8>, String>) {
        let address = result.and_then(|b| {
            String::from_utf8(b)
                .map_err(|e| e.to_string())?
                .parse::<SocketAddr>()
                .map_err(|e| e.to_string())
        });
        let Some(b) = self.brokers[i].as_mut() else {
            return;
        };
        if !matches!(b.link, Link::Resolving) {
            return;
        }
        match address {
            Ok(a) => {
                b.address = Some(a);
                b.link = Link::Opening;
                open_broker(cx, i, b);
            }
            Err(e) => self.broker_lost(cx, i, &format!("cannot resolve the host: {e}")),
        }
    }
}

fn open_broker(cx: &mut Cx, i: usize, b: &MqttBroker) {
    let Some(to) = b.address else { return };
    if b.broker.secure {
        cx.tcp_open_tls(
            MQTT_KEYS[i],
            TlsTarget {
                to,
                server_name: b.broker.host.clone(),
                accept_invalid_certs: true,
                preamble: None,
            },
        );
    } else {
        cx.tcp_open(MQTT_KEYS[i], to);
    }
}

impl Module for NmosEvents {
    fn start(&mut self, cx: &mut Cx) {
        self.engine.start(cx);
        self.start_scan(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        self.engine.command(cx, id, name, params);
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        self.engine.datagram(cx, socket, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        if let Some(i) = WS_KEYS.iter().position(|k| *k == socket) {
            return self.ws_input(
                cx,
                i,
                WsInput::Closed {
                    code: None,
                    reason: message.to_string(),
                },
            );
        }
        if let Some(i) = MQTT_KEYS.iter().position(|k| *k == socket) {
            return self.broker_lost(cx, i, message);
        }
        self.engine.socket_error(cx, socket, message);
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        match MQTT_KEYS.iter().position(|k| *k == socket) {
            Some(i) => self.mqtt_input(cx, i, input),
            None => self.engine.tcp(cx, socket, input),
        }
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        match WS_KEYS.iter().position(|k| *k == socket) {
            Some(i) => self.ws_input(cx, i, input),
            None => self.engine.ws(cx, socket, input),
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        if id < OWN_IDS {
            return self.engine.http_response(cx, id, result);
        }
        self.scanned(cx, id, result);
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.engine.sse(cx, stream, input);
    }

    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        self.engine.file(cx, file, input);
    }

    fn computed(&mut self, cx: &mut Cx, key: Key, result: Result<Vec<u8>, String>) {
        match MQTT_KEYS.iter().position(|k| *k == key) {
            Some(i) => self.resolved(cx, i, result),
            None => self.engine.computed(cx, key, result),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if self.stopped && (key == SCAN || key == TICK) {
            return;
        }
        match key {
            SCAN => self.start_scan(cx),
            TICK => self.tick(cx),
            _ => self.engine.timer(cx, key),
        }
    }

    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        self.engine.stream_watch(cx, stream, watching);
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.stopped = true;
        cx.cancel_timer(SCAN);
        cx.cancel_timer(TICK);
        for (i, s) in self.servers.iter_mut().enumerate() {
            if let Some(s) = s.take() {
                if matches!(s.link, Link::Open | Link::Opening) {
                    cx.ws_close(WS_KEYS[i]);
                }
            }
        }
        for (i, b) in self.brokers.iter_mut().enumerate() {
            if let Some(b) = b.take() {
                if b.connected {
                    cx.tcp_send(MQTT_KEYS[i], mqtt::disconnect());
                }
                if matches!(b.link, Link::Open | Link::Opening) {
                    cx.tcp_close(MQTT_KEYS[i]);
                }
            }
        }
        self.engine.stop(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::Ipv4Addr;
    use std::sync::Arc;

    fn open() -> NmosEvents {
        let spec = crate::catalog::Catalog::source_tree()
            .device("nmos-node")
            .unwrap()
            .clone();
        let ctx = OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: Some(3212),
            model: "node".into(),
            channels: None,
            settings: json!({"token": "tk", "mqtt_username": "u", "mqtt_password": "p"})
                .as_object()
                .unwrap()
                .clone(),
            monitor: true,
        };
        let engine = SpecEngine::new(Arc::new(spec), ctx.clone()).unwrap();
        NmosEvents::new(engine, &ctx)
    }

    fn at(now: Millis, f: impl FnOnce(&mut Cx)) -> Vec<Action> {
        let mut cx = Cx::new(now);
        f(&mut cx);
        cx.take()
    }

    fn own_requests(a: &[Action]) -> Vec<(RequestId, String)> {
        a.iter()
            .filter_map(|x| match x {
                Action::Http { id, request } if *id >= OWN_IDS => Some((*id, request.url.clone())),
                _ => None,
            })
            .collect()
    }

    fn ok(body: Value) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status: 200,
            headers: Vec::new(),
            body: body.to_string().into_bytes(),
        })
    }

    const WS_SENDER: &str = "9f463872-9621-4939-aa3a-dc3c82d8578b";
    const MQTT_SENDER: &str = "772116e0-b4ba-43b1-9ffc-70287c17cb9e";
    const SOURCE: &str = "6cbd0441-7882-44cd-9557-842243a0d618";

    /// Start, list the Senders and answer their active parameters.
    fn scanned(e: &mut NmosEvents) -> Vec<Action> {
        let a = at(0, |cx| e.start(cx));
        let list = own_requests(&a);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].1, "http://127.0.0.1:3212/x-nmos/node/v1.3/senders/");
        let a = at(10, |cx| {
            e.http_response(
                cx,
                list[0].0,
                ok(json!([
                    {"id": WS_SENDER, "transport": "urn:x-nmos:transport:websocket"},
                    {"id": MQTT_SENDER, "transport": "urn:x-nmos:transport:mqtt"},
                    {"id": "other", "transport": "urn:x-nmos:transport:rtp.mcast"},
                ])),
            )
        });
        let reads = own_requests(&a);
        assert_eq!(
            reads.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(),
            [
                format!("http://127.0.0.1:3212/x-nmos/connection/v1.1/single/senders/{WS_SENDER}/active"),
                format!("http://127.0.0.1:3212/x-nmos/connection/v1.1/single/senders/{MQTT_SENDER}/active"),
            ]
        );
        at(20, |cx| {
            e.http_response(
                cx,
                reads[0].0,
                ok(json!({"master_enable": true, "transport_params": [{
                    "connection_uri": "ws://10.0.0.5:9000/x-nmos/events/v1.0/devices/d1",
                    "connection_authorization": true,
                    "ext_is_07_source_id": SOURCE,
                }]})),
            )
        });
        at(30, |cx| {
            e.http_response(
                cx,
                reads[1].0,
                ok(json!({"master_enable": true, "transport_params": [{
                    "destination_host": "10.0.0.9", "destination_port": 1883,
                    "broker_protocol": "mqtt", "broker_authorization": true,
                    "broker_topic": "x-nmos/events/v1.0/sources/s2",
                    "connection_status_broker_topic": "x-nmos/events/v1.0/connections/c2",
                }]})),
            )
        })
    }

    #[test]
    fn a_websocket_sender_is_subscribed_and_kept_healthy() {
        let mut e2 = open();
        let a = scanned(&mut e2);
        let opened = a.iter().find_map(|x| match x {
            Action::WsOpen { socket, request } if *socket == WS_KEYS[0] => Some(request.clone()),
            _ => None,
        });
        let request = opened.expect("the event websocket opens");
        assert_eq!(
            request.url,
            "ws://10.0.0.5:9000/x-nmos/events/v1.0/devices/d1"
        );
        assert!(request
            .headers
            .contains(&("Authorization".into(), "Bearer tk".into())));
        let a = at(40, |cx| e2.ws(cx, WS_KEYS[0], WsInput::Opened));
        let sent: Vec<&String> = a
            .iter()
            .filter_map(|x| match x {
                Action::WsSend { socket, text } if *socket == WS_KEYS[0] => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(
            sent,
            [&json!({"command": "subscription", "sources": [SOURCE]}).to_string()]
        );
        // A state message goes to the rules.
        let state = json!({"identity": {"source_id": SOURCE, "flow_id": "f"}, "event_type": "boolean",
            "timing": {"creation_timestamp": "1531680501:280709600"}, "payload": {"value": true},
            "message_type": "state"});
        let a = at(50, |cx| {
            e2.ws(cx, WS_KEYS[0], WsInput::Text(state.to_string()))
        });
        assert!(a.iter().any(|x| matches!(x, Action::State(p)
            if p["events"][SOURCE]["value"] == json!("true"))));
        // Health every 5 seconds.
        let a = at(5040, |cx| e2.timer(cx, TICK));
        assert!(a.iter().any(|x| matches!(x, Action::WsSend { socket, text }
            if *socket == WS_KEYS[0] && text.contains("\"command\":\"health\""))));
        // Silent for 12 seconds: reopened after the backoff.
        let a = at(12_100, |cx| e2.timer(cx, TICK));
        assert!(a.contains(&Action::WsClose { socket: WS_KEYS[0] }));
        let a = at(13_200, |cx| e2.timer(cx, TICK));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::WsOpen { socket, .. } if *socket == WS_KEYS[0])));
    }

    #[test]
    fn an_mqtt_sender_is_followed_at_its_broker() {
        let mut e = open();
        let a = scanned(&mut e);
        let to = SocketAddr::from(([10, 0, 0, 9], 1883));
        assert!(a.contains(&Action::TcpOpen {
            socket: MQTT_KEYS[0],
            to
        }));
        let a = at(40, |cx| e.tcp(cx, MQTT_KEYS[0], TcpInput::Connected));
        let connect = mqtt::connect("meros-127.0.0.1-3212", MQTT_KEEP_ALIVE_S, Some(("u", "p")));
        assert!(a.contains(&Action::TcpSend {
            socket: MQTT_KEYS[0],
            data: connect
        }));
        let a = at(50, |cx| {
            e.tcp(cx, MQTT_KEYS[0], TcpInput::Data(vec![0x20, 2, 0, 0]))
        });
        let subscribe = mqtt::subscribe(
            1,
            &[
                "x-nmos/events/v1.0/connections/c2".into(),
                "x-nmos/events/v1.0/sources/s2".into(),
            ],
        );
        assert!(a.contains(&Action::TcpSend {
            socket: MQTT_KEYS[0],
            data: subscribe
        }));
        // A retained state on the source's topic, then the connection status.
        let publish = |topic: &str, body: &Value| {
            let mut b = Vec::new();
            b.extend_from_slice(&(topic.len() as u16).to_be_bytes());
            b.extend_from_slice(topic.as_bytes());
            b.extend_from_slice(body.to_string().as_bytes());
            // The remaining length in MQTT's variable-length form (§2.2.3).
            let mut p = vec![0x31];
            let mut n = b.len();
            loop {
                let byte = (n % 128) as u8;
                n /= 128;
                p.push(if n > 0 { byte | 0x80 } else { byte });
                if n == 0 {
                    break;
                }
            }
            p.extend_from_slice(&b);
            p
        };
        let state = json!({"identity": {"source_id": "s2"}, "event_type": "number",
            "timing": {"creation_timestamp": "1:0"}, "payload": {"value": 21, "scale": 10},
            "message_type": "state"});
        let a = at(60, |cx| {
            e.tcp(
                cx,
                MQTT_KEYS[0],
                TcpInput::Data(publish("x-nmos/events/v1.0/sources/s2", &state)),
            )
        });
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::State(p) if p["events"]["s2"]["scale"] == json!(10))));
        let status = json!({"active": false, "message_type": "connection_status"});
        let a = at(70, |cx| {
            e.tcp(
                cx,
                MQTT_KEYS[0],
                TcpInput::Data(publish("x-nmos/events/v1.0/connections/c2", &status)),
            )
        });
        assert!(a.iter().any(|x| matches!(x, Action::State(p)
            if p["event_senders"][MQTT_SENDER]["broker_connected"] == json!(false))));
        // The broker gone: reconnected after the backoff.
        let a = at(80, |cx| {
            e.tcp(
                cx,
                MQTT_KEYS[0],
                TcpInput::Closed {
                    reason: "gone".into(),
                },
            )
        });
        assert!(!a.iter().any(|x| matches!(x, Action::TcpOpen { .. })));
        let a = at(1100, |cx| e.timer(cx, TICK));
        assert!(a.contains(&Action::TcpOpen {
            socket: MQTT_KEYS[0],
            to
        }));
    }

    #[test]
    fn senders_not_enabled_or_auto_are_not_followed() {
        assert_eq!(found("s", true, &json!({"master_enable": false})), Ok(None));
        assert!(found(
            "s",
            false,
            &json!({"transport_params": [{"destination_host": "auto"}]})
        )
        .is_err());
        assert_eq!(
            found(
                "s",
                false,
                &json!({"transport_params": [{"destination_host": "b.local",
                "destination_port": "auto", "broker_protocol": "secure-mqtt"}]})
            ),
            Ok(Some(Found::Mqtt {
                broker: Broker {
                    host: "b.local".into(),
                    port: 8883,
                    secure: true
                },
                sender: "s".into(),
                topic: None,
                status_topic: None,
                auth: false,
            }))
        );
        assert!(found(
            "s",
            true,
            &json!({"transport_params": [{"connection_uri": "http://x/"}]})
        )
        .is_err());
        assert_eq!(timestamp(1_531_680_501_280), "1531680501:280000000");
    }

    #[test]
    fn a_sender_gone_closes_its_connection() {
        let mut e = open();
        scanned(&mut e);
        at(40, |cx| e.ws(cx, WS_KEYS[0], WsInput::Opened));
        let a = at(30_000, |cx| e.timer(cx, SCAN));
        let list = own_requests(&a);
        let a = at(30_010, |cx| e.http_response(cx, list[0].0, ok(json!([]))));
        assert!(a.contains(&Action::WsClose { socket: WS_KEYS[0] }));
        assert!(a.contains(&Action::TcpClose {
            socket: MQTT_KEYS[0]
        }));
    }
}
