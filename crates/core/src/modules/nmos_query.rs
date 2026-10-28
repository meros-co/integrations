//! An NMOS registry's Query API websocket subscriptions, added to the
//! spec-driven `nmos-registry`.
//!
//! Commands are the spec's (`specs/nmos-registry.yaml`), run by the spec
//! engine. This adds what the format cannot express: a subscription whose
//! websocket address comes from the answer to a request (AMWA IS-04 v1.3,
//! QueryAPI.raml `/subscriptions`, docs "Behaviour - Querying").
//!
//! - For each resource type (nodes, devices, sources, flows, senders and
//!   receivers) a non-persistent subscription is created with
//!   `POST /x-nmos/query/<version>/subscriptions`, `{"max_update_rate_ms",
//!   "resource_path", "params": {}, "persist": false, "secure"}`; the answer
//!   (200 or 201) names the websocket in `ws_href`.
//! - The websocket sends grains, each `{"grain": {"topic", "data": [{"path",
//!   "pre", "post"}]}}`: the first after connecting holds every resource
//!   (the sync), later ones the changes, a resource gone having no `post`.
//! - The registry deletes a non-persistent subscription once no client is
//!   connected to it, so a websocket that closes is replaced by a new
//!   subscription, after a delay that starts at one second and doubles to
//!   thirty while failures continue.
//!
//! Each grain is offered to the spec's telemetry rules as one JSON message
//! per kind of change, in this module's own shape: `{"nmos": <type>, "op":
//! "sync", "items": [<resource>...]}` for the first grain on a websocket
//! (the whole list, which the spec replaces), `{"nmos": <type>, "op": "set",
//! "items": [...]}` for resources added or changed, and `{"nmos": <type>,
//! "op": "delete", "ids": [...]}` for resources gone. The websocket's
//! address is the registry's to give: it is used as given when it is `ws` or
//! `wss`, with the bearer token where one is set.
//!
//! A device opened for commands only runs without this module (it only keeps
//! state current), so it creates no subscription.

use std::net::IpAddr;

use serde_json::{json, Value};

use crate::catalog::Params;
use crate::engine::SpecEngine;
use crate::module::{
    CommandId, Cx, FileInput, HttpRequest, HttpResponse, Key, Level, Millis, Module, OpenContext,
    RequestId, SseInput, TcpInput, WsInput, WsRequest,
};

/// Request ids at and above this are this module's; the engine's count up
/// from 1 and never reach it.
const OWN_IDS: RequestId = 1 << 48;
const CREATE_TIMEOUT: Millis = 5_000;
const FIRST_RETRY: Millis = 1_000;
const MAX_RETRY: Millis = 30_000;
const DEFAULT_RATE: u64 = 100;

/// The resource types, each with its websocket and retry timer keys.
const TYPES: [(&str, Key, Key); 6] = [
    ("nodes", "nmos-ws-nodes", "nmos-retry-nodes"),
    ("devices", "nmos-ws-devices", "nmos-retry-devices"),
    ("sources", "nmos-ws-sources", "nmos-retry-sources"),
    ("flows", "nmos-ws-flows", "nmos-retry-flows"),
    ("senders", "nmos-ws-senders", "nmos-retry-senders"),
    ("receivers", "nmos-ws-receivers", "nmos-retry-receivers"),
];

#[derive(Debug, Clone, PartialEq)]
enum Phase {
    /// Waiting for the retry timer.
    Idle,
    Creating(RequestId),
    /// The websocket is opening or open; `synced` once its first grain came.
    Subscribed {
        synced: bool,
    },
    Stopped,
}

#[derive(Debug)]
struct Subscription {
    kind: &'static str,
    socket: Key,
    timer: Key,
    phase: Phase,
    retry: Millis,
}

pub(crate) struct NmosQuery {
    engine: SpecEngine,
    /// `scheme://host:port` of the Query API.
    origin: String,
    version: String,
    token: Option<String>,
    rate: u64,
    secure: bool,
    subs: Vec<Subscription>,
    next_id: RequestId,
}

/// What a grain tells: resources set and ids gone.
#[derive(Debug, Default, PartialEq)]
struct Grain {
    set: Vec<Value>,
    gone: Vec<String>,
}

