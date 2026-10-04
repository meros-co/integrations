//! AJA's event connection, added to the spec-driven Ki Pro and KUMO.
//!
//! Commands, queries and telemetry rules are the spec's (`specs/aja-kipro.yaml`,
//! `specs/aja-kumo.yaml`), run by the spec engine. This adds what the format
//! cannot express: a loop that carries a value from one reply into the next
//! request (AJA REST API chapter 1; Ki Pro REST Automation 4.1.0, api.txt and
//! events.py; KUMO python_examples).
//!
//! - `action=connect` returns `{"connectionid": "20"}`.
//! - `action=wait_for_config_events&connectionid=20` is held by the unit until
//!   something changes, and returns an array of
//!   `{param_id, param_type, int_value, str_value, last_config_update}`.
//! - The id expires when no wait uses it for about a minute, so the next wait
//!   goes out as soon as each reply arrives.
//!
//! Both requests are the spec's own `open_event_connection` and
//! `wait_for_events` commands, sent with this module's request ids, so they
//! never occupy the engine's command queue. Each change is offered to the
//! spec's telemetry rules as two text messages,
//! `event <param_id> value=<value>` and `event <param_id> value_name=<name>`,
//! with the value and name as a `/config?action=get` reply would give them.
//! An expired id or a failed request opens a new connection, after a delay
//! that starts at one second and doubles to thirty while failures continue.

use std::net::IpAddr;

use serde_json::Value;

use crate::catalog::{DeviceSpec, Params};
use crate::engine::template::percent_encode;
use crate::engine::SpecEngine;
use crate::module::{
    CommandId, Cx, FileInput, HttpRequest, HttpResponse, Key, Level, Millis, Module, OpenContext,
    RequestId, SseInput, TcpInput, WsInput,
};

const RETRY: Key = "aja-events-retry";
/// Request ids at and above this are this module's; the engine's count up
/// from 1 and never reach it.
const OWN_IDS: RequestId = 1 << 48;
const CONNECT_TIMEOUT: Millis = 4_000;
/// How long one wait may be held. The unit expires an id unused for about a
/// minute, so a wait that ends here with nothing is followed at once by
/// another on the same id.
const WAIT_TIMEOUT: Millis = 30_000;
/// A wait failing this close to its deadline was held open with nothing to
/// report, rather than failing.
const IDLE_MARGIN: Millis = 1_000;
const FIRST_RETRY: Millis = 1_000;
const MAX_RETRY: Millis = 30_000;
/// Refusals of the connect request before saying, once, that the unit
/// appears to have no event connection.
const WARN_AFTER: u32 = 3;

/// One of the spec's two requests: its path and query, in order.
#[derive(Debug, Clone, PartialEq)]
struct Request {
    path: String,
    query: Vec<(String, String)>,
}

impl Request {
    fn from_command(spec: &DeviceSpec, name: &str) -> Result<Request, String> {
        let send = spec
            .commands
            .get(name)
            .and_then(|c| c.send.as_ref())
            .ok_or(format!(
                "aja-config-events needs the spec's '{name}' command"
            ))?;
        let path = send
            .get("path")
            .and_then(Value::as_str)
            .ok_or(format!("aja-config-events: '{name}' has no path"))?
            .to_string();
        let query = send
            .get("query")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(k, v)| {
                let v = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                (k.clone(), v)
            })
            .collect();
        Ok(Request { path, query })
    }

    /// The path and query, with `{connection}` filled in. `swap` sends to the
    /// other of AJA's two documented paths, /json and /config.
    fn target(&self, connection: Option<&str>, swap: bool) -> String {
        let path = match (swap, self.path.as_str()) {
            (true, "/json") => "/config",
            (true, "/config") => "/json",
            (_, p) => p,
        };
        let query: Vec<String> = self
            .query
            .iter()
            .map(|(k, v)| {
                let v = match connection {
                    Some(c) => v.replace("{connection}", c),
                    None => v.clone(),
                };
                format!("{}={}", percent_encode(k), percent_encode(&v))
            })
            .collect();
        if query.is_empty() {
            path.to_string()
        } else {
            format!("{path}?{}", query.join("&"))
        }
    }
}

