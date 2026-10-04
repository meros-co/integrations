//! The Sennheiser Sound Control Protocol v2 (SSCv2) client shared by the
//! Sennheiser devices that speak it (EW-DX, Spectera): HTTPS requests
//! authenticated as user `api`, plus a server-sent event stream whose
//! subscriptions are added with the session id the device issues when the
//! stream opens.
//!
//! The client does what is the same on every SSCv2 device:
//!
//! - it finds the device with `GET /api/ssc/version` (a JSON body is what
//!   separates an SSCv2 device from any other HTTPS server at the address);
//! - it treats a 401 or 403 on any request or on the stream as a terminal
//!   refusal of the credential, after which nothing is sent until the device
//!   is opened again;
//! - when monitored, it reads the device's identity, opens the stream at
//!   `/api/ssc/state/subscriptions`, takes the session id from the first
//!   event, adds the device's resources in batches of four with
//!   `PUT .../{session}/add`, and reads them once;
//! - where the device asks for it, it reads a few resources again at an
//!   interval rather than subscribing to them (meters that would otherwise
//!   arrive many times a second);
//! - it checks a stream that has been quiet for 3 s by asking the version
//!   again, and takes two failed checks in a row as a lost connection.
//!
//! What differs per device (which resources there are, how they map onto
//! state, and which commands write what) is an [`SscDevice`].
//!
//! Opened for commands only, it opens no subscription stream (so takes none
//! of the device's subscription sessions) and reads nothing on connecting:
//! the version request that finds the device, repeated whenever it has been
//! quiet for 3 s, is the liveness check.

use std::collections::HashMap;
use std::net::IpAddr;

use base64::Engine;
use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, HttpRequest, HttpResponse, Key, Level, Millis, Module,
    Outcome, RequestId, SseInput,
};

pub(crate) const STREAM: Key = "subscriptions";

pub(crate) const REQUEST_TIMEOUT: Millis = 2_000;
const SUBSCRIBE_TIMEOUT: Millis = 5_000;
/// Retry after an unreachable device or a dropped stream.
pub(crate) const RETRY_AFTER: Millis = 1_000;
/// Liveness: a stream quiet this long is checked with a request.
pub(crate) const QUIET_AFTER: Millis = 3_000;
const LIVENESS_CHECK_EVERY: Millis = 1_000;
const LIVENESS_STRIKES: u32 = 2;
/// RFDeck subscribes in batches of four, which is what was tested on EW-DX
/// hardware.
pub(crate) const SUBSCRIBE_BATCH: usize = 4;

pub(crate) const RETRY: Key = "retry";
pub(crate) const LIVENESS: Key = "liveness";
pub(crate) const POLL: Key = "poll";

const VERSION: &str = "/api/ssc/version";
const SUBSCRIPTIONS: &str = "/api/ssc/state/subscriptions";

/// What a command's successful answer is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Returns {
    /// Any 2xx acknowledges it.
    Ack,
    /// The JSON body of the 2xx answer is the command's value.
    Value,
}

/// One request a command makes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Call {
    pub method: &'static str,
    pub path: String,
    pub body: Option<Value>,
    pub timeout: Millis,
    pub returns: Returns,
}

impl Call {
    /// A write: a PUT of the resource carrying only the properties to change,
    /// as SSCv2 writes are made.
    pub(crate) fn put(path: impl Into<String>, body: Value, timeout: Millis) -> Call {
        Call {
            method: "PUT",
            path: path.into(),
            body: Some(body),
            timeout,
            returns: Returns::Ack,
        }
    }

    pub(crate) fn get(path: impl Into<String>) -> Call {
        Call {
            method: "GET",
            path: path.into(),
            body: None,
            timeout: REQUEST_TIMEOUT,
            returns: Returns::Value,
        }
    }
}