/// Read a websocket message: `None` when it is not a grain.
fn grain(text: &str) -> Option<Grain> {
    let doc: Value = serde_json::from_str(text).ok()?;
    let data = doc.get("grain")?.get("data")?.as_array()?;
    let mut out = Grain::default();
    for item in data {
        match item.get("post") {
            Some(post) if post.is_object() => out.set.push(post.clone()),
            // No `post`: the resource is gone, named by `path` (its id) or
            // its `pre`.
            _ => {
                let id = item
                    .get("path")
                    .and_then(Value::as_str)
                    .filter(|p| !p.is_empty())
                    .or_else(|| item.get("pre")?.get("id")?.as_str());
                if let Some(id) = id {
                    out.gone.push(id.to_string());
                }
            }
        }
    }
    Some(out)
}

/// The messages a grain is offered to the rules as.
fn messages(kind: &str, grain: Grain, first: bool) -> Vec<Value> {
    let mut out = Vec::new();
    if first {
        out.push(json!({"nmos": kind, "op": "sync", "items": grain.set}));
    } else if !grain.set.is_empty() {
        out.push(json!({"nmos": kind, "op": "set", "items": grain.set}));
    }
    if !grain.gone.is_empty() {
        out.push(json!({"nmos": kind, "op": "delete", "ids": grain.gone}));
    }
    out
}

/// The websocket address a subscription answer names, when it is one.
fn ws_href(body: &[u8]) -> Option<String> {
    let doc: Value = serde_json::from_slice(body).ok()?;
    let href = doc.get("ws_href")?.as_str()?.trim();
    (href.starts_with("ws://") || href.starts_with("wss://")).then(|| href.to_string())
}

impl NmosQuery {
    pub(crate) fn new(engine: SpecEngine, ctx: &OpenContext) -> NmosQuery {
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
        NmosQuery {
            engine,
            origin: format!("{scheme}://{host}:{}", ctx.port.unwrap_or(default_port)),
            version: setting("query_version").unwrap_or_else(|| "v1.3".into()),
            token: setting("token"),
            rate: ctx
                .settings
                .get("max_update_rate_ms")
                .and_then(Value::as_u64)
                .unwrap_or(DEFAULT_RATE),
            secure: scheme == "https",
            subs: TYPES
                .iter()
                .map(|(kind, socket, timer)| Subscription {
                    kind,
                    socket,
                    timer,
                    phase: Phase::Idle,
                    retry: FIRST_RETRY,
                })
                .collect(),
            next_id: OWN_IDS,
        }
    }

    fn auth(&self) -> Vec<(String, String)> {
        self.token
            .iter()
            .map(|t| ("Authorization".to_string(), format!("Bearer {t}")))
            .collect()
    }

    fn create(&mut self, cx: &mut Cx, i: usize) {
        let id = self.next_id;
        self.next_id += 1;
        let body = json!({
            "max_update_rate_ms": self.rate,
            "resource_path": format!("/{}", self.subs[i].kind),
            "params": {},
            "persist": false,
            "secure": self.secure,
        });
        let mut headers = self.auth();
        headers.push(("Content-Type".into(), "application/json".into()));
        cx.http(
            id,
            HttpRequest {
                method: "POST",
                url: format!(
                    "{}/x-nmos/query/{}/subscriptions",
                    self.origin, self.version
                ),
                headers,
                body: Some(body.to_string().into_bytes()),
                body_stream: None,
                timeout: Some(CREATE_TIMEOUT),
                accept_invalid_certs: true,
                digest: None,
            },
        );
        self.subs[i].phase = Phase::Creating(id);
    }

    /// Subscribe again after the current delay, and lengthen the next.
    fn later(&mut self, cx: &mut Cx, i: usize, why: String) {
        let s = &mut self.subs[i];
        if s.phase == Phase::Stopped {
            return;
        }
        cx.log(
            Level::Debug,
            format!(
                "NMOS {} subscription: {why}; subscribing again in {} ms",
                s.kind, s.retry
            ),
        );
        s.phase = Phase::Idle;
        cx.set_timer(s.timer, s.retry);
        s.retry = (s.retry * 2).min(MAX_RETRY);
    }

    fn created(&mut self, cx: &mut Cx, i: usize, result: Result<HttpResponse, String>) {
        let response = match result {
            Ok(r) => r,
            Err(e) => return self.later(cx, i, e),
        };
        if !(200..300).contains(&response.status) {
            if matches!(response.status, 401 | 403) {
                cx.log(
                    Level::Warning,
                    format!(
                        "the registry refused a {} subscription (HTTP {}): check the token",
                        self.subs[i].kind, response.status
                    ),
                );
            }
            return self.later(cx, i, format!("HTTP {}", response.status));
        }
        let Some(href) = ws_href(&response.body) else {
            return self.later(cx, i, "no ws_href in the answer".into());
        };
        self.subs[i].phase = Phase::Subscribed { synced: false };
        cx.ws_open(
            self.subs[i].socket,
            WsRequest {
                url: href,
                headers: self.auth(),
                accept_invalid_certs: true,
            },
        );
    }

