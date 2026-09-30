//! Sennheiser EW-DX over SSCv2: HTTPS requests plus a server-sent event stream.
//!
//! Protocol from Sennheiser's SSCv2 specification. Resource paths are the ones
//! RFDeck exercised on real EW-DX receivers (OpenAPI 1.7): /api/channel/{id},
//! its signalQualityIndicator, level and warnings, /api/rf/channels/{id} and
//! /api/transmitters/{id}/battery. Only mute is writable: see the spec's quirks.

use std::collections::HashMap;
use std::net::IpAddr;

use base64::Engine;
use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, HttpRequest, HttpResponse, Key, Level, Millis, Module,
    OpenContext, Outcome, RequestId, SseInput,
};

const STREAM: Key = "subscriptions";

const REQUEST_TIMEOUT: Millis = 2_000;
const SUBSCRIBE_TIMEOUT: Millis = 5_000;
const MUTE_TIMEOUT: Millis = 3_000;
/// Retry after an unreachable device or a dropped stream.
const RETRY_AFTER: Millis = 1_000;
/// Retry less often after a rejected password: it will not change by itself,
/// but an operator may fix it on the device.
const UNAUTHORIZED_RETRY_AFTER: Millis = 30_000;
/// Liveness: a stream quiet this long is checked with a request.
const QUIET_AFTER: Millis = 3_000;
const LIVENESS_CHECK_EVERY: Millis = 1_000;
const LIVENESS_STRIKES: u32 = 2;
/// RFDeck subscribes in batches of four, which is what was tested on hardware.
const SUBSCRIBE_BATCH: usize = 4;

const RETRY: Key = "retry";
const LIVENESS: Key = "liveness";

#[derive(Debug)]
enum Purpose {
    Probe,
    Identity,
    Subscribe { remaining: Vec<Vec<String>> },
    Initial { path: String },
    Liveness,
    Mute { command: CommandId },
}

#[derive(Debug, PartialEq)]
enum Phase {
    Idle,
    Probing,
    Streaming,
}

pub(crate) struct Ewdx {
    base: String,
    authorization: String,
    channels: u32,
    phase: Phase,
    connected: bool,
    session: Option<String>,
    next_request: RequestId,
    requests: HashMap<RequestId, Purpose>,
    last_activity: Millis,
    liveness_in_flight: bool,
    strikes: u32,
}

impl Ewdx {
    pub(crate) fn new(ctx: OpenContext) -> Ewdx {
        let password = ctx
            .settings
            .get("password")
            .and_then(Value::as_str)
            .unwrap_or("");
        let port = ctx.port.unwrap_or(443);
        Ewdx::for_device(ctx.host, port, password, ctx.channels.unwrap_or(2))
    }

    fn for_device(host: IpAddr, port: u16, password: &str, channels: u32) -> Ewdx {
        let host = match host {
            IpAddr::V6(v6) => format!("[{v6}]"),
            v4 => v4.to_string(),
        };
        let credentials =
            base64::engine::general_purpose::STANDARD.encode(format!("api:{password}"));
        Ewdx {
            base: format!("https://{host}:{port}"),
            authorization: format!("Basic {credentials}"),
            channels,
            phase: Phase::Idle,
            connected: false,
            session: None,
            next_request: 1,
            requests: HashMap::new(),
            last_activity: 0,
            liveness_in_flight: false,
            strikes: 0,
        }
    }

