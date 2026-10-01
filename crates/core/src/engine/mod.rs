//! The spec engine: the only interpreter of the YAML device format.
//!
//! One module drives every spec-driven device. Its behaviour is SPEC.md's, and
//! every rule in SPEC.md is implemented here once: templates in `template`,
//! framing in `framing`, OSC in `osc`, reply evaluation in `expect`.
//!
//! Commands are serialised per device. A reply is matched to the command in
//! flight and nothing else, so a late or unsolicited message can never be taken
//! as the answer to a later command. Liveness probes queue like commands for the
//! same reason.

mod expect;
mod framing;
mod osc;
mod telemetry;
pub(crate) mod template;

use std::collections::{BTreeMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use regex::Regex;
use serde_json::{Map, Value};

use crate::catalog::{DeviceSpec, ParamSpec, Params};
use crate::module::{
    Bind, CommandError, CommandId, Connection, Credentials, Cx, HttpRequest, HttpResponse, Key,
    Level, Millis, Module, OpenContext, Outcome, RequestId, TcpInput,
};
use expect::Reply;
use framing::{Framer, PacketFraming, PacketReader, ReplyFraming, SendFraming};
use template::{no_escape, percent_encode, render, sole_value, Values};

const SOCKET: Key = "device";
const REPLY: Key = "reply";
const PROBE: Key = "probe";
const RECONNECT: Key = "reconnect";
const RENEW: Key = "telemetry-renew";
const POLL: Key = "telemetry-poll";

const DEFAULT_TIMEOUT: Millis = 2_000;
/// A device silent this long, with nothing in flight, is probed.
const PROBE_WHEN_IDLE: Millis = 10_000;
const RECONNECT_MIN: Millis = 1_000;
const RECONNECT_MAX: Millis = 30_000;

#[derive(Debug)]
enum Transport {
    LineTcp {
        port: u16,
        send: SendFraming,
        reply: ReplyFraming,
        ascii: bool,
        replies: bool,
        reply_match: Option<Regex>,
    },
    OscUdp {
        port: u16,
        replies: bool,
    },
    OscTcp {
        port: u16,
        framing: PacketFraming,
    },
    Http {
        base: String,
        auth: HttpAuth,
    },
}

/// How an HTTP device authenticates (SPEC.md §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpAuth {
    None,
    Basic,
    Digest,
}

impl Transport {
    fn is_stream(&self) -> bool {
        matches!(self, Transport::LineTcp { .. } | Transport::OscTcp { .. })
    }

    fn replies(&self) -> bool {
        match self {
            Transport::LineTcp { replies, .. } | Transport::OscUdp { replies, .. } => *replies,
            Transport::OscTcp { .. } | Transport::Http { .. } => true,
        }
    }
}

/// One message of a command, ready to send.
#[derive(Debug)]
enum Outgoing {
    Bytes(Vec<u8>),
    Http(HttpRequest),
}

/// What reply the command in flight is waiting for.
#[derive(Debug, PartialEq)]
enum Await {
    /// The next reply message on a text stream.
    Text,
    /// An OSC message on this address; `None` for a probe, where any will do.
    Osc(Option<String>),
    Http(RequestId),
}

#[derive(Debug)]
struct InFlight {
    /// `None` for a liveness probe.
    id: Option<CommandId>,
    messages: VecDeque<(Outgoing, Option<String>)>,
    expect: Map<String, Value>,
    returns: String,
    awaiting: Option<Await>,
}

struct Job {
    id: Option<CommandId>,
    name: String,
    params: Params,
    /// For an internal job without a command: the message to send, such as a
    /// telemetry subscription. Without one, an internal job is the probe.
    item: Option<Value>,
}

pub(crate) struct SpecEngine {
    spec: Arc<DeviceSpec>,
    host: IpAddr,
    settings: Params,
    transport: Transport,
    timeout: Millis,
    probe: Option<Value>,
    link: Connection,
    queue: VecDeque<Job>,
    current: Option<InFlight>,
    backoff: Millis,
    last_heard: Millis,
    framer: Option<Framer>,
    packets: Option<PacketReader>,
    next_request: RequestId,
    telemetry: telemetry::Telemetry,
    /// The path and query of each HTTP request in flight, so its reply can be
    /// offered to the telemetry rules for that path.
    request_paths: std::collections::HashMap<RequestId, String>,
    /// Set when the device refuses the configured credential. Terminal: the
    /// credential is never presented again, since repeated failures can lock
    /// a device out. The host re-opens the device with corrected settings.
    refused: Option<String>,
}

fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