/// What differs between SSCv2 devices.
pub(crate) trait SscDevice: Send + 'static {
    /// Resources read once the device is found, before the stream opens.
    fn identity_reads(&self) -> Vec<String> {
        vec!["/api/device/identity".into()]
    }
    /// Resources subscribed once the stream has a session.
    fn resources(&self) -> Vec<String>;
    /// Resources read once the stream has a session (by default, every
    /// subscribed one).
    fn initial_reads(&self) -> Vec<String> {
        self.resources()
    }
    /// Map one resource's value onto the state patch.
    fn apply_resource(&self, path: &str, value: &Value, patch: &mut Map<String, Value>);
    /// Resources read again at an interval while monitored, for values the
    /// device would otherwise push too often (meters). None by default.
    fn poll(&self) -> Option<(Vec<String>, Millis)> {
        None
    }
    /// Whether the batches after a refused one are still added (for a device
    /// whose firmware versions differ in the resources they have). By
    /// default a refused batch ends the subscribing.
    fn subscribe_past_refusals(&self) -> bool {
        false
    }
    /// A read answered with something other than 2xx (and not a refusal).
    fn read_failed(&self, _cx: &mut Cx, _path: &str, _status: u16) {}
    /// The request a command makes, or why it cannot be made.
    fn command(&self, name: &str, params: &Params) -> Result<Call, CommandError>;
}

#[derive(Debug)]
enum Purpose {
    Probe,
    Read { path: String },
    Poll { path: String },
    Subscribe { remaining: Vec<Vec<String>> },
    Liveness,
    Command { command: CommandId, call: Call },
}

#[derive(Debug, PartialEq)]
enum Phase {
    Idle,
    Probing,
    /// The device answered: streaming, or for commands only, ready.
    Ready,
}

pub(crate) struct Sscv2<D: SscDevice> {
    pub(crate) device: D,
    base: String,
    authorization: String,
    /// Stream and read state; false for commands only.
    pub(crate) monitor: bool,
    phase: Phase,
    connected: bool,
    session: Option<String>,
    next_request: RequestId,
    /// Requests in flight: why each was made and when it went.
    requests: HashMap<RequestId, (Purpose, Millis)>,
    last_activity: Millis,
    liveness_in_flight: bool,
    /// Poll reads not yet answered; a new round waits for them.
    polls_in_flight: usize,
    strikes: u32,
    /// Set once the device refuses the credential. Terminal: nothing is sent
    /// again. A refused password is never retried on any schedule, because
    /// repeated failed authentications have locked an EW-DX out until it was
    /// re-adopted in Control Cockpit (RFDeck review item O). The host
    /// re-opens the device with a corrected password.
    refused: Option<String>,
}

impl<D: SscDevice> Sscv2<D> {
    /// A client authenticating as user `api`, the SSCv2 third-party user.
    pub(crate) fn new(host: IpAddr, port: u16, password: &str, device: D) -> Sscv2<D> {
        Sscv2::new_as(host, port, "api", password, device)
    }

    /// A client authenticating as the given user.
    pub(crate) fn new_as(
        host: IpAddr,
        port: u16,
        user: &str,
        password: &str,
        device: D,
    ) -> Sscv2<D> {
        let host = match host {
            IpAddr::V6(v6) => format!("[{v6}]"),
            v4 => v4.to_string(),
        };
        let credentials =
            base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
        Sscv2 {
            device,
            base: format!("https://{host}:{port}"),
            authorization: format!("Basic {credentials}"),
            monitor: true,
            phase: Phase::Idle,
            connected: false,
            session: None,
            next_request: 1,
            requests: HashMap::new(),
            last_activity: 0,
            liveness_in_flight: false,
            polls_in_flight: 0,
            strikes: 0,
            refused: None,
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
            digest: None,
        }
    }

    fn send(&mut self, cx: &mut Cx, purpose: Purpose, request: HttpRequest) {
        let id = self.next_request;
        self.next_request += 1;
        self.requests.insert(id, (purpose, cx.now()));
        cx.http(id, request);
    }

    fn probe(&mut self, cx: &mut Cx) {
        self.phase = Phase::Probing;
        let request = self.request("GET", VERSION, None, Some(REQUEST_TIMEOUT));
        self.send(cx, Purpose::Probe, request);
    }

    fn stop_everything(&mut self, cx: &mut Cx) {
        if self.monitor {
            cx.sse_close(STREAM);
        }
        cx.cancel_timer(LIVENESS);
        cx.cancel_timer(RETRY);
        if self.device.poll().is_some() {
            cx.cancel_timer(POLL);
        }
        self.polls_in_flight = 0;
        self.phase = Phase::Idle;
        self.connected = false;
        self.session = None;
        self.strikes = 0;
        self.liveness_in_flight = false;
    }

