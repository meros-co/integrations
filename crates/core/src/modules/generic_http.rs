//! Generic HTTP: make any request to a device the catalogue does not cover
//! yet, and optionally poll one URL into state.
//!
//! Semantics from RFC 9110 (HTTP Semantics, 2022):
//!
//! - Methods GET, HEAD, POST, PUT, DELETE and OPTIONS (§9.3) and PATCH
//!   (RFC 5789). A request body is sent as given whatever the method; §9.3.1
//!   notes a body on GET has no generally defined semantics, so it is the
//!   operator's choice.
//! - A field name is a token (§5.1, §5.6.2); a field value may not contain
//!   CR, LF or NUL (§5.5). Header names and values breaking these are refused
//!   before anything is sent, and surrounding whitespace is not part of a
//!   value (§5.5).
//! - Status codes (§15): 2xx is success. A 4xx or 5xx fails the command by
//!   default, 401 and 403 (§15.5.2, §15.5.4) as an authentication failure;
//!   with `fail_on_status: false` every status is returned as a value.
//! - Credentials: `basic` sends `Authorization: Basic` (RFC 7617) with every
//!   request and answers a Digest challenge too, as the spec engine does;
//!   `digest` answers a Digest challenge (RFC 7616); `bearer` sends
//!   `Authorization: Bearer <token>` (RFC 6750). A request's own
//!   Authorization header replaces the configured one.
//!
//! The path is sent as given, query string included: it must already be
//! percent-encoded (RFC 3986), so only visible ASCII is accepted. A response
//! body that parses as JSON is also returned parsed, whatever its content
//! type, since the core's HTTP action does not report response headers.
//!
//! Polling: with `poll_path` set, a GET of it is made on connecting and then
//! `poll_interval_ms` after each answer (never two at once), and the result
//! is kept under `poll`. With credentials configured, a poll refused with 401
//! or 403 stops polling and reports the device unauthorized: credentials are
//! never retried on a schedule (SPEC.md §2, http), since repeated failed
//! logins can lock a device out.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Credentials, Cx, HttpRequest, HttpResponse, Key, Millis,
    Module, OpenContext, Outcome, RequestId,
};

const POLL: Key = "poll";
const DEFAULT_TIMEOUT: Millis = 5_000;
const DEFAULT_POLL_EVERY: Millis = 5_000;
/// The most of a polled body kept in state.
const MAX_STATE_BODY: usize = 65_536;
/// The most of a body quoted in an error.
const MAX_ERROR_BODY: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Auth {
    None,
    Basic,
    Digest,
    Bearer,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Purpose {
    Command {
        id: CommandId,
        fail_on_status: bool,
    },
    /// A scheduled poll, or one a command asked for.
    Poll {
        command: Option<CommandId>,
    },
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
        Some(_) => return Err("headers must be an object of name to value".into()),
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

fn method(name: &str) -> Option<&'static str> {
    Some(match name {
        "GET" => "GET",
        "HEAD" => "HEAD",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "OPTIONS" => "OPTIONS",
        _ => return None,
    })
}

/// A path as sent: visible ASCII beginning with `/`.
fn check_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') {
        return Err("path must begin with '/'".into());
    }
    if !path.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("path must be percent-encoded: visible ASCII only, no spaces".into());
    }
    Ok(())
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

/// A response as a command returns it.
fn response_value(response: &HttpResponse) -> Value {
    let mut value = json!({
        "status": response.status,
        "ok": (200..300).contains(&response.status),
        "body": String::from_utf8_lossy(&response.body),
    });
    if let Ok(parsed) = serde_json::from_slice::<Value>(&response.body) {
        value["json"] = parsed;
    }
    value
}

pub(crate) struct GenericHttp {
    base: String,
    auth: Auth,
    username: String,
    password: String,
    token: String,
    accept_invalid_certs: bool,
    headers: Vec<(String, String)>,
    timeout: Millis,
    poll_path: Option<String>,
    poll_every: Millis,
    polling: bool,
    /// A poll refused the credentials. Terminal for polling.
    refused: bool,
    next_id: RequestId,
    requests: HashMap<RequestId, Purpose>,
}

