//! HTTP listener: receive HTTP requests from any sender on a port, such as a
//! device's webhooks or HTTP notifications, and report every request.
//!
//! generic-http reaches out to one device; this is the other direction, as
//! osc-listener is to generic-osc.
//!
//! - HTTP/1.1 and 1.0 over TCP (RFC 9112): any number of senders connect at
//!   once (up to the session's limit). Each request is read whole, its body by
//!   `Content-Length` or chunked transfer coding, and answered with the
//!   configured status and body. Requests on one connection are answered in
//!   order; a connection is kept open unless the sender asks to close it.
//! - Every request is reported as an `Event::Message` whose address is the
//!   request's path and whose one argument describes it (method, path, query,
//!   headers, body, and the body parsed as JSON or as a form when it is one),
//!   even when it repeats the last, so two identical webhooks are two events.
//! - State keeps the latest request to each path, the senders heard and the
//!   counts, so a consumer can read a device's last notification.
//! - Credentials, when configured (HTTP Basic and/or a Bearer token), are
//!   required of every request; one without them is answered 401 and not
//!   reported.
//! - "Connected" means listening. A port that cannot be bound is reported
//!   disconnected and tried again after 1 s, doubling to 30 s. Opening for
//!   commands only changes nothing: listening is the whole purpose. There is
//!   no host: the integration is opened without one.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};

use base64::Engine as _;
use serde_json::{json, Map, Value};

use super::generic_tcp_udp::{to_hex, RETRY_MAX, RETRY_MIN};
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, TcpInput,
};

const SOCKET: Key = "http";
const RETRY: Key = "retry";
/// Senders, and paths, kept in state; the least recently heard is dropped
/// first, since every connection's source port is a sender of its own.
const MAX_SENDERS: usize = 256;
const MAX_PATHS: usize = 256;
/// A request line and its headers: what a request says about itself, not
/// its content. Past this the sender is answered 431.
const MAX_HEAD: usize = 64 * 1024;
/// Request bodies kept in state as text: the message event carries the whole
/// body; state keeps the start of it, as generic-http's poll does.
const MAX_STATE_BODY: usize = 65_536;

