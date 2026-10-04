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
mod oauth;
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
    Level, Millis, Module, OpenContext, Outcome, RequestId, SettingsUpdate, SseInput, TcpInput,
    WsInput, WsRequest,
};
use expect::Reply;
use framing::{Framer, PacketFraming, PacketReader, ReplyFraming, SendFraming};
use oauth::{OAuth, Refreshed, Validated};
use template::{
    no_escape, percent_encode, render, sole_converted, sole_value, Conversions, Maps, Values,
};

const SOCKET: Key = "device";
/// The websocket a spec on another transport receives pushed state on
/// (`telemetry.websocket`).
const PUSH: Key = "push";
const PUSH_RECONNECT: Key = "push-reconnect";
/// Engine.IO v3: the client's ping on the push websocket.
const PUSH_PING: Key = "push-ping";
/// `telemetry.websocket.every_ms`: the push websocket's messages again.
const PUSH_RENEW: Key = "push-renew";
/// `telemetry.websocket.idle_ms`: the push websocket gone quiet.
const PUSH_IDLE: Key = "push-idle";
/// The server-sent event stream (`telemetry.sse`).
const EVENTS: Key = "events";
const EVENTS_RECONNECT: Key = "events-reconnect";
const REPLY: Key = "reply";
const PROBE: Key = "probe";
const RECONNECT: Key = "reconnect";
const RENEW: Key = "telemetry-renew";
const POLL: Key = "telemetry-poll";
const PROMPT: Key = "login-prompt";
/// A token refresh that failed, tried again.
const OAUTH_RETRY: Key = "oauth-retry";
/// `oauth.validate`: the access token checked again.
const OAUTH_VALIDATE: Key = "oauth-validate";

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
        /// An error answer, which names nothing it answers: taken as the
        /// reply of a command waiting with `expect.reply_contains`.
        error_match: Option<Regex>,
    },
    /// Text messages, one per datagram (ChamSys, XPression over UDP).
    LineUdp {
        port: u16,
        send: SendFraming,
        ascii: bool,
        replies: bool,
        listen: Option<u16>,
        /// As for line-tcp: a datagram not matching it is never a reply.
        reply_match: Option<Regex>,
    },
    OscUdp {
        port: u16,
        replies: bool,
        /// A fixed local port to receive on, for devices that send to a
        /// configured destination rather than back to the sender.
        listen: Option<u16>,
    },
    OscTcp {
        port: u16,
        framing: PacketFraming,
    },
    Http {
        base: String,
        auth: HttpAuth,
        /// Accept a device's self-signed certificate over HTTPS.
        accept_invalid_certs: bool,
        /// Statuses that mean the credential was refused.
        refusal: Vec<u16>,
        /// `transport.headers`: each header's name and its template over
        /// settings, sent on every request (Twitch's `Client-Id`).
        headers: Vec<(String, String)>,
        /// `transport.refusal_json`: JSON paths to regexes; an error answer
        /// whose body matches them all is a refusal too, whatever its status
        /// (the Graph API's `error.code` 190 under HTTP 400).
        refusal_json: Vec<(String, Regex)>,
    },
    /// Text messages over a websocket, usually JSON.
    Ws {
        port: u16,
        request: WsRequest,
        auth: HttpAuth,
        replies: bool,
        /// As for line-tcp: a message not matching it is never a reply taken
        /// in order.
        reply_match: Option<Regex>,
    },
}

/// `transport.session`: a login whose answer carries a session (a cookie or a
/// header) that later requests send back, renewed when the device says it
/// has ended (Magewell's status 37).
#[derive(Debug)]
struct SessionSpec {
    /// The login request for the device's model.
    login: Value,
    capture: Capture,
    /// An answer to any request meaning the session ended: log in again and
    /// send the request once more.
    relogin_status: Vec<u16>,
    relogin_json: Vec<(String, Regex)>,
    /// An answer to the login refusing the credential: terminal.
    refused_status: Vec<u16>,
    refused_json: Vec<(String, Regex)>,
}

/// What of the login's answer is the session.
#[derive(Debug, Clone, PartialEq)]
enum Capture {
    /// The `name=value` of every `Set-Cookie` (`None`), or of the one named,
    /// sent back as `Cookie`.
    Cookie(Option<String>),
    /// A response header's value, sent back in a header of the same name.
    Header(String),
}

impl SessionSpec {
    fn parse(v: &Value, model: &str) -> Result<SessionSpec, String> {
        let login = match v.get("login") {
            Some(Value::Array(items)) => items.iter().find(|i| for_model(i, model)).cloned(),
            Some(one) if for_model(one, model) => Some(one.clone()),
            _ => None,
        }
        .ok_or(format!("transport.session has no login for model {model}"))?;
        let capture = match v.get("capture") {
            Some(c) => match (c.get("cookie"), c.get("header")) {
                (Some(Value::String(name)), None) if name == "*" => Capture::Cookie(None),
                (Some(Value::String(name)), None) => Capture::Cookie(Some(name.clone())),
                (None, Some(Value::String(h))) => Capture::Header(h.clone()),
                _ => {
                    return Err(
                        "transport.session.capture is {cookie: name} or {header: name}".into(),
                    )
                }
            },
            None => Capture::Cookie(None),
        };
        let statuses = |key: &str| -> Vec<u16> {
            v.get(key)
                .and_then(|m| m.get("status"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_u64)
                .map(|s| s as u16)
                .collect()
        };
        let json = |key: &str| -> Result<Vec<(String, Regex)>, String> {
            let mut out = Vec::new();
            for (path, re) in v
                .get(key)
                .and_then(|m| m.get("json"))
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let re = re
                    .as_str()
                    .ok_or(format!("transport.session.{key}.json values are regexes"))?;
                out.push((
                    path.clone(),
                    Regex::new(re).map_err(|e| format!("transport.session.{key}: {e}"))?,
                ));
            }
            Ok(out)
        };
        Ok(SessionSpec {
            login,
            capture,
            relogin_status: statuses("relogin"),
            relogin_json: json("relogin")?,
            refused_status: statuses("refused"),
            refused_json: json("refused")?,
        })
    }

    /// The session in a login's answer, as the header to send it in.
    fn captured(&self, response: &HttpResponse) -> Option<(String, String)> {
        match &self.capture {
            Capture::Cookie(name) => {
                let cookies: Vec<&str> = response
                    .header_values("set-cookie")
                    .filter_map(|v| v.split(';').next())
                    .map(str::trim)
                    .filter(|c| {
                        c.split_once('=').is_some_and(|(n, _)| {
                            name.as_deref().is_none_or(|want| n.trim() == want)
                        })
                    })
                    .collect();
                (!cookies.is_empty()).then(|| ("Cookie".to_string(), cookies.join("; ")))
            }
            Capture::Header(h) => response
                .header_values(h)
                .next()
                .filter(|v| !v.is_empty())
                .map(|v| (h.clone(), v.to_string())),
        }
    }
}

/// Whether every selector matches the JSON body (at least one selector).
fn body_matches(selectors: &[(String, Regex)], body: &[u8]) -> bool {
    if selectors.is_empty() {
        return false;
    }
    let Ok(doc) = serde_json::from_slice::<Value>(body) else {
        return false;
    };
    selectors.iter().all(|(path, re)| {
        let text = match expect::json_path(&doc, path) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => return false,
            Some(other) => other.to_string(),
        };
        re.is_match(&text)
    })
}

/// Whether a `send` item, probe or login applies to the device's model: one
/// naming `models` applies only to those (SPEC.md §4, "Messages per model").
fn for_model(item: &Value, model: &str) -> bool {
    match item.get("models").and_then(Value::as_array) {
        Some(models) => models.iter().any(|m| m.as_str() == Some(model)),
        None => true,
    }
}

/// The websocket a spec on another transport takes pushed state from.
#[derive(Debug)]
struct Push {
    request: WsRequest,
    /// Messages sent each time it opens: the subscriptions.
    send: Vec<Value>,
    /// `every_ms`: the `send` messages again at this interval while it is
    /// open, for a device that drops a client gone quiet (mimoLive's
    /// keepalive) or answers state only when asked (FreeShow's variables).
    every: Option<Millis>,
    /// Whether its opening request carries the transport's credential, or
    /// its URL a setting (a token in the query).
    credentialed: bool,
    /// Socket.IO over Engine.IO, when the device speaks it.
    socketio: Option<SocketIo>,
    /// `url`: an absolute URL on any host, a template over settings
    /// rendered at each opening, so a refreshed token in it is the current
    /// one (Restream's `?accessToken=`).
    url: Option<String>,
    /// `idle_ms`: reopened when nothing arrives for this long (a service
    /// that sends keepalives, so silence means a dead connection).
    idle: Option<Millis>,
}

/// Engine.IO's query, after a URL with or without one.
fn engine_io_query(url: &str, sio: &SocketIo) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{sep}EIO={}&transport=websocket", sio.version)
}

/// A push websocket URL a spec renders: `wss`, or `ws` only to this machine.
/// The URL may hold a token, so an error never shows it.
fn check_push_url(url: &str) -> Result<(), String> {
    let authority = |rest: &str| rest.split(['/', '?']).next().unwrap_or("").to_string();
    if let Some(rest) = url.strip_prefix("wss://") {
        if !authority(rest).is_empty() {
            return Ok(());
        }
    }
    if let Some(rest) = url.strip_prefix("ws://") {
        let authority = authority(rest);
        let host = if authority.starts_with('[') {
            authority.split(']').next().map(|h| format!("{h}]"))
        } else {
            authority.split(':').next().map(str::to_string)
        }
        .unwrap_or_default();
        let loopback = host == "localhost"
            || host == "[::1]"
            || host
                .parse::<std::net::Ipv4Addr>()
                .is_ok_and(|ip| ip.is_loopback());
        if loopback {
            return Ok(());
        }
    }
    Err("telemetry.websocket.url must be wss, or ws to this machine (localhost, 127.0.0.1 or [::1])".into())
}

/// `telemetry.websocket.socketio`: Socket.IO's framing over the websocket.
/// Engine.IO packets are a digit then a payload: `0` open, `2` ping, `3`
/// pong, `4` message; a Socket.IO message is `4` then `0` connect, `1`
/// disconnect or `2` event, with an optional `/namespace,` before the
/// payload, so an event arrives as `42["name",{...}]`.
#[derive(Debug, Clone)]
struct SocketIo {
    /// Engine.IO protocol version: 4 (Socket.IO 3 and 4), where the server
    /// pings, or 3 (Socket.IO 2), where the client does.
    version: u8,
    /// The namespace prefix, `/name,`, or empty for the main namespace.
    namespace: String,
    /// Engine.IO v3: the ping interval the server's open packet gave.
    ping_every: Option<Millis>,
    /// `socketio.auth`: a JSON object (a template over settings) sent with
    /// the namespace connect, Socket.IO's handshake `auth` (FreeShow's
    /// `{"token": ...}`).
    auth: Option<String>,
}

/// How an HTTP or websocket device authenticates (SPEC.md §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpAuth {
    None,
    Basic,
    Digest,
    /// The `token` setting as `Authorization: Bearer <token>`.
    Bearer,
    /// The `token` setting as is, in the header the transport's
    /// `auth_header` names (mimoLive's `X-MimoLive-Password-SHA256`).
    Header,
    /// The `access_token` setting as Bearer, refreshed with the
    /// `refresh_token` setting (`transport.oauth`).
    OAuth2,
}

impl Transport {
    fn is_stream(&self) -> bool {
        matches!(
            self,
            Transport::LineTcp { .. } | Transport::OscTcp { .. } | Transport::Ws { .. }
        )
    }

    fn is_osc(&self) -> bool {
        matches!(self, Transport::OscUdp { .. } | Transport::OscTcp { .. })
    }

    fn replies(&self) -> bool {
        match self {
            Transport::LineTcp { replies, .. }
            | Transport::LineUdp { replies, .. }
            | Transport::OscUdp { replies, .. }
            | Transport::Ws { replies, .. } => *replies,
            Transport::OscTcp { .. } | Transport::Http { .. } => true,
        }
    }
}

/// One message of a command, ready to send.
#[derive(Debug)]
enum Outgoing {
    Bytes(Vec<u8>),
    Http(HttpRequest),
    Ws(String),
}

/// The OSC reply a message waits for.
#[derive(Debug, Clone)]
enum OscReply {
    /// A command's `expect.address`, or a query answered on its own address.
    Exact(String),
    /// A poll item's `reply_address` (ETC Eos: `/eos/out/get/...`).
    Pattern(Regex),
}

impl OscReply {
    fn matches(&self, address: &str) -> bool {
        match self {
            OscReply::Exact(a) => a == address,
            OscReply::Pattern(re) => re.is_match(address),
        }
    }
}

/// What reply the command in flight is waiting for.
#[derive(Debug)]
enum Await {
    /// The next reply message on a text stream, or with
    /// `expect.reply_contains` the first one holding that rendered text.
    Text(Option<String>),
    /// An OSC message on this address; `None` for a probe, where any will do.
    Osc(Option<OscReply>),
    Http(RequestId),
    /// A websocket message: the one whose JSON holds these values
    /// (`expect.reply_json`), or with `None` the next one, in order.
    Ws(Option<Vec<(String, String)>>),
}

impl Await {
    /// A reply that names what it answers: a late one cannot be taken for
    /// the answer to a later message, so a timeout need not reset a stream.
    fn addressed(&self) -> bool {
        matches!(
            self,
            Await::Osc(Some(_)) | Await::Ws(Some(_)) | Await::Text(Some(_))
        )
    }
}

#[derive(Debug)]
struct InFlight {
    /// `None` for a liveness probe.
    id: Option<CommandId>,
    messages: VecDeque<(Outgoing, Option<OscReply>)>,
    /// Over a websocket: what identifies the reply (`expect.reply_json`).
    ws_reply: Option<Vec<(String, String)>>,
    /// On a line transport: text the reply holds (`expect.reply_contains`).
    text_reply: Option<String>,
    expect: Map<String, Value>,
    returns: String,
    awaiting: Option<Await>,
    /// When the message now awaiting its reply was sent.
    sent_at: Millis,
    /// The HTTP request now awaiting its reply, kept to send again once
    /// after a refreshed OAuth token.
    last_http: Option<HttpRequest>,
    /// The request refused with the old token, waiting for the refresh.
    retry: Option<HttpRequest>,
    /// The token was refreshed for this request: a second refusal is final.
    refreshed: bool,
    /// The job, kept to send again after a fresh login
    /// (`transport.session.relogin`).
    job: Job,
}

#[derive(Debug, Clone)]
struct Job {
    id: Option<CommandId>,
    name: String,
    params: Params,
    /// For an internal job without a command: the message to send, such as a
    /// telemetry subscription. Without one, an internal job is the probe.
    item: Option<Value>,
    /// A connection step that waits for its reply (`on_connect` with
    /// `await_reply`): commands do not go ahead of it.
    setup: bool,
    /// The session login (`transport.session`), its item the login request.
    login: bool,
    /// Sent once more after a fresh login: a second "session ended" stands.
    relogged: bool,
    /// A `then_send` item's captures: the types of `params`, which hold
    /// them.
    specs: Option<Arc<BTreeMap<String, ParamSpec>>>,
}

impl Job {
    fn internal(item: Option<Value>, setup: bool) -> Job {
        Job {
            id: None,
            name: String::new(),
            params: Params::new(),
            item,
            setup,
            login: false,
            relogged: false,
            specs: None,
        }
    }
}

pub(crate) struct SpecEngine {
    spec: Arc<DeviceSpec>,
    /// What the device was opened with, its settings kept current, for
    /// settings changed while open.
    ctx: OpenContext,
    /// `auth: oauth2`: the token refresh.
    oauth: Option<OAuth>,
    /// The push websocket or event stream was opened after a refresh its
    /// refusal asked for: a second refusal is final.
    side_refreshed: bool,
    /// `auth: header`: the header the `token` setting is sent in.
    auth_header: String,
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
    telemetry: Arc<telemetry::Telemetry>,
    /// The path and query of each HTTP request in flight, so its reply can be
    /// offered to the telemetry rules for that path.
    request_paths: std::collections::HashMap<RequestId, (String, Option<Value>)>,
    /// Set when the device refuses the configured credential. Terminal: the
    /// credential is never presented again, since repeated failures can lock
    /// a device out. The host re-opens the device with corrected settings,
    /// or corrects them with `update_settings`, which clears this.
    refused: Option<String>,
    /// An `on_connect` step waiting for its `after_prompt` text: the step's
    /// index, the prompt, and what has arrived so far. Nothing else is sent,
    /// and the link is not reported connected, until the prompt arrives.
    prompt_wait: Option<(usize, String, Vec<u8>)>,
    /// `refused` patterns of the login steps sent: a line matching one is the
    /// device refusing the credential. Each stops applying once a line matches
    /// the step's `accepted` pattern, if it has one.
    refusals: Vec<(Regex, Option<Regex>)>,
    /// The spec's conversions, for templates.
    conversions: Conversions,
    /// The spec's value tables, for templates.
    maps: Maps,
    /// `transport.session`: the login and what of its answer is the session.
    session: Option<SessionSpec>,
    /// The session held: the header it is sent in and its value. A secret,
    /// never logged, offered to the rules or kept in state.
    session_token: Option<(String, String)>,
    /// `telemetry.websocket`: a push channel beside the transport.
    push: Option<Push>,
    push_backoff: Millis,
    /// `telemetry.sse`: an event stream beside an HTTP transport.
    events: Option<HttpRequest>,
    events_backoff: Millis,
    /// `OpenRequest::monitor`. Without it nothing in `telemetry` is sent or
    /// opened: no subscription, poll or push websocket. Replies and anything
    /// the device sends unasked still go to the rules.
    monitor: bool,
}

fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// A scheme, or `{setting: name}` naming the operator's choice of the two.
fn scheme_of(
    t: &Value,
    settings: &Params,
    plain: &'static str,
    secure: &'static str,
) -> Result<String, String> {
    let bad = || format!("scheme needs {plain}, {secure} or {{setting: name}}");
    let scheme = match t.get("scheme") {
        None => plain.to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(o)) => {
            let name = o.get("setting").and_then(Value::as_str).ok_or_else(bad)?;
            settings
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or(plain)
                .to_string()
        }
        Some(_) => return Err(bad()),
    };
    if scheme != plain && scheme != secure {
        return Err(format!("scheme '{scheme}' is not {plain} or {secure}"));
    }
    Ok(scheme)
}