impl GenericHttp {
    pub(crate) fn new(ctx: OpenContext) -> Result<GenericHttp, String> {
        let s = &ctx.settings;
        let text = |name: &str| {
            s.get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let scheme = match s.get("scheme").and_then(Value::as_str) {
            None | Some("http") => "http",
            Some("https") => "https",
            Some(other) => return Err(format!("unknown scheme '{other}'")),
        };
        let port = ctx.port.unwrap_or(if scheme == "https" { 443 } else { 80 });
        let host = match ctx.host {
            std::net::IpAddr::V4(ip) => ip.to_string(),
            std::net::IpAddr::V6(ip) => format!("[{ip}]"),
        };
        let auth = match s.get("auth").and_then(Value::as_str) {
            None | Some("none") => Auth::None,
            Some("basic") => Auth::Basic,
            Some("digest") => Auth::Digest,
            Some("bearer") => Auth::Bearer,
            Some(other) => return Err(format!("unknown auth '{other}'")),
        };
        let poll_path = s
            .get("poll_path")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
            .map(String::from);
        if let Some(path) = &poll_path {
            check_path(path).map_err(|e| format!("poll_path: {e}"))?;
        }
        Ok(GenericHttp {
            base: format!("{scheme}://{host}:{port}"),
            auth,
            username: text("username"),
            password: text("password"),
            token: text("token"),
            accept_invalid_certs: s
                .get("accept_invalid_certs")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            headers: parse_headers(s.get("headers")).map_err(|e| format!("headers: {e}"))?,
            timeout: s
                .get("timeout_ms")
                .and_then(Value::as_u64)
                .unwrap_or(DEFAULT_TIMEOUT),
            poll_path,
            poll_every: s
                .get("poll_interval_ms")
                .and_then(Value::as_u64)
                .unwrap_or(DEFAULT_POLL_EVERY),
            polling: false,
            refused: false,
            next_id: 1,
            requests: HashMap::new(),
        })
    }

    fn build(
        &self,
        method: &'static str,
        path: &str,
        mut headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
        timeout: Millis,
    ) -> HttpRequest {
        let mut all = self.headers.clone();
        for (name, value) in headers.drain(..) {
            set_header(&mut all, &name, value);
        }
        if !has_header(&all, "Authorization") {
            match self.auth {
                Auth::Basic => {
                    use base64::Engine;
                    let token = base64::engine::general_purpose::STANDARD
                        .encode(format!("{}:{}", self.username, self.password));
                    all.push(("Authorization".into(), format!("Basic {token}")));
                }
                Auth::Bearer if !self.token.is_empty() => {
                    all.push(("Authorization".into(), format!("Bearer {}", self.token)));
                }
                _ => {}
            }
        }
        HttpRequest {
            method,
            url: format!("{}{path}", self.base),
            headers: all,
            body,
            timeout: Some(timeout),
            accept_invalid_certs: self.accept_invalid_certs,
            digest: matches!(self.auth, Auth::Basic | Auth::Digest).then(|| Credentials {
                username: self.username.clone(),
                password: self.password.clone(),
            }),
        }
    }

    fn send(&mut self, cx: &mut Cx, purpose: Purpose, request: HttpRequest) {
        let id = self.next_id;
        self.next_id += 1;
        self.requests.insert(id, purpose);
        cx.http(id, request);
    }

    fn poll(&mut self, cx: &mut Cx, command: Option<CommandId>) {
        let Some(path) = self.poll_path.clone() else {
            return;
        };
        if command.is_none() {
            self.polling = true;
        }
        let request = self.build("GET", &path, Vec::new(), None, self.timeout);
        self.send(cx, Purpose::Poll { command }, request);
    }

    fn request(&mut self, cx: &mut Cx, id: CommandId, params: &Params) -> Result<(), String> {
        let name = params
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("GET");
        let method = method(name).ok_or_else(|| format!("unknown method '{name}'"))?;
        let path = params.get("path").and_then(Value::as_str).unwrap_or("/");
        check_path(path)?;
        let mut headers = parse_headers(params.get("headers"))?;
        let body = match params.get("body") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some((s.clone().into_bytes(), "text/plain; charset=utf-8")),
            Some(other) => Some((
                serde_json::to_vec(other).expect("JSON serializes"),
                "application/json",
            )),
        };
        let content_type = params.get("content_type").and_then(Value::as_str);
        if let Some(ct) = content_type {
            set_header(&mut headers, "Content-Type", ct.to_string());
        } else if let Some((_, inferred)) = &body {
            if !has_header(&headers, "Content-Type") && !has_header(&self.headers, "Content-Type") {
                headers.push(("Content-Type".into(), (*inferred).to_string()));
            }
        }
        let timeout = params
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(self.timeout);
        let fail_on_status = params
            .get("fail_on_status")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let request = self.build(method, path, headers, body.map(|(b, _)| b), timeout);
        self.send(cx, Purpose::Command { id, fail_on_status }, request);
        Ok(())
    }

