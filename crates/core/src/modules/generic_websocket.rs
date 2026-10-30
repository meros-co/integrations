//! Generic WebSocket client: send text, bytes or JSON to a device or service
//! the catalogue does not cover yet, and see what comes back.
//!
//! Semantics from RFC 6455 (The WebSocket Protocol, 2011):
//!
//! - The URL is `ws://` or `wss://`, the device's host and port (80 or 443
//!   by default, §3), and the `path` setting as the request target, sent as
//!   given, so already percent-encoded.
//! - The opening handshake carries the configured headers (§4.1): `headers`,
//!   then `secret_headers` replacing same-named ones, and `Authorization`
//!   from `auth` (Basic, RFC 7617, or Bearer, RFC 6750) unless a header
//!   already gives one. Header names must be tokens and values free of
//!   control characters (RFC 9110 §5.1, §5.5); others are refused when the
//!   device is opened. `subprotocols` is offered as `Sec-WebSocket-Protocol`
//!   and the device's choice kept in state (§4.1, §4.2.2).
//! - Text messages are UTF-8 and binary messages any bytes (§5.6). JSON is
//!   sent as compact text. Each received message is a `message` event, whole,
//!   and the last one is kept in state, at most 64 KiB of it. A text message
//!   that parses as JSON is also given parsed; with `key_path`, a JSON message
//!   with a string, number or boolean at that path is kept under `messages`
//!   by that value, the least recently received key going first past
//!   `max_keys`, as the generic OSC module keeps addresses.
//! - A ping goes every `ping_interval_ms` with a counter as its payload; its
//!   pong echoes the payload (§5.5.2, §5.5.3), and the time between is the
//!   device's latency. A ping still unanswered when the next is due drops the
//!   connection, which notices a peer gone without a close. A `request` waits
//!   for the next message matching it, and its time is reported too.
//! - A connection that fails or is closed is opened again after 1 s, doubling
//!   to 30 s, unless `reconnect` is off or `disconnect` asked for it closed.
//!   With credentials configured, a handshake answered 401 or 403 reports the
//!   device unauthorized and is not retried: retrying credentials on a
//!   schedule can lock a device out (as the generic HTTP module does).
//!
//! A binary parameter given as a host file is read whole before it is sent,
//! since a WebSocket message carries its length up front; messages go in the
//! order their commands arrived, a file's included.
//!
//! Opening for commands only changes nothing: the module never subscribes or
//! polls (what a device needs to start sending is the operator's to send),
//! and its pings are the liveness check.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;

use crate::{
    json,
    value::{Map, Value},
};
use base64::Engine;
use regex::Regex;

use crate::catalog::Params;
use crate::engine::json_path;
use crate::module::{
    CommandError, CommandId, Connection, Cx, FileInput, Key, Level, Millis, Module, OpenContext,
    Outcome, WsInput, WsRequest,
};

const SOCKET: Key = "ws";
const RETRY: Key = "retry";
const PING: Key = "ping";
const WAIT: Key = "wait";
const FILE: Key = "file";
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const DEFAULT_PING: Millis = 10_000;
const DEFAULT_MAX_KEYS: usize = 256;
const DEFAULT_TIMEOUT: Millis = 2_000;
/// The most of a message kept in state; events and replies carry it whole.
const MAX_STATE: usize = 65_536;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Auth {
    None,
    Basic,
    Bearer,
}

/// RFC 9110 §5.6.2 tchar.
fn is_tchar(c: char) -> bool {
    c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c)
}

/// Headers from a JSON object of name to value.
fn parse_headers(v: Option<&Value>) -> Result<Vec<(String, String)>, String> {
    let map = match v {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Object(map)) => map,
        Some(_) => return Err("must be an object of name to value".into()),
    };
    let mut out = Vec::new();
    for (name, value) in map {
        if name.is_empty() || !name.chars().all(is_tchar) {
            return Err(format!(
                "'{name}' is not a valid header name (RFC 9110 §5.1)"
            ));
        }
        let value = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => return Err(format!("header '{name}' must be a string")),
        };
        if value
            .chars()
            .any(|c| (c.is_control() && c != '\t') || c == '\u{7f}')
        {
            return Err(format!(
                "header '{name}' contains a control character (RFC 9110 §5.5)"
            ));
        }
        out.push((name.clone(), value.trim_matches([' ', '\t']).to_string()));
    }
    Ok(out)
}

/// Add or replace a header, matching names case-insensitively (§5.1).
fn set_header(headers: &mut Vec<(String, String)>, name: &str, value: String) {
    headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
    headers.push((name.to_string(), value));
}

fn has_header(headers: &[(String, String)], name: &str) -> bool {
    headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
}

/// The subprotocols setting as `Sec-WebSocket-Protocol` lists them: tokens
/// joined by ", " (RFC 6455 §4.1), or `None` when it names none.
fn subprotocols(text: &str) -> Result<Option<String>, String> {
    let names: Vec<&str> = text
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(bad) = names.iter().find(|n| !n.chars().all(is_tchar)) {
        return Err(format!("'{bad}' is not a token (RFC 6455 §4.1)"));
    }
    Ok((!names.is_empty()).then(|| names.join(", ")))
}

