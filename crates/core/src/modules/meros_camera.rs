//! Meros Camera over its control protocol, version 1: HTTP/1.1 with JSON,
//! one request per connection, every path under `/v1/`, and the fleet key as
//! `Authorization: Bearer` on every request but the hello.
//!
//! Connecting reads `GET /v1/hello` (no key), which identifies the camera,
//! then `GET /v1/status`, the status document, into state. A status
//! answered 401 means the fleet key is missing or wrong: the camera is
//! reported unauthorized and asked nothing more until its settings change or
//! the consumer reconnects it. After the status come the `capabilities`
//! command (the ops the camera offers, its capability document, settings,
//! monitoring and outputs), the clip list, and `GET /v1/events`: the status
//! document as NDJSON, a line per change and the same line again at least
//! every 5 s. The event stream never ends, so it is read on a TCP connection
//! of its own with its HTTP response parsed here. When it ends, or 10 s pass
//! without a line, the status is read again and the stream reopened (§8);
//! when the camera does not answer, it is reported disconnected and tried
//! again after 1 s doubling to 30 s.
//!
//! Consecutive status documents are compared, and what changed between them
//! (recording started or stopped, tally, errors and warnings, battery, heat
//! and storage, outputs up or down, the operator's settings) is reported as
//! message events, since version 1 has no event types of its own. The
//! status's `monitoring` and `outputSettings` documents keep `monitoring`
//! and `output_settings` current whoever changed them; output credentials
//! (key, token, passphrase) are dropped wherever they arrive, leaving only
//! `keySet`, `tokenSet` or `passphraseSet`.
//!
//! The clip list is read again when the status's clip count changes or a
//! recording stops. Clips are read in byte ranges returned to the caller; the
//! consumer writes the files. The preview (`GET /v1/preview`) is opened only
//! while the `live` stream is watched, and each frame is published whole, as
//! the camera framed it.
//!
//! An op command is sent only once the camera's capabilities are known: it
//! fails before anything is sent when the camera does not list its op, and
//! a value outside the connected camera's ranges (its `limits`, and the
//! lens's ISO, exposure time, EV, Kelvin, zoom and video formats) fails
//! with invalid parameters, as the camera itself refuses out-of-range
//! output values. A command given before the capabilities are known waits
//! for them; when they cannot be read, it fails saying so.
//!
//! Opened for commands only (`monitor` false), the module reads the hello on
//! connecting and every 10 s as its liveness check, and reads no status or
//! clips and opens no event stream; the capabilities are read only when the
//! first op command needs them. What a command's answer carries is still
//! applied to state. Each request's time to its answer is reported as the
//! camera's latency.
//!
//! The fleet key is never logged and never put in state; it is in a request
//! header, and in a clip URL only as `clip_url`'s value. The talkback
//! address is a credential too (§6.6): state, logs and every answer show it
//! only as set (`talkback.url_set`, `"talkback": "set"`), and only
//! `get_talkback_url` returns it, for the director joining.

use std::collections::{BTreeSet, HashMap};
use std::net::{IpAddr, SocketAddr};

use serde_json::{json, Map, Value};

use super::meros_camera_wire::{self as wire, Frames, Lines, Piece, ResponseReader};
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, HttpRequest, HttpResponse, Key, Level, Millis, Module,
    OpenContext, Outcome, RequestId, TcpInput,
};

pub(crate) const PORT: u16 = 8790;
/// The stream this module publishes.
pub(crate) const LIVE: Key = "live";
/// The format of its frames: one frame of the preview stream, header and all.
pub(crate) const FRAME_FORMAT: &str = "meros-preview";

const EVENTS: Key = "events";
const PREVIEW: Key = "preview";

const RETRY: Key = "retry";
const EVENTS_SILENCE: Key = "events-silence";
const LIVENESS: Key = "liveness";
const PREVIEW_RETRY: Key = "preview-retry";

const HELLO_TIMEOUT: Millis = 3_000;
const STATUS_TIMEOUT: Millis = 5_000;
const CLIPS_TIMEOUT: Millis = 5_000;
/// The camera answers a command within 10 s or with 504.
const COMMAND_TIMEOUT: Millis = 12_000;
const CLIP_READ_TIMEOUT: Millis = 25_000;
/// §8: reconnect when 10 s pass without a line.
const SILENCE: Millis = 10_000;
/// Commands only: the hello is read this often to tell the camera is there.
const LIVENESS_EVERY: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// What `read_clip` reads when no length is given (the spec's default).
const DEFAULT_READ: u64 = 1 << 20;

/// Status document fields copied into state under the same name.
const SAME_NAME: &[&str] = &[
    "id",
    "name",
    "model",
    "recording",
    "clip",
    "error",
    "warnings",
    "tally",
    "lens",
    "lenses",
    "settings",
    "running",
    "resolved",
    "presets",
    "luts",
    "looks",
    "slate",
    "battery",
    "thermal",
    "network",
    "audio",
    "visca",
];

/// Output fields that hold a credential; the camera never sends them back,
/// and the module drops them from anything it keeps or returns, keeping
/// only whether one is set (`keySet`, `tokenSet`, `passphraseSet`).
const CREDENTIALS: &[&str] = &["key", "token", "passphrase"];

/// How the talkback address is shown wherever it would be shown (§6.6).
const SET: &str = "set";

/// The OMT output's qualities (its VMX profiles, §7.5), as §7.6 lists them.
const OMT_QUALITY: &[&str] = &["low", "standard", "high"];