    fn reachable(&self, cx: &mut Cx) {
        if !self.refused {
            cx.connection(Connection::Connected);
        }
    }

    fn poll_result(&mut self, cx: &mut Cx, result: &Result<HttpResponse, String>) {
        match result {
            Ok(response) => {
                let body = String::from_utf8_lossy(&response.body);
                let parsed = serde_json::from_slice::<Value>(&response.body).ok();
                // A merge patch only adds to an object; clear it first so
                // members the device no longer sends go.
                if parsed.as_ref().is_some_and(Value::is_object) {
                    cx.state(json!({"poll": {"json": null}}));
                }
                cx.state(json!({"poll": {
                    "status": response.status,
                    "ok": (200..300).contains(&response.status),
                    "body": excerpt(&body, MAX_STATE_BODY),
                    "json": parsed,
                    "error": null,
                }}));
            }
            Err(message) => cx.state(json!({"poll": {"error": message}})),
        }
    }
}

fn transport_error(message: String) -> CommandError {
    if message.contains("timed out") {
        CommandError::Timeout
    } else {
        CommandError::Transport { message }
    }
}

impl Module for GenericHttp {
    fn start(&mut self, cx: &mut Cx) {
        if self.poll_path.is_some() {
            cx.connection(Connection::Connecting);
            self.poll(cx, None);
        } else {
            // Nothing is asked of the device until a command is.
            cx.connection(Connection::Unmonitored);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match name {
            "request" => {
                if let Err(message) = self.request(cx, id, params) {
                    cx.complete(id, Err(CommandError::InvalidParams { message }));
                }
            }
            "poll_now" => {
                if self.poll_path.is_none() {
                    cx.complete(
                        id,
                        Err(CommandError::InvalidParams {
                            message: "no poll_path is set".into(),
                        }),
                    );
                } else {
                    self.poll(cx, Some(id));
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

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let Some(purpose) = self.requests.remove(&id) else {
            return;
        };
        match &result {
            Ok(_) => {
                cx.alive();
                self.reachable(cx);
            }
            Err(message) if !self.refused => cx.connection(Connection::Disconnected {
                reason: message.clone(),
            }),
            Err(_) => {}
        }
        match purpose {
            Purpose::Command { id, fail_on_status } => {
                let outcome = match result {
                    Ok(response) => {
                        let value = response_value(&response);
                        let status = response.status;
                        let body = String::from_utf8_lossy(&response.body);
                        let message = format!("HTTP {status}: {}", excerpt(&body, MAX_ERROR_BODY));
                        match status {
                            401 | 403 if fail_on_status => Err(CommandError::Auth { message }),
                            400.. if fail_on_status => Err(CommandError::DeviceError {
                                code: Some(status.to_string()),
                                message,
                            }),
                            _ => Ok(Outcome::Value { value }),
                        }
                    }
                    Err(message) => Err(transport_error(message)),
                };
                cx.complete(id, outcome);
            }
            Purpose::Poll { command } => {
                self.poll_result(cx, &result);
                if let Some(id) = command {
                    cx.complete(
                        id,
                        match result {
                            Ok(response) => Ok(Outcome::Value {
                                value: response_value(&response),
                            }),
                            Err(message) => Err(transport_error(message)),
                        },
                    );
                    return;
                }
                self.polling = false;
                let refused = matches!(&result, Ok(r) if r.status == 401 || r.status == 403);
                if refused && self.auth != Auth::None {
                    self.refused = true;
                    cx.connection(Connection::Unauthorized {
                        reason: format!(
                            "the poll was refused with HTTP {}; polling stopped until the \
                             device is opened again",
                            result.as_ref().map_or(0, |r| r.status)
                        ),
                    });
                    return;
                }
                cx.set_timer(POLL, self.poll_every);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == POLL && !self.refused && !self.polling {
            self.poll(cx, None);
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.cancel_timer(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn module(settings: Value) -> GenericHttp {
        GenericHttp::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 30)),
            port: None,
            model: "http".into(),
            channels: None,
            settings: params(settings),
            monitor: true,
        })
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

    fn header<'a>(r: &'a HttpRequest, name: &str) -> Option<&'a str> {
        r.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn completion(actions: &[Action], id: CommandId) -> Option<crate::module::CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    #[test]
    fn builds_requests() {
        let mut m = module(json!({
            "auth": "basic", "username": "admin", "password": "secret",
            "headers": {"X-Api-Key": "k1", "Accept": "application/json"}
        }));
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            1,
            "request",
            &params(json!({
                "method": "POST", "path": "/api/v1/preset?id=3",
                "headers": {"accept": "text/plain"}, "body": {"recall": 3}
            })),
        );
        let (_, r) = &requests(&cx.take())[0];
        assert_eq!(r.method, "POST");
        assert_eq!(r.url, "http://192.0.2.30:80/api/v1/preset?id=3");
        assert_eq!(r.body.as_deref(), Some(&br#"{"recall":3}"#[..]));
        assert_eq!(header(r, "content-type"), Some("application/json"));
        // RFC 7617 §2: base64 of "admin:secret".
        assert_eq!(header(r, "authorization"), Some("Basic YWRtaW46c2VjcmV0"));
        assert_eq!(header(r, "accept"), Some("text/plain"));
        assert_eq!(header(r, "x-api-key"), Some("k1"));
        assert!(r.digest.is_some());

        // A string body is text; content_type wins.
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            2,
            "request",
            &params(json!({"method": "PUT", "path": "/x", "body": "a\nb", "content_type": "application/xml"})),
        );
        let (_, r) = &requests(&cx.take())[0];
        assert_eq!(r.body.as_deref(), Some(&b"a\nb"[..]));
        assert_eq!(header(r, "content-type"), Some("application/xml"));

        let mut m = module(json!({"scheme": "https", "auth": "bearer", "token": "t0k"}));
        let mut cx = Cx::new(0);
        m.command(&mut cx, 3, "request", &params(json!({"path": "/"})));
        let (_, r) = &requests(&cx.take())[0];
        assert_eq!(r.url, "https://192.0.2.30:443/");
        assert_eq!(header(r, "authorization"), Some("Bearer t0k"));
        assert!(r.digest.is_none());
        assert!(r.body.is_none());

        let m = GenericHttp::new(OpenContext {
            host: IpAddr::V6(Ipv6Addr::LOCALHOST),
            port: Some(8080),
            model: "http".into(),
            channels: None,
            settings: Params::new(),
            monitor: true,
        })
        .unwrap();
        assert_eq!(m.base, "http://[::1]:8080");
    }

    #[test]
    fn refuses_bad_input() {
        assert!(parse_headers(Some(&json!({"Bad Name": "x"}))).is_err());
        assert!(parse_headers(Some(&json!({"X": "a\r\nInjected: 1"}))).is_err());
        assert!(parse_headers(Some(&json!(["X"]))).is_err());
        assert_eq!(
            parse_headers(Some(&json!({"X": "  v\t", "N": 5}))).unwrap(),
            vec![("X".into(), "v".into()), ("N".into(), "5".into())]
        );
        assert!(check_path("/a b").is_err());
        assert!(check_path("a").is_err());
        assert!(check_path("/a%20b?c=d&e").is_ok());
        let mut m = module(json!({}));
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "request", &params(json!({"path": "/ü"})));
        assert!(matches!(
            completion(&cx.take(), 1),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        assert!(GenericHttp::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: None,
            model: "http".into(),
            channels: None,
            settings: params(json!({"poll_path": "status"})),
            monitor: true,
        })
        .is_err());
    }

    #[test]
    fn statuses_and_results() {
        let mut m = module(json!({}));
        let respond = |m: &mut GenericHttp, fail: bool, status: u16, body: &str| {
            let mut cx = Cx::new(0);
            m.command(
                &mut cx,
                9,
                "request",
                &params(json!({"path": "/", "fail_on_status": fail})),
            );
            let (rid, _) = requests(&cx.take())[0].clone();
            let mut cx = Cx::new(1);
            m.http_response(
                &mut cx,
                rid,
                Ok(HttpResponse {
                    status,
                    body: body.as_bytes().to_vec(),
                }),
            );
            completion(&cx.take(), 9).unwrap()
        };
        assert_eq!(
            respond(&mut m, true, 200, r#"{"a":1}"#),
            Ok(Outcome::Value {
                value: json!({"status": 200, "ok": true, "body": "{\"a\":1}", "json": {"a": 1}})
            })
        );
        assert_eq!(
            respond(&mut m, true, 204, ""),
            Ok(Outcome::Value {
                value: json!({"status": 204, "ok": true, "body": ""})
            })
        );
        assert_eq!(
            respond(&mut m, true, 404, "no such"),
            Err(CommandError::DeviceError {
                code: Some("404".into()),
                message: "HTTP 404: no such".into()
            })
        );
        assert!(matches!(
            respond(&mut m, true, 401, ""),
            Err(CommandError::Auth { .. })
        ));
        assert!(matches!(
            respond(&mut m, false, 500, "x"),
            Ok(Outcome::Value { value }) if value["status"] == 500 && value["ok"] == false
        ));

        let mut cx = Cx::new(0);
        m.command(&mut cx, 10, "request", &params(json!({"path": "/"})));
        let (rid, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(1);
        m.http_response(
            &mut cx,
            rid,
            Err("error sending request: operation timed out".into()),
        );
        let a = cx.take();
        assert_eq!(completion(&a, 10), Some(Err(CommandError::Timeout)));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
    }

    #[test]
    fn polls_into_state_and_stops_on_refusal() {
        let mut m = module(
            json!({"poll_path": "/status", "poll_interval_ms": 2000, "auth": "digest", "username": "u", "password": "p"}),
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::Connection(Connection::Connecting)));
        let (rid, r) = requests(&a)[0].clone();
        assert_eq!(r.url, "http://192.0.2.30:80/status");
        assert!(header(&r, "authorization").is_none());

        let mut cx = Cx::new(10);
        m.http_response(
            &mut cx,
            rid,
            Ok(HttpResponse {
                status: 200,
                body: br#"{"power":"on"}"#.to_vec(),
            }),
        );
        let a = cx.take();
        let mut state = json!({"poll": {"json": {"stale": 1}}});
        for x in &a {
            if let Action::State(p) = x {
                crate::session::merge_patch(&mut state, p);
            }
        }
        assert_eq!(state["poll"]["json"], json!({"power": "on"}));
        assert_eq!(state["poll"]["status"], 200);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::SetTimer {
            key: POLL,
            after: 2000
        }));

        let mut cx = Cx::new(2010);
        m.timer(&mut cx, POLL);
        let (rid, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(2020);
        m.http_response(
            &mut cx,
            rid,
            Ok(HttpResponse {
                status: 401,
                body: Vec::new(),
            }),
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a.iter().any(|x| matches!(x, Action::SetTimer { .. })));
        let mut cx = Cx::new(9000);
        m.timer(&mut cx, POLL);
        assert!(requests(&cx.take()).is_empty());
    }
}