/// The connection id in a connect reply: `{"connectionid":"20"}` or a number.
/// None for anything else, including 0, which AJA's clients treat as failure.
fn connection_id(body: &[u8]) -> Option<String> {
    let json: Value = serde_json::from_slice(body).ok()?;
    let id = match json.get("connectionid")? {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    let valid = !id.is_empty() && id.chars().all(|c| c.is_ascii_digit());
    (valid && !id.trim_start_matches('0').is_empty()).then_some(id)
}

/// One changed parameter, as a `/config?action=get` reply would give it.
#[derive(Debug, Clone, PartialEq)]
struct Change {
    param: String,
    value: String,
    value_name: String,
}

impl Change {
    /// The two text messages offered to the telemetry rules.
    fn lines(&self) -> [String; 2] {
        [
            format!("event {} value={}", self.param, self.value),
            format!("event {} value_name={}", self.param, self.value_name),
        ]
    }
}

/// A text value on one line: control characters (a pretty-printed salvo,
/// say) become spaces, since the rules match a single line.
fn one_line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn as_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// One element of a wait reply. The parameter is `param_id` (or `name`, as
/// the Companion module also accepts). A string parameter's value is
/// `str_value` and its name empty, as a get reply has it; any other's value is
/// `int_value` and its name `str_value`, or the value when that is empty.
/// When `param_type` does not say which (KUMO sends none), a non-empty
/// `str_value` with no or a zero `int_value` is taken as a string.
fn change(element: &Value) -> Option<Change> {
    let param = element
        .get("param_id")
        .or_else(|| element.get("name"))
        .and_then(Value::as_str)?
        .trim();
    // Ids are words, eParamID_...; anything else could not be a state key.
    if param.is_empty() || !param.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let int = element
        .get("int_value")
        .and_then(as_text)
        .filter(|s| !s.is_empty());
    let text = element
        .get("str_value")
        .and_then(as_text)
        .unwrap_or_default();
    let kind = element
        .get("param_type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_string = if ["string", "data", "text"].iter().any(|k| kind.contains(k)) {
        true
    } else if ["int", "enum", "bool", "float", "number"]
        .iter()
        .any(|k| kind.contains(k))
    {
        false
    } else {
        int.is_none() || (!text.is_empty() && int.as_deref() == Some("0"))
    };
    let (value, value_name) = if is_string {
        (text, String::new())
    } else {
        let value = int.unwrap_or_else(|| text.clone());
        let name = if text.is_empty() { value.clone() } else { text };
        (value, name)
    };
    Some(Change {
        param: param.to_string(),
        value: one_line(&value),
        value_name: one_line(&value_name),
    })
}

/// The changes in a wait reply. None when the reply is not a JSON array: the
/// connection id has expired or the unit refused the wait.
fn changes(body: &[u8]) -> Option<Vec<Change>> {
    let json: Value = serde_json::from_slice(body).ok()?;
    Some(json.as_array()?.iter().filter_map(change).collect())
}

#[derive(Debug, Clone, PartialEq)]
enum Phase {
    /// Waiting for the retry timer.
    Idle,
    Connecting {
        request: RequestId,
    },
    Waiting {
        request: RequestId,
        connection: String,
        since: Millis,
    },
    Stopped,
}

pub(crate) struct AjaConfigEvents {
    engine: SpecEngine,
    origin: String,
    connect: Request,
    wait: Request,
    /// Use the other of /json and /config: the spec's path gave no
    /// connection id last time.
    swapped: bool,
    phase: Phase,
    next_id: RequestId,
    retry: Millis,
    refusals: u32,
    warned: bool,
    /// The loop has delivered at least one reply since it last failed.
    working: bool,
}

impl AjaConfigEvents {
    pub(crate) fn new(
        engine: SpecEngine,
        spec: &DeviceSpec,
        ctx: &OpenContext,
    ) -> Result<AjaConfigEvents, String> {
        let transport = spec.transport.as_ref();
        let scheme = transport
            .and_then(|t| t.get("scheme"))
            .and_then(Value::as_str)
            .unwrap_or("http");
        let default_port = transport
            .and_then(|t| t.get("port"))
            .and_then(Value::as_u64)
            .map(|p| p as u16)
            .unwrap_or(if scheme == "https" { 443 } else { 80 });
        let host = match ctx.host {
            IpAddr::V6(v6) => format!("[{v6}]"),
            v4 => v4.to_string(),
        };
        Ok(AjaConfigEvents {
            engine,
            origin: format!("{scheme}://{host}:{}", ctx.port.unwrap_or(default_port)),
            connect: Request::from_command(spec, "open_event_connection")?,
            wait: Request::from_command(spec, "wait_for_events")?,
            swapped: false,
            phase: Phase::Idle,
            next_id: OWN_IDS,
            retry: FIRST_RETRY,
            refusals: 0,
            warned: false,
            working: false,
        })
    }

    fn get(&mut self, cx: &mut Cx, target: String, timeout: Millis) -> RequestId {
        let id = self.next_id;
        self.next_id += 1;
        cx.http(
            id,
            HttpRequest {
                method: "GET",
                url: format!("{}{target}", self.origin),
                headers: Vec::new(),
                body: None,
                timeout: Some(timeout),
                accept_invalid_certs: false,
                digest: None,
            },
        );
        id
    }

    fn open(&mut self, cx: &mut Cx) {
        let target = self.connect.target(None, self.swapped);
        let request = self.get(cx, target, CONNECT_TIMEOUT);
        self.phase = Phase::Connecting { request };
    }

    fn wait_on(&mut self, cx: &mut Cx, connection: String) {
        let target = self.wait.target(Some(&connection), self.swapped);
        let request = self.get(cx, target, WAIT_TIMEOUT);
        self.phase = Phase::Waiting {
            request,
            connection,
            since: cx.now(),
        };
    }

    /// Open a new connection after the current delay, and lengthen the next.
    fn later(&mut self, cx: &mut Cx, why: String) {
        let level = if self.working {
            Level::Info
        } else {
            Level::Debug
        };
        self.working = false;
        cx.log(
            level,
            format!("event connection: {why}; reconnecting in {} ms", self.retry),
        );
        self.phase = Phase::Idle;
        cx.set_timer(RETRY, self.retry);
        self.retry = (self.retry * 2).min(MAX_RETRY);
    }

    fn connected(&mut self, cx: &mut Cx, result: Result<HttpResponse, String>) {
        let response = match result {
            Ok(r) => r,
            Err(e) => return self.later(cx, e),
        };
        let id = (200..300)
            .contains(&response.status)
            .then(|| connection_id(&response.body))
            .flatten();
        match id {
            Some(connection) => {
                self.refusals = 0;
                cx.log(Level::Debug, format!("event connection {connection} open"));
                self.wait_on(cx, connection);
            }
            None => {
                // The unit answered but gave no id: try AJA's other path.
                self.swapped = !self.swapped;
                self.refusals += 1;
                if self.refusals >= WARN_AFTER && !self.warned {
                    self.warned = true;
                    cx.log(
                        Level::Warning,
                        "the unit gives no event connection; state comes from polling only",
                    );
                }
                self.later(cx, format!("no connection id (HTTP {})", response.status));
            }
        }
    }

    fn waited(
        &mut self,
        cx: &mut Cx,
        connection: String,
        since: Millis,
        result: Result<HttpResponse, String>,
    ) {
        match result {
            Ok(r) if (200..300).contains(&r.status) => match changes(&r.body) {
                Some(changes) => {
                    self.retry = FIRST_RETRY;
                    self.working = true;
                    cx.alive();
                    for change in &changes {
                        for line in change.lines() {
                            self.engine.apply_text(cx, &line);
                        }
                    }
                    self.wait_on(cx, connection);
                }
                None => self.later(cx, format!("connection {connection} expired")),
            },
            Ok(r) => self.later(cx, format!("wait refused (HTTP {})", r.status)),
            // Held to the deadline with nothing to report: the id is still
            // good, so wait on it again at once.
            Err(_) if cx.now().saturating_sub(since) + IDLE_MARGIN >= WAIT_TIMEOUT => {
                self.wait_on(cx, connection)
            }
            Err(e) => self.later(cx, e),
        }
    }
}

impl Module for AjaConfigEvents {
    fn start(&mut self, cx: &mut Cx) {
        self.engine.start(cx);
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        self.engine.command(cx, id, name, params);
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: std::net::SocketAddr, data: &[u8]) {
        self.engine.datagram(cx, socket, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        self.engine.socket_error(cx, socket, message);
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        self.engine.tcp(cx, socket, input);
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        self.engine.ws(cx, socket, input);
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        if id < OWN_IDS {
            return self.engine.http_response(cx, id, result);
        }
        match self.phase.clone() {
            Phase::Connecting { request } if request == id => self.connected(cx, result),
            Phase::Waiting {
                request,
                connection,
                since,
            } if request == id => self.waited(cx, connection, since, result),
            // A reply to a request already given up on.
            _ => {}
        }
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.engine.sse(cx, stream, input);
    }

    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        self.engine.file(cx, file, input);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == RETRY {
            if self.phase == Phase::Idle {
                self.open(cx);
            }
        } else {
            self.engine.timer(cx, key);
        }
    }

    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        self.engine.stream_watch(cx, stream, watching);
    }

    fn stop(&mut self, cx: &mut Cx) {
        // AJA documents no way to close an event connection; the unit
        // expires the id on its own.
        self.phase = Phase::Stopped;
        cx.cancel_timer(RETRY);
        self.engine.stop(cx);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::catalog::Catalog;
    use crate::module::Action;

    fn module(spec_id: &str, model: &str) -> AjaConfigEvents {
        let catalog = Catalog::embedded();
        let spec = catalog.device(spec_id).unwrap().clone();
        let ctx = OpenContext {
            host: "192.0.2.10".parse().unwrap(),
            host_name: None,
            port: None,
            model: model.into(),
            channels: None,
            settings: Params::new(),
            monitor: true,
        };
        let engine = SpecEngine::new(Arc::new(spec.clone()), ctx.clone()).unwrap();
        AjaConfigEvents::new(engine, &spec, &ctx).unwrap()
    }

    /// This module's requests among the actions, as (id, url, timeout).
    fn own_requests(actions: &[Action]) -> Vec<(RequestId, String, Option<Millis>)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Http { id, request } if *id >= OWN_IDS => {
                    Some((*id, request.url.clone(), request.timeout))
                }
                _ => None,
            })
            .collect()
    }

    fn ok(body: &str) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status: 200,
            body: body.as_bytes().to_vec(),
        })
    }

    fn state(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    fn retry_after(actions: &[Action]) -> Option<Millis> {
        actions.iter().find_map(|a| match a {
            Action::SetTimer { key, after } if *key == RETRY => Some(*after),
            _ => None,
        })
    }

    #[test]
    fn connection_ids() {
        assert_eq!(
            connection_id(br#"{"connectionid":"20"}"#),
            Some("20".into())
        );
        assert_eq!(connection_id(br#"{"connectionid":7}"#), Some("7".into()));
        assert_eq!(connection_id(br#"{"connectionid":"0"}"#), None);
        assert_eq!(connection_id(br#"{"connectionid":""}"#), None);
        assert_eq!(connection_id(b"<html>404</html>"), None);
        assert_eq!(connection_id(b"{}"), None);
    }

    #[test]
    fn changes_read_as_a_get_reply_would() {
        let body = json!([
            // Ki Pro: typed elements.
            {"param_id": "eParamID_TransportState", "param_type": "enum",
             "int_value": 2, "str_value": "Recording", "last_config_update": 815},
            {"param_id": "eParamID_DisplayTimecode", "param_type": "string",
             "int_value": 0, "str_value": "00:00:10:05"},
            {"param_id": "eParamID_RecordFormat", "param_type": "integer",
             "int_value": 3, "str_value": ""},
            // KUMO: no param_type.
            {"param_id": "eParamID_XPT_Destination1_Status", "int_value": 5, "str_value": ""},
            {"param_id": "eParamID_XPT_Source2_Line_1", "int_value": 0, "str_value": "CAM\n2"},
            {"param_id": "eParamID_XPT_Source3_Line_1", "str_value": "VTR"},
            // Not usable.
            {"param_id": "bad id", "int_value": 1},
            {"int_value": 1},
            "text"
        ])
        .to_string();
        let got = changes(body.as_bytes()).unwrap();
        let triples: Vec<(&str, &str, &str)> = got
            .iter()
            .map(|c| (c.param.as_str(), c.value.as_str(), c.value_name.as_str()))
            .collect();
        assert_eq!(
            triples,
            [
                ("eParamID_TransportState", "2", "Recording"),
                ("eParamID_DisplayTimecode", "00:00:10:05", ""),
                ("eParamID_RecordFormat", "3", "3"),
                ("eParamID_XPT_Destination1_Status", "5", "5"),
                ("eParamID_XPT_Source2_Line_1", "CAM 2", ""),
                ("eParamID_XPT_Source3_Line_1", "VTR", ""),
            ]
        );
        assert_eq!(
            got[0].lines(),
            [
                "event eParamID_TransportState value=2",
                "event eParamID_TransportState value_name=Recording"
            ]
        );
        assert_eq!(changes(b"[]"), Some(vec![]));
        assert_eq!(changes(br#"{"error":"bad connection id"}"#), None);
        assert_eq!(changes(b"<html></html>"), None);
    }

    #[test]
    fn targets_come_from_the_spec() {
        let m = module("aja-kipro", "kipro");
        assert_eq!(
            m.connect.target(None, false),
            "/json?action=connect&configid=0"
        );
        assert_eq!(
            m.wait.target(Some("20"), false),
            "/json?action=wait_for_config_events&configid=0&connectionid=20"
        );
        assert_eq!(
            m.wait.target(Some("20"), true),
            "/config?action=wait_for_config_events&configid=0&connectionid=20"
        );
        let m = module("aja-kumo", "kumo-1616");
        assert_eq!(m.connect.target(None, false), "/config?action=connect");
        assert_eq!(m.origin, "http://192.0.2.10:80");
    }

    /// Connect, wait, a change, wait again; an expired id and failures back
    /// off and reconnect; a wait held to its deadline is simply repeated; and
    /// commands go out while a wait is open.
    #[test]
    fn the_loop() {
        let mut m = module("aja-kumo", "kumo-1616");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let started = cx.take();
        let reqs = own_requests(&started);
        assert_eq!(reqs.len(), 1);
        let (connect, url, timeout) = reqs[0].clone();
        assert_eq!(url, "http://192.0.2.10:80/config?action=connect");
        assert_eq!(timeout, Some(CONNECT_TIMEOUT));

        let mut cx = Cx::new(10);
        m.http_response(&mut cx, connect, ok(r#"{"connectionid":"20"}"#));
        let reqs = own_requests(&cx.take());
        let (wait, url, timeout) = reqs[0].clone();
        assert_eq!(
            url,
            "http://192.0.2.10:80/config?action=wait_for_config_events&configid=0&connectionid=20"
        );
        assert_eq!(timeout, Some(WAIT_TIMEOUT));

        // While the wait is open the engine's requests (its probe, the reads
        // on connecting) and a command go out and are answered.
        let mut cx = Cx::new(20);
        m.command(
            &mut cx,
            1,
            "get_route",
            json!({"destination": 1}).as_object().unwrap(),
        );
        // The probe from starting is still unanswered.
        let mut actions = started;
        actions.extend(cx.take());
        let mut routed = false;
        for _ in 0..1000 {
            let engine: Vec<(RequestId, String)> = actions
                .iter()
                .filter_map(|a| match a {
                    Action::Http { id, request } if *id < OWN_IDS => {
                        Some((*id, request.url.clone()))
                    }
                    _ => None,
                })
                .collect();
            if engine.is_empty() {
                break;
            }
            let mut cx = Cx::new(30);
            for (id, url) in engine {
                if url.ends_with("action=get&paramid=eParamID_XPT_Destination1_Status") {
                    routed = true;
                }
                m.http_response(
                    &mut cx,
                    id,
                    ok(r#"{"paramid":"1","name":"x","value":"1","value_name":"1"}"#),
                );
            }
            actions = cx.take();
            assert!(
                own_requests(&actions).is_empty(),
                "the wait is not repeated"
            );
            if actions
                .iter()
                .any(|a| matches!(a, Action::Complete { id: 1, .. }))
            {
                break;
            }
        }
        assert!(routed, "the command was sent while the wait was open");

        // A change becomes state through the spec's rules, and the next wait
        // goes out on the same id.
        let mut cx = Cx::new(500);
        m.http_response(
            &mut cx,
            wait,
            ok(
                r#"[{"param_id":"eParamID_XPT_Destination3_Status","int_value":5,"str_value":""},
                  {"param_id":"eParamID_XPT_Source2_Line_1","int_value":0,"str_value":"CAM 2"}]"#,
            ),
        );
        let actions = cx.take();
        let patches = state(&actions);
        assert!(
            patches.iter().any(|p| p["outputs"]["3"]["input"] == 5),
            "{patches:?}"
        );
        assert!(patches
            .iter()
            .any(|p| p["params"]["eParamID_XPT_Destination3_Status"]["value"] == "5"));
        assert!(patches
            .iter()
            .any(|p| p["inputs"]["2"]["label_line_1"] == "CAM 2"));
        assert!(actions.contains(&Action::Alive));
        let (wait, url, _) = own_requests(&actions)[0].clone();
        assert!(url.ends_with("connectionid=20"));

        // Held to the deadline with nothing: the same id, at once.
        let mut cx = Cx::new(500 + WAIT_TIMEOUT);
        m.http_response(&mut cx, wait, Err("timed out".into()));
        let actions = cx.take();
        assert_eq!(retry_after(&actions), None);
        let (wait, url, _) = own_requests(&actions)[0].clone();
        assert!(url.ends_with("connectionid=20"));

        // Expired: reconnect after a second.
        let mut cx = Cx::new(40_000);
        m.http_response(&mut cx, wait, ok(r#"{"error":"invalid connection id"}"#));
        let actions = cx.take();
        assert!(own_requests(&actions).is_empty());
        assert_eq!(retry_after(&actions), Some(1_000));
        let mut cx = Cx::new(41_000);
        m.timer(&mut cx, RETRY);
        let (connect, url, _) = own_requests(&cx.take())[0].clone();
        assert!(url.ends_with("/config?action=connect"));

        // Failures double the delay.
        let mut cx = Cx::new(41_100);
        m.http_response(&mut cx, connect, Err("connection refused".into()));
        assert_eq!(retry_after(&cx.take()), Some(2_000));
        let mut cx = Cx::new(43_100);
        m.timer(&mut cx, RETRY);
        let (connect, _, _) = own_requests(&cx.take())[0].clone();
        let mut cx = Cx::new(43_200);
        m.http_response(&mut cx, connect, ok(r#"{"connectionid":"21"}"#));
        let (wait, url, _) = own_requests(&cx.take())[0].clone();
        assert!(url.ends_with("connectionid=21"));
        // A reply to a request given up on changes nothing.
        let mut cx = Cx::new(43_300);
        m.http_response(&mut cx, wait - 1, ok("[]"));
        assert!(cx.take().is_empty());
        // A wait that fails early is an error, not an idle wait.
        let mut cx = Cx::new(43_400);
        m.http_response(&mut cx, wait, Err("connection reset".into()));
        assert_eq!(retry_after(&cx.take()), Some(4_000));

        // After stopping, the timer opens nothing.
        let mut cx = Cx::new(50_000);
        m.stop(&mut cx);
        let mut cx = Cx::new(60_000);
        m.timer(&mut cx, RETRY);
        assert!(own_requests(&cx.take()).is_empty());
    }

    /// The state a wait reply makes through the spec's rules, merged.
    fn events_state(spec: &str, model: &str, reply: Value) -> Value {
        let mut m = module(spec, model);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let (connect, _, _) = own_requests(&cx.take())[0].clone();
        let mut cx = Cx::new(1);
        m.http_response(&mut cx, connect, ok(r#"{"connectionid":"5"}"#));
        let (wait, _, _) = own_requests(&cx.take())[0].clone();
        let mut cx = Cx::new(2);
        m.http_response(&mut cx, wait, ok(&reply.to_string()));
        let mut st = json!({});
        for p in state(&cx.take()) {
            crate::session::merge_patch(&mut st, &p);
        }
        st
    }

    #[test]
    fn events_reach_the_specific_rules() {
        let st = events_state(
            "aja-kipro",
            "kipro",
            json!([
                {"param_id": "eParamID_TransportState", "param_type": "enum", "int_value": 3, "str_value": "Playing Forward"},
                {"param_id": "eParamID_DisplayTimecode", "param_type": "string", "int_value": 0, "str_value": "01:00:00:12"},
                {"param_id": "eParamID_CurrentClip", "param_type": "string", "int_value": 0, "str_value": "Clip1ATK6.MOV"},
                {"param_id": "eParamID_MediaState", "param_type": "enum", "int_value": 1, "str_value": "Data - LAN"},
                {"param_id": "eParamID_StoragePath", "param_type": "string", "int_value": 0, "str_value": "/mnt/S1/AJA"}
            ]),
        );
        assert_eq!(
            st["transport"],
            json!({"state": "Playing Forward", "state_code": 3})
        );
        assert_eq!(st["timecode"]["display"], "01:00:00:12");
        assert_eq!(st["clip"]["current"], "Clip1ATK6.MOV");
        assert_eq!(
            st["media"],
            json!({"state": "Data - LAN", "data_lan": true, "storage_path": "/mnt/S1/AJA"})
        );
        assert_eq!(
            st["params"]["eParamID_DisplayTimecode"],
            json!({"value": "01:00:00:12", "value_name": ""})
        );
        assert_eq!(
            st["params"]["eParamID_TransportState"],
            json!({"value": "3", "value_name": "Playing Forward"})
        );

        let st = events_state(
            "aja-kumo",
            "kumo-3232",
            json!([
                {"param_id": "eParamID_XPT_Destination7_Status", "int_value": 12, "str_value": ""},
                {"param_id": "eParamID_XPT_Destination7_Line_1", "int_value": 0, "str_value": "MON"},
                {"param_id": "eParamID_XPT_Destination7_Line_2", "int_value": 0, "str_value": "A"},
                {"param_id": "eParamID_XPT_Source12_Line_2", "int_value": 0, "str_value": "CAM 12"},
                {"param_id": "eParamID_XPT_Destination7_Locked", "int_value": 1, "str_value": ""},
                {"param_id": "eParamID_NumberOfSources", "int_value": 32, "str_value": ""},
                {"param_id": "eParamID_NumberOfDestinations", "int_value": 32, "str_value": ""},
                {"param_id": "eParamID_KumoProductID", "int_value": 3, "str_value": "KUMO 32x32 Matrix"}
            ]),
        );
        assert_eq!(
            st["outputs"]["7"],
            json!({"input": 12, "label_line_1": "MON", "label_line_2": "A", "locked": true})
        );
        assert_eq!(st["inputs"]["12"]["label_line_2"], "CAM 12");
        assert_eq!(
            st["device"],
            json!({"sources": 32, "destinations": 32, "product_id": 3, "product": "KUMO 32x32 Matrix"})
        );
    }

    /// A unit answering the connect without an id is tried on AJA's other
    /// path, and a successful wait resets the delay.
    #[test]
    fn other_path_and_reset() {
        let mut m = module("aja-kipro", "kipro-ultra");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let (connect, url, _) = own_requests(&cx.take())[0].clone();
        assert!(url.ends_with("/json?action=connect&configid=0"));
        let mut cx = Cx::new(5);
        m.http_response(
            &mut cx,
            connect,
            Ok(HttpResponse {
                status: 404,
                body: b"<html>404</html>".to_vec(),
            }),
        );
        assert_eq!(retry_after(&cx.take()), Some(1_000));
        let mut cx = Cx::new(1_005);
        m.timer(&mut cx, RETRY);
        let (connect, url, _) = own_requests(&cx.take())[0].clone();
        assert!(url.ends_with("/config?action=connect&configid=0"), "{url}");
        let mut cx = Cx::new(1_010);
        m.http_response(&mut cx, connect, ok(r#"{"connectionid":"3"}"#));
        let (wait, url, _) = own_requests(&cx.take())[0].clone();
        assert!(
            url.contains("/config?action=wait_for_config_events"),
            "{url}"
        );
        let mut cx = Cx::new(1_020);
        m.http_response(
            &mut cx,
            wait,
            ok(r#"[{"param_id":"eParamID_TransportState","param_type":"enum","int_value":2,"str_value":"Recording"}]"#),
        );
        let patches = state(&cx.take());
        assert!(patches
            .iter()
            .any(|p| p["transport"]["state"] == "Recording"));
        assert!(patches.iter().any(|p| p["transport"]["state_code"] == 2));
        assert_eq!(m.retry, FIRST_RETRY);
    }
}
