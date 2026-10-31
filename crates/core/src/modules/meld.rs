//! Meld Studio's WebChannel API: Qt's WebChannel protocol, JSON messages on a
//! WebSocket (TCP 13376), publishing one object, `meld`.
//!
//! The API's methods, signals and properties are Meld's documentation
//! (WebChannelAPI.md); the wire format is read from the Apache-2.0 Macro
//! Deck client (see the spec's sources). Every message is a JSON object with
//! a numeric `type`:
//!
//! - The client opens with init (3) and an id; the response (10) with that
//!   id names, for each published object, its methods and signals as
//!   `[name, index]` pairs and its properties as `[index, name, notify,
//!   value]`, the values being the properties' current ones.
//! - The client then says it is idle (4). The server sends property updates
//!   (2), `{"data": [{"object", "properties": {index: value}}]}`, only while
//!   the client is idle, so the client says so again after each one.
//! - A method is invoked (6) by its index with `args` and an id, and
//!   answered by a response with that id.
//! - A signal is subscribed to (7) by its index, and arrives as a signal
//!   message (1) with its `args`.
//!
//! Monitored, the module mirrors Meld's properties as state: the session
//! (scenes, layers, tracks and effects, keyed by id) whole, deleting what a
//! new session no longer holds, and isStreaming and isRecording. It
//! subscribes to gainUpdated and registers every track for it, since the
//! session carries no gain. Opened for commands only, it still reads the
//! session (the layer and effect toggles name the scene holding the item)
//! but publishes nothing and registers nothing. A WebSocket ping every
//! [`PING_EVERY`] is its liveness check, since Meld sends nothing while idle.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    WsInput, WsRequest,
};

const PORT: u16 = 13376;
const SOCKET: Key = "meld";
const OBJECT: &str = "meld";
/// The context Meld's track observers are registered under.
const OBSERVER: &str = "meros";

const TYPE_SIGNAL: u64 = 1;
const TYPE_PROPERTY_UPDATE: u64 = 2;
const TYPE_INIT: u64 = 3;
const TYPE_IDLE: u64 = 4;
const TYPE_INVOKE: u64 = 6;
const TYPE_CONNECT_TO_SIGNAL: u64 = 7;
const TYPE_RESPONSE: u64 = 10;

const RETRY: Key = "retry";
const INIT: Key = "init";
const PING: Key = "ping";
const REQUEST: Key = "request";

const INIT_TIMEOUT: Millis = 5_000;
const REQUEST_TIMEOUT: Millis = 5_000;
const PING_EVERY: Millis = 5_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Opening,
    Init,
    Ready,
}

/// One method the object offers: its name as listed (a plain name, or a
/// signature such as `sendStreamEvent(QString)`) and its index.
#[derive(Debug, Clone)]
struct Method {
    name: String,
    index: u64,
}

/// A call to make: a method by name and its arguments.
#[derive(Debug, Clone, PartialEq)]
struct Call {
    method: &'static str,
    args: Vec<Value>,
}

fn call(method: &'static str, args: Vec<Value>) -> Call {
    Call { method, args }
}

/// What a command comes to.
#[derive(Debug, PartialEq)]
enum Plan {
    /// These calls, the command answered once all are.
    Calls(Vec<Call>),
    /// Nothing to send: already as asked.
    Done,
    /// The session, as a value.
    Session,
}

pub(crate) struct Meld {
    url: String,
    monitor: bool,
    phase: Phase,
    retry_after: Millis,
    next_id: u64,
    init_id: Option<u64>,
    methods: Vec<Method>,
    signals: HashMap<u64, String>,
    signal_index: HashMap<String, u64>,
    properties: HashMap<u64, String>,
    version: i64,
    /// The session's items, keyed by id, as Meld last sent them.
    items: Map<String, Value>,
    gains: HashMap<String, f64>,
    observed: BTreeSet<String>,
    /// What has been published, to tell what a new session removes.
    published: Value,
    /// Calls in flight: message id to the command waiting on it, and when
    /// it times out.
    waiting: BTreeMap<u64, (Option<CommandId>, Millis)>,
    /// Calls each waiting command still has in flight.
    remaining: HashMap<CommandId, usize>,
    ping_out: Option<Millis>,
}