    /// The device is unreachable or the stream dropped: try again shortly.
    pub(crate) fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.stop_everything(cx);
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, RETRY_AFTER);
    }

    /// The device refused the credential: stop, and never present it again.
    fn refuse(&mut self, cx: &mut Cx, reason: &str) {
        if self.refused.is_some() {
            return;
        }
        self.stop_everything(cx);
        self.refused = Some(reason.to_string());
        cx.log(
            Level::Warning,
            format!("{reason}; no further requests until the device is opened again with a corrected password"),
        );
        cx.connection(Connection::Unauthorized {
            reason: reason.into(),
        });
    }

    fn open_stream(&mut self, cx: &mut Cx) {
        let mut request = self.request("GET", SUBSCRIPTIONS, None, None);
        request
            .headers
            .push(("Accept".into(), "text/event-stream".into()));
        request
            .headers
            .push(("Cache-Control".into(), "no-cache".into()));
        cx.sse_open(STREAM, request);
    }

    fn subscribe_next(&mut self, cx: &mut Cx, mut remaining: Vec<Vec<String>>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        if remaining.is_empty() {
            return;
        }
        let batch = remaining.remove(0);
        let path = format!("{SUBSCRIPTIONS}/{session}/add");
        let request = self.request("PUT", &path, Some(json!(batch)), Some(SUBSCRIBE_TIMEOUT));
        self.send(cx, Purpose::Subscribe { remaining }, request);
    }

    fn read(&mut self, cx: &mut Cx, path: String) {
        let request = self.request("GET", &path, None, Some(REQUEST_TIMEOUT));
        self.send(cx, Purpose::Read { path }, request);
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.last_activity = cx.now();
        self.strikes = 0;
        cx.alive();
    }

    /// Apply one notification: `{"<resource path>": {...}}` per SSCv2 §3.4.8,
    /// or the `{"path": ..., "value": ...}` form RFDeck also handles.
    pub(crate) fn apply(&self, cx: &mut Cx, data: &Value) {
        let mut patch = Map::new();
        match data {
            Value::Object(map) if map.contains_key("path") && map.contains_key("value") => {
                if let Some(path) = map["path"].as_str() {
                    self.device.apply_resource(path, &map["value"], &mut patch);
                }
            }
            Value::Object(map) => {
                for (path, value) in map {
                    self.device.apply_resource(path, value, &mut patch);
                }
            }
            _ => {}
        }
        if !patch.is_empty() {
            cx.state(Value::Object(patch));
        }
    }

    fn fail_command(cx: &mut Cx, purpose: Purpose, message: &str) {
        if let Purpose::Command { command, .. } = purpose {
            cx.complete(
                command,
                Err(CommandError::Auth {
                    message: message.into(),
                }),
            );
        }
    }
}

fn is_success(status: u16) -> bool {
    (200..300).contains(&status)
}

/// A credential refusal, from any request or the stream. 403 is a refusal too:
/// the password authenticated but is not allowed to do what the core needs.
fn refusal(status: Option<u16>) -> Option<&'static str> {
    match status {
        Some(401) => Some("the device rejected the third-party password"),
        Some(403) => Some("the third-party password is not allowed to use this API"),
        _ => None,
    }
}

/// The child object at `key` of a state patch, created when absent.
pub(crate) fn object<'a>(
    patch: &'a mut Map<String, Value>,
    key: &str,
) -> &'a mut Map<String, Value> {
    if !patch.get(key).is_some_and(Value::is_object) {
        patch.insert(key.into(), json!({}));
    }
    patch.get_mut(key).and_then(Value::as_object_mut).unwrap()
}