/// An HTTP transport's `auth`, or `{setting: name}` naming the operator's
/// choice, for a service taking more than one kind of credential (Planning
/// Center: a personal access token as Basic, or an OAuth token as Bearer).
fn auth_of(t: &Value, settings: &Params) -> Result<String, String> {
    match t.get("auth") {
        None => Ok("none".into()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(Value::Object(o)) => {
            let name = o
                .get("setting")
                .and_then(Value::as_str)
                .ok_or("auth needs a method or {setting: name}")?;
            Ok(settings
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or("none")
                .to_string())
        }
        Some(_) => Err("auth needs a method or {setting: name}".into()),
    }
}

/// `Authorization` for Basic and Bearer, from the `username`, `password` and
/// `token` settings, or the `token` in the header `header` names; nothing
/// for a token left empty.
fn auth_headers(auth: HttpAuth, header: &str, settings: &Params) -> Vec<(String, String)> {
    let setting = |name: &str| {
        settings
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    match auth {
        HttpAuth::Basic => {
            use base64::Engine;
            let token = base64::engine::general_purpose::STANDARD.encode(format!(
                "{}:{}",
                setting("username"),
                setting("password")
            ));
            vec![("Authorization".to_string(), format!("Basic {token}"))]
        }
        HttpAuth::Bearer => {
            let token = setting("token");
            if token.is_empty() {
                Vec::new()
            } else {
                vec![("Authorization".to_string(), format!("Bearer {token}"))]
            }
        }
        HttpAuth::Header => {
            let token = setting("token");
            if token.is_empty() || header.is_empty() {
                Vec::new()
            } else {
                vec![(header.to_string(), token)]
            }
        }
        HttpAuth::OAuth2 => {
            let token = setting("access_token");
            if token.is_empty() {
                Vec::new()
            } else {
                vec![("Authorization".to_string(), format!("Bearer {token}"))]
            }
        }
        HttpAuth::None | HttpAuth::Digest => Vec::new(),
    }
}

/// `transport.headers`: a map of header names to templates over settings.
fn header_templates(t: &Value) -> Result<Vec<(String, String)>, String> {
    let Some(map) = t.get("headers") else {
        return Ok(Vec::new());
    };
    let map = map
        .as_object()
        .ok_or("transport.headers maps header names to templates")?;
    let mut out = Vec::new();
    for (name, template) in map {
        let valid =
            !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if !valid {
            return Err(format!("transport.headers: '{name}' is not a header name"));
        }
        if ["authorization", "content-type", "host"].contains(&name.to_ascii_lowercase().as_str()) {
            return Err(format!(
                "transport.headers: '{name}' is set by the core, not by the spec"
            ));
        }
        let template = template
            .as_str()
            .ok_or_else(|| format!("transport.headers.{name} is a template string"))?;
        out.push((name.clone(), template.to_string()));
    }
    Ok(out)
}

/// `transport.refusal_json`: JSON paths to regexes.
fn refusal_json(t: &Value) -> Result<Vec<(String, Regex)>, String> {
    let Some(map) = t.get("refusal_json") else {
        return Ok(Vec::new());
    };
    let map = map
        .as_object()
        .ok_or("transport.refusal_json maps JSON paths to regexes")?;
    let mut out = Vec::new();
    for (path, re) in map {
        let re = re
            .as_str()
            .ok_or_else(|| format!("transport.refusal_json.{path} is a regex"))?;
        let re = Regex::new(re).map_err(|e| format!("transport.refusal_json.{path}: {e}"))?;
        out.push((path.clone(), re));
    }
    Ok(out)
}

/// An error answer (4xx or 5xx) whose JSON body matches every
/// `refusal_json` selector.
fn refused_by_body(selectors: &[(String, Regex)], status: u16, body: &[u8]) -> bool {
    if selectors.is_empty() || status < 400 {
        return false;
    }
    let Ok(doc) = serde_json::from_slice::<Value>(body) else {
        return false;
    };
    selectors.iter().all(|(path, re)| {
        let text = match expect::json_path(&doc, path) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => return false,
            Some(other) => other.to_string(),
        };
        re.is_match(&text)
    })
}

/// The `transport.headers` rendered: a header that renders empty, or names
/// a setting with no value, is left out, and one that would break the
/// request line is an error.
fn render_headers(
    templates: &[(String, String)],
    values: &Values,
) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for (name, template) in templates {
        let Ok(value) = render(template, values, no_escape) else {
            continue;
        };
        if value.contains(['\r', '\n']) {
            return Err(format!("header {name} contains a line break"));
        }
        if !value.is_empty() {
            out.push((name.clone(), value));
        }
    }
    Ok(out)
}

fn host_text(host: IpAddr) -> String {
    match host {
        IpAddr::V6(v6) => format!("[{v6}]"),
        v4 => v4.to_string(),
    }
}

/// The host as a URL names it: the name the device was opened with, so TLS
/// checks the certificate against it, or else its address.
fn url_host(ctx: &OpenContext) -> String {
    ctx.host_name.clone().unwrap_or_else(|| host_text(ctx.host))
}