impl SpecEngine {
    /// `Err` names what in the spec the engine does not implement.
    pub(crate) fn new(spec: Arc<DeviceSpec>, ctx: OpenContext) -> Result<SpecEngine, String> {
        let t = spec.transport.clone().ok_or("spec has no transport")?;
        let kind = str_field(&t, "type").ok_or("transport has no type")?;
        let spec_port = t
            .get("port")
            .and_then(Value::as_u64)
            .ok_or("transport has no port")? as u16;
        let port = ctx.port.unwrap_or(spec_port);
        let timeout = t
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_TIMEOUT);

        let transport = match kind {
            "line-tcp" => {
                let framing = str_field(&t, "framing").unwrap_or("terminated");
                let open = str_field(&t, "open").unwrap_or("").to_string();
                let close = str_field(&t, "close").unwrap_or("").to_string();
                let send = match framing {
                    "delimited" => SendFraming::Delimited {
                        open: open.clone(),
                        close: close.clone(),
                    },
                    "terminated" => SendFraming::Terminated(match str_field(&t, "terminator") {
                        Some("cr") => "\r",
                        Some("lf") => "\n",
                        Some("crlf") => "\r\n",
                        _ => return Err("terminated framing needs a terminator".into()),
                    }),
                    "block" => SendFraming::Block,
                    other => return Err(format!("line-tcp framing '{other}' is not implemented")),
                };
                let reply = match (str_field(&t, "reply_framing"), framing) {
                    (Some("line"), _) => ReplyFraming::Line,
                    (Some("block"), _) => ReplyFraming::Block,
                    (Some("headed-block"), _) => ReplyFraming::HeadedBlock,
                    (None, "block") => ReplyFraming::Block,
                    (None, "delimited") => ReplyFraming::Delimited { open, close },
                    (None, _) => ReplyFraming::Line,
                    (Some(other), _) => {
                        return Err(format!("reply framing '{other}' is not implemented"))
                    }
                };
                let reply_match = match str_field(&t, "reply_match") {
                    Some(p) => Some(Regex::new(p).map_err(|e| format!("reply_match: {e}"))?),
                    None => None,
                };
                Transport::LineTcp {
                    port,
                    send,
                    reply,
                    ascii: str_field(&t, "encoding") != Some("utf-8"),
                    replies: str_field(&t, "reply") != Some("none"),
                    reply_match,
                }
            }
            "osc-udp" => Transport::OscUdp {
                port,
                replies: str_field(&t, "reply") == Some("to-source"),
            },
            "osc-tcp" => Transport::OscTcp {
                port,
                framing: match str_field(&t, "framing") {
                    Some("slip") => PacketFraming::Slip,
                    Some("length-prefixed") => PacketFraming::LengthPrefixed,
                    other => return Err(format!("osc-tcp framing {other:?} is not implemented")),
                },
            },
            "http" => {
                let scheme = str_field(&t, "scheme").unwrap_or("http");
                let auth = match str_field(&t, "auth").unwrap_or("none") {
                    "none" => HttpAuth::None,
                    "basic" => HttpAuth::Basic,
                    "digest" => HttpAuth::Digest,
                    other => return Err(format!("http auth '{other}' is not implemented")),
                };
                let host = match ctx.host {
                    IpAddr::V6(v6) => format!("[{v6}]"),
                    v4 => v4.to_string(),
                };
                Transport::Http {
                    base: format!("{scheme}://{host}:{port}"),
                    auth,
                }
            }
            other => return Err(format!("transport '{other}' is not implemented")),
        };