/// One parsed request.
#[derive(Debug, Clone, PartialEq)]
struct Request {
    method: String,
    target: String,
    version: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Whether the connection stays open after this request (RFC 9112 §9.3).
    fn keep_alive(&self) -> bool {
        let tokens = self
            .header("connection")
            .unwrap_or("")
            .split(',')
            .map(|t| t.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();
        if tokens.iter().any(|t| t == "close") {
            return false;
        }
        self.version == "HTTP/1.1" || tokens.iter().any(|t| t == "keep-alive")
    }

    /// The path, and the query after `?` (without it). An absolute-form
    /// target (`http://host/path`) gives its path.
    fn path_and_query(&self) -> (String, String) {
        let mut t = self.target.as_str();
        if let Some(rest) = t
            .strip_prefix("http://")
            .or_else(|| t.strip_prefix("https://"))
        {
            t = rest.find('/').map_or("/", |i| &rest[i..]);
        }
        let t = t.split('#').next().unwrap_or(t);
        match t.split_once('?') {
            Some((p, q)) => (p.to_string(), q.to_string()),
            None => (t.to_string(), String::new()),
        }
    }
}

/// Why a request cannot be read: answered with this status, then the
/// connection is closed.
#[derive(Debug, Clone, PartialEq)]
struct Refusal {
    status: u16,
    reason: String,
}

fn refuse(status: u16, reason: impl Into<String>) -> Refusal {
    Refusal {
        status,
        reason: reason.into(),
    }
}

/// What a connection's bytes have given so far.
#[derive(Debug, PartialEq)]
enum Parsed {
    /// A whole request, and the bytes it took.
    Request(Request, usize),
    /// More is needed; `continue_now` when the head asked for
    /// `Expect: 100-continue` and the body has not started.
    Incomplete {
        continue_now: bool,
    },
    Refused(Refusal),
}

fn is_tchar(c: char) -> bool {
    c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Read one request from the start of `buf` (RFC 9112 §2-§7). `max_body` 0
/// is no limit.
fn parse(buf: &[u8], max_body: u64) -> Parsed {
    // Blank lines before a request line are ignored (RFC 9112 §2.2).
    let start = buf
        .iter()
        .position(|&b| b != b'\r' && b != b'\n')
        .unwrap_or(buf.len());
    let rest = &buf[start..];
    let Some(head_end) = find(rest, b"\r\n\r\n")
        .map(|i| (i, i + 4))
        .or_else(|| find(rest, b"\n\n").map(|i| (i, i + 2)))
    else {
        if rest.len() > MAX_HEAD {
            return Parsed::Refused(refuse(431, "the request head is too long"));
        }
        return Parsed::Incomplete {
            continue_now: false,
        };
    };
    if head_end.0 > MAX_HEAD {
        return Parsed::Refused(refuse(431, "the request head is too long"));
    }
    let Ok(head) = std::str::from_utf8(&rest[..head_end.0]) else {
        return Parsed::Refused(refuse(400, "the request head is not text"));
    };
    let mut lines = head.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));
    let line = lines.next().unwrap_or("");
    let mut parts = line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Parsed::Refused(refuse(400, "malformed request line"));
    };
    if method.is_empty() || !method.chars().all(is_tchar) || target.is_empty() {
        return Parsed::Refused(refuse(400, "malformed request line"));
    }
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Parsed::Refused(refuse(505, "only HTTP/1.0 and 1.1 are served"));
    }
    let mut headers = Vec::new();
    for l in lines {
        // Line folding is obsolete; a sender using it is refused (§5.2).
        if l.starts_with(' ') || l.starts_with('\t') {
            return Parsed::Refused(refuse(400, "folded header line"));
        }
        let Some((name, value)) = l.split_once(':') else {
            return Parsed::Refused(refuse(400, "malformed header line"));
        };
        if name.is_empty() || !name.chars().all(is_tchar) {
            return Parsed::Refused(refuse(400, "malformed header name"));
        }
        headers.push((name.to_string(), value.trim().to_string()));
    }
    let mut request = Request {
        method: method.to_string(),
        target: target.to_string(),
        version: version.to_string(),
        headers,
        body: Vec::new(),
    };
    let body_at = start + head_end.1;
    let body = &buf[body_at..];
    let continue_now = body.is_empty()
        && request
            .header("expect")
            .is_some_and(|e| e.eq_ignore_ascii_case("100-continue"));
    let chunked = request.header("transfer-encoding").map(|te| {
        te.rsplit(',')
            .next()
            .is_some_and(|last| last.trim().eq_ignore_ascii_case("chunked"))
    });
    match chunked {
        Some(true) => match dechunk(body, max_body) {
            Ok(Some((data, used))) => {
                request.body = data;
                Parsed::Request(request, body_at + used)
            }
            Ok(None) => Parsed::Incomplete { continue_now },
            Err(r) => Parsed::Refused(r),
        },
        // A coding other than chunked last has no length (§6.3).
        Some(false) => Parsed::Refused(refuse(400, "a transfer coding other than chunked")),
        None => {
            let lengths: Vec<&str> = request
                .headers
                .iter()
                .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"))
                .map(|(_, v)| v.as_str())
                .collect();
            let length = match lengths.as_slice() {
                [] => 0,
                [first, others @ ..] if others.iter().all(|o| o == first) => {
                    match first.parse::<u64>() {
                        Ok(n) if first.bytes().all(|b| b.is_ascii_digit()) => n,
                        _ => return Parsed::Refused(refuse(400, "malformed Content-Length")),
                    }
                }
                _ => return Parsed::Refused(refuse(400, "conflicting Content-Length")),
            };
            if max_body > 0 && length > max_body {
                return Parsed::Refused(refuse(413, "the body is larger than max_body_bytes"));
            }
            if (body.len() as u64) < length {
                return Parsed::Incomplete { continue_now };
            }
            let length = length as usize;
            request.body = body[..length].to_vec();
            Parsed::Request(request, body_at + length)
        }
    }
}