/// The ranges outside the capability document (§7.6), as the protocol
/// gives them; a camera's own `limits` replace them once read.
fn default_limits() -> Value {
    json!({
        "outputs": {"heights": [1080, 720, 540], "bitrateMbps": {"min": 1, "max": 50},
                    "srtLatencyMs": {"min": 20, "max": 8000}, "port": {"min": 1024, "max": 65535},
                    "omtQuality": OMT_QUALITY},
        "monitoring": {"zebraLevel": {"min": 0.5, "max": 1}, "zebra2Level": {"min": 0.3, "max": 1},
                       "customGuide": {"min": 0.25, "max": 4}, "desqueeze": {"min": 1, "max": 2}},
        "recording": {"gainDb": {"min": -24, "max": 24}},
        "whiteBalance": {"tint": {"min": -100, "max": 100}}
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Hello,
    Status,
    Ready,
    /// Waiting for the retry timer.
    Waiting,
    /// The fleet key was refused.
    Refused,
}

#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    /// The connect-time hello, or (commands only) the liveness check.
    Hello,
    Status,
    /// The capabilities op: on connecting, for commands waiting for them,
    /// or for `get_capabilities`.
    Capabilities(Option<CommandId>),
    Clips,
    /// A command reading something.
    Read(CommandId, Read),
    /// A command carried out by `POST /v1/command`.
    Op(CommandId, String),
}

#[derive(Debug, Clone, PartialEq)]
enum Read {
    Hello,
    Status,
    /// The status, for its talkback address alone.
    TalkbackUrl,
    Clips,
    Clip {
        name: String,
        offset: u64,
        length: u64,
    },
}

struct InFlight {
    purpose: Purpose,
    sent: Millis,
    /// The connection attempt an automatic request belongs to; answers from
    /// an earlier one are ignored.
    generation: u64,
}

#[derive(Default)]
struct EventStream {
    reader: ResponseReader,
    lines: Lines,
    open: bool,
    /// A line arrived since it was opened.
    heard: bool,
}

#[derive(Default)]
struct PreviewStream {
    reader: ResponseReader,
    frames: Frames,
    open: bool,
}

pub(crate) struct MerosCamera {
    address: SocketAddr,
    /// `host:port` as URLs and `Host` name it.
    authority: String,
    key: String,
    monitor: bool,
    battery_low: f64,
    storage_low: f64,
    phase: Phase,
    generation: u64,
    next_id: RequestId,
    requests: HashMap<RequestId, InFlight>,
    retry_delay: Millis,
    preview_delay: Millis,
    connected: bool,
    last_reason: Option<String>,
    events: Option<EventStream>,
    preview: Option<PreviewStream>,
    watched: bool,
    /// Everything this module has put in state, to send only what changed.
    mirror: Value,
    last_status: Option<Value>,
    camera_id: Option<String>,
    /// The ops the camera offers, once its capabilities are read.
    ops: Option<Vec<String>>,
    /// A capabilities request is out.
    caps_in_flight: bool,
    /// Op commands waiting for the capabilities: their ids and bodies.
    pending: Vec<(CommandId, Value)>,
    clips_in_flight: bool,
    clips_again: bool,
}

fn text(s: &Params, name: &str) -> String {
    s.get(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

impl MerosCamera {
    pub(crate) fn new(ctx: OpenContext) -> MerosCamera {
        let s = &ctx.settings;
        let port = ctx.port.unwrap_or(PORT);
        let host = match (&ctx.host_name, ctx.host) {
            (Some(name), _) => name.clone(),
            (None, IpAddr::V4(ip)) => ip.to_string(),
            (None, IpAddr::V6(ip)) => format!("[{ip}]"),
        };
        let number =
            |name: &str, default: f64| s.get(name).and_then(Value::as_f64).unwrap_or(default);
        MerosCamera {
            address: SocketAddr::new(ctx.host, port),
            authority: format!("{host}:{port}"),
            key: text(s, "fleet_key"),
            monitor: ctx.monitor,
            battery_low: number("battery_low_percent", 20.0),
            storage_low: number("storage_low_gib", 10.0) * (1u64 << 30) as f64,
            phase: Phase::Hello,
            generation: 0,
            next_id: 1,
            requests: HashMap::new(),
            retry_delay: RETRY_MIN,
            preview_delay: RETRY_MIN,
            connected: false,
            last_reason: None,
            events: None,
            preview: None,
            watched: false,
            mirror: json!({}),
            last_status: None,
            camera_id: None,
            ops: None,
            caps_in_flight: false,
            pending: Vec::new(),
            clips_in_flight: false,
            clips_again: false,
        }
    }

    // --- requests ------------------------------------------------------------

    fn send(
        &mut self,
        cx: &mut Cx,
        path: &str,
        // A body is POSTed; without one the request is a GET.
        body: Option<&Value>,
        timeout: Millis,
        headers: Vec<(String, String)>,
        purpose: Purpose,
    ) {
        let id = self.next_id;
        self.next_id += 1;
        let method = if body.is_some() { "POST" } else { "GET" };
        let mut headers = headers;
        if !matches!(purpose, Purpose::Hello | Purpose::Read(_, Read::Hello)) {
            headers.push(("Authorization".into(), format!("Bearer {}", self.key)));
        }
        if body.is_some() {
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        cx.http(
            id,
            HttpRequest {
                method,
                url: format!("http://{}{path}", self.authority),
                headers,
                body: body.map(|b| b.to_string().into_bytes()),
                body_stream: None,
                timeout: Some(timeout),
                accept_invalid_certs: false,
                digest: None,
            },
        );
        self.requests.insert(
            id,
            InFlight {
                purpose,
                sent: cx.now(),
                generation: self.generation,
            },
        );
    }

    fn hello(&mut self, cx: &mut Cx, purpose: Purpose) {
        self.send(cx, "/v1/hello", None, HELLO_TIMEOUT, vec![], purpose);
    }

    fn command_op(&mut self, cx: &mut Cx, body: &Value, purpose: Purpose) {
        self.send(
            cx,
            "/v1/command",
            Some(body),
            COMMAND_TIMEOUT,
            vec![],
            purpose,
        );
    }

    // --- connection ------------------------------------------------------------

    /// Start a connection attempt: the hello, then (monitoring) the status.
    fn connect(&mut self, cx: &mut Cx) {
        self.generation += 1;
        self.phase = Phase::Hello;
        self.hello(cx, Purpose::Hello);
    }

    fn report_disconnected(&mut self, cx: &mut Cx, reason: String) {
        if self.connected || self.last_reason.as_ref() != Some(&reason) {
            cx.connection(Connection::Disconnected {
                reason: reason.clone(),
            });
        }
        self.connected = false;
        self.last_reason = Some(reason);
    }

    fn report_connected(&mut self, cx: &mut Cx) {
        if !self.connected {
            cx.connection(Connection::Connected);
        }
        self.connected = true;
        self.last_reason = None;
    }

    fn close_events(&mut self, cx: &mut Cx) {
        if self.events.take().is_some() {
            cx.tcp_close(EVENTS);
        }
        cx.cancel_timer(EVENTS_SILENCE);
    }

    fn close_preview(&mut self, cx: &mut Cx) {
        if self.preview.take().is_some() {
            cx.tcp_close(PREVIEW);
        }
        cx.cancel_timer(PREVIEW_RETRY);
    }

    /// The camera did not answer: report it and try again later.
    fn fail(&mut self, cx: &mut Cx, reason: String) {
        if self.phase == Phase::Refused {
            return;
        }
        self.close_events(cx);
        cx.cancel_timer(LIVENESS);
        self.generation += 1;
        self.phase = Phase::Waiting;
        self.report_disconnected(cx, reason);
        cx.set_timer(RETRY, self.retry_delay);
        self.retry_delay = (self.retry_delay * 2).min(RETRY_MAX);
    }

    /// The camera refused the fleet key: ask it nothing more.
    fn refuse(&mut self, cx: &mut Cx) {
        if self.phase == Phase::Refused {
            return;
        }
        self.close_events(cx);
        self.close_preview(cx);
        cx.cancel_timer(LIVENESS);
        cx.cancel_timer(RETRY);
        self.generation += 1;
        self.phase = Phase::Refused;
        self.connected = false;
        let reason = if self.key.is_empty() {
            "no fleet key is set: pair with the camera's fleet key".to_string()
        } else {
            "the camera refused the fleet key: pair again with its current key".to_string()
        };
        self.last_reason = Some(reason.clone());
        self.fail_pending(
            cx,
            CommandError::Auth {
                message: reason.clone(),
            },
        );
        cx.connection(Connection::Unauthorized { reason });
    }

    fn open_events(&mut self, cx: &mut Cx) {
        self.events = Some(EventStream::default());
        cx.tcp_open(EVENTS, self.address);
        cx.set_timer(EVENTS_SILENCE, SILENCE);
    }

    /// The event stream ended or went quiet (§8): read the status again,
    /// then reopen it.
    fn events_lost(&mut self, cx: &mut Cx, reason: &str) {
        let heard = self.events.as_ref().is_some_and(|e| e.heard);
        self.close_events(cx);
        if self.phase != Phase::Ready {
            return;
        }
        cx.log(
            Level::Info,
            format!("event stream: {reason}; reading the status again"),
        );
        if heard {
            self.phase = Phase::Status;
            self.send(
                cx,
                "/v1/status",
                None,
                STATUS_TIMEOUT,
                vec![],
                Purpose::Status,
            );
        } else {
            // Nothing came on it: wait before opening it again, so a camera
            // that ends it at once is not asked in a tight loop.
            self.phase = Phase::Waiting;
            cx.set_timer(RETRY, self.retry_delay);
            self.retry_delay = (self.retry_delay * 2).min(RETRY_MAX);
        }
    }

    fn open_preview(&mut self, cx: &mut Cx) {
        if self.phase == Phase::Refused || !self.watched {
            return;
        }
        self.preview = Some(PreviewStream::default());
        cx.tcp_open(PREVIEW, self.address);
    }

    fn preview_lost(&mut self, cx: &mut Cx, reason: &str) {
        if self.preview.take().is_some() {
            cx.tcp_close(PREVIEW);
        }
        if !self.watched || self.phase == Phase::Refused {
            return;
        }
        cx.log(
            Level::Info,
            format!(
                "preview: {reason}; opening it again in {} s",
                self.preview_delay / 1000
            ),
        );
        cx.set_timer(PREVIEW_RETRY, self.preview_delay);
        self.preview_delay = (self.preview_delay * 2).min(RETRY_MAX);
    }

    // --- state -----------------------------------------------------------------

    /// Put `values` (top-level state keys; null removes one) into state,
    /// sending only what changed.
    fn publish(&mut self, cx: &mut Cx, values: Map<String, Value>) {
        let mut patch = Map::new();
        for (k, v) in values {
            if let Some(d) = diff(self.mirror.get(&k), &v) {
                patch.insert(k, d);
            }
        }
        if patch.is_empty() {
            return;
        }
        let patch = Value::Object(patch);
        crate::session::merge_patch(&mut self.mirror, &patch);
        cx.state(patch);
    }

    fn publish_one(&mut self, cx: &mut Cx, key: &str, value: Value) {
        let mut m = Map::new();
        m.insert(key.into(), value);
        self.publish(cx, m);
    }

    fn apply_hello(&mut self, cx: &mut Cx, hello: &Value) {
        let id = hello.get("id").and_then(Value::as_str).map(String::from);
        if let (Some(old), Some(new)) = (&self.camera_id, &id) {
            if old != new {
                cx.log(
                    Level::Warning,
                    format!(
                        "a different camera (id {new}, was {old}) now answers at {}",
                        self.authority
                    ),
                );
                self.last_status = None;
                // Its ops and ranges are its own: read them again.
                self.ops = None;
            }
        }
        if id.is_some() {
            self.camera_id = id;
        }
        let protocol = hello.get("protocol").and_then(Value::as_u64);
        if protocol.is_some_and(|p| p != 1) {
            cx.log(
                Level::Warning,
                format!(
                    "the camera speaks protocol {}; this driver speaks version 1 (/v1/)",
                    protocol.unwrap_or(0)
                ),
            );
        }
        let mut m = Map::new();
        for k in ["id", "name", "model", "app", "protocol"] {
            if let Some(v) = hello.get(k) {
                m.insert(k.into(), v.clone());
            }
        }
        self.publish(cx, m);
    }

    fn apply_status(&mut self, cx: &mut Cx, doc: Value) {
        if !doc.is_object() {
            return;
        }
        // Credentials never kept, even from a camera that wrongly sends them.
        let mut doc = doc;
        if let Some(o) = doc.get("outputSettings").cloned() {
            doc["outputSettings"] = redact(&o);
        }
        let previous = self.last_status.take();
        if let Some(prev) = &previous {
            self.derive(cx, prev, &doc);
        }
        let view = status_view(&doc);
        self.publish(cx, view);
        if let Some(prev) = &previous {
            let count = |d: &Value| d.get("clips").and_then(Value::as_f64);
            let stopped = prev["recording"] == true && doc["recording"] != true;
            if self.monitor && (count(prev) != count(&doc) || stopped) {
                self.refresh_clips(cx);
            }
        }
        self.last_status = Some(doc);
    }

    /// Message events for what changed between two status documents.
    fn derive(&self, cx: &mut Cx, prev: &Value, doc: &Value) {
        let source = self.address;
        let say = |cx: &mut Cx, address: &str, args: Value| {
            cx.message(address, None, args, source);
        };
        let recording = |d: &Value| d["recording"] == true;
        match (recording(prev), recording(doc)) {
            (false, true) => say(cx, "record_started", json!({ "clip": doc["clip"] })),
            (true, false) => say(cx, "record_stopped", json!({ "clip": prev["clip"] })),
            (true, true) if prev["clip"] != doc["clip"] => {
                say(cx, "record_stopped", json!({ "clip": prev["clip"] }));
                say(cx, "record_started", json!({ "clip": doc["clip"] }));
            }
            _ => {}
        }
        for (field, address) in [
            ("tally", "tally_changed"),
            ("thermal", "thermal_changed"),
            ("lens", "lens_changed"),
        ] {
            if prev[field] != doc[field] {
                say(
                    cx,
                    address,
                    json!({ "from": prev[field], "to": doc[field] }),
                );
            }
        }
        let error = |d: &Value| {
            d.get("error")
                .and_then(Value::as_str)
                .filter(|e| !e.is_empty())
                .map(String::from)
        };
        match (error(prev), error(doc)) {
            (old, Some(new)) if old.as_ref() != Some(&new) => {
                cx.log(Level::Warning, format!("camera: {new}"));
                say(cx, "error", json!({ "error": new }));
            }
            (Some(old), None) => say(cx, "error_cleared", json!({ "error": old })),
            _ => {}
        }
        let set = |d: &Value, k: &str| -> BTreeSet<String> {
            d.get(k)
                .and_then(Value::as_array)
                .map(|a| a.iter().map(as_text).collect())
                .unwrap_or_default()
        };
        let (before, after) = (set(prev, "warnings"), set(doc, "warnings"));
        for w in after.difference(&before) {
            say(cx, "warning", json!({ "warning": w }));
        }
        for w in before.difference(&after) {
            say(cx, "warning_cleared", json!({ "warning": w }));
        }
        let number = |d: &Value, k: &str| d.get(k).and_then(Value::as_f64);
        if self.battery_low > 0.0 {
            if let (Some(a), Some(b)) = (number(prev, "battery"), number(doc, "battery")) {
                if a >= self.battery_low && (0.0..self.battery_low).contains(&b) {
                    say(cx, "battery_low", json!({ "battery": doc["battery"] }));
                }
            }
        }
        if self.storage_low > 0.0 {
            if let (Some(a), Some(b)) = (number(prev, "freeBytes"), number(doc, "freeBytes")) {
                if a >= self.storage_low && b < self.storage_low {
                    say(cx, "storage_low", json!({ "free_bytes": doc["freeBytes"] }));
                }
            }
        }
        let on = |d: &Value| -> BTreeSet<String> {
            d.pointer("/outputs/on")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(as_text).collect())
                .unwrap_or_default()
        };
        let (before, after) = (on(prev), on(doc));
        for kind in after.difference(&before) {
            say(cx, "output_up", json!({ "kind": kind }));
        }
        for kind in before.difference(&after) {
            say(cx, "output_down", json!({ "kind": kind }));
        }
        if let Some(problems) = doc.pointer("/outputs/problems").and_then(Value::as_object) {
            for (kind, problem) in problems {
                if prev.pointer(&format!("/outputs/problems/{kind}")) != Some(problem) {
                    say(
                        cx,
                        "output_problem",
                        json!({ "kind": kind, "problem": problem }),
                    );
                }
            }
        }
        let (before, after) = (talkback(prev), talkback(doc));
        if before["on"] != after["on"] || before["latch"] != after["latch"] {
            say(
                cx,
                "talkback_changed",
                json!({ "on": after["on"], "latch": after["latch"] }),
            );
        }
        let mut paths = Vec::new();
        changed_paths(prev.get("settings"), doc.get("settings"), "", &mut paths);
        // The monitoring and outputs documents (a camera from before they
        // were in the status has neither), under their state names.
        for (field, prefix) in [
            ("monitoring", "monitoring"),
            ("outputSettings", "output_settings"),
        ] {
            if let (Some(old), Some(new)) = (
                prev.get(field).filter(|v| v.is_object()),
                doc.get(field).filter(|v| v.is_object()),
            ) {
                changed_paths(Some(old), Some(new), prefix, &mut paths);
            }
        }
        if !paths.is_empty() {
            say(cx, "settings_changed", json!({ "paths": paths }));
        }
    }

    fn apply_capabilities(&mut self, cx: &mut Cx, answer: &Value) {
        if let Some(ops) = answer.get("ops").and_then(Value::as_array) {
            self.ops = Some(
                ops.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect(),
            );
        }
        let mut m = Map::new();
        for (from, to) in [
            ("ops", "ops"),
            ("protocol", "protocol"),
            ("capabilities", "capabilities"),
            ("settings", "settings"),
            ("resolved", "resolved_settings"),
            ("monitoring", "monitoring"),
            ("luts", "luts"),
            ("presets", "presets"),
        ] {
            if let Some(v) = answer.get(from) {
                m.insert(to.into(), v.clone());
            }
        }
        if let Some(outputs) = answer.get("outputs") {
            m.insert("output_settings".into(), redact(outputs));
        }
        // Absent from a camera that does not send them: §7.6's are used.
        m.insert(
            "limits".into(),
            answer
                .get("limits")
                .filter(|l| l.is_object())
                .cloned()
                .unwrap_or(Value::Null),
        );
        self.publish(cx, m);
    }

    // --- ranges ----------------------------------------------------------------

    /// The camera's limits (§7.6), or the protocol's until it gives its own.
    fn limits(&self) -> Value {
        self.mirror
            .get("limits")
            .filter(|l| l.is_object())
            .cloned()
            .unwrap_or_else(default_limits)
    }

    /// The capability document's entry for the lens a settings patch is for:
    /// the one it names, else the one in use.
    fn lens_caps(&self, patch: &Value) -> Option<&Value> {
        let id = patch
            .get("lensId")
            .or_else(|| self.mirror.pointer("/settings/lensId"))
            .or_else(|| self.mirror.get("lens"))
            .and_then(Value::as_str)?;
        self.mirror
            .pointer("/capabilities/lenses")?
            .as_array()?
            .iter()
            .find(|l| l.get("id").and_then(Value::as_str) == Some(id))
    }

    /// Refuse, before sending, a value outside the connected camera's ranges.
    fn check(&self, body: &Value) -> Result<(), CommandError> {
        let limits = self.limits();
        match body["op"].as_str() {
            Some("settings") => self.check_settings(&body["settings"], &limits),
            Some("monitoring") => {
                let m = &body["monitoring"];
                for k in ["zebraLevel", "zebra2Level", "customGuide", "desqueeze"] {
                    within(
                        m.get(k),
                        limits.pointer(&format!("/monitoring/{k}")),
                        k,
                        "the camera's",
                    )?;
                }
                Ok(())
            }
            Some("outputs") => check_outputs(&body["outputs"], &limits),
            _ => Ok(()),
        }
    }

    fn check_settings(&self, s: &Value, limits: &Value) -> Result<(), CommandError> {
        within(
            s.pointer("/recording/gainDb"),
            limits.pointer("/recording/gainDb"),
            "recording.gainDb",
            "the camera's",
        )?;
        within(
            s.pointer("/whiteBalance/tint"),
            limits.pointer("/whiteBalance/tint"),
            "whiteBalance.tint",
            "the camera's",
        )?;
        let Some(lens) = self.lens_caps(s) else {
            return Ok(());
        };
        let lens_range = |path: &str| lens.pointer(path).filter(|r| r.is_object());
        const LENS: &str = "this lens's";
        within(
            s.pointer("/exposure/iso"),
            lens_range("/exposure/iso"),
            "exposure.iso",
            LENS,
        )?;
        within(
            s.pointer("/exposure/ev"),
            lens_range("/exposure/ev"),
            "exposure.ev",
            LENS,
        )?;
        let kelvin = lens_range("/whiteBalance/kelvin")
            .cloned()
            .unwrap_or_else(|| json!({"min": 2000, "max": 10000}));
        within(
            s.pointer("/whiteBalance/kelvin"),
            Some(&kelvin),
            "whiteBalance.kelvin",
            LENS,
        )?;
        within(s.get("zoom"), lens_range("/zoom"), "zoom", LENS)?;
        // The shutter as an exposure time, in the lens's exposureTimeNs.
        if let Some(times) = lens_range("/exposure/exposureTimeNs") {
            let fps = s
                .get("fps")
                .or_else(|| self.mirror.pointer("/settings/fps"))
                .and_then(Value::as_f64)
                .filter(|f| *f > 0.0);
            let shutter = s.pointer("/exposure/shutter");
            let ns = match (
                shutter
                    .and_then(|x| x.get("seconds"))
                    .and_then(Value::as_f64),
                shutter.and_then(|x| x.get("angle")).and_then(Value::as_f64),
            ) {
                (Some(seconds), _) => Some((seconds * 1e9, format!("{seconds} s"))),
                (None, Some(angle)) => fps.map(|f| {
                    (
                        angle / 360.0 / f * 1e9,
                        format!("{angle}° at {f} fps ({} s)", angle / 360.0 / f),
                    )
                }),
                _ => None,
            };
            if let (Some((ns, what)), Some((min, max))) = (ns, range(times)) {
                if ns < min * 0.999 || ns > max * 1.001 {
                    return Err(invalid(format!(
                        "the shutter {what} is outside this lens's exposure times ({} to {} s)",
                        min / 1e9,
                        max / 1e9
                    )));
                }
            }
        }
        self.check_format(s, lens)
    }

    /// A format among the lens's video formats (or its high-speed ones), at
    /// one of that format's rates.
    fn check_format(&self, s: &Value, lens: &Value) -> Result<(), CommandError> {
        if !["width", "height", "fps"]
            .iter()
            .any(|k| s.get(*k).is_some())
        {
            return Ok(());
        }
        let current = |k: &str| {
            s.get(k)
                .or_else(|| self.mirror.pointer(&format!("/settings/{k}")))
                .and_then(Value::as_f64)
        };
        let formats: Vec<&Value> = ["videoFormats", "highSpeedFormats"]
            .iter()
            .filter_map(|k| lens.get(*k).and_then(Value::as_array))
            .flatten()
            .collect();
        if formats.is_empty() {
            return Ok(());
        }
        let (Some(w), Some(h)) = (current("width"), current("height")) else {
            return Ok(());
        };
        let same = |a: Option<f64>, b: f64| a.is_some_and(|a| (a - b).abs() < 0.01);
        let sized: Vec<&&Value> = formats
            .iter()
            .filter(|f| {
                same(f.get("width").and_then(Value::as_f64), w)
                    && same(f.get("height").and_then(Value::as_f64), h)
            })
            .collect();
        if sized.is_empty() {
            let mut sizes: Vec<String> = formats
                .iter()
                .map(|f| format!("{}x{}", f["width"], f["height"]))
                .collect();
            sizes.dedup();
            return Err(invalid(format!(
                "{w}x{h} is not one of this lens's video formats ({})",
                sizes.join(", ")
            )));
        }
        let Some(fps) = current("fps") else {
            return Ok(());
        };
        let rates: Vec<f64> = sized
            .iter()
            .flat_map(|f| {
                ["fps", "manualFps"]
                    .iter()
                    .filter_map(|k| f.get(*k).and_then(Value::as_array))
                    .flatten()
                    .filter_map(Value::as_f64)
                    .collect::<Vec<_>>()
            })
            .collect();
        if !rates.is_empty() && !rates.iter().any(|r| same(Some(fps), *r)) {
            let list: Vec<String> = rates.iter().map(|r| r.to_string()).collect();
            return Err(invalid(format!(
                "{fps} fps is not one of this lens's rates at {w}x{h} ({})",
                list.join(", ")
            )));
        }
        Ok(())
    }

    fn apply_clips(&mut self, cx: &mut Cx, list: &Value) {
        let Some(items) = list.as_array() else {
            return;
        };
        let mut clips = Map::new();
        for c in items {
            let Some(name) = c.get("name").and_then(Value::as_str) else {
                continue;
            };
            clips.insert(
                name.into(),
                json!({
                    "size": c.get("size").cloned().unwrap_or(Value::Null),
                    "modified": c.get("modified").cloned().unwrap_or(Value::Null),
                    "circled": c.get("circled").cloned().unwrap_or(Value::Null),
                }),
            );
        }
        self.publish_one(cx, "clips", Value::Object(clips));
    }

    /// What a command's answer carries, applied to state.
    fn apply_answer(&mut self, cx: &mut Cx, op: &str, answer: &Value) {
        let get = |k: &str| answer.get(k).cloned();
        match op {
            "capabilities" => self.apply_capabilities(cx, answer),
            "settings" => {
                if let Some(r) = get("resolved").filter(Value::is_object) {
                    self.publish_one(cx, "resolved_settings", r);
                }
            }
            "lens" => {
                if let Some(l) = get("lens").filter(|l| !l.is_null()) {
                    self.publish_one(cx, "lens", l);
                }
            }
            "record" => {
                if let Some(r) = get("recording").filter(Value::is_boolean) {
                    self.publish_one(cx, "recording", r);
                }
                if self.monitor && answer["files"].as_array().is_some_and(|f| !f.is_empty()) {
                    self.refresh_clips(cx);
                }
            }
            "slate" => {
                if let Some(s) = get("slate").filter(Value::is_object) {
                    self.publish_one(cx, "slate", s);
                }
            }
            "circle" => {
                let name = answer.get("name").and_then(Value::as_str);
                let circled = answer.get("circled").filter(|c| c.is_boolean());
                if let (Some(name), Some(circled)) = (name, circled) {
                    if let Some(mut clips) = self.mirror.get("clips").cloned() {
                        if clips.get(name).is_some() {
                            clips[name]["circled"] = circled.clone();
                            self.publish_one(cx, "clips", clips);
                        }
                    }
                }
            }
            "monitoring" => {
                if let Some(m) = get("monitoring").filter(Value::is_object) {
                    self.publish_one(cx, "monitoring", m);
                }
            }
            "look" => {
                if let Some(l) = get("looks").filter(Value::is_object) {
                    self.publish_one(cx, "looks", l);
                }
            }
            "outputs" => {
                if let Some(o) = get("outputs").filter(Value::is_object) {
                    self.publish_one(cx, "output_settings", redact(&o));
                }
                if let Some(p) = get("problems") {
                    let on = self
                        .mirror
                        .pointer("/outputs/on")
                        .cloned()
                        .unwrap_or(Value::Null);
                    let problems = if p.is_object() { p } else { json!({}) };
                    self.publish_one(cx, "outputs", json!({ "on": on, "problems": problems }));
                }
            }
            "tally" => {
                if let Some(t) = get("tally").filter(|t| !t.is_null()) {
                    self.publish_one(cx, "tally", t);
                }
            }
            "talkback" => {
                let doc = json!({
                    "talkback": get("url"),
                    "talkbackOn": get("on"),
                    "talkbackLatch": get("latch"),
                });
                self.publish_one(cx, "talkback", talkback(&doc));
            }
            "return" => {
                if let Some(r) = get("return").filter(Value::is_object) {
                    self.publish_one(cx, "return_settings", r);
                }
            }
            _ => {}
        }
    }

    fn refresh_clips(&mut self, cx: &mut Cx) {
        if self.clips_in_flight {
            self.clips_again = true;
            return;
        }
        self.clips_in_flight = true;
        self.send(cx, "/v1/clips", None, CLIPS_TIMEOUT, vec![], Purpose::Clips);
    }

    // --- commands --------------------------------------------------------------

    fn gate(&self, op: &str) -> Result<(), CommandError> {
        match &self.ops {
            Some(ops) if !ops.iter().any(|o| o == op) => Err(CommandError::DeviceError {
                code: Some("unsupported_op".into()),
                message: format!(
                    "this camera does not offer the '{op}' op (its capabilities list {})",
                    ops.join(", ")
                ),
            }),
            _ => Ok(()),
        }
    }

    fn read_clip(&mut self, cx: &mut Cx, id: CommandId, p: &Params) -> Result<(), CommandError> {
        let name = text(p, "name");
        let offset = p.get("offset").and_then(Value::as_u64).unwrap_or(0);
        let length = p
            .get("length")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_READ)
            .max(1);
        let last = offset
            .checked_add(length - 1)
            .ok_or_else(|| invalid("offset and length run past 2^64"))?;
        self.send(
            cx,
            &format!("/v1/clips/{}", wire::encode_segment(&name)),
            None,
            CLIP_READ_TIMEOUT,
            vec![("Range".into(), format!("bytes={offset}-{last}"))],
            Purpose::Read(
                id,
                Read::Clip {
                    name,
                    offset,
                    length,
                },
            ),
        );
        Ok(())
    }

    fn run(
        &mut self,
        cx: &mut Cx,
        id: CommandId,
        name: &str,
        p: &Params,
    ) -> Result<(), CommandError> {
        match name {
            "decode_fleet_link" => {
                let value = wire::decode_fleet_link(&text(p, "link")).map_err(invalid)?;
                cx.complete(id, Ok(Outcome::Value { value }));
                return Ok(());
            }
            "clip_url" => {
                if self.key.is_empty() {
                    return Err(invalid("no fleet key is set"));
                }
                let url = format!(
                    "http://{}/v1/clips/{}?key={}",
                    self.authority,
                    wire::encode_segment(&text(p, "name")),
                    self.key
                );
                cx.complete(
                    id,
                    Ok(Outcome::Value {
                        value: json!({ "url": url }),
                    }),
                );
                return Ok(());
            }
            _ => {}
        }
        if self.phase == Phase::Refused && name != "get_hello" {
            return Err(CommandError::Auth {
                message: self.last_reason.clone().unwrap_or_default(),
            });
        }
        match name {
            "get_hello" => {
                self.hello(cx, Purpose::Read(id, Read::Hello));
                return Ok(());
            }
            "get_status" => {
                self.send(
                    cx,
                    "/v1/status",
                    None,
                    STATUS_TIMEOUT,
                    vec![],
                    Purpose::Read(id, Read::Status),
                );
                return Ok(());
            }
            "list_clips" => {
                self.send(
                    cx,
                    "/v1/clips",
                    None,
                    CLIPS_TIMEOUT,
                    vec![],
                    Purpose::Read(id, Read::Clips),
                );
                return Ok(());
            }
            "read_clip" => return self.read_clip(cx, id, p),
            "get_talkback_url" => {
                self.send(
                    cx,
                    "/v1/status",
                    None,
                    STATUS_TIMEOUT,
                    vec![],
                    Purpose::Read(id, Read::TalkbackUrl),
                );
                return Ok(());
            }
            "get_capabilities" => {
                self.request_capabilities(cx, Some(id));
                return Ok(());
            }
            _ => {}
        }
        let body = op_body(name, p)?;
        if self.ops.is_none() {
            // Which ops it offers and its ranges are needed first.
            self.pending.push((id, body));
            if !self.caps_in_flight {
                self.request_capabilities(cx, None);
            }
            return Ok(());
        }
        self.dispatch(cx, id, body)
    }

    /// Send an op command the camera offers, its values within its ranges.
    fn dispatch(&mut self, cx: &mut Cx, id: CommandId, body: Value) -> Result<(), CommandError> {
        let op = body["op"].as_str().unwrap_or_default().to_string();
        self.gate(&op)?;
        self.check(&body)?;
        self.command_op(cx, &body, Purpose::Op(id, op));
        Ok(())
    }

    fn request_capabilities(&mut self, cx: &mut Cx, id: Option<CommandId>) {
        self.caps_in_flight = true;
        self.command_op(
            cx,
            &json!({ "op": "capabilities" }),
            Purpose::Capabilities(id),
        );
    }

    /// The capabilities are known: send the commands that waited for them.
    fn flush_pending(&mut self, cx: &mut Cx) {
        for (id, body) in std::mem::take(&mut self.pending) {
            if let Err(e) = self.dispatch(cx, id, body) {
                cx.complete(id, Err(e));
            }
        }
    }

    fn fail_pending(&mut self, cx: &mut Cx, error: CommandError) {
        for (id, _) in std::mem::take(&mut self.pending) {
            cx.complete(id, Err(error.clone()));
        }
    }

    /// The capabilities could not be read: the commands waiting fail.
    fn capabilities_unread(&mut self, cx: &mut Cx, why: &str) {
        cx.log(
            Level::Warning,
            format!("the camera's capabilities could not be read: {why}"),
        );
        self.fail_pending(
            cx,
            CommandError::DeviceError {
                code: Some("capabilities_unavailable".into()),
                message: format!(
                    "the camera's capabilities could not be read ({why}), so the ops it \
                     offers and their ranges are unknown"
                ),
            },
        );
    }

    // --- answers -------------------------------------------------------------

    fn answered(
        &mut self,
        cx: &mut Cx,
        purpose: Purpose,
        stale: bool,
        result: Result<HttpResponse, String>,
    ) {
        let response = match result {
            Ok(r) => r,
            Err(e) => return self.failed(cx, purpose, stale, e),
        };
        let status = response.status;
        let body: Value = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
        if status == 401 {
            if let Purpose::Capabilities(_) = purpose {
                self.caps_in_flight = false;
            }
            if let Purpose::Read(id, _) | Purpose::Op(id, _) | Purpose::Capabilities(Some(id)) =
                &purpose
            {
                cx.complete(
                    *id,
                    Err(CommandError::Auth {
                        message: "the camera refused the fleet key".into(),
                    }),
                );
            }
            if purpose == Purpose::Clips {
                self.clips_in_flight = false;
            }
            self.refuse(cx);
            return;
        }
        match purpose {
            Purpose::Hello => {
                if stale {
                    return;
                }
                if status != 200 || !body.is_object() {
                    return self.fail(cx, format!("the hello was answered with HTTP {status}"));
                }
                self.apply_hello(cx, &body);
                if self.monitor {
                    if self.phase == Phase::Hello {
                        self.phase = Phase::Status;
                        self.send(
                            cx,
                            "/v1/status",
                            None,
                            STATUS_TIMEOUT,
                            vec![],
                            Purpose::Status,
                        );
                    }
                } else {
                    self.phase = Phase::Ready;
                    self.retry_delay = RETRY_MIN;
                    self.report_connected(cx);
                    cx.set_timer(LIVENESS, LIVENESS_EVERY);
                }
            }
            Purpose::Status => {
                if stale {
                    return;
                }
                if status != 200 || !body.is_object() {
                    return self.fail(cx, format!("the status was answered with HTTP {status}"));
                }
                self.apply_status(cx, body);
                self.report_connected(cx);
                self.phase = Phase::Ready;
                self.request_capabilities(cx, None);
                self.refresh_clips(cx);
                self.open_events(cx);
            }
            Purpose::Capabilities(id) => {
                // Applied even from an earlier connection attempt: commands
                // may be waiting for it.
                self.caps_in_flight = false;
                let error = answer_error(status, &body).or_else(|| {
                    (!body["ops"].is_array()).then(|| CommandError::DeviceError {
                        code: None,
                        message: "the capabilities answer lists no ops".into(),
                    })
                });
                match error {
                    None => {
                        self.apply_capabilities(cx, &body);
                        if let Some(id) = id {
                            let value = redact_answer(&body, "capabilities");
                            cx.complete(id, Ok(Outcome::Value { value }));
                        }
                        self.flush_pending(cx);
                    }
                    Some(e) => {
                        if let Some(id) = id {
                            cx.complete(id, Err(e.clone()));
                        }
                        self.capabilities_unread(cx, &e.to_string());
                    }
                }
            }
            Purpose::Clips => {
                self.clips_in_flight = false;
                if status == 200 {
                    self.apply_clips(cx, &body);
                }
                if std::mem::take(&mut self.clips_again) {
                    self.refresh_clips(cx);
                }
            }
            Purpose::Read(id, read) => {
                let result = self.read_answer(cx, read, &response, body);
                cx.complete(id, result);
            }
            Purpose::Op(id, op) => {
                let result = match answer_error(status, &body) {
                    Some(e) => {
                        // A refusal: the status carries the same error.
                        if let CommandError::DeviceError {
                            code: None,
                            message,
                        } = &e
                        {
                            self.publish_one(cx, "error", json!(message));
                        }
                        Err(e)
                    }
                    None => {
                        // A command that succeeded clears the status error.
                        self.publish_one(cx, "error", Value::Null);
                        if op == "capabilities" {
                            self.apply_capabilities(cx, &body);
                        } else {
                            self.apply_answer(cx, &op, &body);
                        }
                        Ok(Outcome::Value {
                            value: redact_answer(&body, &op),
                        })
                    }
                };
                cx.complete(id, result);
            }
        }
    }

    fn read_answer(
        &mut self,
        cx: &mut Cx,
        read: Read,
        response: &HttpResponse,
        body: Value,
    ) -> Result<Outcome, CommandError> {
        let status = response.status;
        match read {
            Read::Clip {
                name,
                offset,
                length,
            } => clip_answer(&name, offset, length, response),
            _ if status != 200 => {
                Err(answer_error(status, &body).unwrap_or_else(|| http_error(status, &body)))
            }
            Read::Hello => {
                self.apply_hello(cx, &body);
                Ok(Outcome::Value { value: body })
            }
            Read::Status => {
                self.apply_status(cx, body.clone());
                Ok(Outcome::Value {
                    value: redact_answer(&body, "status"),
                })
            }
            Read::TalkbackUrl => {
                let value = json!({
                    "on": talkback(&body)["on"],
                    "url": body.get("talkback").filter(|u| u.is_string()),
                });
                self.apply_status(cx, body);
                Ok(Outcome::Value { value })
            }
            Read::Clips => {
                self.apply_clips(cx, &body);
                Ok(Outcome::Value { value: body })
            }
        }
    }

    fn failed(&mut self, cx: &mut Cx, purpose: Purpose, stale: bool, error: String) {
        let timeout = error.to_ascii_lowercase().contains("timed out");
        match purpose {
            Purpose::Hello | Purpose::Status if !stale => {
                self.fail(cx, format!("the camera did not answer: {error}"));
            }
            Purpose::Clips => {
                self.clips_in_flight = false;
                self.clips_again = false;
            }
            Purpose::Capabilities(id) => {
                self.caps_in_flight = false;
                if let Some(id) = id {
                    cx.complete(
                        id,
                        Err(if timeout {
                            CommandError::Timeout
                        } else {
                            CommandError::Transport {
                                message: error.clone(),
                            }
                        }),
                    );
                }
                self.capabilities_unread(cx, &format!("the camera did not answer: {error}"));
            }
            Purpose::Read(id, _) | Purpose::Op(id, _) => cx.complete(
                id,
                Err(if timeout {
                    CommandError::Timeout
                } else {
                    CommandError::Transport { message: error }
                }),
            ),
            _ => {}
        }
    }

    // --- streams -------------------------------------------------------------

    fn events_input(&mut self, cx: &mut Cx, input: TcpInput) {
        if self.events.is_none() {
            return;
        }
        match input {
            TcpInput::Connected => {
                if let Some(e) = &mut self.events {
                    e.open = true;
                }
                let request = wire::stream_request(
                    "/v1/events",
                    &self.authority,
                    &self.key,
                    "application/x-ndjson",
                );
                cx.tcp_send(EVENTS, request);
            }
            TcpInput::Data(data) => {
                let Some(events) = &mut self.events else {
                    return;
                };
                let pieces = match events.reader.feed(&data) {
                    Ok(p) => p,
                    Err(e) => return self.events_lost(cx, &e),
                };
                for piece in pieces {
                    match piece {
                        Piece::Head { status: 200 } => {}
                        Piece::Head { status: 401 } => return self.refuse(cx),
                        Piece::Head { status } => {
                            return self.events_lost(
                                cx,
                                &format!("the camera answered it with HTTP {status}"),
                            )
                        }
                        Piece::Body(bytes) => {
                            let Some(events) = &mut self.events else {
                                return;
                            };
                            let lines = match events.lines.feed(&bytes) {
                                Ok(l) => l,
                                Err(e) => return self.events_lost(cx, &e),
                            };
                            for line in lines {
                                if line.trim().is_empty() {
                                    continue;
                                }
                                match serde_json::from_str::<Value>(&line) {
                                    Ok(doc) if doc.is_object() => {
                                        if let Some(e) = &mut self.events {
                                            e.heard = true;
                                        }
                                        self.retry_delay = RETRY_MIN;
                                        cx.alive();
                                        cx.set_timer(EVENTS_SILENCE, SILENCE);
                                        self.apply_status(cx, doc);
                                    }
                                    _ => cx.log(
                                        Level::Debug,
                                        "event stream: a line that is not a status document",
                                    ),
                                }
                            }
                        }
                    }
                }
            }
            TcpInput::Closed { reason } => {
                let reason = if self.events.as_ref().is_some_and(|e| e.open) {
                    format!("the camera ended it ({reason})")
                } else {
                    format!("it could not be opened ({reason})")
                };
                self.events_lost(cx, &reason);
            }
        }
    }

    fn preview_input(&mut self, cx: &mut Cx, input: TcpInput) {
        if self.preview.is_none() {
            return;
        }
        match input {
            TcpInput::Connected => {
                if let Some(p) = &mut self.preview {
                    p.open = true;
                }
                let request = wire::stream_request(
                    "/v1/preview",
                    &self.authority,
                    &self.key,
                    "application/x-meros-preview",
                );
                cx.tcp_send(PREVIEW, request);
            }
            TcpInput::Data(data) => {
                let Some(preview) = &mut self.preview else {
                    return;
                };
                let pieces = match preview.reader.feed(&data) {
                    Ok(p) => p,
                    Err(e) => return self.preview_lost(cx, &e),
                };
                for piece in pieces {
                    match piece {
                        Piece::Head { status: 200 } => {}
                        Piece::Head { status: 401 } => return self.refuse(cx),
                        Piece::Head { status } => {
                            return self.preview_lost(
                                cx,
                                &format!("the camera answered it with HTTP {status}"),
                            )
                        }
                        Piece::Body(bytes) => {
                            let Some(preview) = &mut self.preview else {
                                return;
                            };
                            match preview.frames.feed(&bytes) {
                                Ok(frames) => {
                                    if !frames.is_empty() {
                                        self.preview_delay = RETRY_MIN;
                                    }
                                    for frame in frames {
                                        cx.frame(LIVE, FRAME_FORMAT, frame);
                                    }
                                }
                                Err(e) => {
                                    cx.log(Level::Warning, format!("preview: {e}"));
                                    return self.preview_lost(cx, "a corrupt frame");
                                }
                            }
                        }
                    }
                }
            }
            TcpInput::Closed { reason } => {
                let reason = if self.preview.as_ref().is_some_and(|p| p.open) {
                    format!("the camera ended it ({reason})")
                } else {
                    format!("it could not be opened ({reason})")
                };
                self.preview_lost(cx, &reason);
            }
        }
    }
}

impl Module for MerosCamera {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if let Err(e) = self.run(cx, id, name, params) {
            cx.complete(id, Err(e));
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let Some(request) = self.requests.remove(&id) else {
            return;
        };
        if result.is_ok() {
            cx.round_trip(cx.now().saturating_sub(request.sent));
            cx.alive();
        }
        let stale = request.generation != self.generation;
        self.answered(cx, request.purpose, stale, result);
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        match socket {
            EVENTS => self.events_input(cx, input),
            PREVIEW => self.preview_input(cx, input),
            _ => {}
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY if self.phase == Phase::Waiting => self.connect(cx),
            EVENTS_SILENCE if self.events.is_some() => self.events_lost(cx, "no line for 10 s"),
            LIVENESS if self.phase == Phase::Ready && !self.monitor => {
                self.hello(cx, Purpose::Hello);
                cx.set_timer(LIVENESS, LIVENESS_EVERY);
            }
            PREVIEW_RETRY if self.watched && self.preview.is_none() => self.open_preview(cx),
            _ => {}
        }
    }

    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        if stream != LIVE {
            return;
        }
        self.watched = watching;
        if watching {
            self.preview_delay = RETRY_MIN;
            if self.preview.is_none() {
                self.open_preview(cx);
            }
        } else {
            self.close_preview(cx);
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.close_events(cx);
        self.close_preview(cx);
    }

    fn reconnect(&mut self, cx: &mut Cx) -> bool {
        // A refused key may have been paired again on the camera: ask again.
        self.close_events(cx);
        self.close_preview(cx);
        for timer in [RETRY, LIVENESS] {
            cx.cancel_timer(timer);
        }
        self.retry_delay = RETRY_MIN;
        self.preview_delay = RETRY_MIN;
        // Read the ops and ranges again before the next op command.
        self.ops = None;
        self.connect(cx);
        if self.watched {
            self.open_preview(cx);
        }
        true
    }
}

// --- helpers -------------------------------------------------------------------

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The error an answer carries: a refusal (`error`, a string, belongs to
/// this command alone, §6; absent or null is success), or an HTTP failure.
fn answer_error(status: u16, body: &Value) -> Option<CommandError> {
    if status != 200 {
        return Some(http_error(status, body));
    }
    body.get("error").and_then(Value::as_str).map(|e| {
        let e = e.trim();
        CommandError::DeviceError {
            code: None,
            message: if e.is_empty() {
                "the camera refused the command".into()
            } else {
                e.to_string()
            },
        }
    })
}

/// The `[min, max]` of a `{min, max}` range.
fn range(r: &Value) -> Option<(f64, f64)> {
    Some((r.get("min")?.as_f64()?, r.get("max")?.as_f64()?))
}

/// A number given for `name` within `limit` (`{min, max}`), when both are
/// there.
fn within(
    value: Option<&Value>,
    limit: Option<&Value>,
    name: &str,
    whose: &str,
) -> Result<(), CommandError> {
    let (Some(v), Some((min, max))) = (value.and_then(Value::as_f64), limit.and_then(range)) else {
        return Ok(());
    };
    // A little slack for values sent as decimals.
    let slack = (max - min).abs() * 1e-9;
    if v < min - slack || v > max + slack {
        return Err(invalid(format!(
            "{name} {v} is outside {whose} range ({min} to {max})"
        )));
    }
    Ok(())
}

/// Output values the camera refuses outside its limits (§6.5, §7.6).
fn check_outputs(o: &Value, limits: &Value) -> Result<(), CommandError> {
    const CAMERA: &str = "the camera's";
    if let Some(h) = o.pointer("/format/height").and_then(Value::as_f64) {
        if let Some(heights) = limits.pointer("/outputs/heights").and_then(Value::as_array) {
            if !heights.iter().any(|x| x.as_f64() == Some(h)) {
                let list: Vec<String> = heights.iter().map(Value::to_string).collect();
                return Err(invalid(format!(
                    "format.height {h} is not one of the camera's output heights ({})",
                    list.join(", ")
                )));
            }
        }
    }
    within(
        o.pointer("/format/bitrateMbps"),
        limits.pointer("/outputs/bitrateMbps"),
        "format.bitrateMbps",
        CAMERA,
    )?;
    within(
        o.pointer("/srt/latency_ms"),
        limits.pointer("/outputs/srtLatencyMs"),
        "srt.latency_ms",
        CAMERA,
    )?;
    for kind in ["srt", "hls", "whep"] {
        within(
            o.pointer(&format!("/{kind}/port")),
            limits.pointer("/outputs/port"),
            &format!("{kind}.port"),
            CAMERA,
        )?;
    }
    if let Some(q) = o.pointer("/omt/quality") {
        // A camera whose limits leave the list out takes §7.6's.
        let allowed: Vec<&str> = limits
            .pointer("/outputs/omtQuality")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(Value::as_str).collect())
            .filter(|l: &Vec<&str>| !l.is_empty())
            .unwrap_or_else(|| OMT_QUALITY.to_vec());
        if !q.as_str().is_some_and(|q| allowed.contains(&q)) {
            return Err(invalid(format!(
                "omt.quality {} is not one of the camera's OMT qualities ({})",
                as_text(q),
                allowed.join(", ")
            )));
        }
    }
    Ok(())
}

/// The talkback state a status document (or a talkback answer, in the same
/// names) gives: whether it is on and latched, and whether an address is
/// set. The address itself is a credential and is left out (§6.6).
fn talkback(doc: &Value) -> Value {
    let url_set = doc
        .get("talkback")
        .and_then(Value::as_str)
        .is_some_and(|u| !u.is_empty());
    let on = doc
        .get("talkbackOn")
        .filter(|o| o.is_boolean())
        .cloned()
        .unwrap_or(Value::Bool(url_set));
    let latch = doc
        .get("talkbackLatch")
        .filter(|l| l.is_boolean())
        .cloned()
        .unwrap_or(Value::Null);
    json!({ "on": on, "latch": latch, "url_set": url_set })
}

fn http_error(status: u16, body: &Value) -> CommandError {
    let message = body
        .get("error")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| format!("HTTP {status}"));
    match status {
        504 => CommandError::Timeout,
        401 => CommandError::Auth { message },
        _ => CommandError::DeviceError {
            code: Some(status.to_string()),
            message,
        },
    }
}

/// A `read_clip` answer: 206 with its range, 200 (the whole file, cut here),
/// or 416 past the end.
fn clip_answer(
    name: &str,
    offset: u64,
    length: u64,
    response: &HttpResponse,
) -> Result<Outcome, CommandError> {
    use base64::Engine;
    let b64 = |d: &[u8]| base64::engine::general_purpose::STANDARD.encode(d);
    let range = response
        .header_values("content-range")
        .next()
        .and_then(wire::content_range);
    let (first, data, total) = match response.status {
        206 => {
            let (span, total) = range.ok_or_else(|| CommandError::DeviceError {
                code: Some("206".into()),
                message: "a partial answer without its Content-Range".into(),
            })?;
            let first = span.map_or(offset, |(a, _)| a);
            (first, &response.body[..], total)
        }
        200 => {
            let total = response.body.len() as u64;
            let start = offset.min(total) as usize;
            let end = offset.saturating_add(length).min(total) as usize;
            (offset, &response.body[start..end], Some(total))
        }
        416 => {
            let total = range.and_then(|(_, t)| t);
            let value = json!({
                "name": name, "offset": offset, "length": 0, "total": total,
                "eof": true, "data": "",
            });
            return Ok(Outcome::Value { value });
        }
        status => {
            let body: Value = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
            return Err(http_error(status, &body));
        }
    };
    let read = data.len() as u64;
    let eof = total.is_some_and(|t| first + read >= t);
    Ok(Outcome::Value {
        value: json!({
            "name": name, "offset": first, "length": read, "total": total,
            "eof": eof, "data": b64(data),
        }),
    })
}

/// An outputs document with any credential fields removed.
fn redact(outputs: &Value) -> Value {
    let Some(kinds) = outputs.as_object() else {
        return outputs.clone();
    };
    let mut out = Map::new();
    for (kind, fields) in kinds {
        let mut fields = fields.clone();
        if let Some(f) = fields.as_object_mut() {
            for c in CREDENTIALS {
                if let Some(v) = f.remove(*c) {
                    let set = v.as_str().is_some_and(|s| !s.is_empty());
                    f.insert(format!("{c}Set"), json!(set));
                }
            }
        }
        out.insert(kind.clone(), fields);
    }
    Value::Object(out)
}

/// An answer or a status document as it may be returned: output credentials
/// removed, and the talkback address (a status's `talkback`, the talkback
/// op's `url`) shown only as set.
fn redact_answer(answer: &Value, op: &str) -> Value {
    let mut answer = answer.clone();
    if let Some(o) = answer.get("outputs").cloned() {
        // The status's outputs ({on, problems}) hold no credentials.
        if o.get("on").is_none_or(Value::is_object) {
            answer["outputs"] = redact(&o);
        }
    }
    if let Some(o) = answer.get("outputSettings").cloned() {
        answer["outputSettings"] = redact(&o);
    }
    let address = if op == "talkback" { "url" } else { "talkback" };
    if answer.get(address).is_some_and(Value::is_string) {
        answer[address] = json!(SET);
    }
    answer
}

/// The state a status document gives: the top-level keys it owns, null
/// where the document has nothing.
fn status_view(doc: &Value) -> Map<String, Value> {
    let get = |k: &str| doc.get(k).cloned().unwrap_or(Value::Null);
    let mut v = Map::new();
    for k in SAME_NAME {
        v.insert((*k).into(), get(k));
    }
    // A camera from before `running` gives the same under `resolved`.
    if !doc.get("running").is_some_and(Value::is_object) {
        v.insert("running".into(), get("resolved"));
    }
    v.insert("clip_count".into(), whole(get("clips")));
    v.insert("free_bytes".into(), whole(get("freeBytes")));
    v.insert("signal_dbm".into(), get("signalDbm"));
    v.insert("battery".into(), whole(get("battery")));
    let on = doc.pointer("/outputs/on").cloned().unwrap_or(Value::Null);
    let problems = doc
        .pointer("/outputs/problems")
        .filter(|p| p.is_object())
        .cloned()
        .unwrap_or_else(|| json!({}));
    v.insert(
        "outputs".into(),
        if doc.get("outputs").is_some_and(Value::is_object) {
            json!({ "on": on, "problems": problems })
        } else {
            Value::Null
        },
    );
    // Only from a camera that sends them: otherwise the capabilities
    // answer's and the monitoring and outputs answers' stand.
    if let Some(m) = doc.get("monitoring").filter(|m| m.is_object()) {
        v.insert("monitoring".into(), m.clone());
    }
    if let Some(o) = doc.get("outputSettings").filter(|o| o.is_object()) {
        v.insert("output_settings".into(), redact(o));
    }
    v.insert("talkback".into(), talkback(doc));
    v.insert("return_source".into(), get("return"));
    v.insert("preview_codec".into(), get("previewCodec"));
    v.insert("preview_note".into(), get("previewNote"));
    v
}

/// A number with no fraction as an integer (the camera's JSON may write
/// 64000000000.0 for a byte count).
fn whole(v: Value) -> Value {
    match v.as_f64() {
        Some(f) if v.is_f64() && f.fract() == 0.0 && f.abs() < 9.0e15 => json!(f as i64),
        _ => v,
    }
}

/// The RFC 7386 merge patch that takes `old` to `new` (null: remove).
fn diff(old: Option<&Value>, new: &Value) -> Option<Value> {
    let old = old.filter(|o| !o.is_null());
    match (old, new) {
        (None, Value::Null) => None,
        (Some(_), Value::Null) => Some(Value::Null),
        (Some(Value::Object(o)), Value::Object(n)) => {
            let mut patch = Map::new();
            for (k, v) in n {
                if let Some(d) = diff(o.get(k), v) {
                    patch.insert(k.clone(), d);
                }
            }
            for k in o.keys() {
                if !n.contains_key(k) {
                    patch.insert(k.clone(), Value::Null);
                }
            }
            (!patch.is_empty()).then_some(Value::Object(patch))
        }
        (_, Value::Object(_)) => Some(strip_nulls(new)),
        (Some(o), n) if o == n => None,
        (_, n) => Some(n.clone()),
    }
}

/// An object without its null members, at every depth, so a merge patch
/// creating it does not mistake them for removals.
fn strip_nulls(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), strip_nulls(v)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The dotted paths of the leaves that differ between two documents.
fn changed_paths(old: Option<&Value>, new: Option<&Value>, prefix: &str, out: &mut Vec<String>) {
    let join = |k: &str| {
        if prefix.is_empty() {
            k.to_string()
        } else {
            format!("{prefix}.{k}")
        }
    };
    match (old, new) {
        (Some(Value::Object(o)), Some(Value::Object(n))) => {
            let keys: BTreeSet<&String> = o.keys().chain(n.keys()).collect();
            for k in keys {
                changed_paths(o.get(k), n.get(k), &join(k), out);
            }
        }
        // The first document, or none: nothing changed by anyone.
        (None, _) | (_, None) if prefix.is_empty() => {}
        (o, n) => {
            if o.unwrap_or(&Value::Null) != n.unwrap_or(&Value::Null) {
                out.push(prefix.to_string());
            }
        }
    }
}

/// Copy the parameters given into `obj` under the camera's field names.
fn fields(p: &Params, names: &[(&str, &str)]) -> Map<String, Value> {
    let mut obj = Map::new();
    for (param, field) in names {
        if let Some(v) = p.get(*param) {
            obj.insert((*field).into(), v.clone());
        }
    }
    obj
}

fn nonempty(obj: Map<String, Value>, command: &str) -> Result<Value, CommandError> {
    if obj.is_empty() {
        return Err(invalid(format!(
            "{command}: give at least one value to change"
        )));
    }
    Ok(Value::Object(obj))
}

fn settings(patch: Value) -> Value {
    json!({ "op": "settings", "settings": patch })
}

fn monitoring(patch: Value) -> Value {
    json!({ "op": "monitoring", "monitoring": patch })
}

fn outputs(kind: &str, patch: Value) -> Value {
    json!({ "op": "outputs", "outputs": { kind: patch } })
}

fn object(p: &Params, name: &str) -> Result<Value, CommandError> {
    match p.get(name) {
        Some(v @ Value::Object(_)) => Ok(v.clone()),
        _ => Err(invalid(format!("{name} must be a JSON object"))),
    }
}

/// The `POST /v1/command` body for a command.
fn op_body(name: &str, p: &Params) -> Result<Value, CommandError> {
    let body = match name {
        "set_settings" => settings(object(p, "settings")?),
        "set_exposure" => {
            let mut e = fields(
                p,
                &[
                    ("mode", "mode"),
                    ("iso", "iso"),
                    ("ev", "ev"),
                    ("lock", "lock"),
                    ("antibanding", "antibanding"),
                ],
            );
            match (p.get("shutter_angle"), p.get("shutter_seconds")) {
                (Some(_), Some(_)) => {
                    return Err(invalid("give shutter_angle or shutter_seconds, not both"))
                }
                (Some(a), None) => {
                    e.insert("shutter".into(), json!({ "angle": a }));
                }
                (None, Some(s)) => {
                    e.insert("shutter".into(), json!({ "seconds": s }));
                }
                (None, None) => {}
            }
            settings(json!({ "exposure": nonempty(e, name)? }))
        }
        "set_white_balance" => {
            let w = fields(
                p,
                &[
                    ("mode", "mode"),
                    ("preset", "preset"),
                    ("kelvin", "kelvin"),
                    ("tint", "tint"),
                    ("lock", "lock"),
                ],
            );
            settings(json!({ "whiteBalance": nonempty(w, name)? }))
        }
        "measure_white_balance" => settings(json!({
            "whiteBalance": { "point": [p.get("x"), p.get("y")] }
        })),
        "set_focus" => {
            let f = fields(
                p,
                &[
                    ("mode", "mode"),
                    ("position", "position"),
                    ("faces", "faces"),
                    ("lock", "lock"),
                ],
            );
            settings(json!({ "focus": nonempty(f, name)? }))
        }
        "set_zoom" => settings(json!({ "zoom": p.get("ratio") })),
        "set_format" => {
            let f = fields(
                p,
                &[("width", "width"), ("height", "height"), ("fps", "fps")],
            );
            settings(nonempty(f, name)?)
        }
        "set_profile" => settings(json!({ "profile": p.get("profile") })),
        "set_stabilization" => settings(json!({ "stabilization": p.get("mode") })),
        "set_torch" => settings(json!({ "torch": p.get("level") })),
        "set_aperture" => settings(json!({ "aperture": p.get("f_number") })),
        "set_exact_frame_rate" => settings(json!({ "exactFrameRate": p.get("enabled") })),
        "set_recording_settings" => {
            let mut r = fields(
                p,
                &[
                    ("codec", "codec"),
                    ("dynamic_range", "dynamicRange"),
                    ("container", "container"),
                    ("audio", "audio"),
                    ("timecode", "timecode"),
                    ("gain_db", "gainDb"),
                    ("safety_track", "safetyTrack"),
                    ("split_channels", "splitChannels"),
                ],
            );
            if let Some(b) = p.get("bitrate_mbps").and_then(Value::as_f64) {
                r.insert(
                    "bitrateMbps".into(),
                    if b == 0.0 { Value::Null } else { json!(b) },
                );
            }
            if let Some(a) = p.get("audio_input").and_then(Value::as_str) {
                r.insert(
                    "audioInput".into(),
                    if a.is_empty() { Value::Null } else { json!(a) },
                );
            }
            settings(json!({ "recording": nonempty(r, name)? }))
        }
        "select_lens" => json!({ "op": "lens", "id": p.get("id") }),
        "focus_at" => json!({
            "op": "focusAt", "x": p.get("x"), "y": p.get("y"),
            "lock": p.get("lock").cloned().unwrap_or(Value::Bool(false)),
        }),
        "recall_preset" => json!({ "op": "preset", "name": p.get("name") }),
        "start_recording" => json!({ "op": "record", "on": true }),
        "stop_recording" => json!({ "op": "record", "on": false }),
        "set_slate" => {
            let s = fields(
                p,
                &[
                    ("camera", "camera"),
                    ("reel", "reel"),
                    ("project", "project"),
                    ("scene", "scene"),
                    ("shot", "shot"),
                    ("take", "take"),
                    ("notes", "notes"),
                ],
            );
            json!({ "op": "slate", "slate": nonempty(s, name)? })
        }
        "circle_clip" => json!({
            "op": "circle", "name": p.get("name"),
            "on": p.get("circled").cloned().unwrap_or(Value::Bool(true)),
        }),
        "set_monitoring" => monitoring(object(p, "monitoring")?),
        "set_false_color" => monitoring(json!({ "falseColour": p.get("enabled") })),
        "set_zebra" => monitoring(nonempty(
            fields(p, &[("enabled", "zebra"), ("level", "zebraLevel")]),
            name,
        )?),
        "set_zebra2" => monitoring(nonempty(
            fields(p, &[("enabled", "zebra2"), ("level", "zebra2Level")]),
            name,
        )?),
        "set_peaking" => monitoring(nonempty(
            fields(
                p,
                &[
                    ("enabled", "peaking"),
                    ("color", "peakColour"),
                    ("sensitivity", "peakSensitivity"),
                ],
            ),
            name,
        )?),
        "set_scope" => monitoring(nonempty(
            fields(
                p,
                &[
                    ("scope", "scope"),
                    ("large", "scopeLarge"),
                    ("floating", "scopeFloating"),
                ],
            ),
            name,
        )?),
        "set_frame_guide" => monitoring(nonempty(
            fields(p, &[("guide", "frameGuide"), ("custom", "customGuide")]),
            name,
        )?),
        "set_guides" => monitoring(nonempty(
            fields(
                p,
                &[
                    ("safe_areas", "safeAreas"),
                    ("grid", "grid"),
                    ("center_mark", "centerMark"),
                    ("level", "level"),
                ],
            ),
            name,
        )?),
        "set_monitor_display" => monitoring(nonempty(
            fields(
                p,
                &[
                    ("desqueeze", "desqueeze"),
                    ("rec_border", "recBorder"),
                    ("meter", "meter"),
                    ("punch_in", "punchIn"),
                    ("clean", "clean"),
                ],
            ),
            name,
        )?),
        "set_look" => json!({ "op": "look", "target": p.get("target"), "lut": p.get("lut") }),
        "set_outputs" => json!({ "op": "outputs", "outputs": object(p, "outputs")? }),
        "set_srt_output" => outputs(
            "srt",
            nonempty(
                fields(
                    p,
                    &[
                        ("enabled", "on"),
                        ("mode", "mode"),
                        ("port", "port"),
                        ("target", "target"),
                        ("latency_ms", "latency_ms"),
                    ],
                ),
                name,
            )?,
        ),
        "set_rtmp_output" => outputs(
            "rtmp",
            nonempty(
                fields(
                    p,
                    &[("enabled", "on"), ("url", "url"), ("stream_key", "key")],
                ),
                name,
            )?,
        ),
        "set_hls_output" => outputs(
            "hls",
            nonempty(fields(p, &[("enabled", "on"), ("port", "port")]), name)?,
        ),
        "set_whip_output" => outputs(
            "whip",
            nonempty(
                fields(p, &[("enabled", "on"), ("url", "url"), ("token", "token")]),
                name,
            )?,
        ),
        "set_whep_output" => outputs(
            "whep",
            nonempty(fields(p, &[("enabled", "on"), ("port", "port")]), name)?,
        ),
        "set_omt_output" => outputs(
            "omt",
            nonempty(
                fields(
                    p,
                    &[("enabled", "on"), ("name", "name"), ("quality", "quality")],
                ),
                name,
            )?,
        ),
        "set_ndi_output" => outputs(
            "ndi",
            nonempty(fields(p, &[("enabled", "on"), ("name", "name")]), name)?,
        ),
        "set_external_output" => outputs("external", json!({ "on": p.get("enabled") })),
        "set_output_interface" => outputs("network", json!({ "interface": p.get("interface") })),
        "set_output_look" => outputs(
            "look",
            nonempty(
                fields(p, &[("look", "look"), ("overlays", "overlays")]),
                name,
            )?,
        ),
        "set_output_format" => {
            let mut f = fields(
                p,
                &[("bitrate_mbps", "bitrateMbps"), ("half_rate", "halfRate")],
            );
            // An enum of line counts, sent as the number the camera takes.
            if let Some(h) = p.get("height").and_then(Value::as_str) {
                let lines: u32 = h.parse().map_err(|_| invalid("height: 1080, 720 or 540"))?;
                f.insert("height".into(), json!(lines));
            }
            outputs("format", nonempty(f, name)?)
        }
        "set_tally" => json!({ "op": "tally", "light": p.get("light") }),
        "set_talkback" => {
            let mut t = fields(p, &[("enabled", "on"), ("latch", "latch")]);
            t.insert("op".into(), json!("talkback"));
            if t.len() == 1 {
                return Err(invalid("set_talkback: give enabled or latch"));
            }
            Value::Object(t)
        }
        "new_talkback_address" => json!({ "op": "talkback", "newAddress": true }),
        "set_return" => {
            let mut r = fields(
                p,
                &[("kind", "kind"), ("address", "address"), ("enabled", "on")],
            );
            if r.is_empty() {
                return Err(invalid("set_return: give kind, address or enabled"));
            }
            r.insert("op".into(), json!("return"));
            Value::Object(r)
        }
        "visca" => {
            let mut v = object(p, "fields")?;
            v["op"] = json!("visca");
            v
        }
        "send_op" => {
            let mut v = match p.get("fields") {
                None => json!({}),
                Some(_) => object(p, "fields")?,
            };
            v["op"] = json!(text(p, "op"));
            v
        }
        other => {
            return Err(CommandError::UnknownCommand {
                command: other.into(),
            })
        }
    };
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::Ipv4Addr;

    const KEY: &str = "0123456789abcdef0123456789abcdef";

    fn camera(settings: Value, monitor: bool) -> MerosCamera {
        MerosCamera::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)),
            host_name: None,
            port: None,
            model: "meros-camera".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        })
    }

    fn keyed(monitor: bool) -> MerosCamera {
        camera(json!({ "fleet_key": KEY }), monitor)
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

    fn paths(actions: &[Action]) -> Vec<String> {
        requests(actions)
            .iter()
            .map(|(_, r)| {
                let path = r.url.split_once("8790").unwrap().1.to_string();
                match &r.body {
                    Some(b) => format!(
                        "{} {path} {}",
                        r.method,
                        serde_json::from_slice::<Value>(b).unwrap()["op"]
                            .as_str()
                            .unwrap()
                    ),
                    None => format!("{} {path}", r.method),
                }
            })
            .collect()
    }

    fn respond(
        m: &mut MerosCamera,
        now: Millis,
        id: RequestId,
        status: u16,
        body: Value,
    ) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.http_response(
            &mut cx,
            id,
            Ok(HttpResponse {
                status,
                headers: vec![],
                body: body.to_string().into_bytes(),
            }),
        );
        cx.take()
    }

    fn state(actions: &[Action]) -> Value {
        let mut merged = json!({});
        for a in actions {
            if let Action::State(p) = a {
                crate::session::merge_patch(&mut merged, p);
            }
        }
        merged
    }

    fn messages(actions: &[Action]) -> Vec<(String, Value)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Message { address, args, .. } => Some((address.clone(), args.clone())),
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

    fn hello() -> Value {
        json!({"id": "00112233aabbccdd", "name": "Camera A", "model": "SM-S926U", "app": "1.0.0", "protocol": 1})
    }

    fn status() -> Value {
        json!({
            "id": "00112233aabbccdd", "name": "Camera A", "model": "SM-S926U",
            "recording": false, "clip": null, "error": null, "warnings": [],
            "tally": "off", "lens": "0", "lenses": [{"id": "0", "name": "Wide"}],
            "settings": {"lensId": "0", "width": 1920, "height": 1080, "fps": 25,
                         "exposure": {"mode": "auto", "iso": null, "ev": 0},
                         "whiteBalance": {"mode": "auto", "kelvin": 5600, "tint": 0}},
            "running": {"width": 1920, "height": 1080, "fps": 25, "highSpeed": false},
            "resolved": {"width": 1920, "height": 1080, "fps": 25, "highSpeed": false},
            "presets": ["Wide"], "luts": ["rec709"],
            "looks": {"monitor": "none", "outputs": "recording", "recording": "none"},
            "slate": {"camera": "A", "reel": 1, "project": "", "scene": "", "shot": "", "take": 1, "notes": ""},
            "clips": 2, "battery": 80, "thermal": "none", "freeBytes": 64_000_000_000i64,
            "network": "wifi", "signalDbm": -50,
            "outputs": {"on": [], "problems": {}},
            "audio": [-20.0, -21.5], "talkback": null, "talkbackOn": false,
            "talkbackLatch": false, "return": null,
            "visca": {"power": true, "zoom": 0, "focus": 0, "focusAuto": true, "whiteBalance": 0, "exposure": 0},
            "previewCodec": "h264", "previewNote": null
        })
    }

    /// Every op of protocol version 1.
    const OPS: [&str; 15] = [
        "capabilities",
        "settings",
        "record",
        "preset",
        "lens",
        "focusAt",
        "look",
        "monitoring",
        "outputs",
        "slate",
        "circle",
        "tally",
        "talkback",
        "return",
        "visca",
    ];

    /// A capabilities answer: one lens with its ranges, and limits narrower
    /// than §7.6's on the output bitrate.
    fn caps() -> Value {
        json!({
            "protocol": 1, "ops": OPS,
            "capabilities": {"model": "SM-S926U", "platform": "android", "lenses": [{
                "id": "0", "name": "Wide", "facing": "back",
                "exposure": {"iso": {"min": 50, "max": 3200}, "exposureTimeNs": {"min": 1e5, "max": 1e8},
                             "ev": {"min": -2, "max": 2}, "manual": true},
                "whiteBalance": {"presets": ["daylight"], "kelvin": {"min": 2500, "max": 8000}},
                "zoom": {"min": 1, "max": 8},
                "videoFormats": [{"width": 1920, "height": 1080, "fps": [24, 25, 30], "manualFps": [50]},
                                 {"width": 3840, "height": 2160, "fps": [24, 25, 30]}],
                "highSpeedFormats": [{"width": 1280, "height": 720, "fps": [120, 240]}]
            }]},
            "settings": status()["settings"],
            "resolved": {"lensId": "0", "width": 1920, "height": 1080, "fps": 25, "highSpeed": false},
            "monitoring": {"zebra": false}, "luts": ["rec709"], "presets": ["Wide"],
            "outputs": {"srt": {"on": false, "mode": "listener", "port": 9710, "latency_ms": 120}},
            "limits": {
                "outputs": {"heights": [1080, 720, 540], "bitrateMbps": {"min": 1, "max": 40},
                            "srtLatencyMs": {"min": 20, "max": 8000}, "port": {"min": 1024, "max": 65535}},
                "monitoring": {"zebraLevel": {"min": 0.5, "max": 1}, "zebra2Level": {"min": 0.3, "max": 1},
                               "customGuide": {"min": 0.25, "max": 4}, "desqueeze": {"min": 1, "max": 2}},
                "recording": {"gainDb": {"min": -24, "max": 24}},
                "whiteBalance": {"tint": {"min": -100, "max": 100}}
            }
        })
    }

    /// Connect and answer the capabilities and the clip list.
    fn ready(m: &mut MerosCamera) {
        let a = connected(m);
        let r = requests(&a);
        respond(m, 12, r[0].0, 200, caps());
        respond(m, 13, r[1].0, 200, json!([]));
    }

    /// Run a command: its actions.
    fn command(
        m: &mut MerosCamera,
        now: Millis,
        id: CommandId,
        name: &str,
        p: Value,
    ) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn body(r: &HttpRequest) -> Value {
        serde_json::from_slice(r.body.as_ref().unwrap()).unwrap()
    }

    /// Start, answer the hello and the status: the actions after the status.
    fn connected(m: &mut MerosCamera) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        let (id, r) = &requests(&a)[0];
        assert!(r.url.ends_with("/v1/hello"));
        assert!(r.headers.is_empty(), "the hello carries no key");
        let a = respond(m, 5, *id, 200, hello());
        let (id, r) = &requests(&a)[0];
        assert!(r.url.ends_with("/v1/status"));
        assert!(r
            .headers
            .contains(&("Authorization".into(), format!("Bearer {KEY}"))));
        respond(m, 10, *id, 200, status())
    }

    fn line(m: &mut MerosCamera, now: Millis, doc: &Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(
            &mut cx,
            EVENTS,
            TcpInput::Data(format!("{doc}\n").into_bytes()),
        );
        cx.take()
    }

    /// Connect, answer the clip list, and open the event stream past its head.
    fn streaming(m: &mut MerosCamera) {
        let a = connected(m);
        let (clips, _) = requests(&a)[1].clone();
        respond(m, 15, clips, 200, json!([]));
        let mut cx = Cx::new(20);
        m.tcp(&mut cx, EVENTS, TcpInput::Connected);
        m.tcp(
            &mut cx,
            EVENTS,
            TcpInput::Data(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\n\r\n".to_vec(),
            ),
        );
    }

    #[test]
    fn connecting_reads_hello_status_capabilities_clips_and_opens_events() {
        let mut m = keyed(true);
        let a = connected(&mut m);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(
            paths(&a),
            ["POST /v1/command capabilities", "GET /v1/clips"]
        );
        assert!(a.contains(&Action::TcpOpen {
            socket: EVENTS,
            to: "10.0.0.7:8790".parse().unwrap()
        }));
        let s = state(&a);
        assert_eq!(s["tally"], "off");
        assert_eq!(s["clip_count"], 2);
        assert_eq!(s["free_bytes"], 64_000_000_000i64);
        assert_eq!(s["signal_dbm"], -50);
        assert_eq!(s["settings"]["whiteBalance"]["kelvin"], 5600);
        assert_eq!(s["outputs"], json!({"on": [], "problems": {}}));
        assert_eq!(
            s["talkback"],
            json!({"on": false, "latch": false, "url_set": false})
        );
        assert_eq!(s["running"]["fps"], 25);
        assert_eq!(s["resolved"]["highSpeed"], false);
        assert_eq!(s["preview_codec"], "h264");
        assert!(s.get("clip").is_none(), "null is left out");
        assert!(a.iter().any(|x| matches!(x, Action::RoundTrip(5))));
        // The event stream's request carries the key.
        let mut cx = Cx::new(20);
        m.tcp(&mut cx, EVENTS, TcpInput::Connected);
        let sent = cx.take();
        let Action::TcpSend { data, .. } = &sent[0] else {
            panic!("{sent:?}")
        };
        let text = String::from_utf8_lossy(data);
        assert!(text.starts_with("GET /v1/events HTTP/1.1\r\n"));
        assert!(text.contains(&format!("Authorization: Bearer {KEY}\r\n")));
    }

    #[test]
    fn the_key_is_never_logged_or_kept_in_state() {
        let mut m = keyed(true);
        let mut all = connected(&mut m);
        let mut cx = Cx::new(30);
        m.command(
            &mut cx,
            1,
            "clip_url",
            &json!({"name": "A001C001.mov"}).as_object().unwrap().clone(),
        );
        all.extend(cx.take());
        let value = match &completed(&all)[0].1 {
            Ok(Outcome::Value { value }) => value.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            value["url"],
            format!("http://10.0.0.7:8790/v1/clips/A001C001.mov?key={KEY}")
        );
        for a in &all {
            match a {
                Action::Log { message, .. } => assert!(!message.contains(KEY)),
                Action::State(p) => assert!(!p.to_string().contains(KEY)),
                Action::Message { args, .. } => assert!(!args.to_string().contains(KEY)),
                _ => {}
            }
        }
    }

    #[test]
    fn a_wrong_key_is_unauthorized_and_final() {
        let mut m = keyed(true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let (id, _) = requests(&cx.take())[0].clone();
        let a = respond(&mut m, 1, id, 200, hello());
        let (id, _) = requests(&a)[0].clone();
        let a = respond(
            &mut m,
            2,
            id,
            401,
            json!({"error": "this camera's fleet key is needed"}),
        );
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(requests(&a).is_empty());
        let mut cx = Cx::new(3);
        m.command(&mut cx, 9, "start_recording", &Params::new());
        let done = completed(&cx.take());
        assert!(matches!(done[0].1, Err(CommandError::Auth { .. })));
        // The consumer's reconnect asks again.
        let mut cx = Cx::new(4);
        assert!(m.reconnect(&mut cx));
        assert_eq!(paths(&cx.take()), ["GET /v1/hello"]);
    }

    #[test]
    fn status_lines_become_state_and_derived_events() {
        let mut m = keyed(true);
        streaming(&mut m);
        let mut doc = status();
        doc["recording"] = json!(true);
        doc["clip"] = json!("A001C003.mov");
        doc["tally"] = json!("program");
        doc["warnings"] = json!(["Low battery"]);
        doc["battery"] = json!(15);
        doc["freeBytes"] = json!(5e9);
        doc["outputs"] =
            json!({"on": ["srt"], "problems": {"rtmp": "Enter an rtmp:// or rtmps:// URL"}});
        doc["settings"]["whiteBalance"]["kelvin"] = json!(3200);
        doc["error"] = json!("No space");
        let a = line(&mut m, 100, &doc);
        let s = state(&a);
        assert_eq!(s["recording"], true);
        assert_eq!(s["clip"], "A001C003.mov");
        assert_eq!(s["settings"], json!({"whiteBalance": {"kelvin": 3200}}));
        let got: Vec<String> = messages(&a).into_iter().map(|(k, _)| k).collect();
        for want in [
            "record_started",
            "tally_changed",
            "error",
            "warning",
            "battery_low",
            "storage_low",
            "output_up",
            "output_problem",
            "settings_changed",
        ] {
            assert!(got.contains(&want.to_string()), "{want} in {got:?}");
        }
        let changed = messages(&a)
            .into_iter()
            .find(|(k, _)| k == "settings_changed")
            .unwrap()
            .1;
        assert_eq!(changed["paths"], json!(["whiteBalance.kelvin"]));
        assert!(a.contains(&Action::Alive));
        // The same line again (a keepalive) changes nothing.
        let a = line(&mut m, 5000, &doc);
        assert!(state(&a).as_object().unwrap().is_empty());
        assert!(messages(&a).is_empty());
        // Stopping reads the clip list again.
        doc["recording"] = json!(false);
        doc["clip"] = Value::Null;
        doc["clips"] = json!(3);
        let a = line(&mut m, 6000, &doc);
        assert_eq!(state(&a)["clip"], Value::Null);
        assert!(messages(&a)
            .iter()
            .any(|(k, v)| k == "record_stopped" && v["clip"] == "A001C003.mov"));
        assert_eq!(paths(&a), ["GET /v1/clips"]);
    }

    #[test]
    fn a_silent_event_stream_reads_the_status_again() {
        let mut m = keyed(true);
        streaming(&mut m);
        line(&mut m, 100, &status());
        let mut cx = Cx::new(10_100);
        m.timer(&mut cx, EVENTS_SILENCE);
        let a = cx.take();
        assert!(a.contains(&Action::TcpClose { socket: EVENTS }));
        assert_eq!(paths(&a), ["GET /v1/status"]);
        let (id, _) = requests(&a)[0].clone();
        let a = respond(&mut m, 10_200, id, 200, status());
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::TcpOpen { socket: EVENTS, .. })));
        // Still connected throughout.
        assert!(!a.iter().any(|x| matches!(x, Action::Connection(_))));
    }

    #[test]
    fn a_camera_that_goes_away_is_disconnected_and_retried() {
        let mut m = keyed(true);
        streaming(&mut m);
        line(&mut m, 100, &status());
        let mut cx = Cx::new(200);
        m.tcp(
            &mut cx,
            EVENTS,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        let (id, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(5_200);
        m.http_response(&mut cx, id, Err("operation timed out".into()));
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: 1_000
        }));
        let mut cx = Cx::new(6_200);
        m.timer(&mut cx, RETRY);
        assert_eq!(paths(&cx.take()), ["GET /v1/hello"]);
    }

    #[test]
    fn commands_only_reads_the_hello_and_nothing_else() {
        let mut m = keyed(false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert_eq!(paths(&a), ["GET /v1/hello"]);
        let (id, _) = requests(&a)[0].clone();
        let a = respond(&mut m, 4, id, 200, hello());
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(requests(&a).is_empty());
        assert!(!a.iter().any(|x| matches!(x, Action::TcpOpen { .. })));
        assert!(a.contains(&Action::SetTimer {
            key: LIVENESS,
            after: LIVENESS_EVERY
        }));
        assert_eq!(state(&a)["name"], "Camera A");
        // Liveness: the hello again.
        let mut cx = Cx::new(10_004);
        m.timer(&mut cx, LIVENESS);
        assert_eq!(paths(&cx.take()), ["GET /v1/hello"]);
        // The first op command reads the capabilities, then is sent.
        let a = command(&mut m, 10_010, 3, "set_tally", json!({"light": "program"}));
        assert_eq!(paths(&a), ["POST /v1/command capabilities"]);
        let a = respond(&mut m, 10_015, requests(&a)[0].0, 200, caps());
        assert!(completed(&a).is_empty());
        let (id, r) = requests(&a)[0].clone();
        assert_eq!(body(&r), json!({"op": "tally", "light": "program"}));
        // Its answer reaches state.
        let a = respond(&mut m, 10_025, id, 200, json!({"tally": "program"}));
        assert_eq!(state(&a)["tally"], "program");
        assert!(a.iter().any(|x| matches!(x, Action::RoundTrip(10))));
        assert!(matches!(completed(&a)[0].1, Ok(Outcome::Value { .. })));
        // The next is sent at once.
        let a = command(&mut m, 10_030, 4, "start_recording", json!({}));
        assert_eq!(paths(&a), ["POST /v1/command record"]);
    }

    #[test]
    fn commands_fail_clearly_when_the_capabilities_cannot_be_read() {
        let mut m = keyed(false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let (id, _) = requests(&cx.take())[0].clone();
        respond(&mut m, 4, id, 200, hello());
        let a = command(&mut m, 10, 3, "set_tally", json!({"light": "program"}));
        let caps_id = requests(&a)[0].0;
        // A second command waits on the same read.
        let a = command(&mut m, 11, 4, "start_recording", json!({}));
        assert!(requests(&a).is_empty());
        let a = respond(&mut m, 20, caps_id, 200, json!({"error": "not now"}));
        let done = completed(&a);
        assert_eq!(done.len(), 2);
        for (_, result) in done {
            match result {
                Err(CommandError::DeviceError { code, message }) => {
                    assert_eq!(code.as_deref(), Some("capabilities_unavailable"));
                    assert!(message.contains("not now"), "{message}");
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(requests(&a).is_empty());
        // The next command tries again.
        let a = command(&mut m, 30, 5, "start_recording", json!({}));
        assert_eq!(paths(&a), ["POST /v1/command capabilities"]);
        let mut cx = Cx::new(40);
        m.http_response(
            &mut cx,
            requests(&a)[0].0,
            Err("operation timed out".into()),
        );
        assert!(matches!(
            &completed(&cx.take())[0].1,
            Err(CommandError::DeviceError { code: Some(c), .. }) if c == "capabilities_unavailable"
        ));
    }

    #[test]
    fn ops_the_camera_does_not_offer_fail_before_sending() {
        let mut m = keyed(true);
        let a = connected(&mut m);
        let (id, _) = requests(&a)[0].clone();
        let caps = json!({
            "protocol": 1, "ops": ["capabilities", "settings", "record"],
            "capabilities": {"model": "SM-S926U", "lenses": []},
            "settings": status()["settings"], "resolved": {"width": 1920},
            "monitoring": {"zebra": false}, "luts": ["rec709"], "presets": ["Wide"],
            "outputs": {"rtmp": {"on": false, "url": "", "key": "secret", "keySet": true}},
        });
        let a = respond(&mut m, 30, id, 200, caps);
        let s = state(&a);
        assert_eq!(s["ops"], json!(["capabilities", "settings", "record"]));
        assert_eq!(
            s["output_settings"]["rtmp"],
            json!({"on": false, "url": "", "keySet": true})
        );
        assert_eq!(s["monitoring"]["zebra"], false);
        let mut cx = Cx::new(40);
        m.command(
            &mut cx,
            5,
            "set_tally",
            &json!({"light": "preview"}).as_object().unwrap().clone(),
        );
        let a = cx.take();
        assert!(requests(&a).is_empty());
        match &completed(&a)[0].1 {
            Err(CommandError::DeviceError { code, message }) => {
                assert_eq!(code.as_deref(), Some("unsupported_op"));
                assert!(message.contains("'tally'"), "{message}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_refusal_in_the_answer_fails_the_command() {
        let mut m = keyed(true);
        ready(&mut m);
        let mut cx = Cx::new(50);
        m.command(
            &mut cx,
            6,
            "recall_preset",
            &json!({"name": "Nope"}).as_object().unwrap().clone(),
        );
        let (id, _) = requests(&cx.take())[0].clone();
        let a = respond(&mut m, 60, id, 200, json!({"error": "no preset Nope"}));
        assert_eq!(
            completed(&a)[0].1,
            Err(CommandError::DeviceError {
                code: None,
                message: "no preset Nope".into()
            })
        );
        // The status carries the same error; a command that succeeds (error
        // null) clears it.
        assert_eq!(state(&a)["error"], "no preset Nope");
        let a = command(&mut m, 62, 9, "set_tally", json!({"light": "off"}));
        let a = respond(
            &mut m,
            64,
            requests(&a)[0].0,
            200,
            json!({"tally": "off", "error": null}),
        );
        assert!(completed(&a)[0].1.is_ok());
        assert!(a.contains(&Action::State(json!({"error": null}))));
        // Any string error fails its command, even an empty one.
        let a = command(&mut m, 66, 10, "set_tally", json!({"light": "off"}));
        let a = respond(&mut m, 68, requests(&a)[0].0, 200, json!({"error": ""}));
        assert!(matches!(
            completed(&a)[0].1,
            Err(CommandError::DeviceError { .. })
        ));
        let mut cx = Cx::new(70);
        m.command(&mut cx, 7, "start_recording", &Params::new());
        let (id, _) = requests(&cx.take())[0].clone();
        let a = respond(
            &mut m,
            80,
            id,
            504,
            json!({"error": "the camera didn't answer"}),
        );
        assert_eq!(completed(&a)[0].1, Err(CommandError::Timeout));
    }

    #[test]
    fn typed_commands_build_partial_documents() {
        let cases = [
            (
                "set_exposure",
                json!({"mode": "manual", "iso": 800, "shutter_angle": 180}),
                json!({"op": "settings", "settings": {"exposure": {"mode": "manual", "iso": 800, "shutter": {"angle": 180}}}}),
            ),
            (
                "set_white_balance",
                json!({"mode": "manual", "kelvin": 3200, "tint": 0}),
                json!({"op": "settings", "settings": {"whiteBalance": {"mode": "manual", "kelvin": 3200, "tint": 0}}}),
            ),
            (
                "measure_white_balance",
                json!({"x": 0.5, "y": 0.25}),
                json!({"op": "settings", "settings": {"whiteBalance": {"point": [0.5, 0.25]}}}),
            ),
            (
                "set_zoom",
                json!({"ratio": 2.0}),
                json!({"op": "settings", "settings": {"zoom": 2.0}}),
            ),
            (
                "set_recording_settings",
                json!({"codec": "hevc", "bitrate_mbps": 0, "audio_input": ""}),
                json!({"op": "settings", "settings": {"recording": {"codec": "hevc", "bitrateMbps": null, "audioInput": null}}}),
            ),
            (
                "focus_at",
                json!({"x": 0.1, "y": 0.9}),
                json!({"op": "focusAt", "x": 0.1, "y": 0.9, "lock": false}),
            ),
            (
                "set_zebra",
                json!({"enabled": true, "level": 0.9}),
                json!({"op": "monitoring", "monitoring": {"zebra": true, "zebraLevel": 0.9}}),
            ),
            (
                "set_rtmp_output",
                json!({"enabled": true, "url": "rtmp://x/live", "stream_key": "k"}),
                json!({"op": "outputs", "outputs": {"rtmp": {"on": true, "url": "rtmp://x/live", "key": "k"}}}),
            ),
            (
                "set_output_format",
                json!({"height": "720"}),
                json!({"op": "outputs", "outputs": {"format": {"height": 720}}}),
            ),
            (
                "set_talkback",
                json!({"enabled": true}),
                json!({"op": "talkback", "on": true}),
            ),
            (
                "new_talkback_address",
                json!({}),
                json!({"op": "talkback", "newAddress": true}),
            ),
            (
                "set_return",
                json!({"kind": "srt", "enabled": true}),
                json!({"op": "return", "kind": "srt", "on": true}),
            ),
            (
                "circle_clip",
                json!({"name": "A001C001.mov"}),
                json!({"op": "circle", "name": "A001C001.mov", "on": true}),
            ),
            (
                "visca",
                json!({"fields": {"action": "zoom", "direction": 1, "speed": 3}}),
                json!({"op": "visca", "action": "zoom", "direction": 1, "speed": 3}),
            ),
            (
                "send_op",
                json!({"op": "future", "fields": {"a": 1}}),
                json!({"op": "future", "a": 1}),
            ),
        ];
        for (name, params, want) in cases {
            let got = op_body(name, params.as_object().unwrap()).unwrap();
            assert_eq!(got, want, "{name}");
        }
        assert!(op_body("set_exposure", &Params::new()).is_err());
        assert!(op_body(
            "set_exposure",
            json!({"shutter_angle": 180, "shutter_seconds": 0.01})
                .as_object()
                .unwrap()
        )
        .is_err());
    }

    #[test]
    fn credentials_are_dropped_from_answers_and_state() {
        let mut m = keyed(true);
        ready(&mut m);
        let mut cx = Cx::new(50);
        m.command(
            &mut cx,
            8,
            "set_whip_output",
            &json!({"enabled": true, "url": "https://w/whip", "token": "tok"})
                .as_object()
                .unwrap()
                .clone(),
        );
        let (id, _) = requests(&cx.take())[0].clone();
        let answer = json!({"outputs": {"whip": {"on": true, "url": "https://w/whip", "token": "tok", "tokenSet": true}},
                            "problems": {}});
        let a = respond(&mut m, 60, id, 200, answer);
        assert!(!state(&a).to_string().contains("tok\""));
        match &completed(&a)[0].1 {
            Ok(Outcome::Value { value }) => {
                assert!(value["outputs"]["whip"].get("token").is_none())
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn monitoring_and_output_settings_come_from_the_status_without_credentials() {
        const SECRET: &str = "wr0ngly-s3nt";
        let mut m = keyed(true);
        ready(&mut m);
        let mut cx = Cx::new(20);
        m.tcp(&mut cx, EVENTS, TcpInput::Connected);
        m.tcp(
            &mut cx,
            EVENTS,
            TcpInput::Data(b"HTTP/1.1 200 OK\r\n\r\n".to_vec()),
        );
        cx.take();
        let mut all = Vec::new();
        // A status without them leaves the capabilities answer's in state.
        let a = line(&mut m, 90, &status());
        assert_eq!(m.mirror["monitoring"], json!({"zebra": false}));
        all.extend(a);
        let mut doc = status();
        doc["monitoring"] = json!({"zebra": false, "lut": "none"});
        doc["outputSettings"] = json!({
            "rtmp": {"on": false, "url": "", "key": SECRET},
            "whip": {"on": false, "url": "", "token": "", "tokenSet": false},
            "srt": {"on": false, "passphrase": SECRET},
            "format": {"height": 1080}
        });
        let a = line(&mut m, 100, &doc);
        assert_eq!(
            m.mirror["monitoring"],
            json!({"zebra": false, "lut": "none"})
        );
        assert_eq!(
            m.mirror["output_settings"],
            json!({
                "rtmp": {"on": false, "url": "", "keySet": true},
                "whip": {"on": false, "url": "", "tokenSet": false},
                "srt": {"on": false, "passphraseSet": true},
                "format": {"height": 1080}
            })
        );
        all.extend(a);
        // Whoever changed them, the change is in state and settings_changed.
        doc["monitoring"]["zebra"] = json!(true);
        doc["outputSettings"]["format"]["height"] = json!(720);
        let a = line(&mut m, 110, &doc);
        assert_eq!(state(&a)["monitoring"]["zebra"], true);
        assert_eq!(state(&a)["output_settings"]["format"]["height"], 720);
        assert!(messages(&a).contains(&(
            "settings_changed".into(),
            json!({"paths": ["monitoring.zebra", "output_settings.format.height"]})
        )));
        all.extend(a);
        // get_status returns it without the credential.
        let a = command(&mut m, 130, 2, "get_status", json!({}));
        let a2 = respond(&mut m, 140, requests(&a)[0].0, 200, doc.clone());
        match &completed(&a2)[0].1 {
            Ok(Outcome::Value { value }) => {
                assert_eq!(value["outputSettings"]["rtmp"]["keySet"], true)
            }
            other => panic!("{other:?}"),
        }
        all.extend(a2);
        assert!(!format!("{all:?}").contains(SECRET));
        assert!(!m.mirror.to_string().contains(SECRET));
        assert!(!format!("{:?}", m.last_status).contains(SECRET));
    }

    #[test]
    fn the_talkback_address_is_returned_only_by_get_talkback_url() {
        const URL: &str = "https://10.0.0.7:8791/s3cr3tpath/";
        let mut m = keyed(true);
        ready(&mut m);
        let mut all = Vec::new();
        // The status carries it while talkback is on.
        let mut cx = Cx::new(20);
        m.tcp(&mut cx, EVENTS, TcpInput::Connected);
        m.tcp(
            &mut cx,
            EVENTS,
            TcpInput::Data(b"HTTP/1.1 200 OK\r\n\r\n".to_vec()),
        );
        all.extend(cx.take());
        let mut doc = status();
        doc["talkback"] = json!(URL);
        doc["talkbackOn"] = json!(true);
        doc["talkbackLatch"] = json!(true);
        let a = line(&mut m, 100, &doc);
        assert_eq!(
            state(&a)["talkback"],
            json!({"on": true, "latch": true, "url_set": true})
        );
        assert!(messages(&a)
            .iter()
            .any(|(k, v)| k == "talkback_changed" && *v == json!({"on": true, "latch": true})));
        all.extend(a);
        // The talkback op's answer, and get_status, show it only as set.
        let a = command(&mut m, 110, 1, "new_talkback_address", json!({}));
        let a2 = respond(
            &mut m,
            120,
            requests(&a)[0].0,
            200,
            json!({"on": true, "latch": true, "url": URL, "error": null}),
        );
        match &completed(&a2)[0].1 {
            Ok(Outcome::Value { value }) => assert_eq!(value["url"], "set"),
            other => panic!("{other:?}"),
        }
        all.extend(a2);
        let a = command(&mut m, 130, 2, "get_status", json!({}));
        let a2 = respond(&mut m, 140, requests(&a)[0].0, 200, doc.clone());
        match &completed(&a2)[0].1 {
            Ok(Outcome::Value { value }) => assert_eq!(value["talkback"], "set"),
            other => panic!("{other:?}"),
        }
        all.extend(a2);
        // get_talkback_url gives it, for the director joining.
        let a = command(&mut m, 150, 3, "get_talkback_url", json!({}));
        assert_eq!(paths(&a), ["GET /v1/status"]);
        let a2 = respond(&mut m, 160, requests(&a)[0].0, 200, doc.clone());
        assert_eq!(
            completed(&a2)[0].1,
            Ok(Outcome::Value {
                value: json!({"on": true, "url": URL})
            })
        );
        // Nowhere else: not in state, logs or messages.
        for a in all.iter().chain(&a2) {
            match a {
                Action::Log { message, .. } => assert!(!message.contains("s3cr3t")),
                Action::State(p) => assert!(!p.to_string().contains("s3cr3t")),
                Action::Message { args, .. } => assert!(!args.to_string().contains("s3cr3t")),
                _ => {}
            }
        }
    }

    fn refused(a: &[Action]) -> String {
        assert!(requests(a).is_empty(), "nothing is sent");
        match &completed(a)[0].1 {
            Err(CommandError::InvalidParams { message }) => message.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn values_outside_the_cameras_ranges_fail_before_sending() {
        let mut m = keyed(true);
        ready(&mut m);
        assert_eq!(m.mirror["limits"]["outputs"]["bitrateMbps"]["max"], 40);
        let cases = [
            ("set_exposure", json!({"iso": 6400}), "exposure.iso 6400"),
            ("set_exposure", json!({"ev": 3}), "exposure.ev 3"),
            (
                "set_exposure",
                json!({"shutter_seconds": 0.5}),
                "shutter 0.5 s",
            ),
            // 360° at 1 fps is 1 s, past the lens's 0.1 s.
            (
                "set_settings",
                json!({"settings": {"fps": 1, "exposure": {"shutter": {"angle": 360}}}}),
                "shutter 360° at 1 fps",
            ),
            (
                "set_white_balance",
                json!({"kelvin": 9000}),
                "whiteBalance.kelvin 9000",
            ),
            ("set_zoom", json!({"ratio": 10}), "zoom 10"),
            (
                "set_format",
                json!({"width": 2560, "height": 1440}),
                "2560x1440",
            ),
            ("set_format", json!({"fps": 60}), "60 fps"),
            (
                "set_output_format",
                json!({"bitrate_mbps": 45}),
                "format.bitrateMbps 45",
            ),
            (
                "set_outputs",
                json!({"outputs": {"hls": {"port": 80}}}),
                "hls.port 80",
            ),
            (
                "set_outputs",
                json!({"outputs": {"format": {"height": 480}}}),
                "format.height 480",
            ),
            (
                "set_monitoring",
                json!({"monitoring": {"desqueeze": 3}}),
                "desqueeze 3",
            ),
        ];
        for (n, (name, p, want)) in cases.into_iter().enumerate() {
            let a = command(&mut m, 50, 100 + n as u64, name, p);
            let message = refused(&a);
            assert!(message.contains(want), "{name}: {message}");
        }
        // Within range, including a manual-exposure rate and a high-speed
        // format, they are sent.
        for (name, p) in [
            ("set_exposure", json!({"iso": 800, "shutter_angle": 180})),
            ("set_format", json!({"fps": 50})),
            (
                "set_format",
                json!({"width": 1280, "height": 720, "fps": 240}),
            ),
            ("set_white_balance", json!({"kelvin": 3200})),
            ("set_output_format", json!({"bitrate_mbps": 40})),
        ] {
            let a = command(&mut m, 60, 200, name, p);
            assert_eq!(requests(&a).len(), 1, "{name}");
        }
    }

    #[test]
    fn omt_quality_is_one_of_the_cameras_qualities() {
        // limits without omtQuality: §7.6's low, standard and high.
        let mut m = keyed(true);
        ready(&mut m);
        assert!(m.mirror["limits"]["outputs"].get("omtQuality").is_none());
        for (n, (name, p, want)) in [
            (
                "set_omt_output",
                json!({"quality": "best"}),
                "omt.quality best is not one of the camera's OMT qualities (low, standard, high)",
            ),
            (
                "set_outputs",
                json!({"outputs": {"omt": {"quality": "ultra"}}}),
                "omt.quality ultra",
            ),
            (
                "set_outputs",
                json!({"outputs": {"omt": {"quality": 2}}}),
                "omt.quality 2",
            ),
            (
                "send_op",
                json!({"op": "outputs", "fields": {"outputs": {"omt": {"quality": null}}}}),
                "omt.quality null",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let message = refused(&command(&mut m, 50, 100 + n as u64, name, p));
            assert!(message.contains(want), "{name}: {message}");
        }
        for (n, q) in ["low", "standard", "high"].into_iter().enumerate() {
            let a = command(
                &mut m,
                60,
                200 + n as u64,
                "set_omt_output",
                json!({"enabled": true, "quality": q}),
            );
            assert_eq!(
                body(&requests(&a)[0].1),
                json!({"op": "outputs", "outputs": {"omt": {"on": true, "quality": q}}})
            );
        }

        // A camera listing fewer is held to its own list.
        let mut m = keyed(true);
        let a = connected(&mut m);
        let r = requests(&a);
        let mut c = caps();
        c["limits"]["outputs"]["omtQuality"] = json!(["standard", "high"]);
        c["outputs"]["omt"] = json!({"on": false, "name": "Camera A", "quality": "standard"});
        let a = respond(&mut m, 12, r[0].0, 200, c);
        assert_eq!(state(&a)["output_settings"]["omt"]["quality"], "standard");
        respond(&mut m, 13, r[1].0, 200, json!([]));
        let message = refused(&command(
            &mut m,
            50,
            1,
            "set_omt_output",
            json!({"quality": "low"}),
        ));
        assert!(message.contains("(standard, high)"), "{message}");
        // The outputs answer carries the new quality into state.
        let a = command(&mut m, 60, 2, "set_omt_output", json!({"quality": "high"}));
        let a = respond(
            &mut m,
            70,
            requests(&a)[0].0,
            200,
            json!({"outputs": {"omt": {"on": false, "name": "Camera A", "quality": "high"}},
                   "problems": {}, "error": null}),
        );
        assert_eq!(state(&a)["output_settings"]["omt"]["quality"], "high");
    }

    #[test]
    fn omt_quality_changes_follow_the_status() {
        let mut m = keyed(true);
        streaming(&mut m);
        let mut doc = status();
        doc["outputSettings"] =
            json!({"omt": {"on": true, "name": "Camera A", "quality": "standard"}});
        let a = line(&mut m, 100, &doc);
        assert_eq!(state(&a)["output_settings"]["omt"]["quality"], "standard");
        doc["outputSettings"]["omt"]["quality"] = json!("low");
        let a = line(&mut m, 110, &doc);
        assert_eq!(state(&a)["output_settings"]["omt"]["quality"], "low");
        assert!(messages(&a).contains(&(
            "settings_changed".into(),
            json!({"paths": ["output_settings.omt.quality"]})
        )));
    }

    #[test]
    fn a_status_without_running_gives_it_from_resolved() {
        let mut m = keyed(true);
        streaming(&mut m);
        let mut doc = status();
        doc.as_object_mut().unwrap().remove("running");
        doc["resolved"]["fps"] = json!(30);
        let a = line(&mut m, 100, &doc);
        assert_eq!(state(&a)["running"], json!({"fps": 30}));
        assert_eq!(state(&a)["resolved"], json!({"fps": 30}));
    }

    fn clip_response(
        m: &mut MerosCamera,
        id: RequestId,
        status: u16,
        range: Option<&str>,
        body: &[u8],
    ) -> Vec<Action> {
        let mut cx = Cx::new(90);
        let headers = range
            .map(|r| vec![("content-range".to_string(), r.to_string())])
            .unwrap_or_default();
        m.http_response(
            &mut cx,
            id,
            Ok(HttpResponse {
                status,
                headers,
                body: body.to_vec(),
            }),
        );
        cx.take()
    }

    #[test]
    fn clips_are_read_in_ranges() {
        let mut m = keyed(true);
        connected(&mut m);
        let mut cx = Cx::new(80);
        let p = json!({"name": "A001 C001.mov", "offset": 10, "length": 4});
        m.command(&mut cx, 11, "read_clip", p.as_object().unwrap());
        let (id, r) = requests(&cx.take())[0].clone();
        assert!(r.url.ends_with("/v1/clips/A001%20C001.mov"));
        assert!(r.headers.contains(&("Range".into(), "bytes=10-13".into())));
        let a = clip_response(&mut m, id, 206, Some("bytes 10-13/14"), b"abcd");
        match &completed(&a)[0].1 {
            Ok(Outcome::Value { value }) => {
                assert_eq!(value["data"], "YWJjZA==");
                assert_eq!(value["total"], 14);
                assert_eq!(value["eof"], true);
            }
            other => panic!("{other:?}"),
        }
        let mut cx = Cx::new(80);
        m.command(
            &mut cx,
            12,
            "read_clip",
            json!({"name": "x.mov", "offset": 99, "length": 4})
                .as_object()
                .unwrap(),
        );
        let (id, _) = requests(&cx.take())[0].clone();
        let a = clip_response(&mut m, id, 416, Some("bytes */14"), b"");
        match &completed(&a)[0].1 {
            Ok(Outcome::Value { value }) => {
                assert_eq!(value["length"], 0);
                assert_eq!(value["eof"], true);
                assert_eq!(value["total"], 14);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_clip_list_is_kept_by_name() {
        let mut m = keyed(true);
        let a = connected(&mut m);
        let (id, _) = requests(&a)[1].clone();
        let list = json!([
            {"name": "A001C001.mov", "size": 65536, "modified": 1790000000, "circled": false},
            {"name": "A001C002.mov", "size": 131072, "modified": 1790000100, "circled": true},
        ]);
        let a = respond(&mut m, 30, id, 200, list);
        assert_eq!(state(&a)["clips"]["A001C002.mov"]["circled"], true);
        // A clip gone from the list leaves state.
        m.refresh_clips(&mut Cx::new(31));
        let id = *m.requests.keys().max().unwrap();
        let a = respond(
            &mut m,
            40,
            id,
            200,
            json!([{"name": "A001C002.mov", "size": 131072, "modified": 1790000100, "circled": true}]),
        );
        assert!(a.contains(&Action::State(json!({"clips": {"A001C001.mov": null}}))));
    }

    #[test]
    fn the_preview_runs_only_while_watched() {
        let mut m = keyed(true);
        connected(&mut m);
        let mut cx = Cx::new(100);
        m.stream_watch(&mut cx, LIVE, true);
        let a = cx.take();
        assert!(a.contains(&Action::TcpOpen {
            socket: PREVIEW,
            to: "10.0.0.7:8790".parse().unwrap()
        }));
        let mut cx = Cx::new(110);
        m.tcp(&mut cx, PREVIEW, TcpInput::Connected);
        let mut frame = 4u32.to_be_bytes().to_vec();
        frame.extend_from_slice(&7i64.to_be_bytes());
        frame.push(1);
        frame.extend_from_slice(&[0, 0, 1, 0x65]);
        let mut data =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/x-meros-preview\r\n\r\n".to_vec();
        data.extend_from_slice(&frame);
        m.tcp(&mut cx, PREVIEW, TcpInput::Data(data));
        let a = cx.take();
        assert!(a.contains(&Action::Frame {
            stream: LIVE,
            format: FRAME_FORMAT,
            data: frame
        }));
        let mut cx = Cx::new(120);
        m.stream_watch(&mut cx, LIVE, false);
        assert!(cx.take().contains(&Action::TcpClose { socket: PREVIEW }));
    }

    #[test]
    fn a_corrupt_preview_frame_reopens_the_preview() {
        let mut m = keyed(true);
        connected(&mut m);
        let mut cx = Cx::new(100);
        m.stream_watch(&mut cx, LIVE, true);
        m.tcp(&mut cx, PREVIEW, TcpInput::Connected);
        let mut data = b"HTTP/1.1 200 OK\r\n\r\n".to_vec();
        data.extend_from_slice(&((8u32 << 20) + 1).to_be_bytes());
        data.extend_from_slice(&[0; 9]);
        cx.take();
        let mut cx = Cx::new(110);
        m.tcp(&mut cx, PREVIEW, TcpInput::Data(data));
        let a = cx.take();
        assert!(a.contains(&Action::TcpClose { socket: PREVIEW }));
        assert!(a.contains(&Action::SetTimer {
            key: PREVIEW_RETRY,
            after: 1_000
        }));
        let mut cx = Cx::new(1_110);
        m.timer(&mut cx, PREVIEW_RETRY);
        assert!(cx.take().iter().any(|x| matches!(
            x,
            Action::TcpOpen {
                socket: PREVIEW,
                ..
            }
        )));
    }

    #[test]
    fn a_fleet_link_is_decoded_without_the_camera() {
        use base64::Engine;
        let mut m = camera(json!({}), true);
        let packed =
            miniz_oxide::deflate::compress_to_vec(json!({ "key": KEY }).to_string().as_bytes(), 9);
        let link = format!(
            "meros-camera://fleet?d={}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(packed)
        );
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            1,
            "decode_fleet_link",
            json!({ "link": link }).as_object().unwrap(),
        );
        let a = cx.take();
        assert!(requests(&a).is_empty());
        assert_eq!(
            completed(&a)[0].1,
            Ok(Outcome::Value {
                value: json!({ "key": KEY })
            })
        );
    }

    #[test]
    fn diffs_are_minimal_merge_patches() {
        let old = json!({"a": 1, "b": {"c": 2, "d": 3}, "e": [1]});
        let new = json!({"a": 1, "b": {"c": 2}, "e": [1, 2], "f": {"g": null, "h": 1}});
        assert_eq!(
            diff(Some(&old), &new),
            Some(json!({"b": {"d": null}, "e": [1, 2], "f": {"h": 1}}))
        );
        assert_eq!(diff(Some(&old), &old), None);
    }
}