        let probe = t.get("probe").cloned();
        let telemetry = telemetry::Telemetry::parse(spec.telemetry.as_ref(), &spec.state)?;
        Ok(SpecEngine {
            spec,
            host: ctx.host,
            settings: ctx.settings,
            transport,
            timeout,
            probe,
            link: Connection::Connecting,
            queue: VecDeque::new(),
            current: None,
            backoff: RECONNECT_MIN,
            last_heard: 0,
            framer: None,
            packets: None,
            next_request: 1,
            telemetry,
            request_paths: Default::default(),
            refused: None,
        })
    }

    fn port(&self) -> u16 {
        match &self.transport {
            Transport::LineTcp { port, .. }
            | Transport::OscUdp { port, .. }
            | Transport::OscTcp { port, .. } => *port,
            Transport::Http { .. } => 0,
        }
    }

    fn set_link(&mut self, cx: &mut Cx, link: Connection) {
        if self.link != link {
            self.link = link.clone();
            cx.connection(link);
        }
    }

    fn values<'a>(
        &'a self,
        params: &'a Params,
        specs: &'a BTreeMap<String, ParamSpec>,
    ) -> Values<'a> {
        Values {
            params,
            param_specs: specs,
            settings: &self.settings,
            setting_specs: &self.spec.settings,
        }
    }

    // ── Building messages ────────────────────────────────────────────────

    /// Render one `send` item. Returns the message and, for OSC with an
    /// `expect.address`, nothing extra: the awaited address is rendered
    /// separately.
    fn build(&self, item: &Value, values: &Values) -> Result<Outgoing, String> {
        match &self.transport {
            Transport::LineTcp { send, ascii, .. } => {
                let template = item.as_str().ok_or("a line-tcp message must be a string")?;
                let payload = render(template, values, no_escape)?;
                let framed = framing::frame(send, &payload);
                if *ascii && !framed.is_ascii() {
                    return Err("the message contains non-ASCII characters".into());
                }
                Ok(Outgoing::Bytes(framed.into_bytes()))
            }
            Transport::OscUdp { .. } | Transport::OscTcp { .. } => {
                let packet = build_osc(item, values)?;
                Ok(Outgoing::Bytes(match &self.transport {
                    Transport::OscTcp { framing, .. } => framing::frame_packet(*framing, &packet),
                    _ => packet,
                }))
            }
            Transport::Http { base, auth } => {
                let method = str_field(item, "method").unwrap_or("GET");
                let method: &'static str = match method {
                    "GET" => "GET",
                    "POST" => "POST",
                    "PUT" => "PUT",
                    "DELETE" => "DELETE",
                    other => return Err(format!("method {other} is not supported")),
                };
                let path = render(
                    str_field(item, "path").unwrap_or("/"),
                    values,
                    percent_encode,
                )?;
                let mut url = format!("{base}{path}");
                if let Some(raw) = str_field(item, "raw_query") {
                    url.push('?');
                    url.push_str(&render(raw, values, no_escape)?);
                } else if let Some(query) = item.get("query").and_then(Value::as_object) {
                    let mut pairs = Vec::new();
                    for (k, v) in query {
                        let v = render(v.as_str().unwrap_or(""), values, no_escape)?;
                        pairs.push(format!("{}={}", percent_encode(k), percent_encode(&v)));
                    }
                    url.push('?');
                    url.push_str(&pairs.join("&"));
                }
                let mut headers = Vec::new();
                let setting = |name: &str| {
                    self.settings
                        .get(name)
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                };
                let (user, pass) = (setting("username"), setting("password"));
                if *auth == HttpAuth::Basic {
                    use base64::Engine;
                    let token =
                        base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
                    headers.push(("Authorization".to_string(), format!("Basic {token}")));
                }
                let body = match str_field(item, "body") {
                    Some(b) => Some(render(b, values, no_escape)?.into_bytes()),
                    None => None,
                };
                if let Some(ct) = str_field(item, "content_type") {
                    headers.push(("Content-Type".into(), ct.into()));
                }
                Ok(Outgoing::Http(HttpRequest {
                    method,
                    url,
                    headers,
                    body,
                    timeout: Some(self.timeout),
                    accept_invalid_certs: false,
                    // Basic devices that answer with a Digest challenge get it
                    // answered too (SPEC.md §2).
                    digest: (*auth != HttpAuth::None).then_some(Credentials {
                        username: user,
                        password: pass,
                    }),
                }))
            }
        }
    }

    fn prepare(&mut self, job: &Job) -> Result<InFlight, String> {
        let spec = self.spec.clone();
        let empty_specs = BTreeMap::new();
        let (items, expect, returns, param_specs): (
            Vec<Value>,
            Map<String, Value>,
            String,
            &BTreeMap<String, ParamSpec>,
        ) = match job.id {
            // A probe: any reply at all means the device is there.
            None => {
                let item = match &job.item {
                    Some(item) => item.clone(),
                    None => self.probe.clone().ok_or("no probe")?,
                };
                (vec![item], Map::new(), "ack".into(), &empty_specs)
            }
            Some(_) => {
                let command = spec.commands.get(&job.name).ok_or("unknown command")?;
                let send = command.send.clone().ok_or("command has no send")?;
                let items = match send {
                    Value::Array(items) => items,
                    one => vec![one],
                };
                let expect = command
                    .expect
                    .as_ref()
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                (items, expect, command.returns.clone(), &command.params)
            }
        };
        let values = self.values(&job.params, param_specs);
        let awaited_address = match expect.get("address").and_then(Value::as_str) {
            Some(a) => Some(render(a, &values, no_escape)?),
            // A telemetry query is answered on its own address; anything else
            // arriving meanwhile (a pushed change) is not its reply.
            None => match (&job.item, &self.transport) {
                (
                    Some(Value::String(address)),
                    Transport::OscUdp { .. } | Transport::OscTcp { .. },
                ) => Some(address.clone()),
                _ => None,
            },
        };
        let mut messages = VecDeque::new();
        for item in &items {
            // OSC probes are bare addresses in the spec.
            let item = match (item, &self.transport) {
                (Value::String(address), Transport::OscUdp { .. } | Transport::OscTcp { .. }) => {
                    serde_json::json!({ "address": address })
                }
                _ => item.clone(),
            };
            let outgoing = self.build(&item, &values)?;
            messages.push_back((outgoing, awaited_address.clone()));
        }
        Ok(InFlight {
            id: job.id,
            messages,
            expect,
            returns,
            awaiting: None,
        })
    }

    // ── Running commands ─────────────────────────────────────────────────

    fn can_send(&self) -> bool {
        match self.link {
            Connection::Connected | Connection::Unmonitored => true,
            // HTTP requests are independent; a UDP device may simply be quiet.
            _ => !self.transport.is_stream(),
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        while self.current.is_none() {
            if !self.can_send() && self.queue.front().is_some_and(|j| j.id.is_some()) {
                // Fail rather than hold: the caller should not wait out a
                // reconnect it cannot see.
                let job = self.queue.pop_front().unwrap();
                cx.complete(job.id.unwrap(), Err(CommandError::NotConnected));
                continue;
            }
            let Some(job) = self.queue.pop_front() else {
                return;
            };
            match self.prepare(&job) {
                Ok(flight) => {
                    self.current = Some(flight);
                    self.send_next(cx);
                }
                Err(message) => {
                    if let Some(id) = job.id {
                        cx.complete(id, Err(CommandError::InvalidParams { message }));
                    }
                }
            }
        }
    }

    /// Send the next message of the command in flight, or finish it.
    fn send_next(&mut self, cx: &mut Cx) {
        let replies = self.transport.replies();
        loop {
            let Some(flight) = self.current.as_mut() else {
                return;
            };
            let waits = replies && (flight.returns != "none" || flight.id.is_none());
            let Some((outgoing, address)) = flight.messages.pop_front() else {
                // Everything sent and, where the protocol replies, answered.
                let flight = self.current.take().unwrap();
                if let Some(id) = flight.id {
                    cx.complete(id, Ok(Outcome::Unverified));
                }
                return;
            };
            let awaiting = match outgoing {
                Outgoing::Bytes(bytes) => {
                    match &self.transport {
                        Transport::OscUdp { port, .. } => {
                            cx.udp_send(SOCKET, SocketAddr::new(self.host, *port), bytes)
                        }
                        _ => cx.tcp_send(SOCKET, bytes),
                    }
                    match &self.transport {
                        Transport::LineTcp { .. } => Await::Text,
                        _ => Await::Osc(address),
                    }
                }
                Outgoing::Http(request) => {
                    let id = self.next_request;
                    self.next_request += 1;
                    let target = request.url.splitn(4, '/').nth(3).unwrap_or("");
                    self.request_paths.insert(id, format!("/{target}"));
                    cx.http(id, request);
                    Await::Http(id)
                }
            };
            if waits || matches!(awaiting, Await::Http(_)) {
                self.current.as_mut().unwrap().awaiting = Some(awaiting);
                cx.set_timer(REPLY, self.timeout);
                return;
            }
            let flight = self.current.as_ref().unwrap();
            if flight.messages.is_empty() {
                let flight = self.current.take().unwrap();
                if let Some(id) = flight.id {
                    cx.complete(id, Ok(Outcome::Unverified));
                }
                return;
            }
        }
    }

    /// A reply for the command in flight.
    fn reply(&mut self, cx: &mut Cx, reply: Reply) {
        cx.cancel_timer(REPLY);
        let Some(flight) = self.current.as_mut() else {
            return;
        };
        flight.awaiting = None;

        if flight.id.is_none() {
            // Probe answered.
            self.current = None;
            self.heard(cx);
            self.pump(cx);
            return;
        }

        let headed = matches!(
            self.transport,
            Transport::LineTcp {
                reply: ReplyFraming::HeadedBlock,
                ..
            }
        );
        let result = expect::evaluate(
            &flight.expect,
            &flight.returns,
            &self.spec.codes,
            &reply,
            headed,
        );
        let done = result.is_err() || flight.messages.is_empty();
        if done {
            let flight = self.current.take().unwrap();
            cx.complete(flight.id.unwrap(), result);
            self.pump(cx);
        } else {
            self.send_next(cx);
            if self.current.is_none() {
                self.pump(cx);
            }
        }
    }

    /// The device refused the credential: fail everything, stop all traffic,
    /// and report it until the device is opened again.
    fn refuse(&mut self, cx: &mut Cx, reason: String) {
        let auth = CommandError::Auth {
            message: reason.clone(),
        };
        cx.cancel_timer(REPLY);
        if let Some(id) = self.current.take().and_then(|f| f.id) {
            cx.complete(id, Err(auth.clone()));
        }
        for job in self.queue.drain(..) {
            if let Some(id) = job.id {
                cx.complete(id, Err(auth.clone()));
            }
        }
        cx.cancel_timer(PROBE);
        cx.cancel_timer(RECONNECT);
        cx.log(
            Level::Warning,
            format!("{reason}; no further requests until the device is opened again with corrected settings"),
        );
        self.set_link(
            cx,
            Connection::Unauthorized {
                reason: reason.clone(),
            },
        );
        self.refused = Some(reason);
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.last_heard = cx.now();
        cx.alive();
        if self.link != Connection::Connected && self.link != Connection::Unmonitored {
            self.set_link(cx, Connection::Connected);
        }
    }

    fn fail_all(&mut self, cx: &mut Cx, error: CommandError) {
        cx.cancel_timer(REPLY);
        if let Some(flight) = self.current.take() {
            if let Some(id) = flight.id {
                cx.complete(id, Err(error.clone()));
            }
        }
        for job in self.queue.drain(..) {
            if let Some(id) = job.id {
                cx.complete(id, Err(CommandError::NotConnected));
            }
        }
    }

    // ── Connection lifecycle ─────────────────────────────────────────────

    fn connect(&mut self, cx: &mut Cx) {
        let replies = self.transport.replies();
        match &self.transport {
            Transport::LineTcp { reply, .. } => {
                self.framer = Some(Framer::new(reply.clone()));
                cx.tcp_open(SOCKET, SocketAddr::new(self.host, self.port()));
            }
            Transport::OscTcp { framing, .. } => {
                self.packets = Some(PacketReader::new(*framing));
                cx.tcp_open(SOCKET, SocketAddr::new(self.host, self.port()));
            }
            Transport::OscUdp { .. } => {
                cx.udp_open(SOCKET, Bind::Ephemeral);
                self.send_on_connect(cx);
                self.start_telemetry(cx);
                if replies && self.probe.is_some() {
                    self.enqueue_probe(cx);
                } else {
                    self.set_link(cx, Connection::Unmonitored);
                }
                cx.set_timer(PROBE, PROBE_WHEN_IDLE);
            }
            Transport::Http { .. } => {
                if self.probe.is_some() {
                    self.enqueue_probe(cx);
                } else {
                    self.set_link(cx, Connection::Unmonitored);
                }
                cx.set_timer(PROBE, PROBE_WHEN_IDLE);
            }
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.fail_all(
            cx,
            CommandError::Transport {
                message: reason.clone(),
            },
        );
        cx.tcp_close(SOCKET);
        cx.cancel_timer(PROBE);
        cx.cancel_timer(RENEW);
        cx.cancel_timer(POLL);
        self.set_link(cx, Connection::Disconnected { reason });
        cx.set_timer(RECONNECT, self.backoff);
        self.backoff = (self.backoff * 2).min(RECONNECT_MAX);
    }

    /// The fixed connection sequence, SPEC.md §2. Nothing waits on a reply:
    /// it is not a handshake.
    fn send_on_connect(&mut self, cx: &mut Cx) {
        let empty = Params::new();
        let empty_specs = BTreeMap::new();
        for step in self.spec.on_connect.clone() {
            let (item, when_set) = match &step {
                Value::Object(m) if m.contains_key("when_set") => (
                    m.get("send").cloned().unwrap_or(Value::Null),
                    m.get("when_set").and_then(Value::as_str),
                ),
                other => (other.clone(), None),
            };
            if let Some(setting) = when_set {
                let set = self
                    .settings
                    .get(setting)
                    .is_some_and(|v| !v.is_null() && v.as_str() != Some(""));
                if !set {
                    continue;
                }
            }
            let values = self.values(&empty, &empty_specs);
            match self.build(&item, &values) {
                Ok(Outgoing::Bytes(bytes)) => match &self.transport {
                    Transport::OscUdp { port, .. } => {
                        cx.udp_send(SOCKET, SocketAddr::new(self.host, *port), bytes)
                    }
                    _ => cx.tcp_send(SOCKET, bytes),
                },
                Ok(Outgoing::Http(request)) => {
                    let id = self.next_request;
                    self.next_request += 1;
                    cx.http(id, request);
                }
                Err(e) => cx.log(Level::Warning, format!("on_connect step not sent: {e}")),
            }
        }
    }

    fn enqueue_probe(&mut self, cx: &mut Cx) {
        if self.probe.is_none() || self.queue.iter().any(|j| j.id.is_none()) {
            return;
        }
        self.queue.push_back(Job {
            id: None,
            name: String::new(),
            params: Params::new(),
            item: None,
        });
        self.pump(cx);
    }

    /// Send telemetry messages. On a line transport whose device answers, each
    /// goes through the command queue so its reply is not mistaken for a
    /// command's; otherwise it is sent straight away, like `on_connect`.
    fn send_telemetry(&mut self, cx: &mut Cx, items: Vec<Value>, poll: bool) {
        // HTTP requests are always queued: each reply is matched to its
        // request. Poll items are queries, so on any transport that answers
        // they go one at a time, which also paces a long list.
        let queued = match self.transport {
            Transport::LineTcp { replies, .. } => replies,
            Transport::Http { .. } => true,
            Transport::OscUdp { replies, .. } => poll && replies,
            Transport::OscTcp { .. } => poll,
        };
        for item in items {
            if queued {
                self.queue.push_back(Job {
                    id: None,
                    name: String::new(),
                    params: Params::new(),
                    item: Some(item),
                });
                continue;
            }
            let item = match (&item, &self.transport) {
                (Value::String(address), Transport::OscUdp { .. } | Transport::OscTcp { .. }) => {
                    serde_json::json!({ "address": address })
                }
                _ => item,
            };
            let empty = Params::new();
            let empty_specs = BTreeMap::new();
            let values = self.values(&empty, &empty_specs);
            match self.build(&item, &values) {
                Ok(Outgoing::Bytes(bytes)) => match &self.transport {
                    Transport::OscUdp { port, .. } => {
                        cx.udp_send(SOCKET, SocketAddr::new(self.host, *port), bytes)
                    }
                    _ => cx.tcp_send(SOCKET, bytes),
                },
                Ok(Outgoing::Http(_)) => {
                    cx.log(Level::Warning, "telemetry over HTTP is not implemented")
                }
                Err(e) => cx.log(Level::Warning, format!("telemetry message not sent: {e}")),
            }
        }
        self.pump(cx);
    }

    /// After connecting: subscribe, poll once, and schedule both.
    fn start_telemetry(&mut self, cx: &mut Cx) {
        if self.telemetry.is_empty() {
            return;
        }
        let subscribe = self.telemetry.subscribe.clone();
        let poll = self.telemetry.poll.clone();
        self.send_telemetry(cx, subscribe, false);
        self.send_telemetry(cx, poll, true);
        if let Some(every) = self.telemetry.renew_every {
            cx.set_timer(RENEW, every);
        }
        if let Some(every) = self.telemetry.poll_every {
            cx.set_timer(POLL, every);
        }
    }

    /// Offer a text message to the telemetry rules: a line or block from the
    /// device, a text HTTP reply, or a notification a native extension
    /// received on the spec's behalf.
    pub(crate) fn apply_text(&self, cx: &mut Cx, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if let Some(patch) = self.telemetry.apply(&telemetry::Inbound::Text(text)) {
            cx.state(patch);
        }
    }

    fn inbound_text(&mut self, cx: &mut Cx, message: String) {
        self.apply_text(cx, &message);
        let waiting = matches!(
            self.current.as_ref().and_then(|f| f.awaiting.as_ref()),
            Some(Await::Text)
        );
        if let Transport::LineTcp {
            reply_match: Some(re),
            ..
        } = &self.transport
        {
            if !re.is_match(&message) {
                // Unsolicited: status the device pushes on its own.
                self.last_heard = cx.now();
                cx.alive();
                return;
            }
        }
        if waiting {
            self.reply(cx, Reply::Text(message));
        } else {
            self.last_heard = cx.now();
            cx.alive();
        }
    }

    fn inbound_osc(&mut self, cx: &mut Cx, packet: &[u8]) {
        for message in osc::decode(packet) {
            if let Some(patch) = self.telemetry.apply(&telemetry::Inbound::Osc {
                address: &message.address,
                args: &message.args,
            }) {
                cx.state(patch);
            }
            let matches = match self.current.as_ref().and_then(|f| f.awaiting.as_ref()) {
                Some(Await::Osc(None)) => true,
                Some(Await::Osc(Some(address))) => *address == message.address,
                _ => false,
            };
            if matches {
                self.reply(cx, Reply::Osc { args: message.args });
            } else {
                self.last_heard = cx.now();
                cx.alive();
            }
        }
    }
}

/// Encode one OSC `send` item: `{address, args: [{value, type}]}`.
fn build_osc(item: &Value, values: &Values) -> Result<Vec<u8>, String> {
    let address = render(
        str_field(item, "address").ok_or("OSC message has no address")?,
        values,
        no_escape,
    )?;
    let mut args = Vec::new();
    for arg in item
        .get("args")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let template = str_field(arg, "value").ok_or("OSC argument has no value")?;
        let kind = str_field(arg, "type").ok_or("OSC argument has no type")?;
        let sole = sole_value(template, values);
        args.push(match kind {
            "float" => match sole {
                Some((v, _)) if v.is_number() => osc::Arg::Float(v.as_f64().unwrap() as f32),
                _ => osc::Arg::Float(
                    render(template, values, no_escape)?
                        .parse()
                        .map_err(|_| format!("'{template}' is not a float"))?,
                ),
            },
            "int" => osc::Arg::Int(
                render(template, values, no_escape)?
                    .parse()
                    .map_err(|_| format!("'{template}' is not an int"))?,
            ),
            "string" => osc::Arg::Str(render(template, values, no_escape)?),
            "blob" => osc::Arg::Blob(render(template, values, no_escape)?.into_bytes()),
            other => return Err(format!("OSC type '{other}' is not supported")),
        });
    }
    Ok(osc::encode(&address, &args))
}