impl Meld {
    pub(crate) fn new(ctx: OpenContext) -> Meld {
        let host = match ctx.host {
            std::net::IpAddr::V6(v6) => format!("[{v6}]"),
            v4 => v4.to_string(),
        };
        Meld {
            url: format!("ws://{host}:{}/", ctx.port.unwrap_or(PORT)),
            monitor: ctx.monitor,
            phase: Phase::Closed,
            retry_after: RETRY_MIN,
            next_id: 1,
            init_id: None,
            methods: Vec::new(),
            signals: HashMap::new(),
            signal_index: HashMap::new(),
            properties: HashMap::new(),
            version: 1,
            items: Map::new(),
            gains: HashMap::new(),
            observed: BTreeSet::new(),
            published: json!({}),
            waiting: BTreeMap::new(),
            remaining: HashMap::new(),
            ping_out: None,
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        self.phase = Phase::Opening;
        cx.connection(Connection::Connecting);
        cx.ws_open(
            SOCKET,
            WsRequest {
                url: self.url.clone(),
                headers: Vec::new(),
                accept_invalid_certs: false,
            },
        );
        cx.set_timer(INIT, INIT_TIMEOUT);
    }

    fn send(&self, cx: &mut Cx, message: Value) {
        cx.ws_send(SOCKET, message.to_string());
    }

    fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// The index of `name` for a call with `argc` arguments: a signature
    /// with that many parameters if the object lists one, else the plain
    /// name.
    fn method_index(&self, name: &str, argc: usize) -> Option<u64> {
        let prefix = format!("{name}(");
        let by_signature = self.methods.iter().find(|m| {
            m.name.strip_prefix(&prefix).is_some_and(|rest| {
                let inner = rest.trim_end_matches(')');
                let count = if inner.is_empty() {
                    0
                } else {
                    inner.split(',').count()
                };
                count == argc
            })
        });
        by_signature
            .or_else(|| self.methods.iter().find(|m| m.name == name))
            .map(|m| m.index)
    }

    fn offers(&self, name: &str) -> bool {
        let prefix = format!("{name}(");
        self.methods
            .iter()
            .any(|m| m.name == name || m.name.starts_with(&prefix))
    }

    /// Sends a call; `None` when the object offers no such method.
    fn invoke(&mut self, cx: &mut Cx, call: &Call, command: Option<CommandId>) -> Option<()> {
        let index = self.method_index(call.method, call.args.len())?;
        let id = self.take_id();
        self.send(
            cx,
            json!({"type": TYPE_INVOKE, "object": OBJECT, "method": index, "args": call.args, "id": id}),
        );
        self.waiting
            .insert(id, (command, cx.now() + REQUEST_TIMEOUT));
        self.arm_requests(cx);
        Some(())
    }

    fn arm_requests(&self, cx: &mut Cx) {
        if let Some(due) = self.waiting.values().map(|(_, at)| *at).min() {
            cx.set_timer(REQUEST, due.saturating_sub(cx.now()).max(1));
        } else {
            cx.cancel_timer(REQUEST);
        }
    }

    fn item(&self, id: &str) -> Option<&Map<String, Value>> {
        self.items.get(id).and_then(Value::as_object)
    }

    fn parent_of(&self, id: &str) -> Option<String> {
        self.item(id)?
            .get("parent")
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    fn flag(&self, id: &str, field: &str) -> Option<bool> {
        self.item(id)?.get(field).and_then(Value::as_bool)
    }

    /// What a command comes to, or why it cannot be carried out.
    fn plan(&self, name: &str, params: &Params) -> Result<Plan, CommandError> {
        let s = |k: &str| {
            params
                .get(k)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| invalid(format!("'{k}' is required")))
        };
        let b = |k: &str| {
            params
                .get(k)
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid(format!("'{k}' is required")))
        };
        let f = |k: &str| {
            params
                .get(k)
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid(format!("'{k}' is required")))
        };
        let v = |k: &str| {
            params
                .get(k)
                .cloned()
                .ok_or_else(|| invalid(format!("'{k}' is required")))
        };
        let array = |k: &str| match params.get(k) {
            Some(Value::Array(a)) => Ok(a.clone()),
            _ => Err(invalid(format!("'{k}' must be a JSON array"))),
        };
        let command = |c: &str| Plan::Calls(vec![call("sendCommand", vec![json!(c)])]);
        let one = |c: Call| Plan::Calls(vec![c]);
        let toggle_if = |is: Option<bool>, want: bool, c: Call| match is {
            Some(is) if is == want => Plan::Done,
            _ => Plan::Calls(vec![c]),
        };
        let v2 = |what: &str| {
            if self.version >= 2 {
                Ok(())
            } else {
                Err(CommandError::DeviceError {
                    code: None,
                    message: format!(
                        "{what} needs Meld's WebChannel API version 2; this Meld offers version {}",
                        self.version
                    ),
                })
            }
        };
        let set_property = |id: &str, property: &str, value: Value| {
            call("setProperty", vec![json!(id), json!(property), value])
        };
        Ok(match name {
            "show_scene" => one(call("showScene", vec![json!(s("scene")?)])),
            "stage_scene" => one(call("setStagedScene", vec![json!(s("scene")?)])),
            "show_staged_scene" => one(call("showStagedScene", vec![])),
            "toggle_mute" => one(call("toggleMute", vec![json!(s("track")?)])),
            "set_muted" => {
                let track = s("track")?;
                let muted = b("muted")?;
                if self.offers("setMuted") {
                    one(call("setMuted", vec![json!(track), json!(muted)]))
                } else {
                    toggle_if(
                        self.flag(&track, "muted"),
                        muted,
                        call("toggleMute", vec![json!(track)]),
                    )
                }
            }
            "toggle_monitoring" => one(call("toggleMonitor", vec![json!(s("track")?)])),
            "set_monitoring" => {
                let track = s("track")?;
                toggle_if(
                    self.flag(&track, "monitoring"),
                    b("monitoring")?,
                    call("toggleMonitor", vec![json!(track)]),
                )
            }
            "set_gain" => one(call("setGain", vec![json!(s("track")?), json!(f("gain")?)])),
            "toggle_layer" => {
                let layer = s("layer")?;
                one(self.toggle_layer(&layer)?)
            }
            "set_layer_visible" => {
                let layer = s("layer")?;
                let visible = b("visible")?;
                if self.version >= 2 {
                    one(set_property(&layer, "visible", json!(visible)))
                } else {
                    toggle_if(
                        self.flag(&layer, "visible"),
                        visible,
                        self.toggle_layer(&layer)?,
                    )
                }
            }
            "toggle_effect" => {
                let effect = s("effect")?;
                one(self.toggle_effect(&effect)?)
            }
            "set_effect_enabled" => {
                let effect = s("effect")?;
                let enabled = b("enabled")?;
                if self.version >= 2 {
                    one(set_property(&effect, "enabled", json!(enabled)))
                } else {
                    toggle_if(
                        self.flag(&effect, "enabled"),
                        enabled,
                        self.toggle_effect(&effect)?,
                    )
                }
            }
            "toggle_recording" => one(call("toggleRecord", vec![])),
            "start_recording" => command("meld.startRecordingAction"),
            "stop_recording" => command("meld.stopRecordingAction"),
            "toggle_streaming" => one(call("toggleStream", vec![])),
            "start_streaming" => command("meld.startStreamingAction"),
            "stop_streaming" => command("meld.stopStreamingAction"),
            "toggle_virtual_camera" => command("meld.toggleVirtualCameraAction"),
            "take_screenshot" => command("meld.screenshot"),
            "take_vertical_screenshot" => command("meld.screenshot.vertical"),
            "record_clip" => command("meld.recordClip"),
            "record_clip_seconds" => one(call(
                "sendCommandWithArgs",
                vec![json!("meld.recordClip"), json!([f("seconds")?])],
            )),
            "show_replay" => command("meld.replay.show"),
            "dismiss_replays" => command("meld.replay.dismiss"),
            "play_media" => one(call(
                "callFunction",
                vec![json!(s("layer")?), json!("play")],
            )),
            "pause_media" => one(call(
                "callFunction",
                vec![json!(s("layer")?), json!("pause")],
            )),
            "seek_media" => one(call(
                "callFunctionWithArgs",
                vec![json!(s("layer")?), json!("seekTo"), json!([f("seconds")?])],
            )),
            "send_stream_event" => one(call("sendStreamEvent", vec![json!(s("event")?)])),
            "add_subathon_time" => one(call(
                "sendStreamEvent",
                vec![
                    json!("SUBATHONTIMER_ADDTIME"),
                    json!({"amount": v("seconds")?}),
                ],
            )),
            "set_property" => {
                v2("set_property")?;
                one(set_property(&s("object")?, &s("property")?, v("value")?))
            }
            "set_layer_position" => {
                v2("set_layer_position")?;
                let layer = s("layer")?;
                Plan::Calls(vec![
                    set_property(&layer, "x", v("x")?),
                    set_property(&layer, "y", v("y")?),
                ])
            }
            "set_layer_size" => {
                v2("set_layer_size")?;
                let layer = s("layer")?;
                Plan::Calls(vec![
                    set_property(&layer, "width", v("width")?),
                    set_property(&layer, "height", v("height")?),
                ])
            }
            "set_layer_rotation" => {
                v2("set_layer_rotation")?;
                one(set_property(&s("layer")?, "rotation", v("rotation")?))
            }
            "rename" => {
                v2("rename")?;
                one(set_property(&s("object")?, "name", json!(s("name")?)))
            }
            "send_command" => one(call("sendCommand", vec![json!(s("command")?)])),
            "send_command_with_args" => one(call(
                "sendCommandWithArgs",
                vec![json!(s("command")?), Value::Array(array("args")?)],
            )),
            "call_function" => one(call(
                "callFunction",
                vec![json!(s("layer")?), json!(s("function")?)],
            )),
            "call_function_with_args" => one(call(
                "callFunctionWithArgs",
                vec![
                    json!(s("layer")?),
                    json!(s("function")?),
                    Value::Array(array("args")?),
                ],
            )),
            "get_session" => Plan::Session,
            _ => {
                return Err(CommandError::UnknownCommand {
                    command: name.into(),
                })
            }
        })
    }

    /// toggleLayer names the scene holding the layer.
    fn toggle_layer(&self, layer: &str) -> Result<Call, CommandError> {
        let scene = self
            .parent_of(layer)
            .ok_or_else(|| invalid(format!("'layer': no layer {layer} in Meld's session")))?;
        Ok(call("toggleLayer", vec![json!(scene), json!(layer)]))
    }

    /// toggleEffect names the layer holding the effect and its scene.
    fn toggle_effect(&self, effect: &str) -> Result<Call, CommandError> {
        let layer = self
            .parent_of(effect)
            .ok_or_else(|| invalid(format!("'effect': no effect {effect} in Meld's session")))?;
        let scene = self.parent_of(&layer).ok_or_else(|| {
            invalid(format!(
                "'effect': no layer {layer} holding the effect in Meld's session"
            ))
        })?;
        Ok(call(
            "toggleEffect",
            vec![json!(scene), json!(layer), json!(effect)],
        ))
    }

    fn message(&mut self, cx: &mut Cx, text: &str) {
        let Ok(Value::Object(m)) = serde_json::from_str::<Value>(text) else {
            cx.log(Level::Debug, format!("not a WebChannel message: {text}"));
            return;
        };
        match m.get("type").and_then(Value::as_u64) {
            Some(TYPE_RESPONSE) => {
                let Some(id) = m.get("id").and_then(Value::as_u64) else {
                    return;
                };
                if self.init_id == Some(id) {
                    self.init_id = None;
                    self.initialized(cx, m.get("data").unwrap_or(&Value::Null));
                } else if let Some((command, _)) = self.waiting.remove(&id) {
                    if let Some(command) = command {
                        self.answered(cx, command);
                    }
                    self.arm_requests(cx);
                }
            }
            Some(TYPE_PROPERTY_UPDATE) => {
                if let Some(Value::Array(updates)) = m.get("data") {
                    for update in updates {
                        if update.get("object").and_then(Value::as_str) != Some(OBJECT) {
                            continue;
                        }
                        if let Some(Value::Object(props)) = update.get("properties") {
                            for (index, value) in props {
                                let name = index
                                    .parse::<u64>()
                                    .ok()
                                    .and_then(|i| self.properties.get(&i).cloned());
                                if let Some(name) = name {
                                    self.property(cx, &name, value);
                                }
                            }
                        }
                    }
                }
                self.publish_session(cx);
                self.send(cx, json!({"type": TYPE_IDLE}));
            }
            Some(TYPE_SIGNAL) => {
                let signal = m
                    .get("signal")
                    .and_then(Value::as_u64)
                    .and_then(|i| self.signals.get(&i).cloned());
                if m.get("object").and_then(Value::as_str) == Some(OBJECT)
                    && signal.as_deref() == Some("gainUpdated")
                {
                    let args = m.get("args").and_then(Value::as_array);
                    let arg = |i: usize| args.and_then(|a| a.get(i));
                    if let (Some(track), Some(gain)) = (
                        arg(0).and_then(Value::as_str),
                        arg(1).and_then(Value::as_f64),
                    ) {
                        self.gains.insert(track.to_string(), gain);
                        let mut patch = json!({"tracks": {track: {"gain": gain}}});
                        if let Some(muted) = arg(2).and_then(Value::as_bool) {
                            patch["tracks"][track]["muted"] = json!(muted);
                        }
                        if self.monitor {
                            crate::session::merge_patch(&mut self.published, &patch);
                            cx.state(patch);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn answered(&mut self, cx: &mut Cx, command: CommandId) {
        let Some(left) = self.remaining.get_mut(&command) else {
            return;
        };
        *left -= 1;
        if *left == 0 {
            self.remaining.remove(&command);
            cx.complete(command, Ok(Outcome::Ack));
        }
    }

    /// The init response: the objects' methods, signals and properties.
    fn initialized(&mut self, cx: &mut Cx, data: &Value) {
        cx.cancel_timer(INIT);
        let Some(meld) = data.get(OBJECT) else {
            self.lost(cx, "Meld Studio publishes no meld object".into());
            return;
        };
        let pairs = |key: &str| -> Vec<(String, u64)> {
            meld.get(key)
                .and_then(Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|e| {
                            Some((e.get(0)?.as_str()?.to_string(), e.get(1)?.as_u64()?))
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        self.methods = pairs("methods")
            .into_iter()
            .map(|(name, index)| Method { name, index })
            .collect();
        self.signals = pairs("signals").into_iter().map(|(n, i)| (i, n)).collect();
        self.signal_index = pairs("signals").into_iter().collect();
        self.properties.clear();
        self.version = 1;
        let mut initial = Vec::new();
        if let Some(entries) = meld.get("properties").and_then(Value::as_array) {
            for e in entries {
                let (Some(index), Some(name)) = (
                    e.get(0).and_then(Value::as_u64),
                    e.get(1).and_then(Value::as_str),
                ) else {
                    continue;
                };
                self.properties.insert(index, name.to_string());
                initial.push((name.to_string(), e.get(3).cloned().unwrap_or(Value::Null)));
            }
        }
        self.phase = Phase::Ready;
        self.retry_after = RETRY_MIN;
        cx.connection(Connection::Connected);
        for (name, value) in &initial {
            self.property(cx, name, value);
        }
        if self.monitor {
            cx.state(json!({"api_version": self.version}));
        }
        self.publish_session(cx);
        self.send(cx, json!({"type": TYPE_IDLE}));
        if self.monitor {
            if let Some(index) = self.signal_index.get("gainUpdated") {
                self.send(
                    cx,
                    json!({"type": TYPE_CONNECT_TO_SIGNAL, "object": OBJECT, "signal": index}),
                );
            }
            self.observe_tracks(cx);
        }
        cx.set_timer(PING, PING_EVERY);
    }

    fn property(&mut self, cx: &mut Cx, name: &str, value: &Value) {
        match name {
            "session" => {
                if let Some(Value::Object(items)) = value.get("items") {
                    self.items = items.clone();
                }
            }
            "version" => {
                if let Some(v) = value.as_i64() {
                    self.version = v;
                    if self.monitor {
                        cx.state(json!({"api_version": v}));
                    }
                }
            }
            "isStreaming" | "isRecording" => {
                if let (Some(on), true) = (value.as_bool(), self.monitor) {
                    let key = if name == "isStreaming" {
                        "streaming"
                    } else {
                        "recording"
                    };
                    cx.state(json!({ key: on }));
                }
            }
            _ => {}
        }
    }

    /// The session as state.
    fn session_state(&self) -> Value {
        let mut scenes = Map::new();
        let mut layers = Map::new();
        let mut tracks = Map::new();
        let mut effects = Map::new();
        let mut current = String::new();
        let mut staged = String::new();
        for (id, item) in &self.items {
            let Some(item) = item.as_object() else {
                continue;
            };
            let mut out = Map::new();
            let mut copy = |from: &str, to: &str| {
                if let Some(v) = item.get(from).filter(|v| !v.is_null()) {
                    out.insert(to.into(), v.clone());
                }
            };
            match item.get("type").and_then(Value::as_str) {
                Some("scene") => {
                    for f in ["name", "index", "current", "staged"] {
                        copy(f, f);
                    }
                    if item.get("current") == Some(&Value::Bool(true)) {
                        current = id.clone();
                    }
                    if item.get("staged") == Some(&Value::Bool(true)) {
                        staged = id.clone();
                    }
                    scenes.insert(id.clone(), Value::Object(out));
                }
                Some("layer") => {
                    copy("parent", "scene");
                    for f in [
                        "name", "index", "visible", "x", "y", "width", "height", "rotation",
                        "source", "url",
                    ] {
                        copy(f, f);
                    }
                    copy("mediaSource", "media_source");
                    copy("isPlaying", "playing");
                    layers.insert(id.clone(), Value::Object(out));
                }
                Some("track") => {
                    for f in ["name", "muted", "monitoring"] {
                        copy(f, f);
                    }
                    let layer = item.get("parent").and_then(Value::as_str).unwrap_or("");
                    out.insert("layer".into(), json!(layer));
                    if let Some(gain) = self.gains.get(id) {
                        out.insert("gain".into(), json!(gain));
                    }
                    tracks.insert(id.clone(), Value::Object(out));
                }
                Some("effect") => {
                    copy("parent", "layer");
                    for f in ["name", "enabled"] {
                        copy(f, f);
                    }
                    effects.insert(id.clone(), Value::Object(out));
                }
                _ => {}
            }
        }
        json!({
            "scenes": scenes,
            "layers": layers,
            "tracks": tracks,
            "effects": effects,
            "current_scene": current,
            "staged_scene": staged,
        })
    }

    /// Publishes what the session changed, removing what it no longer holds.
    fn publish_session(&mut self, cx: &mut Cx) {
        if !self.monitor {
            return;
        }
        let new = self.session_state();
        let mut patch = Map::new();
        for (key, value) in new.as_object().expect("an object") {
            if let Some(d) = diff(self.published.get(key).unwrap_or(&Value::Null), value) {
                patch.insert(key.clone(), d);
            }
        }
        if !patch.is_empty() {
            let patch = Value::Object(patch);
            crate::session::merge_patch(&mut self.published, &patch);
            cx.state(patch);
        }
        self.observe_tracks(cx);
    }

    /// Registers every track new to the session for gain updates.
    fn observe_tracks(&mut self, cx: &mut Cx) {
        if !self.monitor || self.phase != Phase::Ready || !self.offers("registerTrackObserver") {
            return;
        }
        let tracks: Vec<String> = self
            .items
            .iter()
            .filter(|(_, item)| item.get("type").and_then(Value::as_str) == Some("track"))
            .map(|(id, _)| id.clone())
            .filter(|id| !self.observed.contains(id))
            .collect();
        for track in tracks {
            let c = call(
                "registerTrackObserver",
                vec![json!(OBSERVER), json!(track.clone())],
            );
            if self.invoke(cx, &c, None).is_some() {
                self.observed.insert(track);
            }
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if self.phase != Phase::Closed {
            cx.ws_close(SOCKET);
        }
        self.phase = Phase::Closed;
        self.init_id = None;
        self.ping_out = None;
        self.observed.clear();
        for key in [INIT, PING, REQUEST] {
            cx.cancel_timer(key);
        }
        self.waiting.clear();
        for (command, _) in self.remaining.drain() {
            cx.complete(
                command,
                Err(CommandError::Transport {
                    message: format!("connection to Meld Studio lost: {reason}"),
                }),
            );
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }
}

fn invalid(message: String) -> CommandError {
    CommandError::InvalidParams { message }
}

/// The merge patch turning `old` into `new`: changed values, and `null` for
/// members `new` no longer has.
fn diff(old: &Value, new: &Value) -> Option<Value> {
    match (old, new) {
        (Value::Object(o), Value::Object(n)) => {
            let mut patch = Map::new();
            for (k, v) in n {
                if let Some(d) = diff(o.get(k).unwrap_or(&Value::Null), v) {
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
        _ if old == new => None,
        _ => Some(new.clone()),
    }
}

impl Module for Meld {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.phase != Phase::Ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.plan(name, params) {
            Err(e) => cx.complete(id, Err(e)),
            Ok(Plan::Done) => cx.complete(id, Ok(Outcome::Ack)),
            Ok(Plan::Session) => cx.complete(
                id,
                Ok(Outcome::Value {
                    value: json!({"items": self.items}),
                }),
            ),
            Ok(Plan::Calls(calls)) => {
                if let Some(missing) = calls
                    .iter()
                    .find(|c| self.method_index(c.method, c.args.len()).is_none())
                {
                    cx.complete(
                        id,
                        Err(CommandError::DeviceError {
                            code: None,
                            message: format!("Meld Studio does not offer {}", missing.method),
                        }),
                    );
                    return;
                }
                self.remaining.insert(id, calls.len());
                for c in &calls {
                    self.invoke(cx, c, Some(id));
                }
            }
        }
    }

    fn ws(&mut self, cx: &mut Cx, _socket: Key, input: WsInput) {
        match input {
            WsInput::Opened => {
                self.phase = Phase::Init;
                let id = self.take_id();
                self.init_id = Some(id);
                self.send(cx, json!({"type": TYPE_INIT, "id": id}));
            }
            WsInput::Subprotocol(_) => {}
            WsInput::Text(text) => {
                cx.alive();
                self.message(cx, &text);
            }
            WsInput::Pong(_) => {
                cx.alive();
                if let Some(sent) = self.ping_out.take() {
                    cx.round_trip(cx.now().saturating_sub(sent));
                }
            }
            WsInput::Binary(_) | WsInput::Activity => cx.alive(),
            WsInput::Closed { code, reason } => {
                self.phase = Phase::Closed;
                let reason = match code {
                    Some(code) => format!("closed by Meld Studio ({code}): {reason}"),
                    None => reason,
                };
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY if self.phase == Phase::Closed => self.open(cx),
            INIT if matches!(self.phase, Phase::Opening | Phase::Init) => {
                self.lost(cx, "no WebChannel handshake from Meld Studio".into())
            }
            PING if self.phase == Phase::Ready => {
                if self.ping_out.is_some() {
                    self.lost(cx, format!("no pong within {PING_EVERY} ms"));
                    return;
                }
                cx.ws_ping(SOCKET, b"meld".to_vec());
                self.ping_out = Some(cx.now());
                cx.set_timer(PING, PING_EVERY);
            }
            REQUEST => {
                let now = cx.now();
                let due: Vec<u64> = self
                    .waiting
                    .iter()
                    .filter(|(_, (_, at))| *at <= now)
                    .map(|(id, _)| *id)
                    .collect();
                for id in due {
                    if let Some((Some(command), _)) = self.waiting.remove(&id) {
                        if self.remaining.remove(&command).is_some() {
                            cx.complete(command, Err(CommandError::Timeout));
                        }
                    }
                }
                self.arm_requests(cx);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Closed {
            cx.ws_close(SOCKET);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    const SCENE_A: &str = "05731BDA0C5DE934359C8F7F24CE6C1B";
    const SCENE_B: &str = "9DA5A633182EB0C0804E671B26A1B581";
    const LAYER: &str = "6A92BA3C4FCC18180F76D3EFE9167EF0";
    const EFFECT: &str = "85DFC4BCD7451D2DB3C55A6E2D9948E2";
    const TRACK: &str = "D4D3457F8AF5D1F5381A903A8C43BE27";

    fn context(monitor: bool) -> OpenContext {
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "meld-studio".into(),
            channels: None,
            settings: Params::new(),
            monitor,
        }
    }

    fn session() -> Value {
        json!({"items": {
            SCENE_A: {"type": "scene", "index": 1, "name": "Just Chatting", "current": true, "staged": false},
            SCENE_B: {"type": "scene", "index": 0, "name": "Gaming", "current": false, "staged": true},
            LAYER: {"type": "layer", "parent": SCENE_B, "index": 1, "name": "Fortnite", "visible": true,
                    "x": 265, "y": 132, "width": 1303, "height": 699, "rotation": 0},
            EFFECT: {"type": "effect", "parent": LAYER, "name": "Glow Styles", "enabled": true},
            TRACK: {"type": "track", "name": "Microphone", "muted": true, "monitoring": false},
        }})
    }

    /// The init response's description of the meld object, as Macro Deck's
    /// client reads it.
    fn description(version: Option<i64>, set_muted: bool) -> Value {
        let mut methods = vec![
            json!(["showScene", 10]),
            json!(["setStagedScene", 11]),
            json!(["showStagedScene", 12]),
            json!(["toggleMute", 13]),
            json!(["toggleMonitor", 14]),
            json!(["toggleLayer", 15]),
            json!(["toggleEffect", 16]),
            json!(["toggleRecord", 17]),
            json!(["toggleStream", 18]),
            json!(["registerTrackObserver", 19]),
            json!(["unregisterTrackObserver", 20]),
            json!(["setGain", 21]),
            json!(["callFunction", 22]),
            json!(["callFunctionWithArgs", 23]),
            json!(["sendCommand", 24]),
            json!(["sendCommandWithArgs", 25]),
            json!(["sendStreamEvent", 26]),
            json!(["sendStreamEvent(QString,QJsonValue)", 26]),
            json!(["sendStreamEvent(QString)", 27]),
            json!(["setProperty", 28]),
        ];
        if set_muted {
            methods.push(json!(["setMuted", 29]));
        }
        let mut properties = vec![
            json!([1, "isStreaming", [1, 5], false]),
            json!([2, "isRecording", [1, 6], true]),
            json!([3, "session", [1, 7], session()]),
        ];
        if let Some(v) = version {
            properties.push(json!([4, "version", [1, 8], v]));
        }
        json!({"meld": {
            "methods": methods,
            "signals": [["gainUpdated", 4], ["isStreamingChanged", 5], ["isRecordingChanged", 6], ["sessionChanged", 7]],
            "properties": properties,
            "enums": {},
        }})
    }

    fn sent(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::WsSend { text, .. } => Some(serde_json::from_str(text).unwrap()),
                _ => None,
            })
            .collect()
    }

    fn state(actions: &[Action]) -> Value {
        let mut merged = json!({});
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(&mut merged, p);
            }
        }
        merged
    }

    fn text(m: &mut Meld, now: Millis, message: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.ws(&mut cx, SOCKET, WsInput::Text(message.to_string()));
        cx.take()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    /// Opened and initialized; returns the module and what initializing did.
    fn ready(monitor: bool, version: Option<i64>, set_muted: bool) -> (Meld, Vec<Action>) {
        let mut m = Meld::new(context(monitor));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.ws(&mut cx, SOCKET, WsInput::Opened);
        let opened = cx.take();
        assert!(opened.iter().any(|a| matches!(a, Action::WsOpen { request, .. } if request.url == "ws://127.0.0.1:13376/")));
        assert_eq!(sent(&opened), [json!({"type": 3, "id": 1})]);
        let a = text(
            &mut m,
            10,
            json!({"type": 10, "id": 1, "data": description(version, set_muted)}),
        );
        (m, a)
    }

    fn run(m: &mut Meld, name: &str, input: Value) -> Vec<Action> {
        let mut cx = Cx::new(100);
        m.command(&mut cx, 7, name, &params(input));
        cx.take()
    }

    /// The invoke a command sends, with the next message id taken as given.
    fn invokes(actions: &[Action]) -> Vec<(u64, Value)> {
        sent(actions)
            .into_iter()
            .filter(|v| v["type"] == 6)
            .map(|v| {
                assert_eq!(v["object"], "meld");
                (v["method"].as_u64().unwrap(), v["args"].clone())
            })
            .collect()
    }

    #[test]
    fn initializing_publishes_the_session_subscribes_and_says_idle() {
        let (_, a) = ready(true, Some(2), false);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let s = state(&a);
        assert_eq!(s["api_version"], 2);
        assert_eq!(s["streaming"], false);
        assert_eq!(s["recording"], true);
        assert_eq!(s["current_scene"], SCENE_A);
        assert_eq!(s["staged_scene"], SCENE_B);
        assert_eq!(s["scenes"][SCENE_A]["name"], "Just Chatting");
        assert_eq!(s["layers"][LAYER]["scene"], SCENE_B);
        assert_eq!(s["layers"][LAYER]["width"], 1303);
        assert_eq!(s["effects"][EFFECT]["layer"], LAYER);
        assert_eq!(
            s["tracks"][TRACK],
            json!({"name": "Microphone", "muted": true, "monitoring": false, "layer": ""})
        );
        let out = sent(&a);
        assert!(out.contains(&json!({"type": 4})));
        assert!(out.contains(&json!({"type": 7, "object": "meld", "signal": 4})));
        assert!(out.contains(
            &json!({"type": 6, "object": "meld", "method": 19, "args": ["meros", TRACK], "id": 2})
        ));
    }

    #[test]
    fn api_version_is_one_without_the_version_property() {
        let (_, a) = ready(true, None, false);
        assert_eq!(state(&a)["api_version"], 1);
    }

    #[test]
    fn each_command_invokes_its_method() {
        let (mut m, _) = ready(true, Some(2), true);
        for (name, input, method, args) in [
            (
                "show_scene",
                json!({"scene": SCENE_B}),
                10,
                json!([SCENE_B]),
            ),
            (
                "stage_scene",
                json!({"scene": SCENE_A}),
                11,
                json!([SCENE_A]),
            ),
            ("show_staged_scene", json!({}), 12, json!([])),
            ("toggle_mute", json!({"track": TRACK}), 13, json!([TRACK])),
            (
                "set_muted",
                json!({"track": TRACK, "muted": false}),
                29,
                json!([TRACK, false]),
            ),
            (
                "toggle_monitoring",
                json!({"track": TRACK}),
                14,
                json!([TRACK]),
            ),
            (
                "set_monitoring",
                json!({"track": TRACK, "monitoring": true}),
                14,
                json!([TRACK]),
            ),
            (
                "set_gain",
                json!({"track": TRACK, "gain": 0.5}),
                21,
                json!([TRACK, 0.5]),
            ),
            (
                "toggle_layer",
                json!({"layer": LAYER}),
                15,
                json!([SCENE_B, LAYER]),
            ),
            (
                "set_layer_visible",
                json!({"layer": LAYER, "visible": false}),
                28,
                json!([LAYER, "visible", false]),
            ),
            (
                "toggle_effect",
                json!({"effect": EFFECT}),
                16,
                json!([SCENE_B, LAYER, EFFECT]),
            ),
            (
                "set_effect_enabled",
                json!({"effect": EFFECT, "enabled": false}),
                28,
                json!([EFFECT, "enabled", false]),
            ),
            ("toggle_recording", json!({}), 17, json!([])),
            (
                "start_recording",
                json!({}),
                24,
                json!(["meld.startRecordingAction"]),
            ),
            (
                "stop_recording",
                json!({}),
                24,
                json!(["meld.stopRecordingAction"]),
            ),
            ("toggle_streaming", json!({}), 18, json!([])),
            (
                "start_streaming",
                json!({}),
                24,
                json!(["meld.startStreamingAction"]),
            ),
            (
                "stop_streaming",
                json!({}),
                24,
                json!(["meld.stopStreamingAction"]),
            ),
            (
                "toggle_virtual_camera",
                json!({}),
                24,
                json!(["meld.toggleVirtualCameraAction"]),
            ),
            ("take_screenshot", json!({}), 24, json!(["meld.screenshot"])),
            (
                "take_vertical_screenshot",
                json!({}),
                24,
                json!(["meld.screenshot.vertical"]),
            ),
            ("record_clip", json!({}), 24, json!(["meld.recordClip"])),
            (
                "record_clip_seconds",
                json!({"seconds": 30.0}),
                25,
                json!(["meld.recordClip", [30.0]]),
            ),
            ("show_replay", json!({}), 24, json!(["meld.replay.show"])),
            (
                "dismiss_replays",
                json!({}),
                24,
                json!(["meld.replay.dismiss"]),
            ),
            (
                "play_media",
                json!({"layer": LAYER}),
                22,
                json!([LAYER, "play"]),
            ),
            (
                "pause_media",
                json!({"layer": LAYER}),
                22,
                json!([LAYER, "pause"]),
            ),
            (
                "seek_media",
                json!({"layer": LAYER, "seconds": 12.5}),
                23,
                json!([LAYER, "seekTo", [12.5]]),
            ),
            (
                "send_stream_event",
                json!({"event": "WHEELSPIN_SPIN"}),
                27,
                json!(["WHEELSPIN_SPIN"]),
            ),
            (
                "add_subathon_time",
                json!({"seconds": 120}),
                26,
                json!(["SUBATHONTIMER_ADDTIME", {"amount": 120}]),
            ),
            (
                "set_property",
                json!({"object": LAYER, "property": "name", "value": "Game"}),
                28,
                json!([LAYER, "name", "Game"]),
            ),
            (
                "set_layer_rotation",
                json!({"layer": LAYER, "rotation": 90.0}),
                28,
                json!([LAYER, "rotation", 90.0]),
            ),
            (
                "rename",
                json!({"object": SCENE_A, "name": "Intro"}),
                28,
                json!([SCENE_A, "name", "Intro"]),
            ),
            (
                "send_command",
                json!({"command": "meld.screenshot"}),
                24,
                json!(["meld.screenshot"]),
            ),
            (
                "send_command_with_args",
                json!({"command": "meld.recordClip", "args": [10]}),
                25,
                json!(["meld.recordClip", [10]]),
            ),
            (
                "call_function",
                json!({"layer": LAYER, "function": "play"}),
                22,
                json!([LAYER, "play"]),
            ),
            (
                "call_function_with_args",
                json!({"layer": LAYER, "function": "seekTo", "args": [0]}),
                23,
                json!([LAYER, "seekTo", [0]]),
            ),
        ] {
            let a = run(&mut m, name, input);
            assert_eq!(invokes(&a), [(method, args)], "{name}");
        }
    }

    #[test]
    fn two_property_commands_answer_once_both_calls_are() {
        let (mut m, _) = ready(true, Some(2), false);
        let a = run(
            &mut m,
            "set_layer_position",
            json!({"layer": LAYER, "x": 10, "y": 20}),
        );
        let ids: Vec<u64> = sent(&a).iter().map(|v| v["id"].as_u64().unwrap()).collect();
        assert_eq!(
            invokes(&a),
            [(28, json!([LAYER, "x", 10])), (28, json!([LAYER, "y", 20]))]
        );
        let first = text(&mut m, 120, json!({"type": 10, "id": ids[0], "data": null}));
        assert!(!first.iter().any(|a| matches!(a, Action::Complete { .. })));
        let second = text(&mut m, 121, json!({"type": 10, "id": ids[1], "data": null}));
        assert!(second.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Ack)
        }));
        let a = run(
            &mut m,
            "set_layer_size",
            json!({"layer": LAYER, "width": 640, "height": 360}),
        );
        assert_eq!(
            invokes(&a),
            [
                (28, json!([LAYER, "width", 640])),
                (28, json!([LAYER, "height", 360]))
            ]
        );
    }

    #[test]
    fn version_one_toggles_only_when_the_session_differs() {
        let (mut m, _) = ready(true, None, false);
        // Visible already: nothing to send.
        let a = run(
            &mut m,
            "set_layer_visible",
            json!({"layer": LAYER, "visible": true}),
        );
        assert!(invokes(&a).is_empty());
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Ack)
        }));
        let a = run(
            &mut m,
            "set_layer_visible",
            json!({"layer": LAYER, "visible": false}),
        );
        assert_eq!(invokes(&a), [(15, json!([SCENE_B, LAYER]))]);
        // Without setMuted, mute toggles when the track differs.
        let a = run(&mut m, "set_muted", json!({"track": TRACK, "muted": true}));
        assert!(invokes(&a).is_empty());
        let a = run(&mut m, "set_muted", json!({"track": TRACK, "muted": false}));
        assert_eq!(invokes(&a), [(13, json!([TRACK]))]);
        let a = run(
            &mut m,
            "set_effect_enabled",
            json!({"effect": EFFECT, "enabled": false}),
        );
        assert_eq!(invokes(&a), [(16, json!([SCENE_B, LAYER, EFFECT]))]);
        // setProperty needs version 2.
        let a = run(&mut m, "rename", json!({"object": LAYER, "name": "x"}));
        assert!(matches!(
            a.as_slice(),
            [Action::Complete {
                result: Err(CommandError::DeviceError { .. }),
                ..
            }]
        ));
    }

    #[test]
    fn an_unknown_item_or_method_is_refused() {
        let (mut m, _) = ready(true, Some(1), false);
        let a = run(
            &mut m,
            "toggle_layer",
            json!({"layer": "00000000000000000000000000000000"}),
        );
        assert!(matches!(
            a.as_slice(),
            [Action::Complete {
                result: Err(CommandError::InvalidParams { .. }),
                ..
            }]
        ));
        m.methods.retain(|x| x.name != "toggleStream");
        let a = run(&mut m, "toggle_streaming", json!({}));
        assert!(matches!(
            a.as_slice(),
            [Action::Complete {
                result: Err(CommandError::DeviceError { .. }),
                ..
            }]
        ));
    }

    #[test]
    fn property_updates_replace_the_session_and_ask_for_more() {
        let (mut m, _) = ready(true, Some(2), false);
        let mut next = session();
        next["items"][SCENE_A]["current"] = json!(false);
        next["items"][SCENE_B]["current"] = json!(true);
        next["items"].as_object_mut().unwrap().remove(EFFECT);
        let a = text(
            &mut m,
            200,
            json!({"type": 2, "data": [{"object": "meld", "signals": {"7": []}, "properties": {"3": next, "1": true}}]}),
        );
        assert!(a.contains(&Action::State(json!({"streaming": true}))));
        // The effect the session no longer holds is removed.
        assert!(a.contains(&Action::State(json!({
            "current_scene": SCENE_B,
            "scenes": {SCENE_A: {"current": false}, SCENE_B: {"current": true}},
            "effects": {EFFECT: null},
        }))));
        assert_eq!(sent(&a), [json!({"type": 4})]);
        // The same session again changes nothing.
        let a = text(
            &mut m,
            210,
            json!({"type": 2, "data": [{"object": "meld", "properties": {"3": next}}]}),
        );
        assert_eq!(state(&a), json!({}));
    }

    #[test]
    fn gain_updates_become_track_state() {
        let (mut m, _) = ready(true, Some(2), false);
        let a = text(
            &mut m,
            300,
            json!({"type": 1, "object": "meld", "signal": 4, "args": [TRACK, 0.25, false]}),
        );
        assert_eq!(
            state(&a),
            json!({"tracks": {TRACK: {"gain": 0.25, "muted": false}}})
        );
    }

    #[test]
    fn get_session_returns_the_items() {
        let (mut m, _) = ready(true, Some(2), false);
        let a = run(&mut m, "get_session", json!({}));
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Value { value: session() })
        }));
    }

    #[test]
    fn commands_only_publishes_and_registers_nothing() {
        let (mut m, a) = ready(false, Some(2), false);
        assert_eq!(state(&a), json!({}));
        assert!(invokes(&a).is_empty());
        assert!(!sent(&a).iter().any(|v| v["type"] == 7));
        // The session is still read: a layer toggle names its scene.
        let a = run(&mut m, "toggle_layer", json!({"layer": LAYER}));
        assert_eq!(invokes(&a), [(15, json!([SCENE_B, LAYER]))]);
    }

    #[test]
    fn timeouts_pings_and_reconnects() {
        let (mut m, _) = ready(true, Some(2), false);
        let _ = run(&mut m, "toggle_recording", json!({}));
        let mut cx = Cx::new(100 + REQUEST_TIMEOUT);
        m.timer(&mut cx, REQUEST);
        assert!(cx.take().contains(&Action::Complete {
            id: 7,
            result: Err(CommandError::Timeout)
        }));

        let mut cx = Cx::new(6_000);
        m.timer(&mut cx, PING);
        assert!(cx.take().iter().any(|a| matches!(a, Action::WsPing { .. })));
        let mut cx = Cx::new(6_010);
        m.ws(&mut cx, SOCKET, WsInput::Pong(b"meld".to_vec()));
        assert!(cx.take().contains(&Action::RoundTrip(10)));
        let mut cx = Cx::new(11_000);
        m.timer(&mut cx, PING);
        let mut cx = Cx::new(16_000);
        m.timer(&mut cx, PING);
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        let mut cx = Cx::new(16_001);
        m.command(&mut cx, 8, "toggle_streaming", &Params::new());
        assert!(cx.take().contains(&Action::Complete {
            id: 8,
            result: Err(CommandError::NotConnected)
        }));
        let mut cx = Cx::new(17_001);
        m.timer(&mut cx, RETRY);
        assert!(cx.take().iter().any(|a| matches!(a, Action::WsOpen { .. })));
    }

    #[test]
    fn no_handshake_is_a_lost_connection() {
        let mut m = Meld::new(context(true));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.ws(&mut cx, SOCKET, WsInput::Opened);
        let mut cx = Cx::new(INIT_TIMEOUT);
        m.timer(&mut cx, INIT);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
    }
}