    fn input(&mut self, cx: &mut Cx, i: usize, input: WsInput) {
        match input {
            WsInput::Opened => self.subs[i].retry = FIRST_RETRY,
            WsInput::Text(text) => {
                let Some(g) = grain(&text) else { return };
                let first = matches!(self.subs[i].phase, Phase::Subscribed { synced: false });
                if let Phase::Subscribed { synced } = &mut self.subs[i].phase {
                    *synced = true;
                }
                cx.alive();
                for message in messages(self.subs[i].kind, g, first) {
                    self.engine.apply_message(cx, &message.to_string());
                }
            }
            WsInput::Closed { reason, .. } => {
                self.later(cx, i, format!("websocket closed: {reason}"))
            }
            _ => {}
        }
    }
}

impl Module for NmosQuery {
    fn start(&mut self, cx: &mut Cx) {
        self.engine.start(cx);
        for i in 0..self.subs.len() {
            self.create(cx, i);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        self.engine.command(cx, id, name, params);
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: std::net::SocketAddr, data: &[u8]) {
        self.engine.datagram(cx, socket, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        match self.subs.iter().position(|s| s.socket == socket) {
            Some(i) => self.later(cx, i, message.to_string()),
            None => self.engine.socket_error(cx, socket, message),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        self.engine.tcp(cx, socket, input);
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        match self.subs.iter().position(|s| s.socket == socket) {
            Some(i) => self.input(cx, i, input),
            None => self.engine.ws(cx, socket, input),
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        if id < OWN_IDS {
            return self.engine.http_response(cx, id, result);
        }
        // A reply to a request already given up on is dropped.
        if let Some(i) = self
            .subs
            .iter()
            .position(|s| s.phase == Phase::Creating(id))
        {
            self.created(cx, i, result);
        }
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.engine.sse(cx, stream, input);
    }

    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        self.engine.file(cx, file, input);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match self.subs.iter().position(|s| s.timer == key) {
            Some(i) if self.subs[i].phase == Phase::Idle => self.create(cx, i),
            Some(_) => {}
            None => self.engine.timer(cx, key),
        }
    }

    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        self.engine.stream_watch(cx, stream, watching);
    }

    fn stop(&mut self, cx: &mut Cx) {
        // Closing the websockets lets the registry delete the subscriptions.
        for s in &mut self.subs {
            if matches!(s.phase, Phase::Subscribed { .. }) {
                cx.ws_close(s.socket);
            }
            cx.cancel_timer(s.timer);
            s.phase = Phase::Stopped;
        }
        self.engine.stop(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grain_gives_its_resources_and_the_ids_gone() {
        let text = json!({
            "grain_type": "event",
            "grain": {"type": "urn:x-nmos:format:data.event", "topic": "/senders/", "data": [
                {"path": "a", "pre": {"id": "a", "label": "Old"}, "post": {"id": "a", "label": "New"}},
                {"path": "b", "post": {"id": "b", "label": "Added"}},
                {"path": "c", "pre": {"id": "c", "label": "Gone"}},
            ]}
        })
        .to_string();
        let g = grain(&text).unwrap();
        assert_eq!(g.set.len(), 2);
        assert_eq!(g.gone, ["c"]);
        let m = messages("senders", g, false);
        assert_eq!(m[0]["op"], "set");
        assert_eq!(m[0]["items"][1]["label"], "Added");
        assert_eq!(
            m[1],
            json!({"nmos": "senders", "op": "delete", "ids": ["c"]})
        );
        assert!(grain("{\"other\":1}").is_none());
    }

    #[test]
    fn the_first_grain_is_the_whole_list_even_when_empty() {
        let g = grain(r#"{"grain":{"data":[]}}"#).unwrap();
        assert_eq!(
            messages("flows", g, true),
            [json!({"nmos": "flows", "op": "sync", "items": []})]
        );
        assert!(messages("flows", Grain::default(), false).is_empty());
    }

    #[test]
    fn only_a_websocket_address_is_followed() {
        assert_eq!(
            ws_href(br#"{"ws_href":"ws://10.0.0.5:8870/x-nmos/query/v1.3/subscriptions/x"}"#)
                .as_deref(),
            Some("ws://10.0.0.5:8870/x-nmos/query/v1.3/subscriptions/x")
        );
        assert_eq!(ws_href(br#"{"ws_href":"http://10.0.0.5/"}"#), None);
        assert_eq!(ws_href(b"{}"), None);
    }
}