impl Module for SpecEngine {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if let Some(reason) = &self.refused {
            let message = reason.clone();
            cx.complete(id, Err(CommandError::Auth { message }));
            return;
        }
        // Ahead of queued telemetry queries, behind other commands: an
        // operator's command does not wait for a poll of every channel.
        // Replies are still matched one at a time, in sending order.
        let at = self
            .queue
            .iter()
            .position(|j| j.id.is_none() && j.item.is_some())
            .unwrap_or(self.queue.len());
        self.queue.insert(
            at,
            Job {
                id: Some(id),
                name: name.to_string(),
                params: params.clone(),
                item: None,
            },
        );
        self.pump(cx);
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.backoff = RECONNECT_MIN;
                self.last_heard = cx.now();
                self.send_on_connect(cx);
                self.set_link(cx, Connection::Connected);
                cx.set_timer(PROBE, PROBE_WHEN_IDLE);
                self.start_telemetry(cx);
                self.pump(cx);
            }
            TcpInput::Data(bytes) => {
                if let Some(framer) = self.framer.as_mut() {
                    for message in framer.feed(&bytes) {
                        self.inbound_text(cx, message);
                    }
                } else if let Some(reader) = self.packets.as_mut() {
                    for packet in reader.feed(&bytes) {
                        self.inbound_osc(cx, &packet);
                    }
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, _from: SocketAddr, data: &[u8]) {
        self.inbound_osc(cx, data);
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        if let (Some(path), Ok(response)) = (self.request_paths.remove(&id), &result) {
            if (200..300).contains(&response.status) {
                let inbound = telemetry::Inbound::Http {
                    path: &path,
                    body: &response.body,
                };
                if let Some(patch) = self.telemetry.apply(&inbound) {
                    cx.state(patch);
                }
                // A text reply ("p1" from a Panasonic camera) is also offered to
                // the rules for text messages.
                if let Ok(text) = std::str::from_utf8(&response.body) {
                    self.apply_text(cx, text);
                }
            }
        }
        let ours = matches!(
            self.current.as_ref().and_then(|f| f.awaiting.as_ref()),
            Some(Await::Http(r)) if *r == id
        );
        if !ours {
            return;
        }
        let credentialed = matches!(
            self.transport,
            Transport::Http {
                auth: HttpAuth::Basic,
                ..
            }
        );
        match result {
            Ok(response) if credentialed && matches!(response.status, 401 | 403) => self.refuse(
                cx,
                format!(
                    "the device refused the credential (HTTP {})",
                    response.status
                ),
            ),
            Ok(response) => self.reply(
                cx,
                Reply::Http {
                    status: response.status,
                    body: response.body,
                },
            ),
            Err(message) => {
                cx.cancel_timer(REPLY);
                let flight = self.current.take().unwrap();
                match flight.id {
                    Some(id) => cx.complete(id, Err(CommandError::Transport { message })),
                    None => self.set_link(cx, Connection::Disconnected { reason: message }),
                }
                self.pump(cx);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if self.refused.is_some() {
            return;
        }
        match key {
            REPLY => {
                let Some(flight) = self.current.take() else {
                    return;
                };
                match flight.id {
                    Some(id) => cx.complete(id, Err(CommandError::Timeout)),
                    None => {
                        if !self.transport.is_stream() {
                            self.set_link(
                                cx,
                                Connection::Disconnected {
                                    reason: "no reply to probe".into(),
                                },
                            );
                        }
                    }
                }
                if self.transport.is_stream() {
                    // A late reply would otherwise be read as the answer to
                    // the next command. Start the stream afresh.
                    self.lost(cx, "no reply within the timeout".into());
                } else {
                    self.pump(cx);
                }
            }
            PROBE => {
                let idle = cx.now().saturating_sub(self.last_heard) >= PROBE_WHEN_IDLE;
                let replies = self.transport.replies();
                if idle && replies && self.current.is_none() && self.queue.is_empty() {
                    self.enqueue_probe(cx);
                }
                cx.set_timer(PROBE, PROBE_WHEN_IDLE);
            }
            RECONNECT => self.connect(cx),
            RENEW => {
                let items = self.telemetry.subscribe.clone();
                self.send_telemetry(cx, items, false);
                if let Some(every) = self.telemetry.renew_every {
                    cx.set_timer(RENEW, every);
                }
            }
            POLL => {
                let items = self.telemetry.poll.clone();
                self.send_telemetry(cx, items, true);
                if let Some(every) = self.telemetry.poll_every {
                    cx.set_timer(POLL, every);
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(SOCKET);
    }
}

#[cfg(test)]
mod vectors;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::module::Action;
    use serde_json::json;
    use std::net::Ipv4Addr;

    /// ProPresenter's HTTP spec, switched to basic auth.
    fn credentialed() -> SpecEngine {
        let mut spec = Catalog::embedded().device("propresenter").unwrap().clone();
        spec.transport.as_mut().unwrap()["auth"] = json!("basic");
        let settings = json!({"username": "u", "password": "wrong"})
            .as_object()
            .unwrap()
            .clone();
        SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: None,
                model: "propresenter-7".into(),
                channels: None,
                settings,
            },
        )
        .unwrap()
    }

    fn http_ids(actions: &[Action]) -> Vec<RequestId> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Http { id, .. } => Some(*id),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn commands_go_ahead_of_queued_telemetry_queries() {
        let spec = Catalog::embedded().device("behringer-x32").unwrap().clone();
        let mut e = SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: None,
                model: "x32".into(),
                channels: Some(32),
                settings: Params::new(),
            },
        )
        .unwrap();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        cx.take();
        // The first of the current-value queries is in flight; the rest wait.
        let queued = e.queue.len();
        assert!(queued > 100, "{queued} queries queued");

        let mut cx = Cx::new(1);
        let params = json!({"channel": 3}).as_object().unwrap().clone();
        e.command(&mut cx, 9, "get_channel_name", &params);
        assert_eq!(e.queue.front().and_then(|j| j.id), Some(9));

        // Once the query in flight is answered, the command is sent next.
        let reply = osc::encode("/ch/01/mix/on", &[osc::Arg::Int(1)]);
        let mut cx = Cx::new(2);
        e.datagram(
            &mut cx,
            SOCKET,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10023),
            &reply,
        );
        let sent: Vec<Vec<u8>> = cx
            .take()
            .into_iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } => Some(data),
                _ => None,
            })
            .collect();
        assert_eq!(sent, [osc::encode("/ch/03/config/name", &[])]);
    }

    #[test]
    fn a_refused_credential_stops_probing_and_fails_commands() {
        let mut e = credentialed();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let probe = http_ids(&cx.take())[0];

        let mut cx = Cx::new(10);
        e.http_response(
            &mut cx,
            probe,
            Ok(HttpResponse {
                status: 401,
                body: Vec::new(),
            }),
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a.contains(&Action::Alive));
        assert!(a.contains(&Action::CancelTimer { key: PROBE }));

        // The probe timer, if it fires anyway, sends nothing.
        let mut cx = Cx::new(PROBE_WHEN_IDLE + 10);
        e.timer(&mut cx, PROBE);
        assert!(cx.take().is_empty());

        let mut cx = Cx::new(PROBE_WHEN_IDLE + 20);
        e.command(&mut cx, 5, "anything", &Params::new());
        let a = cx.take();
        assert!(http_ids(&a).is_empty());
        assert!(matches!(
            &a[..],
            [Action::Complete {
                id: 5,
                result: Err(CommandError::Auth { .. })
            }]
        ));
    }
}