impl<D: SscDevice> Module for Sscv2<D> {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.probe(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if let Some(reason) = &self.refused {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: reason.clone(),
                }),
            );
            return;
        }
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.device.command(name, params) {
            Ok(call) => {
                let request = self.request(
                    call.method,
                    &call.path,
                    call.body.clone(),
                    Some(call.timeout),
                );
                self.send(cx, Purpose::Command { command: id, call }, request);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let Some((purpose, sent)) = self.requests.remove(&id) else {
            return;
        };
        if let Some(reason) = self.refused.clone() {
            Self::fail_command(cx, purpose, &reason);
            return;
        }
        // Every request is answered directly; a transport failure is no answer.
        if result.is_ok() {
            cx.round_trip(cx.now().saturating_sub(sent));
        }
        let status = result.as_ref().ok().map(|r| r.status);
        if let Some(reason) = refusal(status) {
            Self::fail_command(cx, purpose, reason);
            self.refuse(cx, reason);
            return;
        }
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
                    // A JSON body is what separates an SSCv2 device from any
                    // other HTTPS server at the address.
                    (Some(s), Some(Value::Object(_))) if is_success(s) && !self.monitor => {
                        // Commands only: no stream, no reads. The liveness
                        // check keeps asking the version while it is quiet.
                        self.phase = Phase::Ready;
                        self.heard(cx);
                        self.connected = true;
                        cx.connection(Connection::Connected);
                        cx.set_timer(LIVENESS, LIVENESS_CHECK_EVERY);
                    }
                    (Some(s), Some(Value::Object(_))) if is_success(s) => {
                        self.phase = Phase::Ready;
                        self.last_activity = cx.now();
                        for path in self.device.identity_reads() {
                            self.read(cx, path);
                        }
                        self.open_stream(cx);
                    }
                    (Some(s), _) => self.lost(cx, format!("not an SSCv2 device (HTTP {s})")),
                    (None, _) => {
                        let reason = result.err().unwrap_or_default();
                        self.lost(cx, reason);
                    }
                }
            }
            Purpose::Read { path } => match (status, body) {
                (Some(s), Some(value)) if is_success(s) => self.apply(cx, &json!({ path: value })),
                (Some(s), _) if !is_success(s) => self.device.read_failed(cx, &path, s),
                _ => {}
            },
            Purpose::Poll { path } => {
                self.polls_in_flight = self.polls_in_flight.saturating_sub(1);
                if let (Some(s), Some(value)) = (status, body) {
                    if is_success(s) {
                        self.apply(cx, &json!({ path: value }));
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
                    // A device answering a path it lacks with 400 has
                    // subscribed the batch's paths before it; the later
                    // batches can still be added.
                    if status.is_some() && self.device.subscribe_past_refusals() {
                        self.subscribe_next(cx, remaining);
                    }
                }
            },
            Purpose::Liveness => {
                self.liveness_in_flight = false;
                // Any answer proves the device is there; a refusal never
                // reaches this point.
                if status.is_some() {
                    self.heard(cx);
                } else {
                    self.strikes += 1;
                    if self.strikes >= LIVENESS_STRIKES {
                        self.lost(cx, "unreachable behind a silent stream".into());
                    }
                }
            }
            Purpose::Command { command, call } => {
                let outcome = match (status, result) {
                    (Some(s), _) if is_success(s) => match call.returns {
                        Returns::Ack => Ok(Outcome::Ack),
                        Returns::Value => {
                            let value = body.unwrap_or(Value::Null);
                            // A read carries the resource: keep state current
                            // with it too.
                            if call.method == "GET" && !value.is_null() {
                                self.apply(cx, &json!({ call.path: value.clone() }));
                            }
                            Ok(Outcome::Value { value })
                        }
                    },
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
        if self.refused.is_some() {
            return;
        }
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
                            .device
                            .resources()
                            .chunks(SUBSCRIBE_BATCH)
                            .map(|c| c.to_vec())
                            .collect();
                        self.subscribe_next(cx, batches);
                        for path in self.device.initial_reads() {
                            self.read(cx, path);
                        }
                        if let Some((_, every)) = self.device.poll() {
                            cx.set_timer(POLL, every);
                        }
                        return;
                    }
                }
                if event.event != "open" && event.event != "close" {
                    self.apply(cx, &data);
                }
            }
            SseInput::Closed { status, reason } => match refusal(status) {
                Some(refused) => self.refuse(cx, refused),
                None => self.lost(cx, format!("subscription stream: {reason}")),
            },
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY if self.refused.is_none() => self.probe(cx),
            POLL if self.refused.is_none() && self.phase == Phase::Ready => {
                let Some((paths, every)) = self.device.poll() else {
                    return;
                };
                // A slow device is not asked again until it has answered.
                if self.polls_in_flight == 0 {
                    for path in paths {
                        self.polls_in_flight += 1;
                        let request = self.request("GET", &path, None, Some(REQUEST_TIMEOUT));
                        self.send(cx, Purpose::Poll { path }, request);
                    }
                }
                cx.set_timer(POLL, every);
            }
            LIVENESS => {
                if self.phase != Phase::Ready {
                    return;
                }
                let quiet = cx.now().saturating_sub(self.last_activity);
                if quiet >= QUIET_AFTER && !self.liveness_in_flight {
                    self.liveness_in_flight = true;
                    let request = self.request("GET", VERSION, None, Some(REQUEST_TIMEOUT));
                    self.send(cx, Purpose::Liveness, request);
                }
                cx.set_timer(LIVENESS, LIVENESS_CHECK_EVERY);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.monitor {
            cx.sse_close(STREAM);
        }
    }
}