/// A chunked body (RFC 9112 §7.1): the data and the bytes it took, trailers
/// included, or `None` until it is whole.
fn dechunk(buf: &[u8], max_body: u64) -> Result<Option<(Vec<u8>, usize)>, Refusal> {
    let mut data = Vec::new();
    let mut at = 0;
    loop {
        let Some(eol) = find(&buf[at..], b"\n") else {
            return Ok(None);
        };
        let line = std::str::from_utf8(&buf[at..at + eol])
            .map_err(|_| refuse(400, "malformed chunk size"))?
            .trim_end_matches('\r');
        let size = line.split(';').next().unwrap_or("").trim();
        let size =
            u64::from_str_radix(size, 16).map_err(|_| refuse(400, "malformed chunk size"))?;
        at += eol + 1;
        if size == 0 {
            // Trailer fields, then a blank line.
            loop {
                let Some(eol) = find(&buf[at..], b"\n") else {
                    return Ok(None);
                };
                let blank = buf[at..at + eol].iter().all(|&b| b == b'\r');
                at += eol + 1;
                if blank {
                    return Ok(Some((data, at)));
                }
            }
        }
        if max_body > 0 && data.len() as u64 + size > max_body {
            return Err(refuse(413, "the body is larger than max_body_bytes"));
        }
        let size = usize::try_from(size).map_err(|_| refuse(413, "chunk too large"))?;
        let end = at
            .checked_add(size)
            .ok_or_else(|| refuse(413, "chunk too large"))?;
        if buf.len() < end {
            return Ok(None);
        }
        data.extend_from_slice(&buf[at..end]);
        at = end;
        // The CRLF after the chunk's data.
        if buf.len() < at + 1 {
            return Ok(None);
        }
        if buf[at] == b'\r' {
            if buf.len() < at + 2 {
                return Ok(None);
            }
            if buf[at + 1] != b'\n' {
                return Err(refuse(400, "malformed chunk"));
            }
            at += 2;
        } else if buf[at] == b'\n' {
            at += 1;
        } else {
            return Err(refuse(400, "malformed chunk"));
        }
    }
}

/// `application/x-www-form-urlencoded` and query strings: name to value,
/// the last of a repeated name kept.
fn form(text: &str) -> Map<String, Value> {
    let mut out = Map::new();
    for pair in text.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        out.insert(decode_component(k), Value::String(decode_component(v)));
    }
    out
}

fn decode_component(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = |b: u8| (b as char).to_digit(16);
                match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    (Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 2;
                    }
                    _ => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn reason(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        505 => "HTTP Version Not Supported",
        _ => "",
    }
}

fn response(
    status: u16,
    content_type: &str,
    body: &[u8],
    head_only: bool,
    close: bool,
    extra: &[(&str, &str)],
) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status} {}\r\n", reason(status));
    // 204 carries no body and no length (RFC 9110 §8.6).
    if status != 204 {
        if !body.is_empty() {
            out += &format!("Content-Type: {content_type}\r\n");
        }
        out += &format!("Content-Length: {}\r\n", body.len());
    }
    for (n, v) in extra {
        out += &format!("{n}: {v}\r\n");
    }
    out += if close {
        "Connection: close\r\n"
    } else {
        "Connection: keep-alive\r\n"
    };
    out += "\r\n";
    let mut out = out.into_bytes();
    if !head_only && status != 204 {
        out.extend_from_slice(body);
    }
    out
}

/// Compare without stopping at the first difference.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

struct Client {
    buf: Vec<u8>,
    /// `100 Continue` already sent for the request being read.
    continued: bool,
    /// The sending side is shut down: nothing more is read from it.
    closing: bool,
}