    fn request(
        &self,
        method: &'static str,
        path: &str,
        body: Option<Value>,
        timeout: Option<Millis>,
    ) -> HttpRequest {
        let mut headers = vec![("Authorization".to_string(), self.authorization.clone())];
        if body.is_some() {
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        HttpRequest {
            method,
            url: format!("{}{}", self.base, path),
            headers,
            body: body.map(|b| b.to_string().into_bytes()),
            timeout,
            accept_invalid_certs: true,
        }
    }

    fn send(&mut self, cx: &mut Cx, purpose: Purpose, request: HttpRequest) {
        let id = self.next_request;
        self.next_request += 1;
        self.requests.insert(id, purpose);
        cx.http(id, request);
    }

    fn probe(&mut self, cx: &mut Cx) {
        self.phase = Phase::Probing;
        let request = self.request("GET", "/api/ssc/version", None, Some(REQUEST_TIMEOUT));
        self.send(cx, Purpose::Probe, request);
    }

    fn lost(&mut self, cx: &mut Cx, connection: Connection, retry: Millis) {
        cx.sse_close(STREAM);
        cx.cancel_timer(LIVENESS);
        self.phase = Phase::Idle;
        self.connected = false;
        self.session = None;
        self.strikes = 0;
        self.liveness_in_flight = false;
        cx.connection(connection);
        cx.set_timer(RETRY, retry);
    }

    fn open_stream(&mut self, cx: &mut Cx) {
        let mut request = self.request("GET", "/api/ssc/state/subscriptions", None, None);
        request
            .headers
            .push(("Accept".into(), "text/event-stream".into()));
        request
            .headers
            .push(("Cache-Control".into(), "no-cache".into()));
        cx.sse_open(STREAM, request);
    }

    fn resources(&self) -> Vec<String> {
        let mut paths = Vec::new();
        for id in 0..self.channels {
            paths.push(format!("/api/channel/{id}"));
            paths.push(format!("/api/channel/{id}/signalQualityIndicator"));
            paths.push(format!("/api/channel/{id}/level"));
            paths.push(format!("/api/channel/{id}/warnings"));
            paths.push(format!("/api/rf/channels/{id}"));
            paths.push(format!("/api/transmitters/{id}/battery"));
        }
        paths
    }

    fn subscribe_next(&mut self, cx: &mut Cx, mut remaining: Vec<Vec<String>>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        if remaining.is_empty() {
            return;
        }
        let batch = remaining.remove(0);
        let path = format!("/api/ssc/state/subscriptions/{session}/add");
        let request = self.request("PUT", &path, Some(json!(batch)), Some(SUBSCRIBE_TIMEOUT));
        self.send(cx, Purpose::Subscribe { remaining }, request);
    }

    fn fetch_initial(&mut self, cx: &mut Cx) {
        for path in self.resources() {
            if path.ends_with("/warnings") {
                continue;
            }
            let request = self.request("GET", &path, None, Some(REQUEST_TIMEOUT));
            self.send(cx, Purpose::Initial { path }, request);
        }
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.last_activity = cx.now();
        self.strikes = 0;
        cx.alive();
    }

    /// Apply one notification: `{"<resource path>": {...}}` per SSCv2 §3.4.8,
    /// or the `{"path": ..., "value": ...}` form RFDeck also handles.
    fn apply(&self, cx: &mut Cx, data: &Value) {
        let mut channels = Map::new();
        let mut device = Map::new();
        let mut take = |path: &str, value: &Value| {
            apply_resource(path, value, &mut channels, &mut device);
        };
        match data {
            Value::Object(map) if map.contains_key("path") && map.contains_key("value") => {
                if let Some(path) = map["path"].as_str() {
                    take(path, &map["value"]);
                }
            }
            Value::Object(map) => {
                for (path, value) in map {
                    take(path, value);
                }
            }
            _ => {}
        }
        let mut patch = Map::new();
        if !channels.is_empty() {
            patch.insert("channels".into(), Value::Object(channels));
        }
        if !device.is_empty() {
            patch.insert("device".into(), Value::Object(device));
        }
        if !patch.is_empty() {
            cx.state(Value::Object(patch));
        }
    }
}

/// Map one resource onto the state tree. Channels are 1-based in the state and
/// 0-based in the API.
fn apply_resource(
    path: &str,
    value: &Value,
    channels: &mut Map<String, Value>,
    device: &mut Map<String, Value>,
) {
    if path == "/api/device/identity" {
        device.insert("identity".into(), value.clone());
        return;
    }
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let (id, field): (Option<u32>, Option<&str>) = match parts.as_slice() {
        ["api", "channel", id] => (id.parse().ok(), None),
        ["api", "channel", id, field] => (id.parse().ok(), Some(field)),
        ["api", "rf", "channels", id] => (id.parse().ok(), Some("rf")),
        ["api", "transmitters", id, "battery"] => (id.parse().ok(), Some("battery")),
        _ => (None, None),
    };
    let Some(id) = id else { return };
    let channel = channels
        .entry((id + 1).to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .unwrap();

    match field {
        None => {
            if let Some(v) = value.get("name").and_then(Value::as_str) {
                channel.insert("name".into(), json!(v));
            }
            if let Some(v) = value.get("mute").and_then(Value::as_bool) {
                channel.insert("mute".into(), json!(v));
            }
        }
        Some("signalQualityIndicator") => {
            if let Some(v) = value.get("value").filter(|v| v.is_number()) {
                channel.insert("rf".into(), json!({"quality_pct": v}));
            }
        }
        Some("level") => {
            if let Some(v) = value.get("value").filter(|v| v.is_number()) {
                channel.insert("af".into(), json!({"level_dbfs": v}));
            }
        }
        Some("warnings") => {
            channel.insert("warnings".into(), value.clone());
        }
        Some("rf") => {
            if let Some(v) = value.get("frequency").filter(|v| v.is_number()) {
                channel.insert("frequency_khz".into(), v.clone());
            }
        }
        Some("battery") => {
            if let Some(v) = value.get("gauge").filter(|v| v.is_number()) {
                channel.insert("transmitter".into(), json!({"battery_percent": v}));
            }
        }
        _ => {}
    }
}

fn is_success(status: u16) -> bool {
    (200..300).contains(&status)
}

impl Module for Ewdx {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.probe(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match name {
            "mute" => {
                let channel = params.get("channel").and_then(Value::as_i64).unwrap_or(1);
                if channel > self.channels as i64 {
                    cx.complete(
                        id,
                        Err(CommandError::InvalidParams {
                            message: format!("this model has {} channels", self.channels),
                        }),
                    );
                    return;
                }
                let muted = params.get("muted").and_then(Value::as_bool).unwrap_or(true);
                // "Per SSCv2, a write is a PUT of that resource carrying only the
                // properties to change" (RFDeck, verified on OpenAPI 1.7).
                let path = format!("/api/channel/{}", channel - 1);
                let request = self.request(
                    "PUT",
                    &path,
                    Some(json!({"mute": muted})),
                    Some(MUTE_TIMEOUT),
                );
                self.send(cx, Purpose::Mute { command: id }, request);
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
        let status = result.as_ref().ok().map(|r| r.status);
        let body = result
            .as_ref()
            .ok()
            .and_then(|r| serde_json::from_slice::<Value>(&r.body).ok());

        match purpose {
            Purpose::Probe => {
                if self.phase != Phase::Probing {
                    return;
                }
                match (status, &body) {
                    (Some(401), _) => self.lost(
                        cx,
                        Connection::Unauthorized {
                            reason: "the device rejected the third-party password".into(),
                        },
                        UNAUTHORIZED_RETRY_AFTER,
                    ),
                    // A JSON body is what separates an SSCv2 device from any
                    // other HTTPS server at the address.
                    (Some(s), Some(Value::Object(_))) if is_success(s) => {
                        self.phase = Phase::Streaming;
                        self.last_activity = cx.now();
                        let request = self.request(
                            "GET",
                            "/api/device/identity",
                            None,
                            Some(REQUEST_TIMEOUT),
                        );
                        self.send(cx, Purpose::Identity, request);
                        self.open_stream(cx);
                    }
                    (Some(s), _) => self.lost(
                        cx,
                        Connection::Disconnected {
                            reason: format!("not an SSCv2 device (HTTP {s})"),
                        },
                        RETRY_AFTER,
                    ),
                    (None, _) => {
                        let reason = result.err().unwrap_or_default();
                        self.lost(cx, Connection::Disconnected { reason }, RETRY_AFTER);
                    }
                }
            }
            Purpose::Identity => {
                if let (Some(s), Some(identity)) = (status, body) {
                    if is_success(s) {
                        self.apply(cx, &json!({"/api/device/identity": identity}));
                    }
                }
            }
            Purpose::Subscribe { remaining } => match status {
                Some(s) if is_success(s) => self.subscribe_next(cx, remaining),
                _ => {
                    let detail = status
                        .map(|s| format!("HTTP {s}"))
                        .unwrap_or_else(|| result.err().unwrap_or_default());
                    cx.log(Level::Warning, format!("subscription refused: {detail}"));
                }
            },
            Purpose::Initial { path } => match (status, body) {
                (Some(s), Some(value)) if is_success(s) => self.apply(cx, &json!({ path: value })),
                // No transmitter linked: the battery resource answers 422.
                (Some(422), _) if path.contains("/transmitters/") => {
                    let id: u32 = path
                        .split('/')
                        .nth(3)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    cx.state(json!({"channels": {(id + 1).to_string(): {"transmitter": null}}}));
                }
                (Some(404), _) if path.starts_with("/api/channel/") => cx.log(
                    Level::Warning,
                    format!("{path} does not exist; the device has fewer channels than this model"),
                ),
                _ => {}
            },
            Purpose::Liveness => {
                self.liveness_in_flight = false;
                if status.is_some() {
                    self.heard(cx);
                } else {
                    self.strikes += 1;
                    if self.strikes >= LIVENESS_STRIKES {
                        self.lost(
                            cx,
                            Connection::Disconnected {
                                reason: "unreachable behind a silent stream".into(),
                            },
                            RETRY_AFTER,
                        );
                    }
                }
            }
            Purpose::Mute { command } => {
                let outcome = match (status, result) {
                    (Some(s), _) if is_success(s) => Ok(Outcome::Ack),
                    (Some(401), _) => Err(CommandError::Auth {
                        message: "the device rejected the third-party password".into(),
                    }),
                    (Some(s), Ok(response)) => Err(CommandError::DeviceError {
                        code: Some(s.to_string()),
                        message: String::from_utf8_lossy(&response.body).into_owned(),
                    }),
                    (_, Err(message)) => Err(CommandError::Transport { message }),
                    (None, Ok(_)) => unreachable!("a response always has a status"),
                };
                cx.complete(command, outcome);
            }
        }
    }

    fn sse(&mut self, cx: &mut Cx, _stream: Key, input: SseInput) {
        match input {
            SseInput::Opened => {
                self.heard(cx);
                if !self.connected {
                    self.connected = true;
                    cx.connection(Connection::Connected);
                }
                cx.set_timer(LIVENESS, LIVENESS_CHECK_EVERY);
            }
            SseInput::Activity => self.heard(cx),
            SseInput::Event(event) => {
                let Ok(data) = serde_json::from_str::<Value>(&event.data) else {
                    return;
                };
                if self.session.is_none() {
                    let uuid = data
                        .get("sessionUUID")
                        .or_else(|| data.get("session_uuid"))
                        .and_then(Value::as_str);
                    if let Some(uuid) = uuid {
                        self.session = Some(uuid.to_string());
                        let batches = self
                            .resources()
                            .chunks(SUBSCRIBE_BATCH)
                            .map(|c| c.to_vec())
                            .collect();
                        self.subscribe_next(cx, batches);
                        self.fetch_initial(cx);
                        return;
                    }
                }
                if event.event != "open" && event.event != "close" {
                    self.apply(cx, &data);
                }
            }
            SseInput::Closed { status, reason } => {
                let connection = match status {
                    Some(401) => Connection::Unauthorized {
                        reason: "the device rejected the third-party password".into(),
                    },
                    Some(403) => Connection::Unauthorized {
                        reason: "the password is not allowed to subscribe".into(),
                    },
                    _ => Connection::Disconnected {
                        reason: format!("subscription stream: {reason}"),
                    },
                };
                let retry = if status.is_some_and(|s| s == 401 || s == 403) {
                    UNAUTHORIZED_RETRY_AFTER
                } else {
                    RETRY_AFTER
                };
                self.lost(cx, connection, retry);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.probe(cx),
            LIVENESS => {
                if self.phase != Phase::Streaming {
                    return;
                }
                let quiet = cx.now().saturating_sub(self.last_activity);
                if quiet >= QUIET_AFTER && !self.liveness_in_flight {
                    self.liveness_in_flight = true;
                    let request =
                        self.request("GET", "/api/ssc/version", None, Some(REQUEST_TIMEOUT));
                    self.send(cx, Purpose::Liveness, request);
                }
                cx.set_timer(LIVENESS, LIVENESS_CHECK_EVERY);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.sse_close(STREAM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Action, CommandResult};
    use crate::sse::SseEvent;
    use std::net::Ipv4Addr;

    fn device() -> Ewdx {
        Ewdx::for_device(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)), 443, "secret", 2)
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

    fn ok(body: Value) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status: 200,
            body: body.to_string().into_bytes(),
        })
    }

    fn status(code: u16) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status: code,
            body: Vec::new(),
        })
    }

    fn completed(actions: &[Action]) -> Vec<(CommandId, CommandResult)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Complete { id, result } => Some((*id, result.clone())),
                _ => None,
            })
            .collect()
    }

    /// Start, answer the probe, open the stream and deliver the session id.
    fn streaming() -> (Ewdx, Vec<Action>) {
        let mut d = device();
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(
            &mut cx,
            probe,
            ok(json!({"protocol": "2.0", "schema": "1.5"})),
        );
        cx.take();
        let mut cx = Cx::new(20);
        d.sse(&mut cx, STREAM, SseInput::Opened);
        d.sse(
            &mut cx,
            STREAM,
            SseInput::Event(SseEvent {
                event: "open".into(),
                data: json!({"path": "/api/ssc/state/subscriptions/abc", "sessionUUID": "abc"})
                    .to_string(),
            }),
        );
        let actions = cx.take();
        (d, actions)
    }

    #[test]
    fn requests_are_authenticated_as_api() {
        let mut d = device();
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let (_, probe) = requests(&cx.take()).remove(0);
        assert_eq!(probe.url, "https://10.0.0.5:443/api/ssc/version");
        // base64("api:secret")
        assert!(probe
            .headers
            .contains(&("Authorization".into(), "Basic YXBpOnNlY3JldA==".into())));
        assert!(probe.accept_invalid_certs);
    }

    #[test]
    fn a_rejected_password_is_unauthorized_and_retried_slowly() {
        let mut d = device();
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(&mut cx, probe, status(401));
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: UNAUTHORIZED_RETRY_AFTER
        }));
    }

    #[test]
    fn subscribes_every_channel_in_batches_after_the_session_id() {
        let (mut d, actions) = streaming();
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        let reqs = requests(&actions);
        let (first_id, first) = &reqs[0];
        assert_eq!(first.method, "PUT");
        assert_eq!(
            first.url,
            "https://10.0.0.5:443/api/ssc/state/subscriptions/abc/add"
        );
        let body: Value = serde_json::from_slice(first.body.as_ref().unwrap()).unwrap();
        assert_eq!(body.as_array().unwrap().len(), 4);

        // The next batch follows the first's success, until all twelve paths
        // (six per channel, two channels) are subscribed.
        let mut id = *first_id;
        let mut total = 4;
        loop {
            let mut cx = Cx::new(30);
            d.http_response(&mut cx, id, status(200));
            let next: Vec<_> = requests(&cx.take())
                .into_iter()
                .filter(|(_, r)| r.method == "PUT")
                .collect();
            let Some((next_id, req)) = next.into_iter().next() else {
                break;
            };
            let body: Value = serde_json::from_slice(req.body.as_ref().unwrap()).unwrap();
            total += body.as_array().unwrap().len();
            id = next_id;
        }
        assert_eq!(total, 12);
    }

    #[test]
    fn notifications_map_onto_channels() {
        let (d, _) = streaming();
        let mut cx = Cx::new(40);
        d.apply(
            &mut cx,
            &json!({
                "/api/channel/1": {"name": "Vox", "mute": false},
                "/api/channel/1/signalQualityIndicator": {"value": 87},
                "/api/channel/1/level": {"value": -42.5},
                "/api/rf/channels/1": {"frequency": 606500},
                "/api/transmitters/1/battery": {"gauge": 65},
            }),
        );
        let patch = cx.take().into_iter().find_map(|a| match a {
            Action::State(p) => Some(p),
            _ => None,
        });
        let ch = &patch.unwrap()["channels"]["2"];
        assert_eq!(ch["name"], "Vox");
        assert_eq!(ch["rf"]["quality_pct"], 87);
        assert_eq!(ch["af"]["level_dbfs"], -42.5);
        assert_eq!(ch["frequency_khz"], 606500);
        // A percentage, per EW-DX SSC §8.106 (RFDeck review item A).
        assert_eq!(ch["transmitter"]["battery_percent"], 65);
    }

    #[test]
    fn path_value_notifications_are_accepted() {
        let (d, _) = streaming();
        let mut cx = Cx::new(40);
        d.apply(
            &mut cx,
            &json!({"path": "/api/channel/0", "value": {"mute": true}}),
        );
        let patch = cx.take().into_iter().find_map(|a| match a {
            Action::State(p) => Some(p),
            _ => None,
        });
        assert_eq!(patch.unwrap()["channels"]["1"]["mute"], true);
    }

    #[test]
    fn mute_puts_the_channel_resource() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(50);
        let params = json!({"channel": 2, "muted": true})
            .as_object()
            .unwrap()
            .clone();
        d.command(&mut cx, 9, "mute", &params);
        let (id, req) = requests(&cx.take()).remove(0);
        assert_eq!(req.method, "PUT");
        assert_eq!(req.url, "https://10.0.0.5:443/api/channel/1");
        assert_eq!(req.body.as_deref(), Some(br#"{"mute":true}"#.as_slice()));

        let mut cx = Cx::new(60);
        d.http_response(&mut cx, id, status(200));
        assert_eq!(completed(&cx.take()), [(9, Ok(Outcome::Ack))]);
    }

    #[test]
    fn mute_failures_are_classified() {
        let (mut d, _) = streaming();
        let params = json!({"channel": 1}).as_object().unwrap().clone();
        let mut cx = Cx::new(50);
        d.command(&mut cx, 1, "mute", &params);
        d.command(&mut cx, 2, "mute", &params);
        let ids: Vec<_> = requests(&cx.take()).into_iter().map(|(i, _)| i).collect();
        let mut cx = Cx::new(60);
        d.http_response(&mut cx, ids[0], status(404));
        d.http_response(&mut cx, ids[1], Err("connection reset".into()));
        let done = completed(&cx.take());
        assert!(
            matches!(&done[0].1, Err(CommandError::DeviceError { code: Some(c), .. }) if c == "404")
        );
        assert!(matches!(&done[1].1, Err(CommandError::Transport { .. })));
    }

    #[test]
    fn a_quiet_stream_is_probed_and_dropped_after_two_failures() {
        let (mut d, _) = streaming();
        let mut failures = 0;
        for now in [3_100, 6_200] {
            let mut cx = Cx::new(now);
            d.timer(&mut cx, LIVENESS);
            let (id, req) = requests(&cx.take()).remove(0);
            assert!(req.url.ends_with("/api/ssc/version"));
            let mut cx = Cx::new(now + 10);
            d.http_response(&mut cx, id, Err("timed out".into()));
            failures += 1;
            let a = cx.take();
            let dropped = a.contains(&Action::SseClose { stream: STREAM });
            assert_eq!(dropped, failures == 2);
        }
    }

    #[test]
    fn a_closed_stream_reconnects() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(70);
        d.sse(
            &mut cx,
            STREAM,
            SseInput::Closed {
                status: None,
                reason: "stream ended".into(),
            },
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_AFTER
        }));
        // The last known state stays; the connection status says it is stale.
        assert!(!a.iter().any(|x| matches!(x, Action::State(_))));
    }
}