/// A websocket's opening request: `scheme://host:port/path`, the subprotocol
/// and any credential.
fn ws_request(
    t: &Value,
    host: &str,
    port: u16,
    settings: &Params,
    auth: HttpAuth,
    auth_header: &str,
) -> Result<WsRequest, String> {
    let scheme = scheme_of(t, settings, "ws", "wss")?;
    let path = str_field(t, "path").unwrap_or("/");
    if !path.starts_with('/') {
        return Err("a websocket path starts with /".into());
    }
    let mut headers = auth_headers(auth, auth_header, settings);
    if let Some(p) = str_field(t, "subprotocol") {
        headers.push(("Sec-WebSocket-Protocol".to_string(), p.to_string()));
    }
    Ok(WsRequest {
        url: format!("{scheme}://{host}:{port}{path}"),
        headers,
        accept_invalid_certs: t
            .get("accept_invalid_certs")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// A websocket handshake the device answered 401 or 403: the credential
/// refused.
fn handshake_refused(reason: &str) -> bool {
    reason.starts_with("connect: HTTP 401") || reason.starts_with("connect: HTTP 403")
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
        // `https_port`: the default port when the operator chooses HTTPS.
        let https_port = match t.get("https_port") {
            None => None,
            Some(v) => match v.as_u64() {
                Some(p @ 1..=65535) => Some(p as u16),
                _ => return Err("transport.https_port is not a port".into()),
            },
        };
        let secure = kind == "http"
            && https_port.is_some()
            && scheme_of(&t, &ctx.settings, "http", "https")? == "https";
        let port = ctx.port.unwrap_or(match https_port {
            Some(p) if secure => p,
            _ => spec_port,
        });
        let timeout = t
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_TIMEOUT);

        // `auth: header`: the header the `token` setting goes in.
        let auth_header = str_field(&t, "auth_header").unwrap_or("").to_string();
        if kind != "http" && t.get("headers").is_some() {
            return Err("transport.headers is for the http transport".into());
        }
        if kind != "http" && t.get("refusal_json").is_some() {
            return Err("transport.refusal_json is for the http transport".into());
        }
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
                    (Some("terminated"), _) => ReplyFraming::Terminated(
                        t.get("reply_terminators")
                            .and_then(Value::as_array)
                            .map(|a| {
                                a.iter()
                                    .filter_map(Value::as_str)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_string)
                                    .collect::<Vec<_>>()
                            })
                            .filter(|a| !a.is_empty())
                            .ok_or("reply_framing terminated needs reply_terminators")?,
                    ),
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
                    error_match: match str_field(&t, "error_match") {
                        Some(p) => Some(Regex::new(p).map_err(|e| format!("error_match: {e}"))?),
                        None => None,
                    },
                }
            }
            "line-udp" => Transport::LineUdp {
                port,
                send: SendFraming::Terminated(match str_field(&t, "terminator") {
                    None | Some("none") => "",
                    Some("cr") => "\r",
                    Some("lf") => "\n",
                    Some("crlf") => "\r\n",
                    Some(other) => return Err(format!("line-udp terminator '{other}'")),
                }),
                ascii: str_field(&t, "encoding") != Some("utf-8"),
                replies: str_field(&t, "reply") == Some("to-source"),
                listen: listen_port(t.get("listen_port"), &ctx.settings)?,
                reply_match: match str_field(&t, "reply_match") {
                    Some(p) => Some(Regex::new(p).map_err(|e| format!("reply_match: {e}"))?),
                    None => None,
                },
            },
            "osc-udp" => Transport::OscUdp {
                port,
                replies: str_field(&t, "reply") == Some("to-source"),
                listen: listen_port(t.get("listen_port"), &ctx.settings)?,
            },
            "osc-tcp" => Transport::OscTcp {
                port,
                framing: match str_field(&t, "framing") {
                    Some("slip") => PacketFraming::Slip,
                    Some("length-prefixed") => PacketFraming::LengthPrefixed,
                    other => return Err(format!("osc-tcp framing {other:?} is not implemented")),
                },
            },
            "ws" => {
                let auth = match str_field(&t, "auth").unwrap_or("none") {
                    "none" => HttpAuth::None,
                    "basic" => HttpAuth::Basic,
                    "bearer" => HttpAuth::Bearer,
                    other => return Err(format!("ws auth '{other}' is not implemented")),
                };
                let reply_match = match str_field(&t, "reply_match") {
                    Some(p) => Some(Regex::new(p).map_err(|e| format!("reply_match: {e}"))?),
                    None => None,
                };
                Transport::Ws {
                    port,
                    request: ws_request(&t, &url_host(&ctx), port, &ctx.settings, auth, "")?,
                    auth,
                    replies: str_field(&t, "reply") != Some("none"),
                    reply_match,
                }
            }
            "http" => {
                let scheme = scheme_of(&t, &ctx.settings, "http", "https")?;
                let auth = match auth_of(&t, &ctx.settings)?.as_str() {
                    "none" => HttpAuth::None,
                    "basic" => HttpAuth::Basic,
                    "digest" => HttpAuth::Digest,
                    "bearer" => HttpAuth::Bearer,
                    "header" => HttpAuth::Header,
                    "oauth2" => HttpAuth::OAuth2,
                    other => return Err(format!("http auth '{other}' is not implemented")),
                };
                if auth == HttpAuth::Header && auth_header.is_empty() {
                    return Err("http auth header needs auth_header".into());
                }
                let host = url_host(&ctx);
                Transport::Http {
                    base: format!("{scheme}://{host}:{port}"),
                    auth,
                    headers: header_templates(&t)?,
                    refusal_json: refusal_json(&t)?,
                    accept_invalid_certs: t
                        .get("accept_invalid_certs")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    refusal: match t.get("refusal_status").and_then(Value::as_array) {
                        Some(list) => list
                            .iter()
                            .filter_map(Value::as_u64)
                            .map(|s| s as u16)
                            .collect(),
                        None => vec![401, 403],
                    },
                }
            }
            other => return Err(format!("transport '{other}' is not implemented")),
        };

        // A list of probes, each for some models: the first for this one.
        let probe = match t.get("probe") {
            Some(Value::Array(items)) => items.iter().find(|i| for_model(i, &ctx.model)).cloned(),
            Some(one) if for_model(one, &ctx.model) => Some(one.clone()),
            _ => None,
        };
        let maps = template::maps(spec.maps.as_ref())?;
        let session = match t.get("session") {
            None => None,
            Some(_) if kind != "http" => {
                return Err("transport.session is for the http transport".into())
            }
            Some(s) => Some(SessionSpec::parse(s, &ctx.model)?),
        };
        let telemetry = telemetry::Telemetry::shared(
            spec.telemetry.as_ref(),
            &spec.state,
            spec.conversions.as_ref(),
        )?;
        let conversions = template::conversions(spec.conversions.as_ref())?;
        // `transport.headers` over the settings, for the push websocket and
        // the event stream, which are opened from them alone.
        let side_headers = match &transport {
            Transport::Http { headers, .. } => {
                let (no_params, no_specs) = (Params::new(), BTreeMap::new());
                render_headers(
                    headers,
                    &Values {
                        params: &no_params,
                        param_specs: &no_specs,
                        settings: &ctx.settings,
                        setting_specs: &spec.settings,
                        conversions: &conversions,
                        maps: &maps,
                    },
                )?
            }
            _ => Vec::new(),
        };
        let push = match spec.telemetry.as_ref().and_then(|t| t.get("websocket")) {
            None => None,
            Some(w) => {
                // A port, or `{setting: name}` naming the operator's (an
                // empty setting is the transport's port).
                let push_port = match w.get("port") {
                    None => port,
                    Some(Value::Object(o)) => {
                        let name = o
                            .get("setting")
                            .and_then(Value::as_str)
                            .ok_or("telemetry.websocket.port is a port or {setting: name}")?;
                        match ctx.settings.get(name) {
                            None | Some(Value::Null) => port,
                            Some(v) => match v.as_u64() {
                                Some(p @ 1..=65535) => p as u16,
                                _ => return Err(format!("setting {name} is not a port")),
                            },
                        }
                    }
                    Some(v) => match v.as_u64() {
                        Some(p @ 1..=65535) => p as u16,
                        _ => return Err("telemetry.websocket.port is not a port".into()),
                    },
                };
                let url = match w.get("url") {
                    None => None,
                    Some(Value::String(u)) => {
                        if w.get("path").is_some()
                            || w.get("port").is_some()
                            || w.get("scheme").is_some()
                        {
                            return Err(
                                "telemetry.websocket.url replaces path, port and scheme".into()
                            );
                        }
                        // The scheme and host are the spec's: checked now, and
                        // the rendered URL again at each opening.
                        if !(u.starts_with("wss://") || u.starts_with("ws://")) {
                            return Err(
                                "telemetry.websocket.url starts with wss:// or ws://".into()
                            );
                        }
                        Some(u.clone())
                    }
                    Some(_) => return Err("telemetry.websocket.url is a template".into()),
                };
                // The transport's credential goes on the websocket's opening
                // request too.
                let auth = match &transport {
                    Transport::Http { auth, .. } | Transport::Ws { auth, .. } => *auth,
                    _ => HttpAuth::None,
                };
                let socketio = match w.get("socketio") {
                    None | Some(Value::Bool(false)) => None,
                    Some(Value::Bool(true)) => Some(SocketIo {
                        version: 4,
                        namespace: String::new(),
                        ping_every: None,
                        auth: None,
                    }),
                    Some(Value::Object(o)) => Some(SocketIo {
                        version: match o.get("engine_io").and_then(Value::as_u64) {
                            None | Some(4) => 4,
                            Some(3) => 3,
                            Some(v) => return Err(format!("socketio.engine_io {v} is not 3 or 4")),
                        },
                        namespace: match o.get("namespace").and_then(Value::as_str) {
                            None | Some("/") => String::new(),
                            Some(ns) if ns.starts_with('/') => format!("{ns},"),
                            Some(ns) => {
                                return Err(format!("socketio.namespace '{ns}' starts with /"))
                            }
                        },
                        ping_every: None,
                        auth: o.get("auth").and_then(Value::as_str).map(str::to_string),
                    }),
                    Some(_) => {
                        return Err("telemetry.websocket.socketio is true or an object".into())
                    }
                };
                let mut request = match &url {
                    // Another host: neither the transport's credential nor
                    // its headers go there; the URL carries what it needs.
                    Some(_) => WsRequest {
                        url: String::new(),
                        headers: str_field(w, "subprotocol")
                            .map(|p| vec![("Sec-WebSocket-Protocol".to_string(), p.to_string())])
                            .unwrap_or_default(),
                        accept_invalid_certs: w
                            .get("accept_invalid_certs")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    },
                    None => {
                        let mut request = ws_request(
                            w,
                            &url_host(&ctx),
                            push_port,
                            &ctx.settings,
                            auth,
                            &auth_header,
                        )?;
                        request.headers.extend(side_headers.iter().cloned());
                        request
                    }
                };
                if let (Some(sio), None) = (&socketio, &url) {
                    request.url.push_str(&engine_io_query(&request.url, sio));
                }
                // A device serving HTTPS with a self-signed certificate
                // serves wss with it too, unless the websocket says otherwise.
                if let (
                    None,
                    None,
                    Transport::Http {
                        accept_invalid_certs: true,
                        ..
                    },
                ) = (w.get("accept_invalid_certs"), &url, &transport)
                {
                    request.accept_invalid_certs = true;
                }
                let credentialed = request
                    .headers
                    .iter()
                    .any(|(name, _)| name != "Sec-WebSocket-Protocol")
                    || url.as_deref().is_some_and(|u| u.contains("{settings."));
                let idle = match w.get("idle_ms") {
                    None => None,
                    Some(v) => match v.as_u64() {
                        Some(ms @ 1..) => Some(ms),
                        _ => {
                            return Err(
                                "telemetry.websocket.idle_ms is not a positive integer".into()
                            )
                        }
                    },
                };
                Some(Push {
                    request,
                    send: match w.get("send") {
                        Some(Value::Array(items)) => items.clone(),
                        Some(one) => vec![one.clone()],
                        None => Vec::new(),
                    },
                    every: match w.get("every_ms") {
                        None => None,
                        Some(v) => match v.as_u64() {
                            Some(ms @ 1..) => Some(ms),
                            _ => {
                                return Err(
                                    "telemetry.websocket.every_ms is not a positive integer".into(),
                                )
                            }
                        },
                    },
                    credentialed,
                    socketio,
                    url,
                    idle,
                })
            }
        };
        let events = match spec.telemetry.as_ref().and_then(|t| t.get("sse")) {
            None => None,
            Some(e) => {
                let Transport::Http {
                    base,
                    auth,
                    accept_invalid_certs,
                    ..
                } = &transport
                else {
                    return Err("telemetry.sse needs an http transport".into());
                };
                let path = str_field(e, "path").ok_or("telemetry.sse needs a path")?;
                let mut headers = auth_headers(*auth, &auth_header, &ctx.settings);
                headers.extend(side_headers.iter().cloned());
                headers.push(("Accept".into(), "text/event-stream".into()));
                let setting = |name: &str| {
                    ctx.settings
                        .get(name)
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                };
                Some(HttpRequest {
                    method: "GET",
                    url: format!("{base}{path}"),
                    headers,
                    body: None,
                    // A stream stays open; the session watches it for activity.
                    timeout: None,
                    accept_invalid_certs: *accept_invalid_certs,
                    digest: matches!(auth, HttpAuth::Basic | HttpAuth::Digest).then_some(
                        Credentials {
                            username: setting("username"),
                            password: setting("password"),
                        },
                    ),
                })
            }
        };
        let oauth = match &transport {
            Transport::Http {
                auth: HttpAuth::OAuth2,
                ..
            } => {
                let oauth = OAuth::new(t.get("oauth"))?;
                oauth.token_url(&ctx.settings)?;
                Some(oauth)
            }
            _ => None,
        };
        Ok(SpecEngine {
            spec,
            ctx: ctx.clone(),
            oauth,
            side_refreshed: false,
            auth_header,
            host: ctx.host,
            monitor: ctx.monitor,
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
            prompt_wait: None,
            refusals: Vec::new(),
            conversions,
            maps,
            session,
            session_token: None,
            push,
            push_backoff: RECONNECT_MIN,
            events,
            events_backoff: RECONNECT_MIN,
        })
    }

    fn port(&self) -> u16 {
        match &self.transport {
            Transport::LineTcp { port, .. }
            | Transport::LineUdp { port, .. }
            | Transport::OscUdp { port, .. }
            | Transport::OscTcp { port, .. }
            | Transport::Ws { port, .. } => *port,
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
            conversions: &self.conversions,
            maps: &self.maps,
        }
    }

    // ── Building messages ────────────────────────────────────────────────

    /// Render one `send` item. Returns the message and, for OSC with an
    /// `expect.address`, nothing extra: the awaited address is rendered
    /// separately.
    fn build(&self, item: &Value, values: &Values) -> Result<Outgoing, String> {
        match &self.transport {
            Transport::LineTcp { send, ascii, .. } | Transport::LineUdp { send, ascii, .. } => {
                let template = item.as_str().ok_or("a line message must be a string")?;
                let payload = render(template, values, no_escape)?;
                let framed = framing::frame(send, &payload);
                if *ascii && !framed.is_ascii() {
                    return Err("the message contains non-ASCII characters".into());
                }
                Ok(Outgoing::Bytes(framed.into_bytes()))
            }
            Transport::Ws { .. } => {
                let template = item
                    .as_str()
                    .ok_or("a websocket message must be a string")?;
                Ok(Outgoing::Ws(render(template, values, no_escape)?))
            }
            Transport::OscUdp { .. } | Transport::OscTcp { .. } => {
                let packet = build_osc(item, values)?;
                Ok(Outgoing::Bytes(match &self.transport {
                    Transport::OscTcp { framing, .. } => framing::frame_packet(*framing, &packet),
                    _ => packet,
                }))
            }
            Transport::Http {
                base,
                auth,
                accept_invalid_certs,
                headers: extra,
                ..
            } => {
                let method = str_field(item, "method").unwrap_or("GET");
                let method: &'static str = match method {
                    "GET" => "GET",
                    "POST" => "POST",
                    "PUT" => "PUT",
                    "PATCH" => "PATCH",
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
                let mut headers = auth_headers(*auth, &self.auth_header, &self.settings);
                headers.extend(render_headers(extra, values)?);
                // The session held (`transport.session`), on every request
                // but one sent without it.
                if let Some((name, value)) = &self.session_token {
                    if item.get("session") != Some(&Value::Bool(false)) {
                        headers.push((name.clone(), value.clone()));
                    }
                }
                let setting = |name: &str| {
                    self.settings
                        .get(name)
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                };
                let (user, pass) = (setting("username"), setting("password"));
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
                    accept_invalid_certs: *accept_invalid_certs,
                    // Basic devices that answer with a Digest challenge get it
                    // answered too (SPEC.md §2).
                    digest: matches!(auth, HttpAuth::Basic | HttpAuth::Digest).then_some(
                        Credentials {
                            username: user,
                            password: pass,
                        },
                    ),
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
                let specs = job.specs.as_deref().unwrap_or(&empty_specs);
                (vec![item], Map::new(), "ack".into(), specs)
            }
            Some(_) => {
                let command = spec.commands.get(&job.name).ok_or("unknown command")?;
                let items = self.command_items(&job.name)?;
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
        let osc = self.transport.is_osc();
        let awaited_address = match expect.get("address").and_then(Value::as_str) {
            Some(a) => Some(OscReply::Exact(render(a, &values, no_escape)?)),
            // A telemetry query is answered on its own address, or on the
            // `reply_address` it names; anything else arriving meanwhile (a
            // pushed change) is not its reply.
            None => match &job.item {
                Some(Value::String(address)) if osc => Some(OscReply::Exact(address.clone())),
                Some(Value::Object(o)) if osc => match o.get("reply_address") {
                    Some(Value::String(re)) => Some(OscReply::Pattern(
                        Regex::new(re).map_err(|e| format!("reply_address: {e}"))?,
                    )),
                    _ => None,
                },
                _ => None,
            },
        };
        let ws_reply = match expect.get("reply_json").and_then(Value::as_object) {
            Some(fields) => {
                let mut out = Vec::new();
                for (path, template) in fields {
                    let template = template.as_str().ok_or("reply_json values are templates")?;
                    out.push((path.clone(), render(template, &values, no_escape)?));
                }
                Some(out)
            }
            None => None,
        };
        let text_reply = match expect.get("reply_contains").and_then(Value::as_str) {
            Some(template) => Some(render(template, &values, no_escape)?),
            None => None,
        };
        let mut messages = VecDeque::new();
        for item in &items {
            // OSC probes are bare addresses in the spec.
            let item = match item {
                Value::String(address) if osc => serde_json::json!({ "address": address }),
                _ => item.clone(),
            };
            let outgoing = self.build(&item, &values)?;
            messages.push_back((outgoing, awaited_address.clone()));
        }
        Ok(InFlight {
            id: job.id,
            messages,
            ws_reply,
            text_reply,
            expect,
            returns,
            awaiting: None,
            sent_at: 0,
            last_http: None,
            retry: None,
            refreshed: false,
            job: job.clone(),
        })
    }

    /// A command's `send` items for the device's model.
    fn command_items(&self, name: &str) -> Result<Vec<Value>, String> {
        let command = self.spec.commands.get(name).ok_or("unknown command")?;
        let send = command.send.clone().ok_or("command has no send")?;
        let items: Vec<Value> = match send {
            Value::Array(items) => items,
            one => vec![one],
        }
        .into_iter()
        .filter(|i| for_model(i, &self.ctx.model))
        .collect();
        if items.is_empty() {
            return Err(format!(
                "command {name} has no message for model {}",
                self.ctx.model
            ));
        }
        Ok(items)
    }

    /// Whether a job's requests carry the session (`transport.session`):
    /// all but the login, the probe and items sent without it.
    fn needs_session(&self, job: &Job) -> bool {
        if self.session.is_none() || job.login {
            return false;
        }
        let items = match (&job.id, &job.item) {
            (None, None) => return false,
            (None, Some(item)) => vec![item.clone()],
            (Some(_), _) => self.command_items(&job.name).unwrap_or_default(),
        };
        items
            .iter()
            .any(|i| i.get("session") != Some(&Value::Bool(false)))
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
            if !self.queue.is_empty() && self.oauth_holds(cx) {
                return;
            }
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
            // No session yet: the login goes first.
            if self.session_token.is_none() && self.needs_session(&job) {
                let login = self.session.as_ref().map(|s| s.login.clone());
                self.queue.push_front(job);
                self.queue.push_front(Job {
                    login: true,
                    ..Job::internal(login, true)
                });
                continue;
            }
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
                Outgoing::Ws(text) => {
                    cx.ws_send(SOCKET, text);
                    Await::Ws(flight.ws_reply.clone())
                }
                Outgoing::Bytes(bytes) => {
                    match &self.transport {
                        Transport::OscUdp { port, .. } | Transport::LineUdp { port, .. } => {
                            cx.udp_send(SOCKET, SocketAddr::new(self.host, *port), bytes)
                        }
                        _ => cx.tcp_send(SOCKET, bytes),
                    }
                    match &self.transport {
                        Transport::LineTcp { .. } | Transport::LineUdp { .. } => {
                            Await::Text(flight.text_reply.clone())
                        }
                        _ => Await::Osc(address),
                    }
                }
                Outgoing::Http(request) => {
                    if self.oauth.is_some() {
                        flight.last_http = Some(request.clone());
                    }
                    Await::Http(self.send_http(cx, request))
                }
            };
            if waits || matches!(awaiting, Await::Http(_)) {
                let flight = self.current.as_mut().unwrap();
                flight.awaiting = Some(awaiting);
                flight.sent_at = cx.now();
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

    /// Send an HTTP request whose reply goes to the telemetry rules.
    fn send_http(&mut self, cx: &mut Cx, request: HttpRequest) -> RequestId {
        let id = self.next_request;
        self.next_request += 1;
        let target = request.url.splitn(4, '/').nth(3).unwrap_or("");
        let body = request
            .body
            .as_deref()
            .and_then(|b| serde_json::from_slice::<Value>(b).ok());
        self.request_paths.insert(id, (format!("/{target}"), body));
        cx.http(id, request);
        id
    }

    // ── OAuth ────────────────────────────────────────────────────────────

    /// Whether queued requests wait for the access token: a refresh is in
    /// flight, or one is due and is started now. After a refresh failed,
    /// until its retry, queued commands fail rather than wait.
    fn oauth_holds(&mut self, cx: &mut Cx) -> bool {
        let Some(oauth) = &self.oauth else {
            return false;
        };
        if oauth.refreshing.is_some() {
            return true;
        }
        if !oauth.needs_refresh(&self.settings, cx.unix_millis()) {
            return false;
        }
        if cx.now() < oauth.retry_at {
            let message = format!(
                "the access token could not be refreshed: {}",
                oauth.last_error.clone().unwrap_or_default()
            );
            for job in self.queue.drain(..) {
                if let Some(id) = job.id {
                    cx.complete(
                        id,
                        Err(CommandError::Transport {
                            message: message.clone(),
                        }),
                    );
                }
            }
            return true;
        }
        self.start_refresh(cx);
        true
    }

    fn start_refresh(&mut self, cx: &mut Cx) {
        let accept_invalid_certs = matches!(
            self.transport,
            Transport::Http {
                accept_invalid_certs: true,
                ..
            }
        );
        let Some(oauth) = self.oauth.as_mut() else {
            return;
        };
        if oauth.refreshing.is_some() {
            return;
        }
        match oauth.request(&self.settings, self.timeout, accept_invalid_certs) {
            Ok(request) => {
                let id = self.next_request;
                self.next_request += 1;
                oauth.refreshing = Some(id);
                cx.cancel_timer(OAUTH_RETRY);
                cx.http(id, request);
            }
            Err(reason) => self.refuse(cx, reason),
        }
    }

    /// The token endpoint answered, or could not be reached.
    fn refreshed(&mut self, cx: &mut Cx, result: Result<HttpResponse, String>) {
        let Some(oauth) = self.oauth.as_mut() else {
            return;
        };
        match oauth.response(&self.settings, result, cx.unix_millis(), cx.now()) {
            Refreshed::Tokens(tokens) => {
                for (name, value) in &tokens {
                    self.settings.insert(name.clone(), value.clone());
                }
                self.ctx.settings = self.settings.clone();
                cx.credentials(tokens);
                cx.log(Level::Info, "OAuth access token refreshed");
                // A new token is validated at once, where the service wants it.
                if let Some(oauth) = self.oauth.as_mut() {
                    oauth.validate_waits = false;
                }
                self.validate_token(cx);
                self.resend(cx);
                if self.current.is_none()
                    && self.queue.is_empty()
                    && self.link != Connection::Connected
                {
                    self.enqueue_probe(cx);
                }
                self.pump(cx);
            }
            Refreshed::Refused(reason) => self.refuse(cx, reason),
            Refreshed::Failed(reason) => {
                let wait = oauth.retry_at.saturating_sub(cx.now());
                if std::mem::take(&mut oauth.validate_waits) {
                    // Not refreshed, so not refused yet either: validated
                    // again once the refresh may be tried again.
                    oauth.validate_refreshed = false;
                    cx.set_timer(OAUTH_VALIDATE, wait.max(1));
                }
                let message = format!("the access token could not be refreshed: {reason}");
                cx.log(
                    Level::Warning,
                    format!("{message}; trying again in {} s", wait.div_ceil(1000)),
                );
                if self.current.as_ref().is_some_and(|f| f.retry.is_some()) {
                    if let Some(id) = self.current.take().and_then(|f| f.id) {
                        cx.complete(
                            id,
                            Err(CommandError::Transport {
                                message: message.clone(),
                            }),
                        );
                    }
                }
                for job in self.queue.drain(..) {
                    if let Some(id) = job.id {
                        cx.complete(
                            id,
                            Err(CommandError::Transport {
                                message: message.clone(),
                            }),
                        );
                    }
                }
                self.set_link(cx, Connection::Disconnected { reason: message });
                cx.set_timer(OAUTH_RETRY, wait.max(1));
            }
        }
    }

    /// The request refused with the old token goes again with the new one.
    fn resend(&mut self, cx: &mut Cx) {
        let Some(mut request) = self.current.as_mut().and_then(|f| f.retry.take()) else {
            return;
        };
        request.headers.retain(|(name, _)| name != "Authorization");
        request
            .headers
            .extend(auth_headers(HttpAuth::OAuth2, "", &self.settings));
        if let Some(flight) = self.current.as_mut() {
            flight.last_http = Some(request.clone());
        }
        let id = self.send_http(cx, request);
        if let Some(flight) = self.current.as_mut() {
            flight.awaiting = Some(Await::Http(id));
            flight.sent_at = cx.now();
        }
        cx.set_timer(REPLY, self.timeout);
    }

    /// The service refused the request in flight with 401: refresh the token
    /// and send it once more, when there is a refresh token and this request
    /// has not been sent with a refreshed one already. False when the
    /// refusal stands.
    fn retry_after_refresh(&mut self, cx: &mut Cx) -> bool {
        // A refresh already in flight (one validation asked for) serves too.
        let refreshable = self.oauth.is_some() && OAuth::can_refresh(&self.settings);
        let Some(flight) = self.current.as_mut() else {
            return false;
        };
        if !refreshable || flight.refreshed || flight.last_http.is_none() {
            return false;
        }
        flight.refreshed = true;
        flight.retry = flight.last_http.take();
        flight.awaiting = None;
        cx.cancel_timer(REPLY);
        self.start_refresh(cx);
        true
    }

    /// `oauth.validate`: check the access token at the service's validation
    /// endpoint now, or once the refresh due or in flight is done. Nothing
    /// without `oauth.validate`, after a refusal, or while one is in flight.
    fn validate_token(&mut self, cx: &mut Cx) {
        let unix = cx.unix_millis();
        let Some(oauth) = self.oauth.as_mut() else {
            return;
        };
        let Some(every) = oauth.validate.as_ref().map(|v| v.every) else {
            return;
        };
        if self.refused.is_some() || oauth.validating.is_some() {
            return;
        }
        cx.cancel_timer(OAUTH_VALIDATE);
        if oauth.refreshing.is_some() || oauth.needs_refresh(&self.settings, unix) {
            // The new token is validated when the refresh ends; a refresh
            // backing off is started by its own retry.
            oauth.validate_waits = true;
            if oauth.refreshing.is_none() && cx.now() >= oauth.retry_at {
                self.start_refresh(cx);
            }
            return;
        }
        match oauth.validate_request(&self.settings, self.timeout) {
            Some(request) => {
                let id = self.next_request;
                self.next_request += 1;
                oauth.validating = Some(id);
                cx.http(id, request);
            }
            // No token to validate yet: the next one is, when it comes.
            None => cx.set_timer(OAUTH_VALIDATE, every),
        }
    }

    /// The validation endpoint answered, or could not be reached. A 401
    /// refreshes the token and validates the new one, when there is a
    /// refresh token; otherwise, or when the new token is refused too, the
    /// refusal is terminal. Anything else is tried again with backoff.
    fn validated(&mut self, cx: &mut Cx, result: Result<HttpResponse, String>) {
        let can_refresh = OAuth::can_refresh(&self.settings);
        let Some(oauth) = self.oauth.as_mut() else {
            return;
        };
        let every = oauth.validate.as_ref().map_or(0, |v| v.every);
        match oauth.validated(result) {
            Validated::Valid => cx.set_timer(OAUTH_VALIDATE, every),
            Validated::Invalid if can_refresh && !oauth.validate_refreshed => {
                oauth.validate_refreshed = true;
                oauth.validate_waits = true;
                cx.log(
                    Level::Info,
                    "the access token failed validation (HTTP 401): refreshing it",
                );
                self.start_refresh(cx);
            }
            Validated::Invalid => self.refuse(
                cx,
                "the service's token validation refused the access token (HTTP 401)".into(),
            ),
            Validated::Failed(reason, wait) => {
                cx.log(
                    Level::Warning,
                    format!(
                        "the access token could not be validated: {reason}; trying again in {} s",
                        wait.div_ceil(1000)
                    ),
                );
                cx.set_timer(OAUTH_VALIDATE, wait);
            }
        }
    }

    /// A push websocket or event stream refused its token: refresh it and
    /// reopen, once. False when the refusal stands.
    fn side_refresh(&mut self, cx: &mut Cx) -> bool {
        if self.oauth.is_none() || !OAuth::can_refresh(&self.settings) || self.side_refreshed {
            return false;
        }
        self.side_refreshed = true;
        self.start_refresh(cx);
        true
    }

    /// Headers with the current access token in place of the one they were
    /// built with.
    fn current_auth(&self, headers: &mut Vec<(String, String)>) {
        if self.oauth.is_some() {
            headers.retain(|(name, _)| name != "Authorization");
            headers.extend(auth_headers(HttpAuth::OAuth2, "", &self.settings));
        }
    }

    /// A reply for the command in flight.
    fn reply(&mut self, cx: &mut Cx, reply: Reply) {
        cx.cancel_timer(REPLY);
        let Some(flight) = self.current.as_mut() else {
            return;
        };
        if flight.awaiting.take().is_some() {
            cx.round_trip(cx.now().saturating_sub(flight.sent_at));
        }

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
        // `expect.convert`: the returned wire value in the operator's terms.
        let result = match (result, flight.expect.get("convert").and_then(Value::as_str)) {
            (Ok(Outcome::Value { value }), Some(name)) => {
                let wire = value
                    .as_f64()
                    .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()));
                match (wire, self.conversions.get(name)) {
                    (Some(w), Some(c)) => match c.wire_to_value(w) {
                        Some(x) => Ok(Outcome::Value {
                            value: Value::from(x),
                        }),
                        None => Err(CommandError::DeviceError {
                            code: None,
                            message: format!("reply {w} is outside conversion '{name}'"),
                        }),
                    },
                    _ => Err(CommandError::DeviceError {
                        code: None,
                        message: format!("reply {value} cannot be converted with '{name}'"),
                    }),
                }
            }
            (result, _) => result,
        };
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
            format!("{reason}; no further requests until the device's settings are corrected"),
        );
        self.set_link(
            cx,
            Connection::Unauthorized {
                reason: reason.clone(),
            },
        );
        self.refused = Some(reason);
    }

    /// The session login (`transport.session`) answered, or failed. A refusal
    /// is terminal; an answer holding the session lets the queue go on; any
    /// other fails what waits for a session, and the next request needing
    /// one logs in again.
    fn logged_in(&mut self, cx: &mut Cx, result: Result<HttpResponse, String>) {
        cx.cancel_timer(REPLY);
        let Some(flight) = self.current.take() else {
            return;
        };
        cx.round_trip(cx.now().saturating_sub(flight.sent_at));
        let Some(session) = &self.session else {
            return;
        };
        let failure = match result {
            // Not the error's text: it can name the login's URL, whose query
            // may hold a hashed password.
            Err(_) => CommandError::Transport {
                message: "the login request failed".into(),
            },
            Ok(response) => {
                if session.refused_status.contains(&response.status)
                    || body_matches(&session.refused_json, &response.body)
                {
                    let detail = serde_json::from_slice::<Value>(&response.body)
                        .ok()
                        .filter(|_| !session.refused_json.is_empty())
                        .and_then(|doc| {
                            session
                                .refused_json
                                .iter()
                                .filter_map(|(path, _)| {
                                    expect::json_path(&doc, path).map(|v| format!("{path} {v}"))
                                })
                                .next()
                        })
                        .unwrap_or_else(|| format!("HTTP {}", response.status));
                    self.refuse(cx, format!("the device refused the login ({detail})"));
                    return;
                }
                let captured = (200..300)
                    .contains(&response.status)
                    .then(|| session.captured(&response))
                    .flatten();
                if let Some(token) = captured {
                    self.session_token = Some(token);
                    self.heard(cx);
                    self.pump(cx);
                    return;
                }
                CommandError::DeviceError {
                    code: Some(response.status.to_string()),
                    message: format!(
                        "the login was not accepted (HTTP {}, no session in the answer)",
                        response.status
                    ),
                }
            }
        };
        cx.log(Level::Warning, format!("login failed: {failure:?}"));
        // What waits for a session fails with the login, rather than each
        // logging in again in turn.
        let queue = std::mem::take(&mut self.queue);
        for job in queue {
            if self.needs_session(&job) {
                if let Some(id) = job.id {
                    cx.complete(id, Err(failure.clone()));
                }
            } else {
                self.queue.push_back(job);
            }
        }
        if let CommandError::Transport { message } = failure {
            self.set_link(cx, Connection::Disconnected { reason: message });
        }
        self.pump(cx);
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
            Transport::Ws { request, .. } => cx.ws_open(SOCKET, request.clone()),
            Transport::OscUdp { listen, .. } | Transport::LineUdp { listen, .. } => {
                cx.udp_open(SOCKET, listen.map_or(Bind::Ephemeral, Bind::Shared));
                self.send_on_connect(cx, 0, false);
                self.start_telemetry(cx);
                if replies && self.probe.is_some() {
                    self.enqueue_probe(cx);
                } else {
                    self.set_link(cx, Connection::Unmonitored);
                }
                cx.set_timer(PROBE, PROBE_WHEN_IDLE);
            }
            Transport::Http { .. } => {
                // No connection to wait for: the polls go now, queued like
                // commands, each reply offered to the rules.
                self.start_telemetry(cx);
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
        match self.transport {
            Transport::Ws { .. } => cx.ws_close(SOCKET),
            _ => cx.tcp_close(SOCKET),
        }
        cx.cancel_timer(PROBE);
        cx.cancel_timer(RENEW);
        cx.cancel_timer(POLL);
        self.set_link(cx, Connection::Disconnected { reason });
        cx.set_timer(RECONNECT, self.backoff);
        self.backoff = (self.backoff * 2).min(RECONNECT_MAX);
    }

    /// The fixed connection sequence, SPEC.md §2, from step `from`. Nothing
    /// waits on a reply: it is not a handshake. A step with `after_prompt`
    /// first waits for the device's prompt text: this then returns false, and
    /// the sequence resumes at that step, with `prompted`, when it arrives.
    fn send_on_connect(&mut self, cx: &mut Cx, from: usize, prompted: bool) -> bool {
        let empty = Params::new();
        let empty_specs = BTreeMap::new();
        let steps = self.spec.on_connect.clone();
        for (index, step) in steps.iter().enumerate().skip(from) {
            if !for_model(step, &self.ctx.model) {
                continue;
            }
            let (item, when_set, prompt, refused, accepted, await_reply) = match step {
                Value::Object(m) if m.contains_key("send") => (
                    m.get("send").cloned().unwrap_or(Value::Null),
                    m.get("when_set").and_then(Value::as_str),
                    m.get("after_prompt").and_then(Value::as_str),
                    m.get("refused").and_then(Value::as_str),
                    m.get("accepted").and_then(Value::as_str),
                    m.get("await_reply").and_then(Value::as_bool) == Some(true),
                ),
                other => (other.clone(), None, None, None, None, false),
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
            if let Some(prompt) = prompt {
                if !(prompted && index == from) {
                    self.prompt_wait = Some((index, prompt.to_string(), Vec::new()));
                    cx.set_timer(PROMPT, self.timeout);
                    return false;
                }
            }
            if let Some(pattern) = refused {
                let accepted = accepted.and_then(|p| match Regex::new(p) {
                    Ok(re) => Some(re),
                    Err(e) => {
                        cx.log(Level::Warning, format!("on_connect accepted: {e}"));
                        None
                    }
                });
                match Regex::new(pattern) {
                    Ok(re) => self.refusals.push((re, accepted)),
                    Err(e) => cx.log(Level::Warning, format!("on_connect refused: {e}")),
                }
            }
            if await_reply && self.transport.replies() {
                // Queued like a query: sent in turn, its reply consumed, and
                // nothing else goes ahead of it.
                self.queue.push_back(Job::internal(Some(item), true));
                continue;
            }
            let values = self.values(&empty, &empty_specs);
            match self.build(&item, &values) {
                Ok(Outgoing::Bytes(bytes)) => self.transmit(cx, bytes),
                Ok(Outgoing::Ws(text)) => cx.ws_send(SOCKET, text),
                Ok(Outgoing::Http(request)) => {
                    let id = self.next_request;
                    self.next_request += 1;
                    cx.http(id, request);
                }
                Err(e) => cx.log(Level::Warning, format!("on_connect step not sent: {e}")),
            }
        }
        true
    }

    /// Bytes to the device, over the transport's socket.
    fn transmit(&self, cx: &mut Cx, bytes: Vec<u8>) {
        match &self.transport {
            Transport::OscUdp { port, .. } | Transport::LineUdp { port, .. } => {
                cx.udp_send(SOCKET, SocketAddr::new(self.host, *port), bytes)
            }
            _ => cx.tcp_send(SOCKET, bytes),
        }
    }

    /// The stream is open: the connection sequence, then traffic.
    fn opened(&mut self, cx: &mut Cx) {
        self.backoff = RECONNECT_MIN;
        self.last_heard = cx.now();
        self.prompt_wait = None;
        self.refusals.clear();
        if self.send_on_connect(cx, 0, false) {
            self.ready(cx);
        }
    }

    /// The connection sequence is done: report the link and start traffic.
    fn ready(&mut self, cx: &mut Cx) {
        self.set_link(cx, Connection::Connected);
        cx.set_timer(PROBE, PROBE_WHEN_IDLE);
        self.start_telemetry(cx);
        self.pump(cx);
    }

    fn enqueue_probe(&mut self, cx: &mut Cx) {
        if self.probe.is_none() || self.queue.iter().any(|j| j.id.is_none()) {
            return;
        }
        self.queue.push_back(Job::internal(None, false));
        self.pump(cx);
    }

    /// Send telemetry messages. On a line transport whose device answers, each
    /// goes through the command queue so its reply is not mistaken for a
    /// command's; otherwise it is sent straight away, like `on_connect`.
    fn send_telemetry(&mut self, cx: &mut Cx, items: Vec<Value>, poll: bool) {
        let items = items
            .into_iter()
            .map(|item| telemetry::Triggered {
                item,
                params: Params::new(),
                specs: BTreeMap::new(),
            })
            .collect();
        self.send_items(cx, items, poll);
    }

    /// Telemetry messages, each rendered from its own values: none for a
    /// subscription or a poll, a rule's captures for `then_send`.
    fn send_items(&mut self, cx: &mut Cx, items: Vec<telemetry::Triggered>, poll: bool) {
        let items: Vec<telemetry::Triggered> = items
            .into_iter()
            .filter(|t| for_model(&t.item, &self.ctx.model))
            .collect();
        // HTTP requests are always queued: each reply is matched to its
        // request. Poll items are queries, so on any transport that answers
        // they go one at a time, which also paces a long list.
        let queued = match self.transport {
            Transport::LineTcp { replies, .. } => replies,
            Transport::Http { .. } => true,
            Transport::OscUdp { replies, .. } | Transport::LineUdp { replies, .. } => {
                poll && replies
            }
            Transport::OscTcp { .. } => poll,
            // A websocket device pushes; what it sends back after a
            // subscription goes to the rules like anything else.
            Transport::Ws { .. } => false,
        };
        for telemetry::Triggered {
            item,
            params,
            specs,
        } in items
        {
            if queued {
                // A slow device must not fall behind: a poll item still
                // waiting from the last round is not queued again, and a
                // re-read already waiting serves a second push too.
                if poll
                    && self.queue.iter().any(|j| {
                        j.id.is_none() && j.item.as_ref() == Some(&item) && j.params == params
                    })
                {
                    continue;
                }
                self.queue.push_back(Job {
                    params,
                    specs: (!specs.is_empty()).then(|| Arc::new(specs)),
                    ..Job::internal(Some(item), false)
                });
                continue;
            }
            let item = match &item {
                Value::String(address) if self.transport.is_osc() => {
                    serde_json::json!({ "address": address })
                }
                _ => item,
            };
            let values = self.values(&params, &specs);
            match self.build(&item, &values) {
                Ok(Outgoing::Bytes(bytes)) => self.transmit(cx, bytes),
                Ok(Outgoing::Ws(text)) => cx.ws_send(SOCKET, text),
                Ok(Outgoing::Http(_)) => {
                    cx.log(Level::Warning, "telemetry over HTTP is not implemented")
                }
                Err(e) => cx.log(Level::Warning, format!("telemetry message not sent: {e}")),
            }
        }
        self.pump(cx);
    }

    /// Offer a message to the telemetry rules: the state it changes, and
    /// the `then_send` items of the rules it matches, queued like polls
    /// (only while monitored).
    fn offer(&mut self, cx: &mut Cx, message: &telemetry::Inbound) {
        let mut triggered = Vec::new();
        if let Some(patch) = self.telemetry.apply_into(message, &mut triggered) {
            cx.state(patch);
        }
        if !triggered.is_empty() && self.monitor && self.refused.is_none() {
            self.send_items(cx, triggered, true);
        }
    }

    /// After connecting: subscribe, poll once, and schedule both.
    fn start_telemetry(&mut self, cx: &mut Cx) {
        if self.telemetry.is_empty() || !self.monitor {
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
    pub(crate) fn apply_text(&mut self, cx: &mut Cx, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.offer(cx, &telemetry::Inbound::Text(text));
    }

    fn inbound_text(&mut self, cx: &mut Cx, message: String) {
        if self.refusals.iter().any(|(re, _)| re.is_match(&message)) {
            self.refuse(cx, format!("login refused: {}", message.trim()));
            return;
        }
        // A login the device confirmed can no longer be refused: later free
        // text that happens to match is not about the login.
        self.refusals
            .retain(|(_, accepted)| !accepted.as_ref().is_some_and(|a| a.is_match(&message)));
        self.apply_message(cx, &message);
        let waiting = match self.current.as_ref().and_then(|f| f.awaiting.as_ref()) {
            Some(Await::Text(None)) => true,
            // `expect.reply_contains`: a message that does not hold the text
            // answers something else, or nothing.
            Some(Await::Text(Some(text))) => {
                message.contains(text.as_str())
                    || matches!(&self.transport, Transport::LineTcp { error_match: Some(re), .. } if re.is_match(&message))
            }
            _ => false,
        };
        if let Transport::LineTcp {
            reply_match: Some(re),
            ..
        }
        | Transport::LineUdp {
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

    /// Offer a message to the telemetry rules: as JSON where it parses (a
    /// websocket message, a line or block of a JSON protocol), and as text.
    fn apply_message(&mut self, cx: &mut Cx, text: &str) {
        if let Ok(doc) = serde_json::from_str::<Value>(text) {
            self.offer(cx, &telemetry::Inbound::Json(&doc));
        }
        self.apply_text(cx, text);
    }

    fn inbound_ws(&mut self, cx: &mut Cx, text: String) {
        self.apply_message(cx, &text);
        let ours =
            match self.current.as_ref().and_then(|f| f.awaiting.as_ref()) {
                Some(Await::Ws(Some(fields))) => serde_json::from_str::<Value>(&text)
                    .ok()
                    .is_some_and(|doc| {
                        fields
                            .iter()
                            .all(|(path, want)| match expect::json_path(&doc, path) {
                                Some(Value::String(s)) => s == want,
                                Some(other) => {
                                    let text = other.to_string();
                                    text == *want
                                }
                                None => false,
                            })
                    }),
                Some(Await::Ws(None)) => match &self.transport {
                    Transport::Ws {
                        reply_match: Some(re),
                        ..
                    } => re.is_match(&text),
                    _ => true,
                },
                _ => false,
            };
        if ours {
            self.reply(cx, Reply::Text(text));
        } else {
            self.last_heard = cx.now();
            cx.alive();
        }
    }

    /// The push channel (`telemetry.websocket`).
    fn open_push(&mut self, cx: &mut Cx) {
        if !self.monitor {
            return;
        }
        let Some(push) = &self.push else {
            return;
        };
        let mut request = push.request.clone();
        if let Some(template) = &push.url {
            // Rendered from the settings now, so a refreshed token is used;
            // each value percent-encoded, as in a query.
            let (no_params, no_specs) = (Params::new(), BTreeMap::new());
            let rendered = render(
                template,
                &self.values(&no_params, &no_specs),
                percent_encode,
            )
            .and_then(|url| check_push_url(&url).map(|()| url));
            match rendered {
                Ok(url) => request.url = url,
                Err(e) => {
                    cx.log(Level::Warning, format!("push websocket not opened: {e}"));
                    return;
                }
            }
            if let Some(sio) = &push.socketio {
                request.url.push_str(&engine_io_query(&request.url, sio));
            }
        } else {
            self.current_auth(&mut request.headers);
        }
        cx.ws_open(PUSH, request);
    }

    /// `idle_ms`: the push websocket heard from; reopened if it then goes
    /// quiet for that long.
    fn push_heard(&mut self, cx: &mut Cx) {
        if let Some(idle) = self.push.as_ref().and_then(|p| p.idle) {
            cx.set_timer(PUSH_IDLE, idle);
        }
    }

    fn open_events(&mut self, cx: &mut Cx) {
        if !self.monitor || self.refused.is_some() {
            return;
        }
        if let Some(request) = &self.events {
            let mut request = request.clone();
            self.current_auth(&mut request.headers);
            cx.sse_open(EVENTS, request);
        }
    }

    /// The event stream: each event goes to the JSON rules as
    /// `{"event": name, "data": data}`, with `data` parsed where it is JSON,
    /// and its data to the text rules.
    fn events_input(&mut self, cx: &mut Cx, input: SseInput) {
        if self.refused.is_some() {
            return;
        }
        match input {
            SseInput::Opened => {
                self.events_backoff = RECONNECT_MIN;
                self.side_refreshed = false;
                self.last_heard = cx.now();
                cx.alive();
            }
            SseInput::Event(event) => {
                let data = serde_json::from_str::<Value>(&event.data)
                    .unwrap_or_else(|_| Value::String(event.data.clone()));
                let doc = serde_json::json!({"event": event.event, "data": data});
                self.offer(cx, &telemetry::Inbound::Json(&doc));
                self.apply_text(cx, &event.data);
                self.last_heard = cx.now();
                cx.alive();
            }
            SseInput::Activity => {
                self.last_heard = cx.now();
                cx.alive();
            }
            SseInput::Closed { status, reason } => {
                let refusal = match &self.transport {
                    Transport::Http { auth, refusal, .. } => {
                        *auth != HttpAuth::None && status.is_some_and(|s| refusal.contains(&s))
                    }
                    _ => false,
                };
                if refusal && self.side_refresh(cx) {
                    cx.set_timer(EVENTS_RECONNECT, RECONNECT_MIN);
                    return;
                }
                if refusal {
                    self.refuse(
                        cx,
                        format!("the device refused the credential on its event stream ({reason})"),
                    );
                    return;
                }
                cx.log(
                    Level::Info,
                    format!("event stream closed: {reason}; reopening"),
                );
                cx.set_timer(EVENTS_RECONNECT, self.events_backoff);
                self.events_backoff = (self.events_backoff * 2).min(RECONNECT_MAX);
            }
        }
    }

    fn push_input(&mut self, cx: &mut Cx, input: WsInput) {
        if self.refused.is_some() {
            return;
        }
        match input {
            WsInput::Opened => {
                self.push_backoff = RECONNECT_MIN;
                self.side_refreshed = false;
                // Over Socket.IO the subscriptions wait for the namespace to
                // be joined.
                if self.push.as_ref().is_none_or(|p| p.socketio.is_none()) {
                    self.send_push_items(cx, "");
                }
                self.last_heard = cx.now();
                self.push_heard(cx);
                cx.alive();
            }
            // Text pushed in binary frames (OpenLP's state): offered to the
            // rules as text when it is UTF-8.
            WsInput::Binary(bytes) if std::str::from_utf8(&bytes).is_ok() => {
                let text = String::from_utf8(bytes).unwrap_or_default();
                match self.push.as_ref().and_then(|p| p.socketio.clone()) {
                    Some(sio) => self.socketio_input(cx, &sio, &text),
                    None => self.apply_message(cx, &text),
                }
                self.last_heard = cx.now();
                self.push_heard(cx);
                cx.alive();
            }
            WsInput::Text(text) => {
                match self.push.as_ref().and_then(|p| p.socketio.clone()) {
                    Some(sio) => self.socketio_input(cx, &sio, &text),
                    None => self.apply_message(cx, &text),
                }
                self.last_heard = cx.now();
                self.push_heard(cx);
                cx.alive();
            }
            WsInput::Binary(_) | WsInput::Activity => {
                self.last_heard = cx.now();
                self.push_heard(cx);
                cx.alive();
            }
            WsInput::Closed { reason, .. } => {
                cx.cancel_timer(PUSH_PING);
                cx.cancel_timer(PUSH_RENEW);
                cx.cancel_timer(PUSH_IDLE);
                let credentialed = !auth_headers_empty(&self.push);
                if credentialed && handshake_refused(&reason) && self.side_refresh(cx) {
                    cx.set_timer(PUSH_RECONNECT, RECONNECT_MIN);
                    return;
                }
                if credentialed && handshake_refused(&reason) {
                    self.refuse(
                        cx,
                        format!("the device refused the credential on its websocket ({reason})"),
                    );
                    return;
                }
                cx.log(
                    Level::Info,
                    format!("push websocket closed: {reason}; reopening"),
                );
                cx.set_timer(PUSH_RECONNECT, self.push_backoff);
                self.push_backoff = (self.push_backoff * 2).min(RECONNECT_MAX);
            }
        }
    }

    /// The push websocket's `send` items, each prefixed (`42` plus the
    /// namespace over Socket.IO, where each item is an event's JSON array).
    fn send_push_items(&mut self, cx: &mut Cx, prefix: &str) {
        let items = self
            .push
            .as_ref()
            .map(|p| p.send.clone())
            .unwrap_or_default();
        if let Some(every) = self.push.as_ref().and_then(|p| p.every) {
            cx.set_timer(PUSH_RENEW, every);
        }
        let empty = Params::new();
        let empty_specs = BTreeMap::new();
        let values = self.values(&empty, &empty_specs);
        for item in items {
            match item.as_str().map(|t| render(t, &values, no_escape)) {
                Some(Ok(text)) => cx.ws_send(PUSH, format!("{prefix}{text}")),
                Some(Err(e)) => cx.log(Level::Warning, format!("websocket message not sent: {e}")),
                None => cx.log(Level::Warning, "a websocket message must be a string"),
            }
        }
    }

    /// One Engine.IO packet on the push websocket.
    fn socketio_input(&mut self, cx: &mut Cx, sio: &SocketIo, text: &str) {
        let (kind, payload) = text.split_at(text.len().min(1));
        match kind {
            // Open: join the namespace (Engine.IO v3 also starts pinging).
            "0" => {
                if sio.version == 3 {
                    let every = serde_json::from_str::<Value>(payload)
                        .ok()
                        .and_then(|v| v.get("pingInterval").and_then(Value::as_u64))
                        .unwrap_or(25_000);
                    if let Some(p) = self.push.as_mut().and_then(|p| p.socketio.as_mut()) {
                        p.ping_every = Some(every);
                    }
                    cx.set_timer(PUSH_PING, every);
                }
                let auth = match &sio.auth {
                    None => String::new(),
                    Some(template) => {
                        let empty = Params::new();
                        let empty_specs = BTreeMap::new();
                        let values = self.values(&empty, &empty_specs);
                        match render(template, &values, no_escape) {
                            Ok(text) => text,
                            Err(e) => {
                                cx.log(Level::Warning, format!("socket.io auth not sent: {e}"));
                                String::new()
                            }
                        }
                    }
                };
                cx.ws_send(PUSH, format!("40{}{auth}", sio.namespace));
            }
            // The server's ping (v4), answered with a pong.
            "2" => cx.ws_send(PUSH, format!("3{payload}")),
            "4" => {
                let (sio_kind, rest) = payload.split_at(payload.len().min(1));
                let rest = rest.strip_prefix(sio.namespace.as_str()).unwrap_or(rest);
                match sio_kind {
                    // Joined: now the subscriptions.
                    "0" => self.send_push_items(cx, &format!("42{}", sio.namespace)),
                    "1" => cx.ws_close(PUSH),
                    "2" => {
                        // An optional ack id before the array.
                        let array = rest.trim_start_matches(|c: char| c.is_ascii_digit());
                        if let Ok(Value::Array(args)) = serde_json::from_str::<Value>(array) {
                            let event = args.first().cloned().unwrap_or(Value::Null);
                            let data = args.get(1).cloned().unwrap_or(Value::Null);
                            let doc = serde_json::json!({
                                "event": event,
                                "data": data,
                                "args": args.get(1..).map(<[Value]>::to_vec).unwrap_or_default(),
                            });
                            self.offer(cx, &telemetry::Inbound::Json(&doc));
                        }
                    }
                    "4" => cx.log(
                        Level::Warning,
                        format!("socket.io refused the namespace: {rest}"),
                    ),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn inbound_osc(&mut self, cx: &mut Cx, packet: &[u8]) {
        for message in osc::decode(packet) {
            self.offer(
                cx,
                &telemetry::Inbound::Osc {
                    address: &message.address,
                    types: &message.types,
                    args: &message.args,
                },
            );
            let matches = match self.current.as_ref().and_then(|f| f.awaiting.as_ref()) {
                Some(Await::Osc(None)) => true,
                Some(Await::Osc(Some(reply))) => reply.matches(&message.address),
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

/// Whether the push channel's opening request carries a credential.
fn auth_headers_empty(push: &Option<Push>) -> bool {
    push.as_ref().is_none_or(|p| !p.credentialed)
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
            // A converted value (dB to a fader position) is a number too.
            "float" if sole_converted(template, values).is_some() => {
                osc::Arg::Float(sole_converted(template, values).unwrap()? as f32)
            }
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
        self.open_push(cx);
        self.open_events(cx);
        self.validate_token(cx);
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
            .position(|j| j.id.is_none() && j.item.is_some() && !j.setup)
            .unwrap_or(self.queue.len());
        self.queue.insert(
            at,
            Job {
                id: Some(id),
                name: name.to_string(),
                params: params.clone(),
                ..Job::internal(None, false)
            },
        );
        self.pump(cx);
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => self.opened(cx),
            TcpInput::Data(mut bytes) => {
                if let Some((step, prompt, seen)) = self.prompt_wait.as_mut() {
                    // The prompt has no line ending, so it is looked for in the
                    // raw stream; what came before it is the greeting.
                    seen.extend_from_slice(&bytes);
                    let Some(at) = find(seen, prompt.as_bytes()) else {
                        if seen.len() > 4096 {
                            let keep = prompt.len();
                            seen.drain(..seen.len() - keep);
                        }
                        return;
                    };
                    bytes = seen.split_off(at + prompt.len());
                    let step = *step;
                    self.prompt_wait = None;
                    cx.cancel_timer(PROMPT);
                    self.last_heard = cx.now();
                    if self.send_on_connect(cx, step, true) {
                        self.ready(cx);
                    }
                    if self.prompt_wait.is_some() || bytes.is_empty() {
                        return;
                    }
                }
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

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        if socket == PUSH {
            self.push_input(cx, input);
            return;
        }
        if self.refused.is_some() {
            return;
        }
        match input {
            WsInput::Opened => self.opened(cx),
            WsInput::Text(text) => self.inbound_ws(cx, text),
            WsInput::Binary(_) | WsInput::Activity => {
                self.last_heard = cx.now();
                cx.alive();
            }
            WsInput::Closed { reason, .. } => {
                let credentialed = matches!(
                    self.transport,
                    Transport::Ws { auth, .. } if auth != HttpAuth::None
                );
                if credentialed && handshake_refused(&reason) {
                    self.refuse(cx, format!("the device refused the credential ({reason})"));
                } else {
                    self.lost(cx, reason);
                }
            }
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, _from: SocketAddr, data: &[u8]) {
        if matches!(self.transport, Transport::LineUdp { .. }) {
            // One message per datagram; its line ending is not part of it.
            let text = String::from_utf8_lossy(data);
            self.inbound_text(cx, text.trim_end_matches(['\r', '\n']).to_string());
        } else {
            self.inbound_osc(cx, data);
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        if self
            .oauth
            .as_ref()
            .is_some_and(|o| o.validating == Some(id))
        {
            // The validation endpoint's answer: never offered to the rules.
            if self.refused.is_none() {
                self.validated(cx, result);
            }
            return;
        }
        if self
            .oauth
            .as_ref()
            .is_some_and(|o| o.refreshing == Some(id))
        {
            // The token endpoint's answer: never offered to the rules.
            if self.refused.is_none() {
                self.refreshed(cx, result);
            }
            return;
        }
        let ours = matches!(
            self.current.as_ref().and_then(|f| f.awaiting.as_ref()),
            Some(Await::Http(r)) if *r == id
        );
        // The login's answer holds the session: never offered to the rules.
        if ours && self.current.as_ref().is_some_and(|f| f.job.login) {
            self.request_paths.remove(&id);
            if self.refused.is_none() {
                self.logged_in(cx, result);
            }
            return;
        }
        if let (Some((path, request)), Ok(response)) = (self.request_paths.remove(&id), &result) {
            if (200..300).contains(&response.status) {
                let inbound = telemetry::Inbound::Http {
                    path: &path,
                    headers: &response.headers,
                    body: &response.body,
                    request: request.as_ref(),
                };
                self.offer(cx, &inbound);
                // A text reply ("p1" from a Panasonic camera) is also offered to
                // the rules for text messages.
                if let Ok(text) = std::str::from_utf8(&response.body) {
                    self.apply_text(cx, text);
                }
            }
        }
        if !ours {
            return;
        }
        // Any credential the device can refuse: Basic, Digest or a token.
        let refused = match (&self.transport, &result) {
            (
                Transport::Http {
                    auth,
                    refusal,
                    refusal_json,
                    ..
                },
                Ok(response),
            ) => {
                *auth != HttpAuth::None
                    && (refusal.contains(&response.status)
                        || refused_by_body(refusal_json, response.status, &response.body))
            }
            _ => false,
        };
        // An OAuth token refused: refreshed, and the request sent once more.
        if refused && self.retry_after_refresh(cx) {
            return;
        }
        // The device says the session ended: log in again and send the
        // request once more.
        if let (Some(session), Ok(response)) = (&self.session, &result) {
            let ended = session.relogin_status.contains(&response.status)
                || body_matches(&session.relogin_json, &response.body);
            let flight = self.current.as_ref().unwrap();
            if ended && !refused && !flight.job.relogged && self.needs_session(&flight.job) {
                cx.cancel_timer(REPLY);
                let flight = self.current.take().unwrap();
                cx.log(
                    Level::Info,
                    "the device ended the session: logging in again",
                );
                self.session_token = None;
                self.queue.push_front(Job {
                    relogged: true,
                    ..flight.job
                });
                self.pump(cx);
                return;
            }
        }
        match result {
            Ok(response) if refused => self.refuse(
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
                    headers: response.headers,
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
                let addressed = self
                    .current
                    .as_ref()
                    .and_then(|f| f.awaiting.as_ref())
                    .is_some_and(Await::addressed);
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
                if self.transport.is_stream() && !addressed {
                    // A late reply would otherwise be read as the answer to
                    // the next command. Start the stream afresh.
                    self.lost(cx, "no reply within the timeout".into());
                } else {
                    // Replies matched by address cannot be mistaken.
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
            PUSH_RECONNECT => self.open_push(cx),
            // `idle_ms`: nothing heard, so the connection is dead.
            PUSH_IDLE => {
                cx.log(Level::Info, "push websocket silent too long; reopening");
                cx.cancel_timer(PUSH_PING);
                cx.cancel_timer(PUSH_RENEW);
                cx.ws_close(PUSH);
                cx.set_timer(PUSH_RECONNECT, self.push_backoff);
                self.push_backoff = (self.push_backoff * 2).min(RECONNECT_MAX);
            }
            // `every_ms`: the push websocket's messages again.
            PUSH_RENEW => {
                let prefix = match self.push.as_ref().and_then(|p| p.socketio.as_ref()) {
                    Some(sio) => format!("42{}", sio.namespace),
                    None => String::new(),
                };
                self.send_push_items(cx, &prefix);
            }
            PUSH_PING => {
                // Engine.IO v3: the client pings while the websocket is open.
                let every = self
                    .push
                    .as_ref()
                    .and_then(|p| p.socketio.as_ref())
                    .and_then(|s| s.ping_every);
                if let Some(every) = every {
                    cx.ws_send(PUSH, "2".to_string());
                    cx.set_timer(PUSH_PING, every);
                }
            }
            EVENTS_RECONNECT => self.open_events(cx),
            OAUTH_VALIDATE => self.validate_token(cx),
            OAUTH_RETRY => {
                let due = self
                    .oauth
                    .as_ref()
                    .is_some_and(|o| o.needs_refresh(&self.settings, cx.unix_millis()));
                if due {
                    self.start_refresh(cx);
                }
            }
            PROMPT => {
                if self.prompt_wait.take().is_some() {
                    self.lost(cx, "no login prompt within the timeout".into());
                }
            }
            RENEW | POLL if !self.monitor => {}
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
        match self.transport {
            Transport::Ws { .. } => cx.ws_close(SOCKET),
            _ => cx.tcp_close(SOCKET),
        }
        if self.push.is_some() {
            cx.ws_close(PUSH);
        }
        if self.events.is_some() {
            cx.sse_close(EVENTS);
        }
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        if stream == EVENTS {
            self.events_input(cx, input);
        }
    }

    /// New settings, applied live: every later request and connection uses
    /// them; the connection, state and commands in flight are kept. A
    /// refused credential is let go: the device connects again.
    fn update_settings(&mut self, cx: &mut Cx, settings: &Params) -> SettingsUpdate {
        let ctx = OpenContext {
            settings: settings.clone(),
            ..self.ctx.clone()
        };
        let fresh = match SpecEngine::new(self.spec.clone(), ctx) {
            Ok(fresh) => fresh,
            Err(reason) => return SettingsUpdate::Rejected(reason),
        };
        self.ctx = fresh.ctx;
        self.settings = fresh.settings;
        self.transport = fresh.transport;
        self.events = fresh.events;
        // A session was opened with the old credential: the next request
        // logs in with the new one.
        self.session = fresh.session;
        self.session_token = None;
        if let (Some(push), Some(new)) = (self.push.as_mut(), fresh.push) {
            push.request = new.request;
            push.credentialed = new.credentialed;
            push.send = new.send;
        }
        // A refresh in flight was for the old tokens: its answer is ignored.
        self.oauth = fresh.oauth;
        self.side_refreshed = false;
        if self.refused.take().is_some() {
            cx.log(Level::Info, "settings changed: connecting again");
            self.refusals.clear();
            self.set_link(cx, Connection::Connecting);
            self.backoff = RECONNECT_MIN;
            self.connect(cx);
            self.open_push(cx);
            self.open_events(cx);
            self.validate_token(cx);
            return SettingsUpdate::Applied;
        }
        // New settings may carry a new token: it is validated now.
        self.validate_token(cx);
        // A request waiting on a refresh goes with the new tokens, refreshed
        // first if they need it.
        if self.current.as_ref().is_some_and(|f| f.retry.is_some()) {
            let due = self
                .oauth
                .as_ref()
                .is_some_and(|o| o.needs_refresh(&self.settings, cx.unix_millis()));
            if due {
                self.start_refresh(cx);
            } else {
                self.resend(cx);
            }
        }
        self.pump(cx);
        SettingsUpdate::Applied
    }
}

/// `transport.listen_port`: a port, or `{setting: name}` naming an integer
/// setting; a setting left empty means no fixed port.
fn listen_port(v: Option<&Value>, settings: &Params) -> Result<Option<u16>, String> {
    let port = match v {
        None => return Ok(None),
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::Object(o)) => {
            let name = o
                .get("setting")
                .and_then(Value::as_str)
                .ok_or("listen_port needs a port or {setting: name}")?;
            match settings.get(name) {
                None | Some(Value::Null) => return Ok(None),
                Some(v) => v.as_u64(),
            }
        }
        Some(_) => None,
    };
    match port {
        Some(p @ 1..=65535) => Ok(Some(p as u16)),
        _ => Err("listen_port must be a port number, 1 to 65535".into()),
    }
}

/// Where `needle` first occurs in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
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
        let mut spec = Catalog::source_tree()
            .device("propresenter")
            .unwrap()
            .clone();
        spec.transport.as_mut().unwrap()["auth"] = json!("basic");
        let settings = json!({"username": "u", "password": "wrong"})
            .as_object()
            .unwrap()
            .clone();
        SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "propresenter-7".into(),
                channels: None,
                settings,
                monitor: true,
            },
        )
        .unwrap()
    }

    /// Kramer's line spec, with a login that waits for a password prompt.
    fn prompted_login() -> SpecEngine {
        let mut spec = Catalog::source_tree()
            .device("kramer-p3000")
            .unwrap()
            .clone();
        spec.on_connect = vec![json!({
            "after_prompt": "Enter password:",
            "send": "s3cret",
            "refused": "^Authentication error",
        })];
        SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "p3000-generic".into(),
                channels: None,
                settings: Params::new(),
                monitor: true,
            },
        )
        .unwrap()
    }

    fn tcp_sent(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(String::from_utf8_lossy(data).into_owned()),
                _ => None,
            })
            .collect()
    }

    fn connected(actions: &[Action]) -> bool {
        actions
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Connected)))
    }

    #[test]
    fn bearer_tokens_and_a_chosen_scheme() {
        let mut spec = Catalog::source_tree()
            .device("propresenter")
            .unwrap()
            .clone();
        let t = spec.transport.as_mut().unwrap();
        t["auth"] = json!("bearer");
        t["scheme"] = json!({"setting": "scheme"});
        t["accept_invalid_certs"] = json!(true);
        let settings = json!({"token": "abc123", "scheme": "https"});
        let mut e = SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "propresenter-7".into(),
                channels: None,
                settings: settings.as_object().unwrap().clone(),
                monitor: true,
            },
        )
        .unwrap();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let request = cx
            .take()
            .into_iter()
            .find_map(|a| match a {
                Action::Http { request, .. } => Some(request),
                _ => None,
            })
            .expect("the probe");
        assert!(
            request.url.starts_with("https://127.0.0.1:"),
            "{}",
            request.url
        );
        assert!(request.accept_invalid_certs);
        assert!(request.digest.is_none());
        assert!(request
            .headers
            .contains(&("Authorization".to_string(), "Bearer abc123".to_string())));
    }

    #[test]
    fn a_host_opened_by_name_keeps_its_name_in_urls() {
        let mut spec = Catalog::source_tree()
            .device("propresenter")
            .unwrap()
            .clone();
        let t = spec.transport.as_mut().unwrap();
        t["scheme"] = json!("https");
        t["port"] = json!(443);
        let mut e = SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
                host_name: Some("api.example.com".into()),
                port: None,
                model: "propresenter-7".into(),
                channels: None,
                settings: Params::new(),
                monitor: true,
            },
        )
        .unwrap();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let request = cx
            .take()
            .into_iter()
            .find_map(|a| match a {
                Action::Http { request, .. } => Some(request),
                _ => None,
            })
            .expect("the probe");
        assert!(
            request.url.starts_with("https://api.example.com:443/"),
            "{}",
            request.url
        );
    }

    #[test]
    fn the_operator_chooses_basic_or_bearer() {
        let probe_auth = |settings: Value| {
            let mut spec = Catalog::source_tree()
                .device("propresenter")
                .unwrap()
                .clone();
            spec.transport.as_mut().unwrap()["auth"] = json!({"setting": "auth"});
            let mut e = SpecEngine::new(
                Arc::new(spec),
                OpenContext {
                    host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    host_name: None,
                    port: None,
                    model: "propresenter-7".into(),
                    channels: None,
                    settings: settings.as_object().unwrap().clone(),
                    monitor: true,
                },
            )
            .unwrap();
            let mut cx = Cx::new(0);
            e.start(&mut cx);
            cx.take()
                .into_iter()
                .find_map(|a| match a {
                    Action::Http { request, .. } => Some(request),
                    _ => None,
                })
                .expect("the probe")
                .headers
                .into_iter()
                .find(|(k, _)| k == "Authorization")
                .map(|(_, v)| v)
        };
        assert_eq!(
            probe_auth(json!({"auth": "bearer", "token": "t0k"})).as_deref(),
            Some("Bearer t0k")
        );
        assert_eq!(
            probe_auth(json!({"auth": "basic", "username": "a", "password": "b"})).as_deref(),
            Some("Basic YTpi")
        );
        assert_eq!(probe_auth(json!({})), None);
    }

    #[test]
    fn planning_center_takes_a_personal_access_token_or_oauth_tokens() {
        let first_request = |settings: Value| {
            let spec = Catalog::source_tree()
                .device("planningcenter-services")
                .unwrap()
                .clone();
            let mut settings = settings.as_object().unwrap().clone();
            settings.insert("service_type_id".into(), json!("1"));
            settings.insert("plan_id".into(), json!("2"));
            let mut e = SpecEngine::new(
                Arc::new(spec),
                OpenContext {
                    host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    host_name: Some("api.planningcenteronline.com".into()),
                    port: None,
                    model: "services-v2".into(),
                    channels: None,
                    settings,
                    monitor: false,
                },
            )
            .unwrap();
            let mut cx = Cx::new(0);
            e.start(&mut cx);
            cx.take()
                .into_iter()
                .find_map(|a| match a {
                    Action::Http { request, .. } => Some(request),
                    _ => None,
                })
                .expect("a request")
        };
        let auth = |r: &HttpRequest| {
            r.headers
                .iter()
                .find(|(k, _)| k == "Authorization")
                .map(|(_, v)| v.clone())
        };
        let basic = first_request(json!({"auth": "basic", "username": "a", "password": "b"}));
        assert_eq!(auth(&basic).as_deref(), Some("Basic YTpi"));
        let plain = first_request(json!({"auth": "oauth2", "access_token": "t0k"}));
        assert_eq!(auth(&plain).as_deref(), Some("Bearer t0k"));
        assert_eq!(
            plain.url,
            "https://api.planningcenteronline.com:443/services/v2"
        );
        // No access token yet: the refresh goes first, to the token endpoint.
        let refresh = first_request(json!({"auth": "oauth2", "refresh_token": "r",
                                            "client_id": "id", "client_secret": "s"}));
        assert_eq!(
            refresh.url,
            "https://api.planningcenteronline.com/oauth/token"
        );
        assert_eq!(refresh.method, "POST");
        assert_eq!(
            refresh.body.as_deref(),
            Some(&b"grant_type=refresh_token&refresh_token=r&client_id=id&client_secret=s"[..])
        );
        assert_eq!(auth(&refresh), None);
    }

    #[test]
    fn line_udp_sends_one_message_per_datagram() {
        let mut spec = Catalog::source_tree().device("rosstalk").unwrap().clone();
        spec.transport = Some(json!({"type": "line-udp", "port": 6553, "terminator": "none"}));
        spec.telemetry = None;
        let mut e = SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "carbonite".into(),
                channels: None,
                settings: Params::new(),
                monitor: true,
            },
        )
        .unwrap();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(
            a,
            Action::UdpOpen {
                bind: Bind::Ephemeral,
                ..
            }
        )));
        let mut cx = Cx::new(1);
        e.command(&mut cx, 1, "fade_to_black", &Params::new());
        let a = cx.take();
        let sent: Vec<&[u8]> = a
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, to, .. } if to.port() == 6553 => Some(data.as_slice()),
                _ => None,
            })
            .collect();
        assert_eq!(sent, vec![&b"FTB"[..]]);
        assert!(a.iter().any(|a| matches!(
            a,
            Action::Complete {
                result: Ok(Outcome::Unverified),
                ..
            }
        )));
    }

    #[test]
    fn osc_udp_can_listen_on_a_fixed_port() {
        let open = |listen: Value, settings: Value| {
            let mut spec = Catalog::source_tree()
                .device("behringer-x32")
                .unwrap()
                .clone();
            spec.transport.as_mut().unwrap()["listen_port"] = listen;
            let mut e = SpecEngine::new(
                Arc::new(spec),
                OpenContext {
                    host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    host_name: None,
                    port: None,
                    model: "x32".into(),
                    channels: Some(32),
                    settings: settings.as_object().unwrap().clone(),
                    monitor: true,
                },
            )
            .unwrap();
            let mut cx = Cx::new(0);
            e.start(&mut cx);
            cx.take().into_iter().find_map(|a| match a {
                Action::UdpOpen { bind, .. } => Some(bind),
                _ => None,
            })
        };
        assert_eq!(open(json!(8001), json!({})), Some(Bind::Shared(8001)));
        let by_setting = json!({"setting": "feedback_port"});
        assert_eq!(
            open(by_setting.clone(), json!({"feedback_port": 9000})),
            Some(Bind::Shared(9000))
        );
        assert_eq!(open(by_setting, json!({})), Some(Bind::Ephemeral));
    }

    #[test]
    fn a_login_step_waits_for_its_prompt() {
        let mut e = prompted_login();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        cx.take();

        // Nothing is sent, and the link is not up, until the prompt arrives.
        let mut cx = Cx::new(1);
        e.tcp(&mut cx, SOCKET, TcpInput::Connected);
        e.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"Welcome\r\nEnter pass".to_vec()),
        );
        let a = cx.take();
        assert!(tcp_sent(&a).is_empty() && !connected(&a), "{a:?}");

        // The prompt has no line ending; it can arrive split.
        let mut cx = Cx::new(2);
        e.tcp(&mut cx, SOCKET, TcpInput::Data(b"word:".to_vec()));
        let a = cx.take();
        let sent = tcp_sent(&a);
        assert_eq!(sent.first().map(String::as_str), Some("s3cret\r"));
        assert!(connected(&a));
        assert!(sent.len() > 1, "telemetry starts after the login: {sent:?}");

        // A refusal is terminal.
        let mut cx = Cx::new(3);
        e.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"Authentication error.\r\n".to_vec()),
        );
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
    }

    #[test]
    fn a_login_step_can_wait_for_its_reply() {
        let mut spec = Catalog::source_tree()
            .device("kramer-p3000")
            .unwrap()
            .clone();
        spec.on_connect = vec![json!({"send": "#LOGIN admin", "await_reply": true})];
        spec.telemetry = None;
        let mut e = SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "p3000-generic".into(),
                channels: None,
                settings: Params::new(),
                monitor: true,
            },
        )
        .unwrap();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.tcp(&mut cx, SOCKET, TcpInput::Connected);
        // A command issued now waits behind the login step.
        let params = json!({"input": 3, "output": 2})
            .as_object()
            .unwrap()
            .clone();
        e.command(&mut cx, 7, "route_video", &params);
        assert_eq!(tcp_sent(&cx.take()), vec!["#LOGIN admin\r"]);
        // The login's reply is consumed by the login; the command goes next.
        let mut cx = Cx::new(1);
        e.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"~01@LOGIN admin OK\r\n".to_vec()),
        );
        assert_eq!(tcp_sent(&cx.take()), vec!["#ROUTE 1,2,3\r"]);
        let mut cx = Cx::new(2);
        e.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"~01@ROUTE 1,2,3 OK\r\n".to_vec()),
        );
        assert!(cx.take().iter().any(|a| matches!(
            a,
            Action::Complete {
                id: 7,
                result: Ok(Outcome::Ack)
            }
        )));
    }

    #[test]
    fn no_prompt_drops_the_connection() {
        let mut e = prompted_login();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        let mut cx = Cx::new(5_000);
        e.timer(&mut cx, PROMPT);
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        assert!(tcp_sent(&a).is_empty());
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
        let spec = Catalog::source_tree()
            .device("behringer-x32")
            .unwrap()
            .clone();
        let mut e = SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "x32".into(),
                channels: Some(32),
                settings: Params::new(),
                monitor: true,
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
                headers: Vec::new(),
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
    fn open_spec(spec: DeviceSpec, model: &str, settings: Value) -> SpecEngine {
        SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: model.into(),
                channels: None,
                settings: settings.as_object().unwrap().clone(),
                monitor: true,
            },
        )
        .unwrap()
    }

    fn ws_sent(actions: &[Action], socket: Key) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::WsSend { socket: s, text } if *s == socket => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn completed(actions: &[Action]) -> Vec<(CommandId, crate::module::CommandResult)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Complete { id, result } => Some((*id, result.clone())),
                _ => None,
            })
            .collect()
    }

    /// A websocket device: JSON requests, replies matched by an id field or
    /// taken in order, pushed messages through the rules.
    fn ws_device(auth: &str) -> SpecEngine {
        let mut spec = Catalog::source_tree()
            .device("kramer-p3000")
            .unwrap()
            .clone();
        spec.transport = Some(json!({
            "type": "ws", "port": 9000, "path": "/api/v1", "subprotocol": "v1.ctl",
            "auth": auth, "timeout_ms": 1000,
        }));
        spec.settings = serde_json::from_value(json!({
            "token": {"type": "string", "secret": true, "default": "t0k"},
        }))
        .unwrap();
        spec.on_connect = vec![json!(r#"{"action":"hello"}"#)];
        spec.commands = serde_json::from_value(json!({
            "get_level": {
                "params": {"layer": {"type": "int", "min": 1, "required": true}},
                "send": r#"{"action":"get","parameter":"/layers/{layer}/level"}"#,
                "expect": {"reply_json": {"$.type": "parameter_get", "$.path": "/layers/{layer}/level"},
                           "json_path": "$.value"},
                "returns": "value",
            },
            "ping": {"send": "{\"ping\":1}", "expect": {"json_equals": {"$.pong": 1}}, "returns": "ack"},
        }))
        .unwrap();
        spec.models[0].supports = vec!["get_level".into(), "ping".into()];
        spec.state = serde_json::from_value(json!({
            "layers.*.level": {"type": "float", "description": "x"},
        }))
        .unwrap();
        spec.telemetry = Some(json!({
            "subscribe": {"send": [r#"{"action":"subscribe","parameter":"/layers/1/level"}"#]},
            "updates": [{
                "json_match": {"$.type": "^parameter_(update|get)$", "$.path": "^/layers/(\\d+)/level$"},
                "json": {"value": "$.value"},
                "state": {"layers.{2}.level": "{value}"},
            }],
        }));
        open_spec(spec, "p3000-generic", json!({"token": "t0k"}))
    }

    #[test]
    fn a_websocket_transport_matches_replies_by_id_or_order() {
        let mut e = ws_device("bearer");
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let request = cx
            .take()
            .into_iter()
            .find_map(|a| match a {
                Action::WsOpen {
                    socket: SOCKET,
                    request,
                } => Some(request),
                _ => None,
            })
            .expect("the websocket opens");
        assert_eq!(request.url, "ws://127.0.0.1:9000/api/v1");
        assert!(!request.accept_invalid_certs);
        assert!(request
            .headers
            .contains(&("Sec-WebSocket-Protocol".into(), "v1.ctl".into())));
        assert!(request
            .headers
            .contains(&("Authorization".into(), "Bearer t0k".into())));

        // Opened: the connection step, then the subscription, straight away.
        let mut cx = Cx::new(1);
        e.ws(&mut cx, SOCKET, WsInput::Opened);
        let a = cx.take();
        assert!(connected(&a));
        assert_eq!(
            ws_sent(&a, SOCKET),
            vec![
                r#"{"action":"hello"}"#.to_string(),
                r#"{"action":"subscribe","parameter":"/layers/1/level"}"#.to_string()
            ]
        );

        // A reply is the message holding the rendered id fields; a push that
        // arrives first is state, not the reply.
        let mut cx = Cx::new(2);
        let params = json!({"layer": 2}).as_object().unwrap().clone();
        e.command(&mut cx, 1, "get_level", &params);
        assert_eq!(
            ws_sent(&cx.take(), SOCKET),
            vec![r#"{"action":"get","parameter":"/layers/2/level"}"#.to_string()]
        );
        let mut cx = Cx::new(3);
        e.ws(
            &mut cx,
            SOCKET,
            WsInput::Text(
                r#"{"type":"parameter_update","path":"/layers/1/level","value":0.5}"#.into(),
            ),
        );
        let a = cx.take();
        assert!(completed(&a).is_empty());
        assert!(a.contains(&Action::State(json!({"layers": {"1": {"level": 0.5}}}))));
        let mut cx = Cx::new(4);
        e.ws(
            &mut cx,
            SOCKET,
            WsInput::Text(
                r#"{"type":"parameter_get","path":"/layers/2/level","value":0.25}"#.into(),
            ),
        );
        assert_eq!(
            completed(&cx.take()),
            vec![(1, Ok(Outcome::Value { value: json!(0.25) }))]
        );

        // Without reply_json, the next message is the reply.
        let mut cx = Cx::new(5);
        e.command(&mut cx, 2, "ping", &Params::new());
        e.ws(&mut cx, SOCKET, WsInput::Text(r#"{"pong":2}"#.into()));
        assert!(matches!(
            &completed(&cx.take())[..],
            [(2, Err(CommandError::DeviceError { .. }))]
        ));

        // An unanswered id-matched request times out without resetting the
        // stream: a late reply names what it answers.
        let mut cx = Cx::new(6);
        e.command(&mut cx, 3, "get_level", &params);
        e.timer(&mut cx, REPLY);
        let a = cx.take();
        assert_eq!(completed(&a), vec![(3, Err(CommandError::Timeout))]);
        assert!(!a.iter().any(|a| matches!(a, Action::WsClose { .. })));
    }

    #[test]
    fn a_refused_websocket_handshake_is_terminal() {
        let mut e = ws_device("bearer");
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.ws(
            &mut cx,
            SOCKET,
            WsInput::Closed {
                code: None,
                reason: "connect: HTTP 401 Unauthorized".into(),
            },
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a.contains(&Action::SetTimer {
            key: RECONNECT,
            after: RECONNECT_MIN
        }));
        // Without a credential it is an ordinary loss, retried.
        let mut e = ws_device("none");
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.ws(
            &mut cx,
            SOCKET,
            WsInput::Closed {
                code: None,
                reason: "connect: HTTP 401 Unauthorized".into(),
            },
        );
        assert!(cx.take().contains(&Action::SetTimer {
            key: RECONNECT,
            after: RECONNECT_MIN
        }));
    }

    #[test]
    fn a_secure_websocket_may_accept_a_self_signed_certificate() {
        let mut spec = Catalog::source_tree()
            .device("kramer-p3000")
            .unwrap()
            .clone();
        spec.transport = Some(json!({
            "type": "ws", "port": 9443, "path": "/", "scheme": "wss",
            "accept_invalid_certs": true,
        }));
        let mut e = open_spec(spec, "p3000-generic", json!({}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let request = cx
            .take()
            .into_iter()
            .find_map(|a| match a {
                Action::WsOpen { request, .. } => Some(request),
                _ => None,
            })
            .expect("the websocket opens");
        assert_eq!(request.url, "wss://127.0.0.1:9443/");
        assert!(request.accept_invalid_certs);

        // A push websocket inherits it from an HTTPS transport.
        let mut spec = with_push_websocket();
        spec.transport.as_mut().unwrap()["accept_invalid_certs"] = json!(true);
        let mut e = open_spec(spec, "arena", json!({}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        assert!(cx.take().iter().any(|a| matches!(
            a,
            Action::WsOpen { socket: PUSH, request } if request.accept_invalid_certs
        )));
    }

    /// Resolume's REST spec with a push websocket declared in the format
    /// itself (the shipped spec uses its native extension instead).
    fn with_push_websocket() -> crate::catalog::DeviceSpec {
        let mut spec = Catalog::source_tree().device("resolume").unwrap().clone();
        spec.telemetry = Some(json!({
            "websocket": {
                "path": "/api/v1",
                "send": [r#"{"action":"subscribe","parameter":"/composition/master"}"#],
            },
            "updates": [{
                "json_match": {
                    "$.type": "^parameter_(subscribed|update|get|set)$",
                    "$.path": "^/composition/master$",
                },
                "json": {"value": "$.value"},
                "state": {"composition.master": "{value}"},
            }],
        }));
        spec
    }

    #[test]
    fn a_push_websocket_beside_http() {
        let mut spec = with_push_websocket();
        spec.transport.as_mut().unwrap()["port"] = json!(8080);
        let mut e = open_spec(spec, "arena", json!({}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let a = cx.take();
        let request = a
            .iter()
            .find_map(|a| match a {
                Action::WsOpen {
                    socket: PUSH,
                    request,
                } => Some(request.clone()),
                _ => None,
            })
            .expect("the push channel opens");
        assert_eq!(request.url, "ws://127.0.0.1:8080/api/v1");
        assert!(!request.accept_invalid_certs);

        let mut cx = Cx::new(1);
        e.ws(&mut cx, PUSH, WsInput::Opened);
        let subscriptions = ws_sent(&cx.take(), PUSH);
        assert!(subscriptions
            .iter()
            .any(|t| t.contains(r#""action":"subscribe""#)));

        let mut cx = Cx::new(2);
        e.ws(
            &mut cx,
            PUSH,
            WsInput::Text(
                r#"{"type":"parameter_update","path":"/composition/master","id":7,"valuetype":"ParamRange","value":0.5}"#
                    .into(),
            ),
        );
        assert!(cx
            .take()
            .contains(&Action::State(json!({"composition": {"master": 0.5}}))));

        // Closed: reopened with backoff; the HTTP side is untouched.
        let mut cx = Cx::new(3);
        e.ws(
            &mut cx,
            PUSH,
            WsInput::Closed {
                code: None,
                reason: "closed by the device".into(),
            },
        );
        let a = cx.take();
        assert!(a.contains(&Action::SetTimer {
            key: PUSH_RECONNECT,
            after: RECONNECT_MIN
        }));
        assert!(!a.iter().any(|a| matches!(a, Action::Connection(_))));
        let mut cx = Cx::new(4);
        e.timer(&mut cx, PUSH_RECONNECT);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::WsOpen { socket: PUSH, .. })));
    }

    #[test]
    fn an_osc_poll_item_waits_for_its_reply_address() {
        let spec = Catalog::source_tree().device("etc-eos").unwrap().clone();
        let mut e = open_spec(spec, "eos", json!({}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        // The first poll query is in flight; a push on another address and
        // the query's own address are not its reply, /eos/out/get/... is.
        let push = framing::frame_packet(
            PacketFraming::LengthPrefixed,
            &osc::encode("/eos/out/get/version", &[osc::Arg::Str("3.3.0".into())]),
        );
        let before = e.queue.len();
        let mut cx = Cx::new(1);
        e.tcp(&mut cx, SOCKET, TcpInput::Data(push));
        assert!(cx
            .take()
            .contains(&Action::State(json!({"version": {"eos": "3.3.0"}}))));
        assert_eq!(e.queue.len() + 1, before, "the next query was sent");
        // A query that is never answered times out without dropping the link.
        let mut cx = Cx::new(2);
        e.timer(&mut cx, REPLY);
        let a = cx.take();
        assert!(!a.iter().any(|a| matches!(a, Action::TcpClose { .. })));
        assert!(e.current.is_some(), "and the next query goes");
    }

    #[test]
    fn an_accepted_login_ends_the_refusal_watch() {
        let mut spec = Catalog::source_tree()
            .device("kramer-p3000")
            .unwrap()
            .clone();
        spec.on_connect = vec![json!({
            "send": "login", "refused": "denied", "accepted": "^Welcome",
        })];
        spec.telemetry = None;
        let mut e = open_spec(spec, "p3000-generic", json!({}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.tcp(&mut cx, SOCKET, TcpInput::Connected);
        e.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"Welcome admin\r\n".to_vec()),
        );
        e.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"access denied to file\r\n".to_vec()),
        );
        assert!(!cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
    }

    #[test]
    fn a_converted_value_is_sent_as_an_osc_float() {
        let spec = Catalog::source_tree()
            .device("behringer-x32")
            .unwrap()
            .clone();
        let mut e = open_spec(spec, "x32", json!({}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.queue.clear();
        e.current = None;
        cx.take();
        let mut cx = Cx::new(1);
        let params = json!({"channel": 1, "level_db": 0.0})
            .as_object()
            .unwrap()
            .clone();
        e.command(&mut cx, 1, "set_channel_fader_db", &params);
        let sent: Vec<Vec<u8>> = cx
            .take()
            .into_iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } => Some(data),
                _ => None,
            })
            .collect();
        assert_eq!(
            sent,
            [osc::encode("/ch/01/mix/fader", &[osc::Arg::Float(0.75)])]
        );
    }

    fn x32(monitor: bool) -> SpecEngine {
        let spec = Catalog::source_tree()
            .device("behringer-x32")
            .unwrap()
            .clone();
        SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "x32".into(),
                channels: Some(32),
                settings: Params::new(),
                monitor,
            },
        )
        .unwrap()
    }

    fn osc_addresses(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } => {
                    let end = data.iter().position(|b| *b == 0).unwrap_or(data.len());
                    Some(String::from_utf8_lossy(&data[..end]).into_owned())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_device_opened_for_commands_only_is_not_subscribed_or_read() {
        // Monitored: the subscription and the connect-time read go out.
        let mut e = x32(true);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let sent = osc_addresses(&cx.take());
        assert!(sent.iter().any(|a| a == "/xremote"), "{sent:?}");

        // Commands only: nothing but the liveness probe, and commands still go.
        let mut e = x32(false);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let sent = osc_addresses(&cx.take());
        assert_eq!(sent, ["/info"]);
        let mut cx = Cx::new(10);
        e.timer(&mut cx, RENEW);
        e.timer(&mut cx, POLL);
        assert!(osc_addresses(&cx.take()).is_empty());
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let mut spec = Catalog::source_tree()
            .device("kramer-p3000")
            .unwrap()
            .clone();
        spec.telemetry = None;
        let mut e = SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "p3000-generic".into(),
                channels: None,
                settings: Params::new(),
                monitor: true,
            },
        )
        .unwrap();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let params = json!({"input": 3, "output": 2})
            .as_object()
            .unwrap()
            .clone();
        let mut cx = Cx::new(100);
        e.command(&mut cx, 7, "route_video", &params);
        cx.take();
        let mut cx = Cx::new(142);
        e.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(
                b"~01@ROUTE 1,2,3 OK
"
                .to_vec(),
            ),
        );
        assert!(cx.take().contains(&Action::RoundTrip(42)));
    }

    /// ProPresenter's HTTP spec with the given telemetry.
    fn http_with(telemetry: Value, state: Value, monitor: bool) -> SpecEngine {
        let mut spec = Catalog::source_tree()
            .device("propresenter")
            .unwrap()
            .clone();
        spec.telemetry = Some(telemetry);
        spec.state = serde_json::from_value(state).unwrap();
        SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "propresenter-7".into(),
                channels: None,
                settings: Params::new(),
                monitor,
            },
        )
        .unwrap()
    }

    #[test]
    fn an_http_device_is_polled_from_the_start() {
        let telemetry = json!({
            "poll": {"send": [{"method": "GET", "path": "/polled"}], "every_ms": 5000},
            "updates": [{"path": "^/polled$", "json": {"v": "$.v"}, "state": {"x": "{v}"}}],
        });
        let state = json!({"x": {"type": "int", "description": "x"}});
        let mut e = http_with(telemetry.clone(), state.clone(), true);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::SetTimer {
            key: POLL,
            after: 5000
        }));
        // The poll goes first, then the probe; their replies in turn.
        let poll = http_ids(&a)[0];
        let mut cx = Cx::new(5);
        e.http_response(
            &mut cx,
            poll,
            Ok(HttpResponse {
                headers: Vec::new(),
                status: 200,
                body: br#"{"v": 7}"#.to_vec(),
            }),
        );
        let a = cx.take();
        assert!(a.contains(&Action::State(json!({"x": 7}))));
        assert_eq!(http_ids(&a).len(), 1, "then the probe");

        // Commands only: no poll.
        let mut e = http_with(telemetry, state, false);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let a = cx.take();
        assert_eq!(http_ids(&a).len(), 1, "the probe alone");
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: POLL, .. })));
    }

    #[test]
    fn an_event_stream_beside_http_keeps_state_current() {
        let telemetry = json!({
            "sse": {"path": "/events"},
            "updates": [{
                "json_match": {"$.event": "^brightness$"},
                "json": {"value": "$.data.value"},
                "state": {"output.brightness": "{value}"},
            }],
        });
        let state = json!({"output.brightness": {"type": "float", "description": "x"}});
        let mut e = http_with(telemetry.clone(), state.clone(), true);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let opened = cx.take().into_iter().find_map(|a| match a {
            Action::SseOpen { stream, request } => Some((stream, request)),
            _ => None,
        });
        let (stream, request) = opened.expect("the event stream");
        assert!(request.url.ends_with("/events"), "{}", request.url);
        assert!(request
            .headers
            .contains(&("Accept".to_string(), "text/event-stream".to_string())));

        let mut cx = Cx::new(10);
        e.sse(
            &mut cx,
            stream,
            SseInput::Event(crate::sse::SseEvent {
                event: "brightness".into(),
                data: r#"{"value": 0.5}"#.into(),
            }),
        );
        assert!(cx
            .take()
            .contains(&Action::State(json!({"output": {"brightness": 0.5}}))));

        // A closed stream is reopened after a backoff.
        let mut cx = Cx::new(20);
        e.sse(
            &mut cx,
            stream,
            SseInput::Closed {
                status: None,
                reason: "eof".into(),
            },
        );
        assert!(cx.take().iter().any(|a| matches!(
            a,
            Action::SetTimer {
                key: EVENTS_RECONNECT,
                ..
            }
        )));

        // Commands only: no stream.
        let mut e = http_with(telemetry, state, false);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        assert!(!cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::SseOpen { .. })));
    }

    #[test]
    fn json_lines_go_to_the_json_rules() {
        let mut spec = Catalog::source_tree()
            .device("kramer-p3000")
            .unwrap()
            .clone();
        spec.telemetry = Some(json!({"updates": [{
            "json_match": {"$.type": "^level$"},
            "json": {"db": "$.db"},
            "state": {"level": "{db}"},
        }]}));
        spec.state =
            serde_json::from_value(json!({"level": {"type": "float", "description": "x"}}))
                .unwrap();
        let mut e = SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "p3000-generic".into(),
                channels: None,
                settings: Params::new(),
                monitor: true,
            },
        )
        .unwrap();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let mut cx = Cx::new(1);
        e.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"{\"type\":\"level\",\"db\":-6.5}\r\n".to_vec()),
        );
        assert!(cx.take().contains(&Action::State(json!({"level": -6.5}))));
    }

    #[test]
    fn requests_may_be_patched() {
        let e = credentialed();
        let empty = Params::new();
        let specs = BTreeMap::new();
        let values = e.values(&empty, &specs);
        let Outgoing::Http(request) = e
            .build(
                &json!({"method": "PATCH", "path": "/v1/x", "body": "{}"}),
                &values,
            )
            .unwrap()
        else {
            panic!("an HTTP request");
        };
        assert_eq!(request.method, "PATCH");
    }

    #[test]
    fn a_socketio_push_channel_joins_pings_and_applies_events() {
        let telemetry = json!({
            "websocket": {"path": "/socket.io/", "socketio": true,
                          "send": [r#"["subscribe",{"topic":"slides"}]"#]},
            "updates": [{
                "json_match": {"$.event": "^slide$"},
                "json": {"index": "$.data.index"},
                "state": {"slide.index": "{index}"},
            }],
        });
        let state = json!({"slide.index": {"type": "int", "description": "x"}});
        let mut e = http_with(telemetry, state, true);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let url = cx.take().into_iter().find_map(|a| match a {
            Action::WsOpen {
                socket: PUSH,
                request,
            } => Some(request.url),
            _ => None,
        });
        assert!(url
            .as_deref()
            .is_some_and(|u| u.ends_with("/socket.io/?EIO=4&transport=websocket")));

        let sent = |cx: Cx| -> Vec<String> {
            cx.take()
                .into_iter()
                .filter_map(|a| match a {
                    Action::WsSend { socket: PUSH, text } => Some(text),
                    _ => None,
                })
                .collect()
        };
        let mut cx = Cx::new(1);
        e.ws(&mut cx, PUSH, WsInput::Opened);
        assert!(sent(cx).is_empty());
        let mut cx = Cx::new(2);
        e.ws(
            &mut cx,
            PUSH,
            WsInput::Text(r#"0{"sid":"a","pingInterval":25000}"#.into()),
        );
        assert_eq!(sent(cx), ["40"]);
        let mut cx = Cx::new(3);
        e.ws(&mut cx, PUSH, WsInput::Text(r#"40{"sid":"b"}"#.into()));
        assert_eq!(sent(cx), [r#"42["subscribe",{"topic":"slides"}]"#]);
        let mut cx = Cx::new(4);
        e.ws(&mut cx, PUSH, WsInput::Text("2".into()));
        assert_eq!(sent(cx), ["3"]);
        let mut cx = Cx::new(5);
        e.ws(
            &mut cx,
            PUSH,
            WsInput::Text(r#"42["slide",{"index":4}]"#.into()),
        );
        assert!(cx
            .take()
            .contains(&Action::State(json!({"slide": {"index": 4}}))));
    }

    fn push_sent(cx: Cx) -> Vec<String> {
        cx.take()
            .into_iter()
            .filter_map(|a| match a {
                Action::WsSend { socket: PUSH, text } => Some(text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_push_websocket_resends_its_messages_every_interval_while_open() {
        let telemetry = json!({
            "websocket": {"path": "/api/v1/socket", "send": r#"{"event":"ping"}"#, "every_ms": 5000},
        });
        let mut e = http_with(telemetry, json!({}), true);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(1);
        e.ws(&mut cx, PUSH, WsInput::Opened);
        let actions = cx.take();
        assert!(actions.contains(&Action::WsSend {
            socket: PUSH,
            text: r#"{"event":"ping"}"#.into()
        }));
        assert!(actions.contains(&Action::SetTimer {
            key: PUSH_RENEW,
            after: 5000
        }));
        let mut cx = Cx::new(5001);
        e.timer(&mut cx, PUSH_RENEW);
        assert_eq!(push_sent(cx), [r#"{"event":"ping"}"#]);
        // Closed: no more until it opens again.
        let mut cx = Cx::new(6000);
        e.ws(
            &mut cx,
            PUSH,
            WsInput::Closed {
                code: None,
                reason: "gone".into(),
            },
        );
        assert!(cx.take().contains(&Action::CancelTimer { key: PUSH_RENEW }));
    }

    #[test]
    fn a_socketio_push_channel_sends_its_auth_and_renews_its_events() {
        let telemetry = json!({
            "websocket": {"path": "/socket.io/",
                          "socketio": {"auth": r#"{"token":{settings.token:json}}"#},
                          "send": r#"["data","{\"isVariable\":true}"]"#, "every_ms": 1000},
        });
        let mut spec = Catalog::source_tree()
            .device("propresenter")
            .unwrap()
            .clone();
        spec.telemetry = Some(telemetry);
        spec.state = Default::default();
        spec.settings.insert(
            "token".into(),
            serde_json::from_value(json!({"type": "string", "label": "x", "description": "x"}))
                .unwrap(),
        );
        let mut e = SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "propresenter-7".into(),
                channels: None,
                settings: json!({"token": "k3y"}).as_object().unwrap().clone(),
                monitor: true,
            },
        )
        .unwrap();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.ws(&mut cx, PUSH, WsInput::Opened);
        cx.take();
        let mut cx = Cx::new(1);
        e.ws(&mut cx, PUSH, WsInput::Text(r#"0{"sid":"a"}"#.into()));
        assert_eq!(push_sent(cx), [r#"40{"token":"k3y"}"#]);
        let mut cx = Cx::new(2);
        e.ws(&mut cx, PUSH, WsInput::Text("40".into()));
        assert_eq!(push_sent(cx), [r#"42["data","{\"isVariable\":true}"]"#]);
        let mut cx = Cx::new(1002);
        e.timer(&mut cx, PUSH_RENEW);
        assert_eq!(push_sent(cx), [r#"42["data","{\"isVariable\":true}"]"#]);
    }

    #[test]
    fn text_in_binary_push_frames_goes_to_the_rules() {
        let telemetry = json!({
            "websocket": {"path": "/"},
            "updates": [{
                "json_match": {"$.results.blank": "^(true|false)$"},
                "json": {"blank": "$.results.blank"},
                "state": {"display.blank": "{blank}"},
            }],
        });
        let state = json!({"display.blank": {"type": "bool", "description": "x"}});
        let mut e = http_with(telemetry, state, true);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.ws(&mut cx, PUSH, WsInput::Opened);
        cx.take();
        let mut cx = Cx::new(1);
        e.ws(
            &mut cx,
            PUSH,
            WsInput::Binary(br#"{"results":{"blank":true}}"#.to_vec()),
        );
        assert!(cx
            .take()
            .contains(&Action::State(json!({"display": {"blank": true}}))));
    }

    #[test]
    fn a_token_in_a_named_header() {
        let mut spec = Catalog::source_tree()
            .device("propresenter")
            .unwrap()
            .clone();
        let t = spec.transport.as_mut().unwrap();
        t["auth"] = json!("header");
        t["auth_header"] = json!("X-MimoLive-Password-SHA256");
        spec.telemetry = Some(json!({"websocket": {"path": "/api/v1/socket"}}));
        let open = |token: &str| {
            let mut e = SpecEngine::new(
                Arc::new(spec.clone()),
                OpenContext {
                    host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    host_name: None,
                    port: None,
                    model: "propresenter-7".into(),
                    channels: None,
                    settings: json!({"token": token}).as_object().unwrap().clone(),
                    monitor: true,
                },
            )
            .unwrap();
            let mut cx = Cx::new(0);
            e.start(&mut cx);
            let actions = cx.take();
            let http = actions
                .iter()
                .find_map(|a| match a {
                    Action::Http { request, .. } => Some(request.headers.clone()),
                    _ => None,
                })
                .expect("the probe");
            let ws = actions
                .iter()
                .find_map(|a| match a {
                    Action::WsOpen { request, .. } => Some(request.headers.clone()),
                    _ => None,
                })
                .expect("the push websocket");
            (http, ws)
        };
        let header = ("X-MimoLive-Password-SHA256".to_string(), "ab12".to_string());
        let (http, ws) = open("ab12");
        assert!(http.contains(&header));
        assert!(ws.contains(&header));
        let (http, ws) = open("");
        assert!(http.iter().all(|(name, _)| name != &header.0));
        assert!(ws.is_empty());

        // A refusal of the token is terminal, as for any credential.
        let mut e = SpecEngine::new(
            Arc::new(spec.clone()),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "propresenter-7".into(),
                channels: None,
                settings: json!({"token": "bad"}).as_object().unwrap().clone(),
                monitor: false,
            },
        )
        .unwrap();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let id = cx
            .take()
            .into_iter()
            .find_map(|a| match a {
                Action::Http { id, .. } => Some(id),
                _ => None,
            })
            .unwrap();
        let mut cx = Cx::new(1);
        e.http_response(
            &mut cx,
            id,
            Ok(HttpResponse {
                headers: Vec::new(),
                status: 401,
                body: Vec::new(),
            }),
        );
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
    }

    #[test]
    fn an_error_body_matching_refusal_json_is_a_refusal() {
        let mut spec = Catalog::source_tree()
            .device("propresenter")
            .unwrap()
            .clone();
        let t = spec.transport.as_mut().unwrap();
        t["auth"] = json!("bearer");
        t["refusal_status"] = json!([401]);
        t["refusal_json"] = json!({"$.error.code": "^190$"});
        spec.telemetry = None;
        let answer = |status: u16, body: &str| {
            let mut e = SpecEngine::new(
                Arc::new(spec.clone()),
                OpenContext {
                    host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    host_name: None,
                    port: None,
                    model: "propresenter-7".into(),
                    channels: None,
                    settings: json!({"token": "t"}).as_object().unwrap().clone(),
                    monitor: false,
                },
            )
            .unwrap();
            let mut cx = Cx::new(0);
            e.start(&mut cx);
            let id = cx
                .take()
                .into_iter()
                .find_map(|a| match a {
                    Action::Http { id, .. } => Some(id),
                    _ => None,
                })
                .unwrap();
            let mut cx = Cx::new(1);
            e.http_response(
                &mut cx,
                id,
                Ok(HttpResponse {
                    headers: Vec::new(),
                    status,
                    body: body.as_bytes().to_vec(),
                }),
            );
            cx.take()
                .iter()
                .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. })))
        };
        let expired = r#"{"error":{"type":"OAuthException","code":190,"error_subcode":463}}"#;
        assert!(answer(400, expired));
        // Another error code, a success, or a body that is no JSON: not a
        // refusal.
        assert!(!answer(
            400,
            r#"{"error":{"type":"OAuthException","code":100}}"#
        ));
        assert!(!answer(200, expired));
        assert!(!answer(400, "Bad Request"));
        // The status list still applies.
        assert!(answer(401, ""));
    }

    #[test]
    fn transport_headers_are_rendered_from_settings_on_every_request() {
        let mut spec = Catalog::source_tree()
            .device("youtube-live")
            .unwrap()
            .clone();
        spec.transport.as_mut().unwrap()["headers"] = json!({"Client-Id": "{settings.client_id}"});
        let open = |client_id: &str| {
            let mut e = SpecEngine::new(
                Arc::new(spec.clone()),
                OpenContext {
                    host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    host_name: Some("www.googleapis.com".into()),
                    port: None,
                    model: "data-api-v3".into(),
                    channels: None,
                    settings: json!({"client_id": client_id, "access_token": "t0k",
                                     "broadcast_id": "b1"})
                    .as_object()
                    .unwrap()
                    .clone(),
                    monitor: false,
                },
            )
            .unwrap();
            let mut cx = Cx::new(0);
            e.start(&mut cx);
            let (id, probe) = cx
                .take()
                .into_iter()
                .find_map(|a| match a {
                    Action::Http { id, request } => Some((id, request.headers)),
                    _ => None,
                })
                .expect("the probe");
            (e, id, probe)
        };
        let header = |headers: &[(String, String)]| {
            headers
                .iter()
                .find(|(k, _)| k == "Client-Id")
                .map(|(_, v)| v.clone())
        };
        let (_, _, probe) = open("abc");
        assert_eq!(header(&probe).as_deref(), Some("abc"));
        assert!(probe.contains(&("Authorization".into(), "Bearer t0k".into())));
        // Empty: left out.
        let (mut e, probe_id, probe) = open("");
        assert_eq!(header(&probe), None);
        // New settings: later requests carry the new value.
        let mut cx = Cx::new(1);
        let settings = json!({"client_id": "xyz", "access_token": "t0k", "broadcast_id": "b1"});
        assert!(matches!(
            e.update_settings(&mut cx, settings.as_object().unwrap()),
            SettingsUpdate::Applied
        ));
        let mut cx = Cx::new(2);
        let mut params = Params::new();
        params.insert("broadcast_id".into(), json!("b1"));
        e.command(&mut cx, 1, "get_broadcast", &params);
        e.http_response(
            &mut cx,
            probe_id,
            Ok(HttpResponse {
                headers: Vec::new(),
                status: 404,
                body: Vec::new(),
            }),
        );
        let sent: Vec<_> = cx
            .take()
            .into_iter()
            .filter_map(|a| match a {
                Action::Http { request, .. } => Some(request.headers),
                _ => None,
            })
            .collect();
        assert!(!sent.is_empty());
        assert!(sent.iter().all(|h| header(h).as_deref() == Some("xyz")));

        // The core's own headers can't be named, nor a header on another
        // transport.
        let mut bad = spec.clone();
        bad.transport.as_mut().unwrap()["headers"] = json!({"Authorization": "x"});
        let ctx = OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "data-api-v3".into(),
            channels: None,
            settings: Params::new(),
            monitor: false,
        };
        assert!(SpecEngine::new(Arc::new(bad), ctx.clone()).is_err());
        let mut udp = Catalog::source_tree().device("rosstalk").unwrap().clone();
        udp.transport = Some(
            json!({"type": "line-udp", "port": 6553, "terminator": "none",
                                    "headers": {"X-A": "b"}}),
        );
        assert!(SpecEngine::new(Arc::new(udp), ctx).is_err());
    }

    #[test]
    fn a_line_reply_can_be_matched_by_the_text_it_holds() {
        let mut spec = Catalog::source_tree()
            .device("kramer-p3000")
            .unwrap()
            .clone();
        let command = spec.commands.get_mut("get_route_video").unwrap();
        command.expect = Some(json!({
            "reply_contains": "ROUTE 1,{output},",
            "matches": r"ROUTE 1,\d+,(\d+)",
        }));
        let mut e = SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "p3000-generic".into(),
                channels: None,
                settings: Params::new(),
                monitor: false,
            },
        )
        .unwrap();
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        e.tcp(&mut cx, SOCKET, TcpInput::Connected);
        e.tcp(&mut cx, SOCKET, TcpInput::Data(b"~01@ OK\r\n".to_vec()));
        cx.take();

        let mut params = Params::new();
        params.insert("output".into(), json!(2));
        let mut cx = Cx::new(10);
        e.command(&mut cx, 1, "get_route_video", &params);
        assert_eq!(tcp_sent(&cx.take()), vec!["#ROUTE? 1,2\r".to_string()]);

        // A change pushed for another output is not the reply.
        let mut cx = Cx::new(11);
        e.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"~01@ROUTE 1,5,3\r\n".to_vec()),
        );
        assert!(completed(&cx.take()).is_empty());
        let mut cx = Cx::new(12);
        e.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"~01@ROUTE 1,2,7\r\n".to_vec()),
        );
        assert_eq!(
            completed(&cx.take()),
            vec![(1, Ok(Outcome::Value { value: json!("7") }))]
        );

        // Unanswered, it times out without resetting the stream: a late
        // reply names what it answers.
        let mut cx = Cx::new(20);
        e.command(&mut cx, 2, "get_route_video", &params);
        e.timer(&mut cx, REPLY);
        let a = cx.take();
        assert_eq!(completed(&a), vec![(2, Err(CommandError::Timeout))]);
        assert!(!a.iter().any(|a| matches!(a, Action::TcpClose { .. })));
    }

    #[test]
    fn a_slow_device_does_not_pile_up_polls() {
        let telemetry = json!({
            "poll": {"every_ms": 1000, "send": [{"method": "GET", "path": "/status"}]},
            "updates": [],
        });
        let mut e = http_with(telemetry, json!({}), true);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        cx.take();
        // The first poll is in flight and unanswered; three more rounds come
        // due, and only one request is left waiting behind it.
        for t in 1..=3 {
            let mut cx = Cx::new(t * 1000);
            e.timer(&mut cx, POLL);
        }
        let waiting = e
            .queue
            .iter()
            .filter(|j| j.item.as_ref().and_then(|i| i.get("path")) == Some(&json!("/status")))
            .count();
        assert!(waiting <= 1, "{waiting} polls queued");
    }

    /// Magewell's spec, its settings' defaults applied.
    fn magewell(model: &str, settings: Value) -> SpecEngine {
        let spec = Catalog::source_tree()
            .device("magewell-proconvert")
            .unwrap()
            .clone();
        let settings =
            crate::catalog::validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: model.into(),
                channels: None,
                settings,
                monitor: false,
            },
        )
        .unwrap()
    }

    fn requests(actions: &[Action]) -> Vec<(RequestId, HttpRequest)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Http { id, request } => Some((*id, request.clone())),
                _ => None,
            })
            .collect()
    }

    fn answer(body: &str, cookie: Option<&str>) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status: 200,
            headers: cookie
                .map(|c| vec![("set-cookie".to_string(), format!("{c}; path=/"))])
                .unwrap_or_default(),
            body: body.as_bytes().to_vec(),
        })
    }

    fn cookie_of(r: &HttpRequest) -> Option<&str> {
        r.headers
            .iter()
            .find(|(k, _)| k == "Cookie")
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn a_session_logs_in_first_and_again_when_it_ends() {
        let mut e = magewell("ndi-to-hdmi", json!({"scheme": "https"}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        // The probe is ping, which needs no session; over HTTPS on 443.
        let (probe, request) = requests(&cx.take()).remove(0);
        assert_eq!(request.url, "https://127.0.0.1:443/mwapi?method=ping");
        assert_eq!(cookie_of(&request), None);
        let mut cx = Cx::new(1);
        e.http_response(&mut cx, probe, answer(r#"{"status":0}"#, None));
        cx.take();

        let name = json!({"name": "STUDIO (Camera 1)"})
            .as_object()
            .unwrap()
            .clone();
        let mut cx = Cx::new(2);
        e.command(&mut cx, 1, "select_ndi_source", &name);
        let (login, request) = requests(&cx.take()).remove(0);
        assert_eq!(
            request.url,
            "https://127.0.0.1:443/mwapi?method=login&id=Admin&pass=e3afed0047b08059d0fada10f400c1e5"
        );
        let mut cx = Cx::new(3);
        e.http_response(&mut cx, login, answer(r#"{"status":0}"#, Some("sid=a")));
        let a = cx.take();
        // The login's answer is never state.
        assert!(!a.iter().any(|a| matches!(a, Action::State(_))));
        let (first, request) = requests(&a).remove(0);
        assert!(request
            .url
            .ends_with("/mwapi?method=set-channel&ndi-name=true&name=STUDIO%20%28Camera%201%29"));
        assert_eq!(cookie_of(&request), Some("sid=a"));

        // "Not logged in": a fresh login, and the request once more.
        let mut cx = Cx::new(4);
        e.http_response(&mut cx, first, answer(r#"{"status":37}"#, None));
        let (relogin, request) = requests(&cx.take()).remove(0);
        assert!(request.url.contains("method=login"));
        let mut cx = Cx::new(5);
        e.http_response(&mut cx, relogin, answer(r#"{"status":0}"#, Some("sid=b")));
        let (again, request) = requests(&cx.take()).remove(0);
        assert!(request.url.contains("method=set-channel"));
        assert_eq!(cookie_of(&request), Some("sid=b"));
        let mut cx = Cx::new(6);
        e.http_response(&mut cx, again, answer(r#"{"status":0}"#, None));
        assert_eq!(completed(&cx.take()), vec![(1, Ok(Outcome::Ack))]);

        // A second "not logged in" for the same request stands.
        let mut cx = Cx::new(7);
        e.command(&mut cx, 2, "reboot", &Params::new());
        let (reboot, _) = requests(&cx.take()).remove(0);
        let mut cx = Cx::new(8);
        e.http_response(&mut cx, reboot, answer(r#"{"status":37}"#, None));
        let (relogin, _) = requests(&cx.take()).remove(0);
        let mut cx = Cx::new(9);
        e.http_response(&mut cx, relogin, answer(r#"{"status":0}"#, Some("sid=c")));
        let (again, _) = requests(&cx.take()).remove(0);
        let mut cx = Cx::new(10);
        e.http_response(&mut cx, again, answer(r#"{"status":37}"#, None));
        let done = completed(&cx.take());
        assert!(
            matches!(&done[..], [(2, Err(CommandError::DeviceError { code: Some(c), message }))]
                if c == "37" && message == "not logged in"),
            "{done:?}"
        );
    }

    #[test]
    fn a_refused_session_login_is_terminal() {
        let mut e = magewell("ip-to-hdmi", json!({"password": "wrong"}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let (probe, request) = requests(&cx.take()).remove(0);
        assert_eq!(request.url, "http://127.0.0.1:80/api/ping");
        let mut cx = Cx::new(1);
        e.http_response(&mut cx, probe, answer(r#"{"status":0}"#, None));
        let mut cx = Cx::new(2);
        e.command(&mut cx, 1, "restart_source", &Params::new());
        let (login, request) = requests(&cx.take()).remove(0);
        assert!(request.url.ends_with("/api/user/login"));
        // SHA-256("wrong").
        assert_eq!(
            request.body.as_deref(),
            Some(&br#"{"username":"Admin","password":"8810ad581e59f2bc3928b261707a71308f7e139eb04820366dc4d5c18d980225"}"#[..])
        );
        let mut cx = Cx::new(3);
        e.http_response(&mut cx, login, answer(r#"{"status":16}"#, None));
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(matches!(
            &completed(&a)[..],
            [(1, Err(CommandError::Auth { .. }))]
        ));
        let mut cx = Cx::new(4);
        e.command(&mut cx, 2, "reboot", &Params::new());
        let a = cx.take();
        assert!(requests(&a).is_empty());
        assert!(matches!(
            &completed(&a)[..],
            [(2, Err(CommandError::Auth { .. }))]
        ));
    }

    #[test]
    fn a_login_answer_without_a_session_fails_what_waits_for_it() {
        let mut e = magewell("sdi-plus", json!({}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let (probe, _) = requests(&cx.take()).remove(0);
        e.http_response(&mut Cx::new(1), probe, answer(r#"{"status":0}"#, None));
        let mut cx = Cx::new(2);
        e.command(&mut cx, 1, "get_caps", &Params::new());
        e.command(&mut cx, 2, "ping", &Params::new());
        let (login, _) = requests(&cx.take()).remove(0);
        let mut cx = Cx::new(3);
        e.http_response(&mut cx, login, answer(r#"{"status":0}"#, None));
        let a = cx.take();
        assert!(matches!(
            &completed(&a)[..],
            [(1, Err(CommandError::DeviceError { .. }))]
        ));
        // ping needs no session: it goes on, without a cookie.
        let sent = requests(&a);
        assert_eq!(sent.len(), 1);
        assert!(sent[0].1.url.ends_with("method=ping"));
        assert_eq!(cookie_of(&sent[0].1), None);
    }

    fn push_opened(actions: &[Action]) -> Option<WsRequest> {
        actions.iter().find_map(|a| match a {
            Action::WsOpen {
                socket: PUSH,
                request,
            } => Some(request.clone()),
            _ => None,
        })
    }

    #[test]
    fn a_push_websocket_on_another_host_takes_the_current_token_in_its_url() {
        let spec = Catalog::source_tree().device("restream").unwrap().clone();
        let mut e = open_spec(
            spec,
            "api-v2",
            json!({"access_token": "t/1+a", "refresh_token": "r", "client_id": "c",
                   "expires_at": 4_000_000_000u64}),
        );
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let request = push_opened(&cx.take()).expect("the push channel opens");
        // The token percent-encoded in the query; neither the transport's
        // credential nor its headers go to the other host.
        assert_eq!(
            request.url,
            "wss://streaming.api.restream.io/ws?accessToken=t%2F1%2Ba"
        );
        assert!(request.headers.is_empty(), "{:?}", request.headers);

        // A refreshed token: the next opening uses it.
        e.settings.insert("access_token".into(), json!("fresh"));
        let mut cx = Cx::new(1);
        e.ws(
            &mut cx,
            PUSH,
            WsInput::Closed {
                code: None,
                reason: "closed".into(),
            },
        );
        cx.take();
        let mut cx = Cx::new(2_000);
        e.timer(&mut cx, PUSH_RECONNECT);
        assert_eq!(
            push_opened(&cx.take()).unwrap().url,
            "wss://streaming.api.restream.io/ws?accessToken=fresh"
        );
    }

    #[test]
    fn a_push_url_must_be_secure_and_a_push_port_may_be_a_setting() {
        let mut spec = with_push_websocket();
        let w = &mut spec.telemetry.as_mut().unwrap()["websocket"];
        *w = json!({"url": "ws://example.com/ws?k={settings.k}"});
        spec.settings.insert(
            "k".into(),
            serde_json::from_value(json!({"type": "string", "default": ""})).unwrap(),
        );
        let mut e = open_spec(spec, "arena", json!({"k": "x"}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        let a = cx.take();
        assert!(push_opened(&a).is_none(), "plain ws to another host");
        assert!(a.iter().any(|a| matches!(a, Action::Log { message, .. }
            if message.contains("wss") && !message.contains("k=x"))));

        // Loopback may be plain ws.
        let mut spec = with_push_websocket();
        spec.telemetry.as_mut().unwrap()["websocket"] = json!({"url": "ws://127.0.0.1:9000/ws"});
        let mut e = open_spec(spec, "arena", json!({}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        assert_eq!(
            push_opened(&cx.take()).unwrap().url,
            "ws://127.0.0.1:9000/ws"
        );

        // OpenLP's websocket port, a setting.
        let open = |settings: Value| {
            let spec = Catalog::source_tree().device("openlp").unwrap().clone();
            let settings =
                crate::catalog::validate(&spec.settings, settings.as_object().unwrap()).unwrap();
            let mut e = SpecEngine::new(
                Arc::new(spec),
                OpenContext {
                    host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    host_name: None,
                    port: None,
                    model: "openlp-3".into(),
                    channels: None,
                    settings,
                    monitor: true,
                },
            )
            .unwrap();
            let mut cx = Cx::new(0);
            e.start(&mut cx);
            push_opened(&cx.take()).unwrap().url
        };
        assert_eq!(open(json!({})), "ws://127.0.0.1:4317/");
        assert_eq!(
            open(json!({"websocket_port": 5000})),
            "ws://127.0.0.1:5000/"
        );
    }

    #[test]
    fn a_quiet_push_websocket_is_reopened() {
        let mut spec = with_push_websocket();
        spec.telemetry.as_mut().unwrap()["websocket"]["idle_ms"] = json!(30_000);
        let mut e = open_spec(spec, "arena", json!({}));
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(1);
        e.ws(&mut cx, PUSH, WsInput::Opened);
        assert!(cx.take().iter().any(|a| matches!(a,
            Action::SetTimer { key: PUSH_IDLE, after } if *after == 30_000)));
        let mut cx = Cx::new(30_001);
        e.timer(&mut cx, PUSH_IDLE);
        let a = cx.take();
        assert!(a.contains(&Action::WsClose { socket: PUSH }));
        assert!(a.iter().any(|a| matches!(
            a,
            Action::SetTimer {
                key: PUSH_RECONNECT,
                ..
            }
        )));
    }

    fn openlp(monitor: bool) -> SpecEngine {
        let spec = Catalog::source_tree().device("openlp").unwrap().clone();
        let settings = crate::catalog::validate(&spec.settings, &Params::new()).unwrap();
        SpecEngine::new(
            Arc::new(spec),
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                host_name: None,
                port: None,
                model: "openlp-3".into(),
                channels: None,
                settings,
                monitor,
            },
        )
        .unwrap()
    }

    #[test]
    fn a_push_queues_its_re_reads_once() {
        let mut e = openlp(true);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        // The first poll is in flight; the rest of the start waits.
        let (first, _) = requests(&cx.take()).remove(0);
        e.queue.clear();
        let push = r#"{"results":{"counter":5,"service":2,"slide":0,"item":"a1"}}"#;
        let mut cx = Cx::new(1);
        e.ws(&mut cx, PUSH, WsInput::Binary(push.as_bytes().to_vec()));
        e.ws(&mut cx, PUSH, WsInput::Binary(push.as_bytes().to_vec()));
        let waiting: Vec<&str> = e
            .queue
            .iter()
            .filter_map(|j| j.item.as_ref()?.get("path")?.as_str())
            .collect();
        // Two pushes, one re-read of each waiting.
        assert_eq!(
            waiting,
            ["/api/v2/controller/live-item", "/api/v2/service/items"]
        );
        // They go in turn once the request in flight is answered.
        let mut cx = Cx::new(2);
        e.http_response(&mut cx, first, answer("{}", None));
        let sent = requests(&cx.take());
        assert!(sent[0].1.url.ends_with("/api/v2/controller/live-item"));

        // Opened for commands only, a push queues nothing.
        let mut e = openlp(false);
        let mut cx = Cx::new(0);
        e.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(1);
        e.ws(&mut cx, PUSH, WsInput::Binary(push.as_bytes().to_vec()));
        assert!(
            e.queue.iter().all(|j| j.item.is_none()),
            "{:?}",
            e.queue.len()
        );
        assert!(requests(&cx.take()).is_empty());
    }
}