pub(crate) struct HttpListener {
    port: u16,
    address: Option<IpAddr>,
    prefix: Option<String>,
    basic: Option<Vec<u8>>,
    token: Option<String>,
    status: u16,
    body: Vec<u8>,
    content_type: String,
    max_body: u64,
    listening: bool,
    retry_after: Millis,
    clients: HashMap<SocketAddr, Client>,
    order: VecDeque<String>,
    counts: HashMap<String, u64>,
    paths: VecDeque<String>,
    path_counts: HashMap<String, u64>,
    total: u64,
}

impl HttpListener {
    pub(crate) fn new(ctx: OpenContext) -> Result<HttpListener, String> {
        let port = ctx
            .port
            .ok_or("an HTTP listener has no standard port: give the port to listen on")?;
        let s = &ctx.settings;
        let text = |name: &str| {
            s.get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let address = match text("bind_address").trim() {
            "" => None,
            a => Some(
                a.parse::<IpAddr>()
                    .map_err(|_| format!("bind_address '{a}' is not an IP address"))?,
            ),
        };
        let prefix = Some(text("path_prefix")).filter(|p| !p.is_empty());
        let (user, pass) = (text("username"), text("password"));
        let basic =
            (!user.is_empty() || !pass.is_empty()).then(|| format!("{user}:{pass}").into_bytes());
        let token = Some(text("token")).filter(|t| !t.is_empty());
        let status = s
            .get("response_status")
            .and_then(Value::as_u64)
            .unwrap_or(200);
        let status = u16::try_from(status)
            .ok()
            .filter(|s| (200..=299).contains(s))
            .ok_or_else(|| format!("response_status {status} is not 200 to 299"))?;
        let content_type = match text("response_content_type") {
            t if t.is_empty() => "text/plain; charset=utf-8".to_string(),
            t if t.contains(['\r', '\n']) => {
                return Err("response_content_type holds a line break".into())
            }
            t => t,
        };
        let max_body = s.get("max_body_bytes").and_then(Value::as_u64).unwrap_or(0);
        Ok(HttpListener {
            port,
            address,
            prefix,
            basic,
            token,
            status,
            body: text("response_body").into_bytes(),
            content_type,
            max_body,
            listening: false,
            retry_after: RETRY_MIN,
            clients: HashMap::new(),
            order: VecDeque::new(),
            counts: HashMap::new(),
            paths: VecDeque::new(),
            path_counts: HashMap::new(),
            total: 0,
        })
    }

    fn listen(&mut self, cx: &mut Cx) {
        self.listening = true;
        cx.connection(Connection::Connected);
        cx.tcp_serve(SOCKET, self.address, self.port);
    }

    /// Whether the request carries a configured credential; true when none is
    /// configured.
    fn authorized(&self, request: &Request) -> bool {
        if self.basic.is_none() && self.token.is_none() {
            return true;
        }
        let Some(auth) = request.header("authorization") else {
            return false;
        };
        let (scheme, value) = auth.split_once(' ').unwrap_or((auth, ""));
        let value = value.trim();
        if scheme.eq_ignore_ascii_case("basic") {
            if let Some(expected) = &self.basic {
                return base64::engine::general_purpose::STANDARD
                    .decode(value)
                    .is_ok_and(|given| same(&given, expected));
            }
        } else if scheme.eq_ignore_ascii_case("bearer") {
            if let Some(expected) = &self.token {
                return same(value.as_bytes(), expected.as_bytes());
            }
        }
        false
    }

    fn send(&self, cx: &mut Cx, peer: SocketAddr, data: Vec<u8>, close: bool) {
        cx.tcp_client_send(SOCKET, peer, data);
        if close {
            cx.tcp_client_shutdown(SOCKET, peer);
        }
    }

    fn data(&mut self, cx: &mut Cx, peer: SocketAddr, bytes: &[u8]) {
        let Some(client) = self.clients.get_mut(&peer) else {
            return;
        };
        if client.closing {
            return;
        }
        client.buf.extend_from_slice(bytes);
        loop {
            let Some(client) = self.clients.get_mut(&peer) else {
                return;
            };
            match parse(&client.buf, self.max_body) {
                Parsed::Incomplete { continue_now } => {
                    if continue_now && !client.continued {
                        client.continued = true;
                        cx.tcp_client_send(SOCKET, peer, b"HTTP/1.1 100 Continue\r\n\r\n".to_vec());
                    }
                    return;
                }
                Parsed::Refused(r) => {
                    client.closing = true;
                    client.buf.clear();
                    cx.log(
                        Level::Warning,
                        format!("refused a request from {peer}: {}", r.reason),
                    );
                    let body = format!("{}\n", r.reason);
                    let data = response(
                        r.status,
                        "text/plain; charset=utf-8",
                        body.as_bytes(),
                        false,
                        true,
                        &[],
                    );
                    self.send(cx, peer, data, true);
                    return;
                }
                Parsed::Request(request, used) => {
                    client.buf.drain(..used);
                    client.continued = false;
                    let close = !request.keep_alive();
                    if close {
                        client.closing = true;
                    }
                    self.request(cx, peer, request, close);
                    if close {
                        return;
                    }
                }
            }
        }
    }

    fn request(&mut self, cx: &mut Cx, peer: SocketAddr, request: Request, close: bool) {
        cx.alive();
        let head_only = request.method == "HEAD";
        if !self.authorized(&request) {
            cx.log(
                Level::Info,
                format!("refused a request from {peer}: no valid credentials"),
            );
            let challenge = if self.basic.is_some() {
                "Basic realm=\"listener\", charset=\"UTF-8\""
            } else {
                "Bearer"
            };
            let data = response(
                401,
                "text/plain; charset=utf-8",
                b"unauthorized\n",
                head_only,
                close,
                &[("WWW-Authenticate", challenge)],
            );
            self.send(cx, peer, data, close);
            return;
        }
        let (path, query) = request.path_and_query();
        if let Some(prefix) = &self.prefix {
            if !path.starts_with(prefix.as_str()) {
                let data = response(
                    404,
                    "text/plain; charset=utf-8",
                    b"not found\n",
                    head_only,
                    close,
                    &[],
                );
                self.send(cx, peer, data, close);
                return;
            }
        }
        let data = response(
            self.status,
            &self.content_type,
            &self.body,
            head_only,
            close,
            &[],
        );
        self.send(cx, peer, data, close);
        self.report(cx, peer, &request, path, query);
    }

    fn report(
        &mut self,
        cx: &mut Cx,
        peer: SocketAddr,
        request: &Request,
        path: String,
        query: String,
    ) {
        let mut headers = Map::new();
        for (n, v) in &request.headers {
            let n = n.to_ascii_lowercase();
            // Credentials never leave the listener.
            if n == "authorization" {
                continue;
            }
            match headers.get_mut(&n) {
                Some(Value::String(old)) => {
                    old.push_str(", ");
                    old.push_str(v);
                }
                _ => {
                    headers.insert(n, Value::String(v.clone()));
                }
            }
        }
        let content_type = request.header("content-type").unwrap_or("").to_string();
        let text = std::str::from_utf8(&request.body).ok();
        let mut described = Map::new();
        described.insert("method".into(), json!(request.method));
        described.insert("path".into(), json!(path));
        described.insert("query".into(), json!(query));
        described.insert("query_params".into(), Value::Object(form(&query)));
        described.insert("headers".into(), Value::Object(headers));
        described.insert("content_type".into(), json!(content_type));
        match text {
            Some(t) => {
                described.insert("body".into(), json!(t));
            }
            None => {
                described.insert("body_hex".into(), json!(to_hex(&request.body)));
            }
        }
        let parsed = text.and_then(|t| serde_json::from_str::<Value>(t.trim()).ok());
        if let Some(parsed) = &parsed {
            described.insert("json".into(), parsed.clone());
        }
        let is_form = content_type.split(';').next().is_some_and(|m| {
            m.trim()
                .eq_ignore_ascii_case("application/x-www-form-urlencoded")
        });
        let form_fields = match (is_form, text) {
            (true, Some(t)) => Some(form(t)),
            _ => None,
        };
        if let Some(f) = &form_fields {
            described.insert("form".into(), Value::Object(f.clone()));
        }
        cx.message(
            path.clone(),
            None,
            Value::Array(vec![Value::Object(described)]),
            peer,
        );

        self.total += 1;
        let mut senders = Map::new();
        let key = peer.to_string();
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
        *count += 1;
        senders.insert(key.clone(), json!({"requests": *count, "last_path": path}));

        let mut paths = Map::new();
        if let Some(at) = self.paths.iter().position(|k| *k == path) {
            self.paths.remove(at);
        } else if self.paths.len() >= MAX_PATHS {
            if let Some(oldest) = self.paths.pop_front() {
                self.path_counts.remove(&oldest);
                paths.insert(oldest, Value::Null);
            }
        }
        self.paths.push_back(path.clone());
        let requests = self.path_counts.entry(path.clone()).or_insert(0);
        *requests += 1;
        // What the last request gave replaces the one before whole: the
        // structured values are removed first, since a merge patch would
        // keep members the new request no longer has.
        let cleared = json!({"json": null, "form": null, "query_params": null, "body_hex": null, "body": null});
        let mut clear_paths = paths.clone();
        clear_paths.insert(path.clone(), cleared.clone());
        cx.state(json!({"paths": clear_paths, "last": cleared}));

        let mut entry = Map::new();
        entry.insert("method".into(), json!(request.method));
        entry.insert("query".into(), json!(query));
        entry.insert("query_params".into(), Value::Object(form(&query)));
        entry.insert("content_type".into(), json!(content_type));
        entry.insert("source".into(), json!(key));
        entry.insert("requests".into(), json!(*requests));
        match text {
            Some(t) => {
                let mut end = t.len().min(MAX_STATE_BODY);
                while !t.is_char_boundary(end) {
                    end -= 1;
                }
                entry.insert("body".into(), json!(&t[..end]));
            }
            None => {
                let end = request.body.len().min(MAX_STATE_BODY / 2);
                entry.insert("body_hex".into(), json!(to_hex(&request.body[..end])));
            }
        }
        if let Some(p) = parsed {
            entry.insert("json".into(), p);
        }
        if let Some(f) = form_fields {
            entry.insert("form".into(), Value::Object(f));
        }
        let mut last = entry.clone();
        last.insert("path".into(), json!(path));
        last.remove("requests");
        paths.insert(path, Value::Object(entry));
        cx.state(json!({
            "request_count": self.total,
            "senders": senders,
            "paths": paths,
            "last": last,
        }));
    }

    fn connected(&self, cx: &mut Cx, peer: SocketAddr, connected: bool) {
        cx.state(json!({
            "connections": self.clients.len(),
            "senders": {peer.to_string(): {"connected": connected}},
        }));
    }
}

impl Module for HttpListener {
    fn start(&mut self, cx: &mut Cx) {
        cx.state(json!({"port": self.port, "request_count": 0, "connections": 0}));
        self.listen(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, _params: &Params) {
        cx.complete(
            id,
            Err(CommandError::UnknownCommand {
                command: name.into(),
            }),
        );
    }

    fn tcp_client(&mut self, cx: &mut Cx, _socket: Key, peer: SocketAddr, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.clients.insert(
                    peer,
                    Client {
                        buf: Vec::new(),
                        continued: false,
                        closing: false,
                    },
                );
                self.connected(cx, peer, true);
            }
            TcpInput::Data(bytes) => self.data(cx, peer, &bytes),
            TcpInput::Closed { .. } => {
                if self.clients.remove(&peer).is_some() {
                    self.connected(cx, peer, false);
                }
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
        cx.tcp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::Ipv4Addr;

    fn listener(settings: Value) -> HttpListener {
        HttpListener::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: Some(8088),
            model: "generic".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: false,
        })
        .unwrap()
    }