/// A JSON path the core reads: `$`, then `.name` or `[n]` steps.
fn check_json_path(path: &str) -> Result<(), String> {
    let mut rest = path
        .strip_prefix('$')
        .ok_or_else(|| format!("'{path}' must start with $"))?;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('[') {
            let (index, tail) = after
                .split_once(']')
                .ok_or_else(|| format!("'{path}' has an unclosed ["))?;
            index
                .parse::<usize>()
                .map_err(|_| format!("'{path}': [{index}] is not an array index"))?;
            rest = tail;
        } else if let Some(after) = rest.strip_prefix('.') {
            let end = after.find(['.', '[']).unwrap_or(after.len());
            if end == 0 {
                return Err(format!("'{path}' has an empty name"));
            }
            rest = &after[end..];
        } else {
            return Err(format!("'{path}': expected . or [ after a step"));
        }
    }
    Ok(())
}

/// A value at a key path, as the key it gives: strings as they are, numbers
/// and booleans as JSON writes them.
fn key_of(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The start of `text`, at most `max` bytes, cut at a character boundary.
fn excerpt(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// What a command sends.
#[derive(Debug)]
enum Out {
    Text(String),
    Binary(Vec<u8>),
    /// A host file, read whole before it goes.
    File {
        path: String,
        reading: bool,
    },
}

/// What a request waits for.
#[derive(Debug)]
struct Wait {
    pattern: Option<Regex>,
    json_match: Vec<(String, Value)>,
    timeout: Millis,
}

/// A send waiting its turn behind a host file being read.
#[derive(Debug)]
struct Queued {
    id: CommandId,
    out: Out,
    wait: Option<Wait>,
}

#[derive(Debug)]
struct Request {
    id: CommandId,
    sent_at: Millis,
    deadline: Millis,
    wait: Wait,
}

impl Wait {
    fn answers(&self, kind: &str, text: Option<&str>, parsed: Option<&Value>) -> bool {
        if kind == "binary" {
            return self.pattern.is_none() && self.json_match.is_empty();
        }
        let text = text.unwrap_or("");
        self.pattern.as_ref().is_none_or(|p| p.is_match(text))
            && self.json_match.iter().all(|(path, want)| {
                parsed
                    .and_then(|j| json_path(j, path))
                    .is_some_and(|got| got == want)
            })
    }
}

pub(crate) struct GenericWebsocket {
    request: WsRequest,
    source: SocketAddr,
    /// Credentials are configured: a 401 or 403 handshake is terminal.
    credentialed: bool,
    ping_every: Millis,
    reconnect: bool,
    key_path: Option<String>,
    max_keys: usize,
    /// Keys in state, least recently received first, with their counts.
    order: VecDeque<String>,
    counts: HashMap<String, u64>,
    /// A websocket is open or being opened.
    active: bool,
    /// The websocket is open.
    open: bool,
    /// Kept closed: by `disconnect`, or after credentials were refused.
    held: bool,
    /// The device chose a subprotocol in the current handshake.
    chose_subprotocol: bool,
    retry_after: Millis,
    ping_seq: u64,
    /// The ping awaiting its pong: its counter and when it went.
    ping_out: Option<(u64, Millis)>,
    /// A host file read is in flight (possibly for a send since failed).
    file_in_flight: bool,
    queue: VecDeque<Queued>,
    requests: VecDeque<Request>,
    received: u64,
    sent: u64,
}

impl GenericWebsocket {
    pub(crate) fn new(ctx: OpenContext) -> Result<GenericWebsocket, String> {
        let s = &ctx.settings;
        let text = |name: &str| {
            s.get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let scheme = match s.get("scheme").and_then(Value::as_str) {
            None | Some("ws") => "ws",
            Some("wss") => "wss",
            Some(other) => return Err(format!("unknown scheme '{other}'")),
        };
        if ctx.host.is_unspecified() && ctx.host_name.is_none() {
            return Err("a WebSocket device needs its host".into());
        }
        let port = ctx.port.unwrap_or(if scheme == "wss" { 443 } else { 80 });
        let host = match (&ctx.host_name, ctx.host) {
            (Some(name), _) => name.clone(),
            (None, std::net::IpAddr::V6(ip)) => format!("[{ip}]"),
            (None, ip) => ip.to_string(),
        };
        let path = match text("path") {
            p if p.is_empty() => "/".to_string(),
            p => p,
        };
        if !path.starts_with('/') || !path.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("path must begin with / and be percent-encoded: visible ASCII only".into());
        }
        let auth = match s.get("auth").and_then(Value::as_str) {
            None | Some("none") => Auth::None,
            Some("basic") => Auth::Basic,
            Some("bearer") => Auth::Bearer,
            Some(other) => return Err(format!("unknown auth '{other}'")),
        };
        let mut headers = parse_headers(s.get("headers")).map_err(|e| format!("headers: {e}"))?;
        let secret =
            parse_headers(s.get("secret_headers")).map_err(|e| format!("secret_headers: {e}"))?;
        let secret_auth = has_header(&secret, "Authorization");
        for (name, value) in secret {
            set_header(&mut headers, &name, value);
        }
        if !has_header(&headers, "Authorization") {
            match auth {
                Auth::Basic => {
                    let pair = format!("{}:{}", text("username"), text("password"));
                    headers.push((
                        "Authorization".into(),
                        format!("Basic {}", B64.encode(pair)),
                    ));
                }
                Auth::Bearer if !text("token").is_empty() => {
                    headers.push(("Authorization".into(), format!("Bearer {}", text("token"))));
                }
                _ => {}
            }
        }
        if let Some(offer) =
            subprotocols(&text("subprotocols")).map_err(|e| format!("subprotocols: {e}"))?
        {
            set_header(&mut headers, "Sec-WebSocket-Protocol", offer);
        }
        let key_path = Some(text("key_path")).filter(|p| !p.is_empty());
        if let Some(p) = &key_path {
            check_json_path(p).map_err(|e| format!("key_path: {e}"))?;
        }
        Ok(GenericWebsocket {
            request: WsRequest {
                url: format!("{scheme}://{host}:{port}{path}"),
                headers,
                accept_invalid_certs: s
                    .get("accept_invalid_certs")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
            source: SocketAddr::new(ctx.host, port),
            credentialed: auth != Auth::None || secret_auth,
            ping_every: s
                .get("ping_interval_ms")
                .and_then(Value::as_u64)
                .unwrap_or(DEFAULT_PING),
            reconnect: s.get("reconnect").and_then(Value::as_bool).unwrap_or(true),
            key_path,
            max_keys: s
                .get("max_keys")
                .and_then(Value::as_u64)
                .map_or(DEFAULT_MAX_KEYS, |n| n as usize),
            order: VecDeque::new(),
            counts: HashMap::new(),
            active: false,
            open: false,
            held: false,
            chose_subprotocol: false,
            retry_after: RETRY_MIN,
            ping_seq: 0,
            ping_out: None,
            file_in_flight: false,
            queue: VecDeque::new(),
            requests: VecDeque::new(),
            received: 0,
            sent: 0,
        })
    }

    fn connect(&mut self, cx: &mut Cx) {
        self.active = true;
        self.chose_subprotocol = false;
        cx.connection(Connection::Connecting);
        cx.ws_open(SOCKET, self.request.clone());
    }

    fn arm(&self, cx: &mut Cx) {
        match self.requests.iter().map(|r| r.deadline).min() {
            Some(at) => cx.set_timer(WAIT, at.saturating_sub(cx.now()).max(1)),
            None => cx.cancel_timer(WAIT),
        }
    }

    /// Put one message on the wire and settle its command: unverified, or
    /// waiting for its reply.
    fn transmit(&mut self, cx: &mut Cx, id: CommandId, data: Out, wait: Option<Wait>) {
        match data {
            Out::Text(text) => cx.ws_send(SOCKET, text),
            Out::Binary(bytes) => cx.ws_send_binary(SOCKET, bytes),
            Out::File { .. } => unreachable!("a file is read before it is sent"),
        }
        self.sent += 1;
        cx.state(json!({"sent_count": self.sent}));
        match wait {
            Some(wait) => {
                let now = cx.now();
                self.requests.push_back(Request {
                    id,
                    sent_at: now,
                    deadline: now + wait.timeout,
                    wait,
                });
                self.arm(cx);
            }
            // WebSocket has no acknowledgement of a message.
            None => cx.complete(id, Ok(Outcome::Unverified)),
        }
    }

    /// Send what is queued, in order, up to a host file not read yet.
    fn pump(&mut self, cx: &mut Cx) {
        while let Some(front) = self.queue.front_mut() {
            if let Out::File { path, reading } = &mut front.out {
                if !*reading && !self.file_in_flight {
                    cx.file_read(FILE, path.clone());
                    *reading = true;
                    self.file_in_flight = true;
                }
                return;
            }
            let q = self.queue.pop_front().expect("front");
            self.transmit(cx, q.id, q.out, q.wait);
        }
    }

    fn send(&mut self, cx: &mut Cx, id: CommandId, out: Out, wait: Option<Wait>) {
        if !self.open {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        self.queue.push_back(Queued { id, out, wait });
        self.pump(cx);
    }

    fn received(&mut self, cx: &mut Cx, kind: &str, text: Option<String>, bytes: Option<Vec<u8>>) {
        cx.alive();
        let size = text
            .as_ref()
            .map_or_else(|| bytes.as_ref().map_or(0, Vec::len), String::len);
        let parsed = text
            .as_deref()
            .and_then(|t| serde_json::from_str::<Value>(t.trim()).ok());
        let key = match (&self.key_path, &parsed) {
            (Some(path), Some(j)) => json_path(j, path).and_then(key_of),
            _ => None,
        };
        let encoded = bytes.as_ref().map(|b| B64.encode(b));

        // Whole, for the event and a request's result.
        let mut whole = Map::new();
        whole.insert("type".into(), json!(kind));
        if let Some(t) = &text {
            whole.insert("text".into(), json!(t));
        }
        if let Some(b) = &encoded {
            whole.insert("base64".into(), json!(b));
        }
        if let Some(j) = &parsed {
            whole.insert("json".into(), j.clone());
        }
        if let Some(k) = &key {
            whole.insert("key".into(), json!(k));
        }
        whole.insert("size".into(), json!(size));
        let whole = Value::Object(whole);

        // State: at most MAX_STATE of it, replacing the last message whole,
        // since a merge patch only adds to an object.
        let small = size <= MAX_STATE;
        let kept = parsed.as_ref().filter(|_| small);
        let (state_text, state_b64, truncated) = match (&text, &encoded) {
            (Some(t), _) => (Some(excerpt(t, MAX_STATE)), None, t.len() > MAX_STATE),
            (None, Some(b)) => (None, Some(excerpt(b, MAX_STATE)), b.len() > MAX_STATE),
            _ => (None, None, false),
        };
        self.received += 1;
        let mut clear = json!({"last_message": null});
        let mut patch = json!({
            "last_message": {
                "type": kind,
                "text": state_text,
                "base64": state_b64,
                "json": kept,
                "key": key,
                "size": size,
                "truncated": truncated,
            },
            "received_count": self.received,
        });
        if let (Some(k), Some(j)) = (&key, kept) {
            let mut messages = Map::new();
            let mut cleared = Map::new();
            if let Some(at) = self.order.iter().position(|o| o == k) {
                self.order.remove(at);
                cleared.insert(k.clone(), Value::Null);
            } else if self.order.len() >= self.max_keys {
                if let Some(oldest) = self.order.pop_front() {
                    self.counts.remove(&oldest);
                    cleared.insert(oldest, Value::Null);
                }
            }
            self.order.push_back(k.clone());
            let count = self.counts.entry(k.clone()).or_insert(0);
            *count += 1;
            messages.insert(k.clone(), json!({"json": j, "count": *count}));
            clear["messages"] = Value::Object(cleared);
            patch["messages"] = Value::Object(messages);
        }
        // State first, so a caller reading the snapshot after its request
        // completes sees the reply.
        cx.state(clear);
        cx.state(patch);
        cx.message(kind, None, Value::Array(vec![whole.clone()]), self.source);

        let answers = self
            .requests
            .iter()
            .position(|r| r.wait.answers(kind, text.as_deref(), parsed.as_ref()));
        if let Some(at) = answers {
            let request = self.requests.remove(at).expect("found");
            cx.round_trip(cx.now().saturating_sub(request.sent_at));
            cx.complete(request.id, Ok(Outcome::Value { value: whole }));
            self.arm(cx);
        }
    }

    /// Fail every command waiting on this connection.
    fn fail_waiting(&mut self, cx: &mut Cx, reason: &str) {
        let error = CommandError::Transport {
            message: reason.to_string(),
        };
        for q in self.queue.drain(..) {
            cx.complete(q.id, Err(error.clone()));
        }
        for r in self.requests.drain(..) {
            cx.complete(r.id, Err(error.clone()));
        }
        cx.cancel_timer(WAIT);
    }

    /// The connection is gone (already closed or failed).
    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.active = false;
        self.open = false;
        self.ping_out = None;
        cx.cancel_timer(PING);
        self.fail_waiting(cx, &reason);
        let refused = self.credentialed
            && (reason.starts_with("connect: HTTP 401") || reason.starts_with("connect: HTTP 403"));
        if refused {
            self.held = true;
            cx.connection(Connection::Unauthorized {
                reason: format!("the device refused the credentials ({reason})"),
            });
            return;
        }
        cx.connection(Connection::Disconnected {
            reason: reason.clone(),
        });
        if self.reconnect && !self.held {
            cx.set_timer(RETRY, self.retry_after);
            self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
        }
    }

    fn wait_from(params: &Params) -> Result<Wait, String> {
        let pattern = match params.get("match").and_then(Value::as_str) {
            Some(p) => Some(Regex::new(p).map_err(|e| format!("match: {e}"))?),
            None => None,
        };
        let json_match = match params.get("json_match") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Object(map)) => {
                let mut out = Vec::new();
                for (path, value) in map {
                    check_json_path(path).map_err(|e| format!("json_match: {e}"))?;
                    out.push((path.clone(), value.clone()));
                }
                out
            }
            Some(_) => {
                return Err("json_match must be an object of JSON path to value".into());
            }
        };
        Ok(Wait {
            pattern,
            json_match,
            timeout: params
                .get("timeout_ms")
                .and_then(Value::as_u64)
                .unwrap_or(DEFAULT_TIMEOUT),
        })
    }
}

impl Module for GenericWebsocket {
    fn start(&mut self, cx: &mut Cx) {
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        let invalid = |message: String| Err(CommandError::InvalidParams { message });
        match name {
            "send_text" => {
                let text = params.get("text").and_then(Value::as_str).unwrap_or("");
                self.send(cx, id, Out::Text(text.to_string()), None);
            }
            "send_json" => {
                let value = params.get("json").cloned().unwrap_or(Value::Null);
                self.send(cx, id, Out::Text(value.to_string()), None);
            }
            "send_binary" => {
                let data = params.get("data").unwrap_or(&Value::Null);
                let out = match (data.as_str(), crate::catalog::binary_file(data)) {
                    (Some(b64), _) => match crate::catalog::decode_binary(b64) {
                        Some(bytes) => Out::Binary(bytes),
                        None => {
                            cx.complete(id, invalid("'data' is not base64".into()));
                            return;
                        }
                    },
                    (None, Some(path)) => Out::File {
                        path: path.to_string(),
                        reading: false,
                    },
                    (None, None) => {
                        cx.complete(
                            id,
                            invalid("'data' must be base64 or {\"file\": \"<path>\"}".into()),
                        );
                        return;
                    }
                };
                self.send(cx, id, out, None);
            }
            "request" => {
                let text = params.get("text").and_then(Value::as_str);
                let value = params.get("json").filter(|v| !v.is_null());
                let out = match (text, value) {
                    (Some(t), None) => Out::Text(t.to_string()),
                    (None, Some(v)) => Out::Text(v.to_string()),
                    _ => {
                        cx.complete(id, invalid("give either 'text' or 'json'".into()));
                        return;
                    }
                };
                match Self::wait_from(params) {
                    Ok(wait) => self.send(cx, id, out, Some(wait)),
                    Err(message) => cx.complete(id, invalid(message)),
                }
            }
            "connect" => {
                self.held = false;
                if !self.active {
                    cx.cancel_timer(RETRY);
                    self.retry_after = RETRY_MIN;
                    self.connect(cx);
                }
                cx.complete(id, Ok(Outcome::Ack));
            }
            "disconnect" => {
                self.held = true;
                cx.cancel_timer(RETRY);
                if self.active {
                    cx.ws_close(SOCKET);
                    self.lost(cx, "closed by the disconnect command".into());
                    cx.state(json!({"close": {"code": 1000, "reason": "closed by the disconnect command"}}));
                }
                cx.complete(id, Ok(Outcome::Ack));
            }
            "clear_messages" => {
                self.order.clear();
                self.counts.clear();
                cx.state(json!({"messages": null, "last_message": null}));
                cx.complete(id, Ok(Outcome::Ack));
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn ws(&mut self, cx: &mut Cx, _socket: Key, input: WsInput) {
        match input {
            WsInput::Subprotocol(protocol) => {
                self.chose_subprotocol = true;
                cx.state(json!({"subprotocol": protocol}));
            }
            WsInput::Opened => {
                self.open = true;
                self.retry_after = RETRY_MIN;
                if !self.chose_subprotocol {
                    cx.state(json!({"subprotocol": null}));
                }
                cx.connection(Connection::Connected);
                if self.ping_every > 0 {
                    cx.set_timer(PING, self.ping_every);
                }
            }
            WsInput::Text(text) => self.received(cx, "text", Some(text), None),
            WsInput::Binary(bytes) => self.received(cx, "binary", None, Some(bytes)),
            WsInput::Activity => cx.alive(),
            WsInput::Pong(data) => {
                cx.alive();
                if let Some((seq, sent_at)) = self.ping_out {
                    if data == seq.to_be_bytes() {
                        self.ping_out = None;
                        cx.round_trip(cx.now().saturating_sub(sent_at));
                    }
                }
            }
            WsInput::Closed { code, reason } => {
                cx.state(json!({"close": {"code": code, "reason": reason}}));
                self.lost(cx, reason);
            }
        }
    }

    fn file(&mut self, cx: &mut Cx, _file: Key, input: FileInput) {
        let result = match input {
            FileInput::Read { data } => Ok(data),
            FileInput::Failed { message } => Err(message),
            _ => return,
        };
        self.file_in_flight = false;
        // The read belongs to the front of the queue only if it asked for
        // it; otherwise it is for a send failed with its connection.
        let ours = matches!(
            self.queue.front(),
            Some(Queued {
                out: Out::File { reading: true, .. },
                ..
            })
        );
        if ours {
            let q = self.queue.pop_front().expect("front");
            match result {
                Ok(data) if self.open => self.transmit(cx, q.id, Out::Binary(data), q.wait),
                Ok(_) => cx.complete(q.id, Err(CommandError::NotConnected)),
                Err(message) => cx.complete(
                    q.id,
                    Err(CommandError::InvalidParams {
                        message: format!("'data': {message}"),
                    }),
                ),
            }
        }
        self.pump(cx);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => {
                if !self.active && !self.held {
                    self.connect(cx);
                }
            }
            PING if self.open => {
                if self.ping_out.is_some() {
                    let reason = format!("no pong within {} ms", self.ping_every);
                    cx.log(Level::Warning, reason.clone());
                    cx.ws_close(SOCKET);
                    cx.state(json!({"close": {"code": null, "reason": reason}}));
                    self.lost(cx, reason);
                    return;
                }
                self.ping_seq += 1;
                cx.ws_ping(SOCKET, self.ping_seq.to_be_bytes());
                self.ping_out = Some((self.ping_seq, cx.now()));
                cx.set_timer(PING, self.ping_every);
            }
            WAIT => {
                let now = cx.now();
                let (due, keep): (Vec<Request>, Vec<Request>) =
                    self.requests.drain(..).partition(|r| r.deadline <= now);
                self.requests = keep.into();
                for request in due {
                    cx.complete(request.id, Err(CommandError::Timeout));
                }
                self.arm(cx);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.active {
            cx.ws_close(SOCKET);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn context(settings: Value, monitor: bool) -> OpenContext {
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)),
            host_name: None,
            port: Some(9000),
            model: "generic".into(),
            channels: None,
            settings: params(settings),
            monitor,
        }
    }

    fn module(settings: Value) -> GenericWebsocket {
        GenericWebsocket::new(context(settings, true)).unwrap()
    }

    fn completion(actions: &[Action], id: CommandId) -> Option<crate::module::CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn state(actions: &[Action], into: &mut Value) {
        for action in actions {
            if let Action::State(p) = action {
                crate::session::merge_patch(into, p);
            }
        }
    }

    fn opened(m: &mut GenericWebsocket) {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.ws(&mut cx, SOCKET, WsInput::Opened);
    }

    #[test]
    fn the_handshake_carries_the_settings() {
        let m = module(json!({
            "scheme": "wss", "path": "/api/v1?x=1", "subprotocols": "v2.json, v1.json",
            "auth": "basic", "username": "u", "password": "p",
            "headers": {"Origin": "http://localhost", "X-Api-Key": "plain"},
            "secret_headers": {"x-api-key": "secret"},
        }));
        assert_eq!(m.request.url, "wss://192.0.2.10:9000/api/v1?x=1");
        assert_eq!(
            m.request.headers,
            vec![
                ("Origin".to_string(), "http://localhost".to_string()),
                ("x-api-key".to_string(), "secret".to_string()),
                ("Authorization".to_string(), "Basic dTpw".to_string()),
                (
                    "Sec-WebSocket-Protocol".to_string(),
                    "v2.json, v1.json".to_string()
                ),
            ]
        );
        assert!(m.credentialed);
        // An Authorization header of its own replaces the configured auth.
        let m = module(json!({
            "auth": "bearer", "token": "t",
            "secret_headers": {"Authorization": "Token abc"},
        }));
        assert_eq!(
            m.request.headers,
            vec![("Authorization".to_string(), "Token abc".to_string())]
        );
        let m = GenericWebsocket::new(OpenContext {
            host_name: Some("console.local".into()),
            port: None,
            ..context(json!({"scheme": "wss"}), true)
        })
        .unwrap();
        assert_eq!(m.request.url, "wss://console.local:443/");
        let m = module(json!({"auth": "bearer"}));
        assert!(m.request.headers.is_empty());
        assert!(!module(json!({"headers": {"Authorization": "x"}})).credentialed);
    }

    #[test]
    fn settings_are_checked() {
        let open = |s: Value| GenericWebsocket::new(context(s, true));
        assert!(open(json!({"headers": {"bad name": "x"}})).is_err());
        assert!(open(json!({"secret_headers": {"X": "a\u{1}b"}})).is_err());
        assert!(open(json!({"headers": ["x"]})).is_err());
        assert!(open(json!({"subprotocols": "a b"})).is_err());
        assert!(open(json!({"key_path": "type"})).is_err());
        assert!(open(json!({"key_path": "$.a..b"})).is_err());
        assert!(open(json!({"key_path": "$.event[0].name"})).is_ok());
        assert!(open(json!({"path": "/a b"})).is_err());
        let no_host = GenericWebsocket::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            ..context(json!({}), true)
        });
        assert!(no_host.is_err());
    }

    #[test]
    fn sends_text_json_and_binary_in_order() {
        let mut m = module(json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.command(&mut cx, 1, "send_text", &params(json!({"text": "hi"})));
        assert_eq!(
            completion(&cx.take(), 1),
            Some(Err(CommandError::NotConnected))
        );
        opened(&mut m);
        let mut cx = Cx::new(10);
        m.command(&mut cx, 2, "send_text", &params(json!({"text": "a\nb"})));
        m.command(
            &mut cx,
            3,
            "send_json",
            &params(json!({"json": {"x": [1, true]}})),
        );
        m.command(
            &mut cx,
            4,
            "send_binary",
            &params(json!({"data": "AAEC/w=="})),
        );
        m.command(
            &mut cx,
            5,
            "send_binary",
            &params(json!({"data": {"file": "D:/clip.bin"}})),
        );
        // Behind the file, in order.
        m.command(&mut cx, 6, "send_text", &params(json!({"text": "after"})));
        let a = cx.take();
        let sent: Vec<&Action> = a
            .iter()
            .filter(|a| {
                matches!(
                    a,
                    Action::WsSend { .. } | Action::WsSendBinary { .. } | Action::FileRead { .. }
                )
            })
            .collect();
        assert_eq!(
            sent,
            [
                &Action::WsSend {
                    socket: SOCKET,
                    text: "a\nb".into()
                },
                &Action::WsSend {
                    socket: SOCKET,
                    text: r#"{"x":[1,true]}"#.into()
                },
                &Action::WsSendBinary {
                    socket: SOCKET,
                    data: vec![0, 1, 2, 255]
                },
                &Action::FileRead {
                    file: FILE,
                    path: "D:/clip.bin".into()
                },
            ]
        );
        for id in 2..=4 {
            assert_eq!(completion(&a, id), Some(Ok(Outcome::Unverified)));
        }
        assert_eq!(completion(&a, 6), None);
        let mut cx = Cx::new(20);
        m.file(&mut cx, FILE, FileInput::Read { data: vec![9, 9] });
        let a = cx.take();
        let order: Vec<&Action> = a
            .iter()
            .filter(|a| matches!(a, Action::WsSend { .. } | Action::WsSendBinary { .. }))
            .collect();
        assert_eq!(
            order,
            [
                &Action::WsSendBinary {
                    socket: SOCKET,
                    data: vec![9, 9]
                },
                &Action::WsSend {
                    socket: SOCKET,
                    text: "after".into()
                },
            ]
        );
        assert_eq!(completion(&a, 5), Some(Ok(Outcome::Unverified)));
        assert_eq!(completion(&a, 6), Some(Ok(Outcome::Unverified)));
        // A missing file refuses its command.
        let mut cx = Cx::new(30);
        m.command(
            &mut cx,
            7,
            "send_binary",
            &params(json!({"data": {"file": "D:/gone.bin"}})),
        );
        m.file(
            &mut cx,
            FILE,
            FileInput::Failed {
                message: "not found".into(),
            },
        );
        assert!(matches!(
            completion(&cx.take(), 7),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
    }

    #[test]
    fn received_messages_go_to_state_events_and_keys() {
        let mut m = module(json!({"key_path": "$.type", "max_keys": 2}));
        opened(&mut m);
        let mut s = json!({});
        let mut cx = Cx::new(10);
        m.ws(
            &mut cx,
            SOCKET,
            WsInput::Text(r#"{"type":"tally","on":[1,2]}"#.into()),
        );
        m.ws(
            &mut cx,
            SOCKET,
            WsInput::Text(r#"{"type":"tally","on":[3]}"#.into()),
        );
        m.ws(&mut cx, SOCKET, WsInput::Text(r#"{"type":7}"#.into()));
        m.ws(&mut cx, SOCKET, WsInput::Text(r#"{"type":"level"}"#.into()));
        m.ws(&mut cx, SOCKET, WsInput::Binary(vec![1, 2, 3]));
        let a = cx.take();
        state(&a, &mut s);
        // A JSON message replaces the last one's JSON rather than merging.
        assert!(s["messages"].get("tally").is_none(), "{s}");
        assert_eq!(s["messages"]["7"], json!({"json": {"type": 7}, "count": 1}));
        assert_eq!(s["messages"]["level"]["count"], 1);
        assert_eq!(
            s["last_message"],
            json!({"type": "binary", "base64": "AQID", "size": 3, "truncated": false})
        );
        assert_eq!(s["received_count"], 5);
        let events: Vec<(&String, &Value)> = a
            .iter()
            .filter_map(|a| match a {
                Action::Message { address, args, .. } => Some((address, args)),
                _ => None,
            })
            .collect();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].0, "text");
        assert_eq!(
            events[0].1,
            &json!([{"type": "text", "text": r#"{"type":"tally","on":[1,2]}"#,
                     "json": {"type": "tally", "on": [1, 2]}, "key": "tally", "size": 27}])
        );
        // The repeat keeps the key's count and replaces its JSON whole.
        let mut s = json!({});
        let mut cx = Cx::new(20);
        m.ws(&mut cx, SOCKET, WsInput::Text(r#"{"type":"7"}"#.into()));
        state(&cx.take(), &mut s);
        assert_eq!(s["messages"]["7"]["count"], 2);
        // Plain text is kept as text; a long one is cut in state only.
        let long = "x".repeat(MAX_STATE + 10);
        let mut cx = Cx::new(30);
        m.ws(&mut cx, SOCKET, WsInput::Text(long.clone()));
        let a = cx.take();
        let mut s = json!({});
        state(&a, &mut s);
        assert_eq!(s["last_message"]["text"].as_str().unwrap().len(), MAX_STATE);
        assert_eq!(s["last_message"]["truncated"], true);
        assert!(a.iter().any(|x| matches!(x,
            Action::Message { args, .. } if args[0]["text"].as_str() == Some(long.as_str()))));
        // clear_messages forgets them.
        let mut cx = Cx::new(40);
        m.command(&mut cx, 1, "clear_messages", &Params::new());
        let a = cx.take();
        assert!(a.contains(&Action::State(
            json!({"messages": null, "last_message": null})
        )));
        assert_eq!(completion(&a, 1), Some(Ok(Outcome::Ack)));
    }

    #[test]
    fn requests_wait_for_their_reply() {
        let mut m = module(json!({}));
        opened(&mut m);
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            1,
            "request",
            &params(json!({"json": {"id": 7, "get": "name"}, "json_match": {"$.id": 7}, "timeout_ms": 500})),
        );
        m.command(
            &mut cx,
            2,
            "request",
            &params(json!({"text": "PING", "match": "^PONG"})),
        );
        let a = cx.take();
        assert!(a.contains(&Action::WsSend {
            socket: SOCKET,
            text: r#"{"id":7,"get":"name"}"#.into()
        }));
        assert!(a.contains(&Action::SetTimer {
            key: WAIT,
            after: 500
        }));
        let mut cx = Cx::new(130);
        m.ws(&mut cx, SOCKET, WsInput::Text(r#"{"id":6}"#.into()));
        m.ws(&mut cx, SOCKET, WsInput::Text("PONG 1".into()));
        m.ws(
            &mut cx,
            SOCKET,
            WsInput::Text(r#"{"id":7,"name":"A"}"#.into()),
        );
        let a = cx.take();
        assert!(a.contains(&Action::RoundTrip(30)));
        assert_eq!(
            completion(&a, 2),
            Some(Ok(Outcome::Value {
                value: json!({"type": "text", "text": "PONG 1", "size": 6})
            }))
        );
        match completion(&a, 1) {
            Some(Ok(Outcome::Value { value })) => assert_eq!(value["json"]["name"], "A"),
            other => panic!("{other:?}"),
        }
        // Both or neither of text and json is refused; a request times out.
        let mut cx = Cx::new(200);
        m.command(
            &mut cx,
            3,
            "request",
            &params(json!({"text": "a", "json": 1})),
        );
        m.command(
            &mut cx,
            4,
            "request",
            &params(json!({"json_match": {"id": 1}, "text": "a"})),
        );
        m.command(
            &mut cx,
            5,
            "request",
            &params(json!({"text": "X", "timeout_ms": 50})),
        );
        let a = cx.take();
        assert!(matches!(
            completion(&a, 3),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        assert!(matches!(
            completion(&a, 4),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let mut cx = Cx::new(250);
        m.timer(&mut cx, WAIT);
        assert_eq!(completion(&cx.take(), 5), Some(Err(CommandError::Timeout)));
    }

    #[test]
    fn pings_time_the_device_and_find_a_dead_connection() {
        let mut m = module(json!({"ping_interval_ms": 1000}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.ws(&mut cx, SOCKET, WsInput::Subprotocol("v1".into()));
        m.ws(&mut cx, SOCKET, WsInput::Opened);
        let a = cx.take();
        assert!(a.contains(&Action::State(json!({"subprotocol": "v1"}))));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::SetTimer {
            key: PING,
            after: 1000
        }));
        let mut cx = Cx::new(1000);
        m.timer(&mut cx, PING);
        assert!(cx.take().contains(&Action::WsPing {
            socket: SOCKET,
            data: 1u64.to_be_bytes().to_vec()
        }));
        // A pong to someone else's ping is not timed.
        let mut cx = Cx::new(1010);
        m.ws(&mut cx, SOCKET, WsInput::Pong(vec![1]));
        assert!(!cx.take().iter().any(|a| matches!(a, Action::RoundTrip(_))));
        let mut cx = Cx::new(1012);
        m.ws(&mut cx, SOCKET, WsInput::Pong(1u64.to_be_bytes().to_vec()));
        assert!(cx.take().contains(&Action::RoundTrip(12)));
        // The next ping goes unanswered: at the one after, the connection is
        // dropped and opened again.
        let mut cx = Cx::new(2000);
        m.timer(&mut cx, PING);
        cx.take();
        let mut cx = Cx::new(3000);
        m.timer(&mut cx, PING);
        let a = cx.take();
        assert!(a.contains(&Action::WsClose { socket: SOCKET }));
        assert!(a.contains(&Action::Connection(Connection::Disconnected {
            reason: "no pong within 1000 ms".into()
        })));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        let mut cx = Cx::new(4000);
        m.timer(&mut cx, RETRY);
        assert!(cx.take().iter().any(|a| matches!(a, Action::WsOpen { .. })));
    }

    #[test]
    fn closes_reconnect_unless_held_or_refused() {
        let mut m = module(json!({}));
        opened(&mut m);
        let mut cx = Cx::new(10);
        m.command(&mut cx, 1, "request", &params(json!({"text": "q"})));
        m.ws(
            &mut cx,
            SOCKET,
            WsInput::Closed {
                code: Some(1001),
                reason: "going away".into(),
            },
        );
        let a = cx.take();
        let mut s = json!({});
        state(&a, &mut s);
        assert_eq!(s["close"], json!({"code": 1001, "reason": "going away"}));
        assert!(matches!(
            completion(&a, 1),
            Some(Err(CommandError::Transport { .. }))
        ));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        let mut cx = Cx::new(1010);
        m.timer(&mut cx, RETRY);
        m.ws(
            &mut cx,
            SOCKET,
            WsInput::Closed {
                code: None,
                reason: "connect: refused".into(),
            },
        );
        assert!(cx.take().contains(&Action::SetTimer {
            key: RETRY,
            after: 2 * RETRY_MIN
        }));
        // disconnect holds it closed until connect.
        let mut cx = Cx::new(1500);
        m.command(&mut cx, 2, "disconnect", &Params::new());
        let a = cx.take();
        assert!(a.contains(&Action::CancelTimer { key: RETRY }));
        assert!(!a.iter().any(|a| matches!(a, Action::WsOpen { .. })));
        let mut cx = Cx::new(5000);
        m.timer(&mut cx, RETRY);
        assert!(cx.take().is_empty());
        let mut cx = Cx::new(6000);
        m.command(&mut cx, 3, "connect", &Params::new());
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(a, Action::WsOpen { .. })));
        assert_eq!(completion(&a, 3), Some(Ok(Outcome::Ack)));

        // With credentials, a refused handshake is terminal.
        let mut m = module(json!({"auth": "bearer", "token": "t"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.ws(
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
        assert!(!a
            .iter()
            .any(|a| matches!(a, Action::SetTimer { key: RETRY, .. })));
        // Without reconnect, a close leaves it closed.
        let mut m = module(json!({"reconnect": false}));
        opened(&mut m);
        let mut cx = Cx::new(0);
        m.ws(
            &mut cx,
            SOCKET,
            WsInput::Closed {
                code: Some(1000),
                reason: "".into(),
            },
        );
        assert!(!cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::SetTimer { key: RETRY, .. })));
    }

    #[test]
    fn opened_for_commands_only_it_asks_nothing_and_still_pings() {
        let mut m =
            GenericWebsocket::new(context(json!({"ping_interval_ms": 500}), false)).unwrap();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.ws(&mut cx, SOCKET, WsInput::Opened);
        let a = cx.take();
        assert!(!a
            .iter()
            .any(|a| matches!(a, Action::WsSend { .. } | Action::WsSendBinary { .. })));
        assert!(a.contains(&Action::SetTimer {
            key: PING,
            after: 500
        }));
        let mut cx = Cx::new(1);
        m.command(&mut cx, 1, "send_text", &params(json!({"text": "x"})));
        assert_eq!(completion(&cx.take(), 1), Some(Ok(Outcome::Unverified)));
    }
}