    fn peer() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)), 50000)
    }

    fn feed(l: &mut HttpListener, chunks: &[&[u8]]) -> Vec<Action> {
        let mut cx = Cx::new(0);
        l.tcp_client(&mut cx, SOCKET, peer(), TcpInput::Connected);
        for c in chunks {
            l.tcp_client(&mut cx, SOCKET, peer(), TcpInput::Data(c.to_vec()));
        }
        cx.take()
    }

    fn sent(actions: &[Action]) -> String {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpClientSend { data, .. } => {
                    Some(String::from_utf8_lossy(data).into_owned())
                }
                _ => None,
            })
            .collect()
    }

    fn messages(actions: &[Action]) -> Vec<(String, Value)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Message { address, args, .. } => Some((address.clone(), args[0].clone())),
                _ => None,
            })
            .collect()
    }

    fn state(actions: &[Action]) -> Value {
        let mut s = json!({});
        for a in actions {
            if let Action::State(p) = a {
                crate::session::merge_patch(&mut s, p);
            }
        }
        s
    }

    #[test]
    fn a_json_webhook_is_reported_and_answered() {
        let mut l = listener(json!({}));
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::TcpServe {
            socket: SOCKET,
            address: None,
            port: 8088
        }));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let body = br#"{"event":"tally","on":true}"#;
        let head = format!(
            "POST /hooks/tally?cam=2 HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        // Split anywhere: the request is read once whole.
        let a = feed(&mut l, &[head.as_bytes(), &body[..5], &body[5..]]);
        let out = sent(&a);
        assert!(out.starts_with("HTTP/1.1 200 OK\r\n"), "{out}");
        assert!(out.contains("Content-Length: 0\r\n"));
        assert!(out.contains("Connection: keep-alive\r\n"));
        let got = messages(&a);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "/hooks/tally");
        assert_eq!(got[0].1["method"], "POST");
        assert_eq!(got[0].1["query_params"], json!({"cam": "2"}));
        assert_eq!(got[0].1["json"], json!({"event": "tally", "on": true}));
        assert_eq!(got[0].1["headers"]["content-type"], "application/json");
        let s = state(&a);
        assert_eq!(s["request_count"], 1);
        assert_eq!(s["paths"]["/hooks/tally"]["json"]["on"], true);
        assert_eq!(s["paths"]["/hooks/tally"]["requests"], 1);
        assert_eq!(s["last"]["path"], "/hooks/tally");
        assert_eq!(s["senders"]["10.0.0.9:50000"]["requests"], 1);
    }

    #[test]
    fn a_later_request_replaces_the_last_whole() {
        let mut l = listener(json!({}));
        let first = b"POST /h HTTP/1.1\r\nContent-Length: 13\r\n\r\n{\"a\":1,\"b\":2}";
        let second = b"POST /h HTTP/1.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 9\r\n\r\nc=3&d=a+b";
        let a = feed(&mut l, &[first, second]);
        let s = state(&a);
        assert_eq!(s["paths"]["/h"]["json"], Value::Null);
        assert_eq!(s["paths"]["/h"]["form"], json!({"c": "3", "d": "a b"}));
        assert_eq!(s["paths"]["/h"]["requests"], 2);
        assert_eq!(messages(&a).len(), 2, "pipelined requests are each read");
        assert_eq!(sent(&a).matches("HTTP/1.1 200 OK").count(), 2);
    }

    #[test]
    fn chunked_bodies_and_expect_continue() {
        let mut l = listener(json!({"response_status": 204}));
        let a = feed(
            &mut l,
            &[
                b"PUT /x HTTP/1.1\r\nExpect: 100-continue\r\nTransfer-Encoding: chunked\r\n\r\n",
                b"4\r\nWiki\r\n5;ext=1\r\npedia\r\n0\r\nX-Trailer: 1\r\n\r\n",
            ],
        );
        let out = sent(&a);
        assert!(
            out.starts_with("HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 204 No Content\r\n"),
            "{out}"
        );
        assert!(!out.contains("Content-Length"));
        assert_eq!(messages(&a)[0].1["body"], "Wikipedia");
    }

    #[test]
    fn connection_close_shuts_the_sending_side_down() {
        let mut l = listener(json!({"response_body": "ok", "response_content_type": "text/plain"}));
        let a = feed(
            &mut l,
            &[b"GET /ping HTTP/1.0\r\n\r\nGET /ignored HTTP/1.0\r\n\r\n"],
        );
        let out = sent(&a);
        assert!(out.contains("Connection: close\r\n"));
        assert!(out.ends_with("\r\n\r\nok"));
        assert!(a.contains(&Action::TcpClientShutdown {
            socket: SOCKET,
            peer: peer()
        }));
        assert_eq!(messages(&a).len(), 1, "nothing is read after a close");
    }

    #[test]
    fn credentials_are_required_when_configured() {
        let mut l = listener(json!({"username": "dev", "password": "pw", "token": "t0k"}));
        let a = feed(&mut l, &[b"POST /h HTTP/1.1\r\nContent-Length: 0\r\n\r\n"]);
        assert!(sent(&a).starts_with("HTTP/1.1 401 Unauthorized\r\n"));
        assert!(sent(&a).contains("WWW-Authenticate: Basic"));
        assert!(messages(&a).is_empty());
        // dev:pw
        let a = feed(
            &mut l,
            &[b"POST /h HTTP/1.1\r\nAuthorization: Basic ZGV2OnB3\r\nContent-Length: 0\r\n\r\n"],
        );
        assert!(sent(&a).starts_with("HTTP/1.1 200 OK"));
        let m = messages(&a);
        assert_eq!(m.len(), 1);
        assert!(
            m[0].1["headers"].get("authorization").is_none(),
            "credentials are not reported"
        );
        let a = feed(
            &mut l,
            &[b"POST /h HTTP/1.1\r\nAuthorization: Bearer t0k\r\nContent-Length: 0\r\n\r\n"],
        );
        assert_eq!(messages(&a).len(), 1);
        let a = feed(
            &mut l,
            &[b"POST /h HTTP/1.1\r\nAuthorization: Bearer nope\r\nContent-Length: 0\r\n\r\n"],
        );
        assert!(messages(&a).is_empty());
    }

    #[test]
    fn bad_requests_are_refused_and_closed() {
        let mut l = listener(json!({"max_body_bytes": 4, "path_prefix": "/in/"}));
        let a = feed(
            &mut l,
            &[b"POST /in/a HTTP/1.1\r\nContent-Length: 5\r\n\r\n"],
        );
        assert!(sent(&a).starts_with("HTTP/1.1 413 "));
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::TcpClientShutdown { .. })));
        let a = feed(&mut l, &[b"garbage\r\n\r\n"]);
        assert!(sent(&a).starts_with("HTTP/1.1 400 "));
        let a = feed(&mut l, &[b"GET /elsewhere HTTP/1.1\r\n\r\n"]);
        assert!(sent(&a).starts_with("HTTP/1.1 404 "));
        assert!(messages(&a).is_empty());
        let a = feed(&mut l, &[b"GET /in/x HTTP/2.0\r\n\r\n"]);
        assert!(sent(&a).starts_with("HTTP/1.1 505 "));
    }

    #[test]
    fn a_port_that_cannot_be_bound_is_retried() {
        let mut l = listener(json!({}));
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(1);
        l.socket_error(&mut cx, SOCKET, "in use");
        let a = cx.take();
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
    }

    #[test]
    fn settings_are_checked() {
        let ctx = |settings: Value| OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: Some(8088),
            model: "generic".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: true,
        };
        assert!(HttpListener::new(ctx(json!({"bind_address": "nowhere"}))).is_err());
        assert!(HttpListener::new(ctx(json!({"response_content_type": "a\r\nb"}))).is_err());
        let mut no_port = ctx(json!({}));
        no_port.port = None;
        assert!(HttpListener::new(no_port).is_err());
    }
}
