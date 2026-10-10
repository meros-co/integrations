//! OBS Studio over obs-websocket 5: JSON messages on a WebSocket.
//!
//! Protocol from obs-websocket's protocol documentation (docs/generated/
//! protocol.md): the server sends Hello (op 0), the client answers Identify
//! (op 1) with the authentication string when Hello carries a challenge, and
//! the server confirms with Identified (op 2). After that the client sends
//! Request (op 6) messages, matched to RequestResponse (op 7) by the client's
//! requestId, and receives Event (op 5) messages for the subscribed categories.
//!
//! Monitored, the module reads everything it mirrors once identified (scenes
//! and their items, groups, inputs and their audio, media and settings,
//! filters, transitions, outputs, profile, scene collection, video settings),
//! then keeps it current from events. What OBS sends no event for is re-read
//! after the command that changes it, or polled: output status and stats every
//! 5 s, and the cursor of playing media every second. The high-volume events
//! (volume meters, item transforms) are coalesced to one state patch every
//! 200 ms.
//!
//! State keyed by name (scenes, inputs, filters, outputs) is replaced, not
//! merged: anything OBS no longer reports is deleted. The module keeps a
//! mirror of the state it published to work out those deletions.
//!
//! Opened for commands only (`monitor` false), the module identifies with no
//! event categories and reads no state; a `GetVersion` request every 5 s is
//! its liveness check, since an open WebSocket alone does not show OBS is
//! answering.

use std::collections::{BTreeMap, HashMap};

use base64::Engine;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    WsInput, WsRequest,
};

const PORT: u16 = 4455;
const SOCKET: Key = "obs";
const SUBPROTOCOL: &str = "obswebsocket.json";
const RPC_VERSION: u64 = 1;

const OP_HELLO: u64 = 0;
const OP_IDENTIFY: u64 = 1;
const OP_IDENTIFIED: u64 = 2;
const OP_EVENT: u64 = 5;
const OP_REQUEST: u64 = 6;
const OP_REQUEST_RESPONSE: u64 = 7;

/// EventSubscription flags: every low-volume category from General (1 << 0)
/// to Ui (1 << 10), and the high-volume InputVolumeMeters (1 << 16),
/// InputActiveStateChanged (1 << 17), InputShowStateChanged (1 << 18) and
/// SceneItemTransformChanged (1 << 19). Canvases (1 << 11, obs-websocket 5.7)
/// is left out: only the main canvas is mirrored.
const EVENTS: u64 = 0x7FF | (1 << 16) | (1 << 17) | (1 << 18) | (1 << 19);

/// WebSocketCloseCode::AuthenticationFailed.
const CLOSE_AUTHENTICATION_FAILED: u16 = 4009;
/// WebSocketCloseCode::UnsupportedRpcVersion.
const CLOSE_UNSUPPORTED_RPC_VERSION: u16 = 4010;

const HELLO_TIMEOUT: Millis = 5_000;
const REQUEST_TIMEOUT: Millis = 5_000;
/// Output status is polled for stream health; the poll doubles as the liveness
/// check, since a WebSocket can stay open across a hung peer.
const POLL_EVERY: Millis = 5_000;
/// Consecutive unanswered polls before the connection is treated as dead.
const POLL_STRIKES: u32 = 2;
/// OBS sends no event as a media input plays; its cursor is read this often
/// while it plays.
const MEDIA_EVERY: Millis = 1_000;
/// Volume meters arrive every 50 ms and transforms as fast as an item is
/// dragged; both are published at most this often.
const FLUSH_EVERY: Millis = 200;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

/// Meter levels below this (silence is -infinity) are reported as this.
const FLOOR_DB: f64 = -100.0;
/// Unversioned input kinds with a media transport: the media source, the VLC
/// source and the image slide show.
const MEDIA_KINDS: &[&str] = &["ffmpeg_source", "vlc_source", "slideshow"];
const MEDIA_PLAYING: &str = "OBS_MEDIA_STATE_PLAYING";

const HELLO: Key = "hello";
const RETRY: Key = "retry";
const POLL: Key = "poll";
const REQUEST: Key = "request";
const MEDIA: Key = "media";
const FLUSH: Key = "flush";

/// Collections keyed by name, cleared when the scene collection changes or the
/// connection is made again, then read afresh.
const COLLECTIONS: &[&str] = &[
    "scene_items",
    "filters",
    "inputs",
    "scene_transitions",
    "outputs",
    "groups",
];

/// A per-input read.
#[derive(Debug, Clone, Copy, PartialEq)]
enum InputRead {
    Mute,
    Volume,
    Balance,
    SyncOffset,
    MonitorType,
    Tracks,
    Settings,
    Active,
    DeinterlaceMode,
    DeinterlaceFieldOrder,
    Media,
}

impl InputRead {
    fn request(self) -> &'static str {
        match self {
            InputRead::Mute => "GetInputMute",
            InputRead::Volume => "GetInputVolume",
            InputRead::Balance => "GetInputAudioBalance",
            InputRead::SyncOffset => "GetInputAudioSyncOffset",
            InputRead::MonitorType => "GetInputAudioMonitorType",
            InputRead::Tracks => "GetInputAudioTracks",
            InputRead::Settings => "GetInputSettings",
            InputRead::Active => "GetSourceActive",
            InputRead::DeinterlaceMode => "GetInputDeinterlaceMode",
            InputRead::DeinterlaceFieldOrder => "GetInputDeinterlaceFieldOrder",
            InputRead::Media => "GetMediaInputStatus",
        }
    }
}

/// Every input read but media, which only media inputs answer.
const INPUT_READS: &[InputRead] = &[
    InputRead::Mute,
    InputRead::Volume,
    InputRead::Balance,
    InputRead::SyncOffset,
    InputRead::MonitorType,
    InputRead::Tracks,
    InputRead::Settings,
    InputRead::Active,
    InputRead::DeinterlaceMode,
    InputRead::DeinterlaceFieldOrder,
];

/// A state read.
#[derive(Debug, Clone, PartialEq)]
enum Read {
    Version,
    /// Commands only: GetVersion in place of the output-status poll.
    Liveness,
    /// `full`: read every scene's items, filters and transition override, not
    /// only those of scenes new to the state.
    SceneList {
        full: bool,
    },
    GroupList,
    StudioMode,
    Transition,
    TransitionList,
    StreamStatus,
    RecordStatus,
    VirtualCam,
    ReplayBuffer,
    InputList,
    ProfileList,
    SceneCollectionList,
    VideoSettings,
    RecordDirectory,
    StreamService,
    Stats,
    OutputList,
    SceneItems {
        scene: String,
        group: bool,
    },
    Filters {
        source: String,
    },
    SceneTransition {
        scene: String,
    },
    Input {
        input: String,
        what: InputRead,
    },
}

impl Read {
    fn request(&self) -> (&'static str, Option<Value>) {
        match self {
            Read::Version | Read::Liveness => ("GetVersion", None),
            Read::SceneList { .. } => ("GetSceneList", None),
            Read::GroupList => ("GetGroupList", None),
            Read::StudioMode => ("GetStudioModeEnabled", None),
            Read::Transition => ("GetCurrentSceneTransition", None),
            Read::TransitionList => ("GetSceneTransitionList", None),
            Read::StreamStatus => ("GetStreamStatus", None),
            Read::RecordStatus => ("GetRecordStatus", None),
            Read::VirtualCam => ("GetVirtualCamStatus", None),
            Read::ReplayBuffer => ("GetReplayBufferStatus", None),
            Read::InputList => ("GetInputList", None),
            Read::ProfileList => ("GetProfileList", None),
            Read::SceneCollectionList => ("GetSceneCollectionList", None),
            Read::VideoSettings => ("GetVideoSettings", None),
            Read::RecordDirectory => ("GetRecordDirectory", None),
            Read::StreamService => ("GetStreamServiceSettings", None),
            Read::Stats => ("GetStats", None),
            Read::OutputList => ("GetOutputList", None),
            Read::SceneItems { scene, group } => (
                if *group {
                    "GetGroupSceneItemList"
                } else {
                    "GetSceneItemList"
                },
                Some(json!({ "sceneName": scene })),
            ),
            Read::Filters { source } => {
                ("GetSourceFilterList", Some(json!({ "sourceName": source })))
            }
            Read::SceneTransition { scene } => (
                "GetSceneSceneTransitionOverride",
                Some(json!({ "sceneName": scene })),
            ),
            Read::Input { input, what } => (
                what.request(),
                Some(if *what == InputRead::Active {
                    json!({ "sourceName": input })
                } else {
                    json!({ "inputName": input })
                }),
            ),
        }
    }

    fn is_poll(&self) -> bool {
        matches!(
            self,
            Read::StreamStatus | Read::RecordStatus | Read::Liveness
        )
    }

    /// Refusals expected in normal use: an input without audio or media, a
    /// replay buffer that is not configured, an OBS older than the request.
    fn refusal_expected(&self) -> bool {
        matches!(
            self,
            Read::Input { .. }
                | Read::ReplayBuffer
                | Read::SceneTransition { .. }
                | Read::Filters { .. }
                | Read::SceneItems { .. }
        )
    }
}

/// Why a request was sent.
#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    /// A command; `then` is re-read once OBS accepts it, for what OBS sends no
    /// event about.
    Command {
        id: CommandId,
        then: Vec<Read>,
    },
    Read(Read),
}

impl Purpose {
    fn is_poll(&self) -> bool {
        matches!(self, Purpose::Read(r) if r.is_poll())
    }
}

struct Pending {
    purpose: Purpose,
    sent_at: Millis,
    deadline: Millis,
}

#[derive(Debug, PartialEq)]
enum Phase {
    Idle,
    /// Socket opening, or open and waiting for Hello.
    Handshake,
    Identified,
}

pub(crate) struct Obs {
    url: String,
    password: String,
    phase: Phase,
    connected: bool,
    next_request: u64,
    pending: HashMap<String, Pending>,
    /// False: commands only, no events and no state reads.
    monitor: bool,
    retry_after: Millis,
    strikes: u32,
    /// Set when OBS refuses the password. Terminal, as for every credential in
    /// the core: nothing is sent again until the device is opened again.
    refused: Option<String>,
    /// The state as published, to replace collections with deletions.
    mirror: Value,
    /// Group names, from GetGroupList.
    groups: Vec<String>,
    /// Input name to unversioned input kind, for every input OBS has.
    kinds: HashMap<String, String>,
    /// Scene and input UUIDs to names, for commands that name them by UUID.
    uuids: HashMap<String, String>,
    /// Volume meters since the last flush: per input, per channel, magnitude
    /// (latest), peak and input peak (highest), as multipliers.
    meters: BTreeMap<String, Vec<[f64; 3]>>,
    /// Latest transform per scene item since the last flush.
    transforms: BTreeMap<(String, String), Value>,
    flush_armed: bool,
    media_armed: bool,
}

/// The authentication string: base64(sha256(base64(sha256(password + salt)) +
/// challenge)), per the protocol documentation's "Creating an authentication
/// string".
fn authentication(password: &str, salt: &str, challenge: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    let secret = b64.encode(Sha256::digest(format!("{password}{salt}")));
    b64.encode(Sha256::digest(format!("{secret}{challenge}")))
}

fn str_param<'a>(params: &'a Params, name: &str) -> &'a str {
    params.get(name).and_then(Value::as_str).unwrap_or("")
}

/// `value` nested under `path`.
fn nest(path: &[&str], value: Value) -> Value {
    path.iter().rev().fold(value, |inner, key| {
        let mut map = Map::new();
        map.insert((*key).to_string(), inner);
        Value::Object(map)
    })
}

/// `value` without null members, recursively: in a merge patch they would be
/// deletions, and a replacement has nothing to delete inside itself.
fn strip_nulls(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k, strip_nulls(v)))
                .collect(),
        ),
        other => other,
    }
}

/// The merge patch turning `old` into `new`, or None when they are equal.
fn diff(old: &Value, new: &Value) -> Option<Value> {
    if old == new {
        return None;
    }
    match (old, new) {
        (Value::Object(o), Value::Object(n)) => {
            let mut out = Map::new();
            for k in o.keys() {
                if !n.contains_key(k) {
                    out.insert(k.clone(), Value::Null);
                }
            }
            for (k, v) in n {
                match o.get(k) {
                    Some(ov) => {
                        if let Some(d) = diff(ov, v) {
                            out.insert(k.clone(), d);
                        }
                    }
                    None => {
                        out.insert(k.clone(), v.clone());
                    }
                }
            }
            Some(Value::Object(out))
        }
        _ => Some(new.clone()),
    }
}

/// A level multiplier in dB, to 0.1 dB, floored at FLOOR_DB.
fn db(mul: f64) -> f64 {
    if mul <= 0.0 {
        return FLOOR_DB;
    }
    ((20.0 * mul.log10()) * 10.0).round().max(FLOOR_DB * 10.0) / 10.0
}

/// The output state's name without OBS's prefix, lower case: "started".
fn output_state(data: &Value) -> &str {
    data["outputState"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches("OBS_WEBSOCKET_OUTPUT_")
}

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

impl Obs {
    pub(crate) fn new(ctx: OpenContext) -> Obs {
        let host = match ctx.host {
            std::net::IpAddr::V6(v6) => format!("[{v6}]"),
            v4 => v4.to_string(),
        };
        Obs {
            url: format!("ws://{host}:{}", ctx.port.unwrap_or(PORT)),
            password: str_param(&ctx.settings, "password").to_string(),
            phase: Phase::Idle,
            connected: false,
            next_request: 1,
            pending: HashMap::new(),
            monitor: ctx.monitor,
            retry_after: RETRY_MIN,
            strikes: 0,
            refused: None,
            mirror: json!({}),
            groups: Vec::new(),
            kinds: HashMap::new(),
            uuids: HashMap::new(),
            meters: BTreeMap::new(),
            transforms: BTreeMap::new(),
            flush_armed: false,
            media_armed: false,
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        self.phase = Phase::Handshake;
        cx.ws_open(
            SOCKET,
            WsRequest {
                url: self.url.clone(),
                headers: vec![("Sec-WebSocket-Protocol".into(), SUBPROTOCOL.into())],
                accept_invalid_certs: false,
            },
        );
        cx.set_timer(HELLO, HELLO_TIMEOUT);
    }

    fn send(&self, cx: &mut Cx, op: u64, d: Value) {
        cx.ws_send(SOCKET, json!({"op": op, "d": d}).to_string());
    }

    fn request(&mut self, cx: &mut Cx, purpose: Purpose, request_type: &str, data: Option<Value>) {
        let id = format!("r{}", self.next_request);
        self.next_request += 1;
        let mut d = json!({"requestType": request_type, "requestId": id});
        if let Some(data) = data {
            d["requestData"] = data;
        }
        self.send(cx, OP_REQUEST, d);
        self.pending.insert(
            id,
            Pending {
                purpose,
                sent_at: cx.now(),
                deadline: cx.now() + REQUEST_TIMEOUT,
            },
        );
        self.arm_request_timer(cx);
    }

    fn read(&mut self, cx: &mut Cx, read: Read) {
        let (request_type, data) = read.request();
        self.request(cx, Purpose::Read(read), request_type, data);
    }

    fn arm_request_timer(&self, cx: &mut Cx) {
        match self.pending.values().map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(REQUEST, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REQUEST),
        }
    }

    /// Fail every command in flight and forget every other request.
    fn fail_pending(&mut self, cx: &mut Cx, error: CommandError) {
        for (_, p) in self.pending.drain() {
            if let Purpose::Command { id, .. } = p.purpose {
                cx.complete(id, Err(error.clone()));
            }
        }
        cx.cancel_timer(REQUEST);
    }

    fn stop_everything(&mut self, cx: &mut Cx) {
        cx.ws_close(SOCKET);
        for key in [HELLO, POLL, RETRY, MEDIA, FLUSH] {
            cx.cancel_timer(key);
        }
        self.phase = Phase::Idle;
        self.connected = false;
        self.strikes = 0;
        self.meters.clear();
        self.transforms.clear();
        self.flush_armed = false;
        self.media_armed = false;
    }

    /// The connection failed or ended: try again, backing off to 30 s.
    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.fail_pending(
            cx,
            CommandError::Transport {
                message: reason.clone(),
            },
        );
        self.stop_everything(cx);
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// OBS refused the password, or needs one and none is configured.
    fn refuse(&mut self, cx: &mut Cx, reason: &str) {
        self.fail_pending(
            cx,
            CommandError::Auth {
                message: reason.into(),
            },
        );
        self.stop_everything(cx);
        self.refused = Some(reason.to_string());
        cx.log(
            Level::Warning,
            format!("{reason}; no further attempts until the device is opened again with a corrected password"),
        );
        cx.connection(Connection::Unauthorized {
            reason: reason.into(),
        });
    }

    fn hello(&mut self, cx: &mut Cx, d: &Value) {
        cx.cancel_timer(HELLO);
        // Commands only: no event categories at all.
        let events = if self.monitor { EVENTS } else { 0 };
        let mut identify = json!({"rpcVersion": RPC_VERSION, "eventSubscriptions": events});
        if let Some(auth) = d.get("authentication") {
            if self.password.is_empty() {
                self.refuse(cx, "OBS requires a password and none is configured");
                return;
            }
            let challenge = auth.get("challenge").and_then(Value::as_str).unwrap_or("");
            let salt = auth.get("salt").and_then(Value::as_str).unwrap_or("");
            identify["authentication"] = json!(authentication(&self.password, salt, challenge));
        }
        self.send(cx, OP_IDENTIFY, identify);
        // OBS answers Identified, or closes the connection (4009 on a wrong
        // password). The Hello timer covers a server that does neither.
        cx.set_timer(HELLO, HELLO_TIMEOUT);
    }

    fn identified(&mut self, cx: &mut Cx) {
        cx.cancel_timer(HELLO);
        self.phase = Phase::Identified;
        self.connected = true;
        self.retry_after = RETRY_MIN;
        cx.connection(Connection::Connected);
        cx.alive();
        if !self.monitor {
            self.poll(cx);
            return;
        }
        // Whatever changed while disconnected is read afresh.
        self.clear_collections(cx);
        for read in [
            Read::Version,
            Read::SceneCollectionList,
            Read::ProfileList,
            Read::SceneList { full: true },
            Read::GroupList,
            Read::StudioMode,
            Read::Transition,
            Read::TransitionList,
            Read::VirtualCam,
            Read::ReplayBuffer,
            Read::InputList,
            Read::VideoSettings,
            Read::RecordDirectory,
            Read::StreamService,
        ] {
            self.read(cx, read);
        }
        self.poll(cx);
    }

    fn poll(&mut self, cx: &mut Cx) {
        if self.monitor {
            for read in [
                Read::StreamStatus,
                Read::RecordStatus,
                Read::Stats,
                Read::OutputList,
            ] {
                self.read(cx, read);
            }
        } else {
            // Commands only: the cheapest request OBS answers, for liveness.
            self.read(cx, Read::Liveness);
        }
        cx.set_timer(POLL, POLL_EVERY);
    }

    // ---- the mirror ----

    fn emit(&mut self, cx: &mut Cx, patch: Value) {
        crate::session::merge_patch(&mut self.mirror, &patch);
        cx.state(patch);
    }

    fn at(&self, path: &[&str]) -> Option<&Value> {
        path.iter().try_fold(&self.mirror, |v, key| v.get(*key))
    }

    /// Replace the value at `path` with `new`: members it no longer has are
    /// deleted. Nothing is published when nothing changed.
    fn replace(&mut self, cx: &mut Cx, path: &[&str], new: Value) {
        let new = strip_nulls(new);
        let patch = match self.at(path) {
            Some(old) => diff(old, &new),
            None if new.is_null() => None,
            None => Some(new),
        };
        if let Some(patch) = patch {
            self.emit(cx, nest(path, patch));
        }
    }

    /// Delete `path` from the state, if it is there.
    fn delete(&mut self, cx: &mut Cx, path: &[&str]) {
        if self.at(path).is_some() {
            self.emit(cx, nest(path, Value::Null));
        }
    }

    fn keys(&self, section: &str) -> Vec<String> {
        self.mirror
            .get(section)
            .and_then(Value::as_object)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn scene_list(&self) -> Vec<String> {
        names(&self.mirror["scenes"])
    }

    fn clear_collections(&mut self, cx: &mut Cx) {
        for section in COLLECTIONS {
            self.delete(cx, &[section]);
        }
        self.groups.clear();
        self.kinds.clear();
        self.uuids.clear();
        self.meters.clear();
        self.transforms.clear();
    }

    /// A source that filters may belong to: a scene, a group or an input.
    fn is_source(&self, name: &str) -> bool {
        self.kinds.contains_key(name)
            || self.groups.iter().any(|g| g == name)
            || self.scene_list().iter().any(|s| s == name)
    }

    fn is_scene(&self, name: &str) -> bool {
        self.groups.iter().any(|g| g == name) || self.scene_list().iter().any(|s| s == name)
    }

    /// Delete the per-scene state of scenes and groups OBS no longer has, and
    /// the filters of sources it no longer has.
    fn prune_scenes(&mut self, cx: &mut Cx) {
        for section in ["scene_items", "scene_transitions"] {
            for key in self.keys(section) {
                if !self.is_scene(&key) {
                    self.delete(cx, &[section, &key]);
                }
            }
        }
        for key in self.keys("filters") {
            if !self.is_source(&key) {
                self.delete(cx, &["filters", &key]);
            }
        }
    }

    fn read_scene(&mut self, cx: &mut Cx, scene: &str, group: bool) {
        self.read(
            cx,
            Read::SceneItems {
                scene: scene.into(),
                group,
            },
        );
        self.read(
            cx,
            Read::Filters {
                source: scene.into(),
            },
        );
        if !group {
            self.read(
                cx,
                Read::SceneTransition {
                    scene: scene.into(),
                },
            );
        }
    }

    fn read_input(&mut self, cx: &mut Cx, input: &str, what: InputRead) {
        self.read(
            cx,
            Read::Input {
                input: input.into(),
                what,
            },
        );
    }

    fn read_input_all(&mut self, cx: &mut Cx, input: &str) {
        for what in INPUT_READS {
            self.read_input(cx, input, *what);
        }
        if self.is_media(input) {
            self.read_input(cx, input, InputRead::Media);
        }
        self.read(
            cx,
            Read::Filters {
                source: input.into(),
            },
        );
    }

    fn is_media(&self, input: &str) -> bool {
        self.kinds
            .get(input)
            .is_some_and(|k| MEDIA_KINDS.contains(&k.as_str()))
    }

    fn forget_input(&mut self, cx: &mut Cx, input: &str) {
        self.kinds.remove(input);
        self.uuids.retain(|_, name| name != input);
        self.meters.remove(input);
        self.delete(cx, &["inputs", input]);
        if !self.is_scene(input) {
            self.delete(cx, &["filters", input]);
        }
    }

    fn playing(&self) -> Vec<String> {
        self.keys("inputs")
            .into_iter()
            .filter(|i| self.mirror["inputs"][i]["media"]["state"] == MEDIA_PLAYING)
            .collect()
    }

    fn arm_media(&mut self, cx: &mut Cx) {
        if !self.media_armed && !self.playing().is_empty() {
            self.media_armed = true;
            cx.set_timer(MEDIA, MEDIA_EVERY);
        }
    }

    fn arm_flush(&mut self, cx: &mut Cx) {
        if !self.flush_armed {
            self.flush_armed = true;
            cx.set_timer(FLUSH, FLUSH_EVERY);
        }
    }

    /// Publish the meters and transforms gathered since the last flush.
    fn flush(&mut self, cx: &mut Cx) {
        let meters = std::mem::take(&mut self.meters);
        for (input, channels) in &meters {
            if !self.kinds.contains_key(input) {
                continue;
            }
            let levels: Vec<Value> = channels
                .iter()
                .map(|[magnitude, peak, input_peak]| {
                    json!({
                        "magnitude_db": db(*magnitude),
                        "peak_db": db(*peak),
                        "input_peak_db": db(*input_peak),
                    })
                })
                .collect();
            self.replace(cx, &["inputs", input, "levels"], json!(levels));
        }
        // Inputs that stopped being reported are no longer active.
        if !meters.is_empty() {
            for input in self.keys("inputs") {
                if !meters.contains_key(&input) {
                    self.delete(cx, &["inputs", &input, "levels"]);
                }
            }
        }
        for ((scene, item), transform) in std::mem::take(&mut self.transforms) {
            if self.at(&["scene_items", &scene, &item]).is_some() {
                self.replace(cx, &["scene_items", &scene, &item, "transform"], transform);
            }
        }
    }

    fn name_param(&self, params: &Params, name: &str, uuid: &str) -> Option<String> {
        params
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                let uuid = params.get(uuid).and_then(Value::as_str)?;
                self.uuids.get(uuid).cloned()
            })
    }

    /// What to read again after a command OBS sends no event for.
    fn then_read(&self, name: &str, params: &Params) -> Vec<Read> {
        if !self.monitor {
            return Vec::new();
        }
        let scene = || self.name_param(params, "scene_name", "scene_uuid");
        let input = || self.name_param(params, "input_name", "input_uuid");
        let source = || self.name_param(params, "source_name", "source_uuid");
        let of_input = |what| {
            input()
                .map(|input| vec![Read::Input { input, what }])
                .unwrap_or_default()
        };
        match name {
            "set_scene_item_blend_mode" => scene()
                .map(|scene| {
                    let group = self.groups.contains(&scene);
                    vec![Read::SceneItems { scene, group }]
                })
                .unwrap_or_default(),
            "set_scene_scene_transition_override" => scene()
                .map(|scene| vec![Read::SceneTransition { scene }])
                .unwrap_or_default(),
            "set_source_filter_settings" => source()
                .map(|source| vec![Read::Filters { source }])
                .unwrap_or_default(),
            "set_input_settings" => of_input(InputRead::Settings),
            "set_input_deinterlace_mode" => of_input(InputRead::DeinterlaceMode),
            "set_input_deinterlace_field_order" => of_input(InputRead::DeinterlaceFieldOrder),
            "set_current_scene_transition_settings" => vec![Read::Transition],
            "set_video_settings" => vec![Read::VideoSettings],
            "set_record_directory" => vec![Read::RecordDirectory],
            "set_stream_service_settings" => vec![Read::StreamService],
            _ => Vec::new(),
        }
    }

    // ---- replies ----

    fn response(&mut self, cx: &mut Cx, d: &Value) {
        let Some(id) = d.get("requestId").and_then(Value::as_str) else {
            return;
        };
        let Some(pending) = self.pending.remove(id) else {
            return;
        };
        self.arm_request_timer(cx);
        cx.round_trip(cx.now().saturating_sub(pending.sent_at));
        let status = &d["requestStatus"];
        let ok = status.get("result").and_then(Value::as_bool) == Some(true);
        let data = d.get("responseData").cloned().unwrap_or(Value::Null);
        if pending.purpose.is_poll() {
            self.strikes = 0;
        }

        match pending.purpose {
            Purpose::Command { id, then } => {
                let result = if !ok {
                    Err(CommandError::DeviceError {
                        code: status
                            .get("code")
                            .and_then(Value::as_i64)
                            .map(|c| c.to_string()),
                        message: status
                            .get("comment")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    })
                } else if data.is_null() {
                    Ok(Outcome::Ack)
                } else {
                    // The request's responseData, as obs-websocket documents it.
                    Ok(Outcome::Value { value: data })
                };
                if result.is_ok() {
                    for read in then {
                        self.read(cx, read);
                    }
                }
                cx.complete(id, result);
            }
            // A failed internal query leaves that part of the state unknown;
            // it is not a reason to drop the connection.
            Purpose::Read(read) if !ok => {
                if !read.refusal_expected() {
                    cx.log(Level::Warning, format!("{read:?} failed: {status}"));
                }
            }
            Purpose::Read(read) => self.answer(cx, read, &data),
        }
    }

    fn answer(&mut self, cx: &mut Cx, read: Read, data: &Value) {
        match read {
            Read::Version | Read::Liveness => self.emit(
                cx,
                json!({"device": {
                    "obs_version": data["obsVersion"],
                    "websocket_version": data["obsWebSocketVersion"],
                    "platform": data["platformDescription"],
                }}),
            ),
            Read::SceneList { full } => {
                let scenes = scene_names(data);
                for s in data["scenes"].as_array().into_iter().flatten() {
                    if let (Some(uuid), Some(name)) =
                        (s["sceneUuid"].as_str(), s["sceneName"].as_str())
                    {
                        self.uuids.insert(uuid.into(), name.into());
                    }
                }
                self.emit(
                    cx,
                    json!({
                        "scenes": scenes,
                        "program_scene": data["currentProgramSceneName"],
                        // null when studio mode is off, which removes it.
                        "preview_scene": data["currentPreviewSceneName"],
                    }),
                );
                self.prune_scenes(cx);
                for scene in scenes {
                    if full || self.at(&["scene_items", &scene]).is_none() {
                        self.read_scene(cx, &scene, false);
                    }
                }
            }
            Read::GroupList => {
                self.groups = names(&data["groups"]);
                let groups = self.groups.clone();
                self.emit(cx, json!({ "groups": groups }));
                self.prune_scenes(cx);
                for group in groups {
                    self.read_scene(cx, &group, true);
                }
            }
            Read::StudioMode => self.emit(cx, json!({"studio_mode": data["studioModeEnabled"]})),
            Read::Transition => {
                self.emit(
                    cx,
                    json!({"transition": {
                        "name": data["transitionName"],
                        "kind": data["transitionKind"],
                        "fixed": data["transitionFixed"],
                        "configurable": data["transitionConfigurable"],
                        // null for a fixed transition, which removes it.
                        "duration_ms": data["transitionDuration"],
                    }}),
                );
                self.replace(
                    cx,
                    &["transition", "settings"],
                    data["transitionSettings"].clone(),
                );
            }
            Read::TransitionList => {
                let transitions: Vec<Value> = data["transitions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|t| {
                        strip_nulls(json!({
                            "name": t["transitionName"],
                            "kind": t["transitionKind"],
                            "fixed": t["transitionFixed"],
                            "configurable": t["transitionConfigurable"],
                        }))
                    })
                    .collect();
                self.emit(cx, json!({ "transitions": transitions }));
            }
            Read::StreamStatus => self.emit(
                cx,
                json!({"streaming": {
                    "active": data["outputActive"],
                    "reconnecting": data["outputReconnecting"],
                    "timecode": data["outputTimecode"],
                    "duration_ms": data["outputDuration"],
                    "congestion": data["outputCongestion"],
                    "bytes": data["outputBytes"],
                    "skipped_frames": data["outputSkippedFrames"],
                    "total_frames": data["outputTotalFrames"],
                }}),
            ),
            Read::RecordStatus => self.emit(
                cx,
                json!({"recording": {
                    "active": data["outputActive"],
                    "paused": data["outputPaused"],
                    "timecode": data["outputTimecode"],
                    "duration_ms": data["outputDuration"],
                    "bytes": data["outputBytes"],
                }}),
            ),
            Read::VirtualCam => {
                self.emit(cx, json!({"virtualcam": {"active": data["outputActive"]}}))
            }
            Read::ReplayBuffer => self.emit(
                cx,
                json!({"replay_buffer": {"active": data["outputActive"]}}),
            ),
            Read::InputList => {
                let mut patch = Map::new();
                let mut listed = Vec::new();
                for i in data["inputs"].as_array().into_iter().flatten() {
                    let Some(name) = i["inputName"].as_str() else {
                        continue;
                    };
                    let kind = i["unversionedInputKind"]
                        .as_str()
                        .or(i["inputKind"].as_str())
                        .unwrap_or("");
                    self.kinds.insert(name.into(), kind.into());
                    if let Some(uuid) = i["inputUuid"].as_str() {
                        self.uuids.insert(uuid.into(), name.into());
                    }
                    patch.insert(name.into(), json!({"kind": i["inputKind"]}));
                    listed.push(name.to_string());
                }
                for gone in self.keys("inputs") {
                    if !listed.contains(&gone) {
                        self.forget_input(cx, &gone);
                    }
                }
                self.kinds.retain(|name, _| listed.contains(name));
                self.emit(cx, json!({ "inputs": patch }));
                for input in listed {
                    self.read_input_all(cx, &input);
                }
            }
            Read::ProfileList => self.emit(
                cx,
                json!({
                    "profile": data["currentProfileName"],
                    "profiles": data["profiles"],
                }),
            ),
            Read::SceneCollectionList => self.emit(
                cx,
                json!({
                    "scene_collection": data["currentSceneCollectionName"],
                    "scene_collections": data["sceneCollections"],
                }),
            ),
            Read::VideoSettings => self.emit(
                cx,
                json!({"video": {
                    "base_width": data["baseWidth"],
                    "base_height": data["baseHeight"],
                    "output_width": data["outputWidth"],
                    "output_height": data["outputHeight"],
                    "fps_numerator": data["fpsNumerator"],
                    "fps_denominator": data["fpsDenominator"],
                }}),
            ),
            Read::RecordDirectory => {
                self.emit(cx, json!({"record_directory": data["recordDirectory"]}))
            }
            // The stream key is in these settings; it is never published.
            Read::StreamService => self.emit(
                cx,
                json!({"stream_service": {
                    "type": data["streamServiceType"],
                    "server": data["streamServiceSettings"]["server"],
                }}),
            ),
            Read::Stats => self.emit(
                cx,
                json!({"stats": {
                    "cpu_percent": data["cpuUsage"],
                    "memory_mb": data["memoryUsage"],
                    "disk_free_mb": data["availableDiskSpace"],
                    "active_fps": data["activeFps"],
                    "average_frame_render_ms": data["averageFrameRenderTime"],
                    "render_skipped_frames": data["renderSkippedFrames"],
                    "render_total_frames": data["renderTotalFrames"],
                    "output_skipped_frames": data["outputSkippedFrames"],
                    "output_total_frames": data["outputTotalFrames"],
                }}),
            ),
            Read::OutputList => {
                let mut outputs = Map::new();
                for o in data["outputs"].as_array().into_iter().flatten() {
                    if let Some(name) = o["outputName"].as_str() {
                        outputs.insert(
                            name.into(),
                            json!({
                                "kind": o["outputKind"],
                                "active": o["outputActive"],
                                "width": o["outputWidth"],
                                "height": o["outputHeight"],
                            }),
                        );
                    }
                }
                self.replace(cx, &["outputs"], Value::Object(outputs));
            }
            Read::SceneItems { scene, .. } => {
                if !self.is_scene(&scene) {
                    return;
                }
                let mut items = Map::new();
                for i in data["sceneItems"].as_array().into_iter().flatten() {
                    let Some(id) = i["sceneItemId"].as_i64() else {
                        continue;
                    };
                    items.insert(
                        id.to_string(),
                        json!({
                            "source": i["sourceName"],
                            "source_type": i["sourceType"],
                            "input_kind": i["inputKind"],
                            "is_group": i["isGroup"],
                            "enabled": i["sceneItemEnabled"],
                            "locked": i["sceneItemLocked"],
                            "index": i["sceneItemIndex"],
                            "blend_mode": i["sceneItemBlendMode"],
                            "transform": i["sceneItemTransform"],
                        }),
                    );
                }
                self.replace(cx, &["scene_items", &scene], Value::Object(items));
            }
            Read::Filters { source } => {
                if !self.is_source(&source) {
                    return;
                }
                let mut filters = Map::new();
                for f in data["filters"].as_array().into_iter().flatten() {
                    if let Some(name) = f["filterName"].as_str() {
                        filters.insert(
                            name.into(),
                            json!({
                                "kind": f["filterKind"],
                                "enabled": f["filterEnabled"],
                                "index": f["filterIndex"],
                                "settings": f["filterSettings"],
                            }),
                        );
                    }
                }
                self.replace(cx, &["filters", &source], Value::Object(filters));
            }
            Read::SceneTransition { scene } => {
                if !self.is_scene(&scene) {
                    return;
                }
                self.replace(
                    cx,
                    &["scene_transitions", &scene],
                    json!({
                        "name": data["transitionName"],
                        "duration_ms": data["transitionDuration"],
                    }),
                );
            }
            Read::Input { input, what } => self.input_answer(cx, &input, what, data),
        }
    }

    fn input_answer(&mut self, cx: &mut Cx, input: &str, what: InputRead, data: &Value) {
        // An answer for an input removed since it was asked.
        if !self.kinds.contains_key(input) {
            return;
        }
        let at = |patch: Value| json!({ "inputs": { input: patch } });
        match what {
            InputRead::Mute => self.emit(cx, at(json!({"muted": data["inputMuted"]}))),
            InputRead::Volume => self.emit(
                cx,
                at(json!({
                    "volume_db": data["inputVolumeDb"],
                    "volume_mul": data["inputVolumeMul"],
                })),
            ),
            InputRead::Balance => self.emit(cx, at(json!({"balance": data["inputAudioBalance"]}))),
            InputRead::SyncOffset => self.emit(
                cx,
                at(json!({"sync_offset_ms": data["inputAudioSyncOffset"]})),
            ),
            InputRead::MonitorType => {
                self.emit(cx, at(json!({"monitor_type": data["monitorType"]})))
            }
            InputRead::Tracks => self.replace(
                cx,
                &["inputs", input, "tracks"],
                data["inputAudioTracks"].clone(),
            ),
            InputRead::Settings => {
                self.emit(cx, at(json!({"kind": data["inputKind"]})));
                self.replace(
                    cx,
                    &["inputs", input, "settings"],
                    data["inputSettings"].clone(),
                );
            }
            InputRead::Active => self.emit(
                cx,
                at(json!({
                    "active": data["videoActive"],
                    "showing": data["videoShowing"],
                })),
            ),
            InputRead::DeinterlaceMode => self.emit(
                cx,
                at(json!({"deinterlace_mode": data["inputDeinterlaceMode"]})),
            ),
            InputRead::DeinterlaceFieldOrder => self.emit(
                cx,
                at(json!({"deinterlace_field_order": data["inputDeinterlaceFieldOrder"]})),
            ),
            InputRead::Media => {
                self.replace(
                    cx,
                    &["inputs", input, "media"],
                    json!({
                        "state": data["mediaState"],
                        "cursor_ms": data["mediaCursor"],
                        "duration_ms": data["mediaDuration"],
                    }),
                );
                self.arm_media(cx);
            }
        }
    }

    // ---- events ----

    fn event(&mut self, cx: &mut Cx, d: &Value) {
        let data = &d["eventData"];
        let text = |field: &str| data[field].as_str().map(str::to_string);
        match d.get("eventType").and_then(Value::as_str).unwrap_or("") {
            // Configuration.
            "CurrentSceneCollectionChanged" => {
                self.clear_collections(cx);
                self.emit(cx, json!({"scene_collection": data["sceneCollectionName"]}));
                for read in [
                    Read::SceneList { full: true },
                    Read::GroupList,
                    Read::InputList,
                    Read::Transition,
                    Read::TransitionList,
                ] {
                    self.read(cx, read);
                }
            }
            "SceneCollectionListChanged" => {
                self.emit(cx, json!({"scene_collections": data["sceneCollections"]}))
            }
            "CurrentProfileChanged" => {
                self.emit(cx, json!({"profile": data["profileName"]}));
                for read in [
                    Read::VideoSettings,
                    Read::RecordDirectory,
                    Read::StreamService,
                    Read::ReplayBuffer,
                ] {
                    self.read(cx, read);
                }
            }
            "ProfileListChanged" => self.emit(cx, json!({"profiles": data["profiles"]})),

            // Scenes.
            "CurrentProgramSceneChanged" => {
                self.emit(cx, json!({"program_scene": data["sceneName"]}))
            }
            "CurrentPreviewSceneChanged" => {
                self.emit(cx, json!({"preview_scene": data["sceneName"]}))
            }
            "StudioModeStateChanged" => {
                let enabled = data["studioModeEnabled"].as_bool() == Some(true);
                let mut patch = json!({"studio_mode": enabled});
                if !enabled {
                    patch["preview_scene"] = Value::Null;
                }
                self.emit(cx, patch);
                if enabled {
                    // Studio mode opens with a preview scene; the event does
                    // not say which.
                    self.read(cx, Read::SceneList { full: false });
                }
            }
            "SceneListChanged" => {
                self.emit(cx, json!({"scenes": scene_names(data)}));
                self.prune_scenes(cx);
            }
            "SceneCreated" => {
                if data["isGroup"].as_bool() == Some(true) {
                    self.read(cx, Read::GroupList);
                } else {
                    self.read(cx, Read::SceneList { full: false });
                }
            }
            "SceneRemoved" | "SceneNameChanged" => {
                let Some(old) = text("oldSceneName").or_else(|| text("sceneName")) else {
                    return;
                };
                let group = data["isGroup"].as_bool() == Some(true) || self.groups.contains(&old);
                for section in ["scene_items", "scene_transitions", "filters"] {
                    self.delete(cx, &[section, &old]);
                }
                if group {
                    self.read(cx, Read::GroupList);
                } else {
                    self.read(cx, Read::SceneList { full: false });
                }
            }

            // Scene items.
            "SceneItemCreated" | "SceneItemRemoved" => {
                if let Some(scene) = text("sceneName") {
                    if d["eventType"] == "SceneItemRemoved" {
                        if let Some(id) = data["sceneItemId"].as_i64() {
                            self.delete(cx, &["scene_items", &scene, &id.to_string()]);
                        }
                    }
                    // Indexes shift; the whole list is read again.
                    let group = self.groups.contains(&scene);
                    self.read(cx, Read::SceneItems { scene, group });
                }
            }
            "SceneItemListReindexed" => {
                if let Some(scene) = text("sceneName") {
                    let mut items = Map::new();
                    for i in data["sceneItems"].as_array().into_iter().flatten() {
                        if let Some(id) = i["sceneItemId"].as_i64() {
                            if self.at(&["scene_items", &scene, &id.to_string()]).is_some() {
                                items.insert(id.to_string(), json!({"index": i["sceneItemIndex"]}));
                            }
                        }
                    }
                    if !items.is_empty() {
                        self.emit(cx, json!({"scene_items": {scene: items}}));
                    }
                }
            }
            "SceneItemEnableStateChanged" => {
                self.item_patch(cx, data, json!({"enabled": data["sceneItemEnabled"]}))
            }
            "SceneItemLockStateChanged" => {
                self.item_patch(cx, data, json!({"locked": data["sceneItemLocked"]}))
            }
            "SceneItemTransformChanged" => {
                if let (Some(scene), Some(id)) = (text("sceneName"), data["sceneItemId"].as_i64()) {
                    self.transforms
                        .insert((scene, id.to_string()), data["sceneItemTransform"].clone());
                    self.arm_flush(cx);
                }
            }

            // Filters.
            "SourceFilterCreated"
            | "SourceFilterRemoved"
            | "SourceFilterNameChanged"
            | "SourceFilterListReindexed" => {
                if let Some(source) = text("sourceName") {
                    self.read(cx, Read::Filters { source });
                }
            }
            "SourceFilterEnableStateChanged" => {
                if let (Some(source), Some(filter)) = (text("sourceName"), text("filterName")) {
                    if self.at(&["filters", &source, &filter]).is_some() {
                        self.emit(
                            cx,
                            json!({"filters": {source: {filter: {"enabled": data["filterEnabled"]}}}}),
                        );
                    }
                }
            }
            "SourceFilterSettingsChanged" => {
                if let (Some(source), Some(filter)) = (text("sourceName"), text("filterName")) {
                    if self.at(&["filters", &source, &filter]).is_some() {
                        self.replace(
                            cx,
                            &["filters", &source, &filter, "settings"],
                            data["filterSettings"].clone(),
                        );
                    }
                }
            }

            // Transitions.
            "CurrentSceneTransitionChanged" => {
                self.emit(cx, json!({"transition": {"name": data["transitionName"]}}));
                self.read(cx, Read::Transition);
            }
            "CurrentSceneTransitionDurationChanged" => self.emit(
                cx,
                json!({"transition": {"duration_ms": data["transitionDuration"]}}),
            ),
            "SceneTransitionStarted" => self.emit(cx, json!({"transition": {"active": true}})),
            "SceneTransitionEnded" => self.emit(cx, json!({"transition": {"active": false}})),

            // Outputs.
            "StreamStateChanged" => {
                let state = output_state(data);
                let mut patch = json!({"active": data["outputActive"]});
                match state {
                    "RECONNECTING" => patch["reconnecting"] = json!(true),
                    "RECONNECTED" | "STARTED" | "STOPPED" => patch["reconnecting"] = json!(false),
                    _ => {}
                }
                self.emit(cx, json!({ "streaming": patch }));
            }
            "RecordStateChanged" => {
                let state = output_state(data);
                let mut patch = json!({
                    "active": data["outputActive"],
                    "paused": state == "PAUSED",
                });
                if state == "STOPPED" {
                    patch["file"] = Value::Null;
                    if data["outputPath"].is_string() {
                        patch["last_file"] = data["outputPath"].clone();
                    }
                }
                self.emit(cx, json!({ "recording": patch }));
            }
            "RecordFileChanged" => {
                self.emit(cx, json!({"recording": {"file": data["newOutputPath"]}}))
            }
            "ReplayBufferStateChanged" => self.emit(
                cx,
                json!({"replay_buffer": {"active": data["outputActive"]}}),
            ),
            "ReplayBufferSaved" => self.emit(
                cx,
                json!({"replay_buffer": {"last_replay": data["savedReplayPath"]}}),
            ),
            "VirtualcamStateChanged" => {
                self.emit(cx, json!({"virtualcam": {"active": data["outputActive"]}}))
            }

            // Inputs.
            "InputCreated" => {
                if let Some(input) = text("inputName") {
                    let kind = data["unversionedInputKind"]
                        .as_str()
                        .or(data["inputKind"].as_str())
                        .unwrap_or("");
                    self.kinds.insert(input.clone(), kind.into());
                    if let Some(uuid) = text("inputUuid") {
                        self.uuids.insert(uuid, input.clone());
                    }
                    self.emit(cx, json!({"inputs": {&input: {"kind": data["inputKind"]}}}));
                    self.read_input_all(cx, &input);
                }
            }
            "InputRemoved" => {
                if let Some(input) = text("inputName") {
                    self.forget_input(cx, &input);
                }
            }
            "InputNameChanged" => {
                if let (Some(old), Some(new)) = (text("oldInputName"), text("inputName")) {
                    let kind = self.kinds.get(&old).cloned().unwrap_or_default();
                    self.forget_input(cx, &old);
                    self.kinds.insert(new.clone(), kind);
                    if let Some(uuid) = text("inputUuid") {
                        self.uuids.insert(uuid, new.clone());
                    }
                    self.read_input_all(cx, &new);
                }
            }
            "InputSettingsChanged" => {
                if let Some(input) = self.known_input(data) {
                    self.replace(
                        cx,
                        &["inputs", &input, "settings"],
                        data["inputSettings"].clone(),
                    );
                }
            }
            "InputMuteStateChanged" => {
                self.input_patch(cx, data, json!({"muted": data["inputMuted"]}))
            }
            "InputVolumeChanged" => self.input_patch(
                cx,
                data,
                json!({"volume_db": data["inputVolumeDb"], "volume_mul": data["inputVolumeMul"]}),
            ),
            "InputAudioBalanceChanged" => {
                self.input_patch(cx, data, json!({"balance": data["inputAudioBalance"]}))
            }
            "InputAudioSyncOffsetChanged" => self.input_patch(
                cx,
                data,
                json!({"sync_offset_ms": data["inputAudioSyncOffset"]}),
            ),
            "InputAudioMonitorTypeChanged" => {
                self.input_patch(cx, data, json!({"monitor_type": data["monitorType"]}))
            }
            "InputAudioTracksChanged" => {
                if let Some(input) = self.known_input(data) {
                    self.replace(
                        cx,
                        &["inputs", &input, "tracks"],
                        data["inputAudioTracks"].clone(),
                    );
                }
            }
            "InputActiveStateChanged" => {
                self.input_patch(cx, data, json!({"active": data["videoActive"]}))
            }
            "InputShowStateChanged" => {
                self.input_patch(cx, data, json!({"showing": data["videoShowing"]}))
            }
            "InputVolumeMeters" => {
                for i in data["inputs"].as_array().into_iter().flatten() {
                    let Some(name) = i["inputName"].as_str() else {
                        continue;
                    };
                    let channels: Vec<[f64; 3]> = i["inputLevelsMul"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|c| {
                            let level = |n: usize| c[n].as_f64().unwrap_or(0.0);
                            [level(0), level(1), level(2)]
                        })
                        .collect();
                    let held = self.meters.entry(name.to_string()).or_default();
                    if held.len() != channels.len() {
                        *held = channels;
                    } else {
                        for (h, c) in held.iter_mut().zip(channels) {
                            h[0] = c[0];
                            h[1] = h[1].max(c[1]);
                            h[2] = h[2].max(c[2]);
                        }
                    }
                }
                self.arm_flush(cx);
            }

            // Media inputs: the event says what happened; the status says
            // where the media is now.
            "MediaInputPlaybackStarted"
            | "MediaInputPlaybackEnded"
            | "MediaInputActionTriggered" => {
                if let Some(input) = self.known_input(data) {
                    self.read_input(cx, &input, InputRead::Media);
                }
            }

            // OBS is shutting down; the close follows.
            "ExitStarted" => cx.log(Level::Info, "OBS is exiting"),
            _ => {}
        }
    }

    fn known_input(&self, data: &Value) -> Option<String> {
        let input = data["inputName"].as_str()?;
        self.kinds.contains_key(input).then(|| input.to_string())
    }

    fn input_patch(&mut self, cx: &mut Cx, data: &Value, patch: Value) {
        if let Some(input) = self.known_input(data) {
            self.emit(cx, json!({"inputs": {input: patch}}));
        }
    }

    fn item_patch(&mut self, cx: &mut Cx, data: &Value, patch: Value) {
        let (Some(scene), Some(id)) = (data["sceneName"].as_str(), data["sceneItemId"].as_i64())
        else {
            return;
        };
        let id = id.to_string();
        if self.at(&["scene_items", scene, &id]).is_some() {
            self.emit(cx, json!({"scene_items": {scene: {id: patch}}}));
        }
    }

    fn message(&mut self, cx: &mut Cx, text: &str) {
        let Ok(message) = serde_json::from_str::<Value>(text) else {
            cx.log(Level::Warning, "unparseable message from OBS");
            return;
        };
        let d = &message["d"];
        match message.get("op").and_then(Value::as_u64) {
            Some(OP_HELLO) if self.phase == Phase::Handshake => self.hello(cx, d),
            Some(OP_IDENTIFIED) if self.phase == Phase::Handshake => self.identified(cx),
            Some(OP_REQUEST_RESPONSE) => self.response(cx, d),
            Some(OP_EVENT) => self.event(cx, d),
            _ => {}
        }
    }
}

/// Scene names, top of OBS's list first. GetSceneList and SceneListChanged
/// give sceneIndex 0 to the bottom scene.
fn scene_names(data: &Value) -> Vec<String> {
    let mut scenes: Vec<(i64, String)> = data["scenes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some((
                s["sceneIndex"].as_i64().unwrap_or(0),
                s["sceneName"].as_str()?.to_string(),
            ))
        })
        .collect();
    scenes.sort_by_key(|s| std::cmp::Reverse(s.0));
    scenes.into_iter().map(|(_, name)| name).collect()
}

/// A command's request: the spec names commands and parameters after
/// obs-websocket's requests and fields in snake_case (`tools/generate_obs.py`),
/// so the conversion back is mechanical: get_scene_list -> GetSceneList,
/// scene_name -> sceneName.
fn camel(name: &str, upper: bool) -> String {
    let mut out = String::with_capacity(name.len());
    let mut up = upper;
    for c in name.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn request_for(name: &str, params: &Params) -> (String, Option<Value>) {
    let data: serde_json::Map<String, Value> = params
        .iter()
        .map(|(k, v)| (camel(k, false), v.clone()))
        .collect();
    let data = (!data.is_empty()).then_some(Value::Object(data));
    (camel(name, true), data)
}

impl Module for Obs {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.open(cx);
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
        // The catalogue has already checked the command and its parameters.
        let (request_type, data) = request_for(name, params);
        let then = self.then_read(name, params);
        self.request(cx, Purpose::Command { id, then }, &request_type, data);
    }

    fn ws(&mut self, cx: &mut Cx, _socket: Key, input: WsInput) {
        if self.refused.is_some() {
            return;
        }
        match input {
            WsInput::Opened => {}
            WsInput::Text(text) => {
                cx.alive();
                self.message(cx, &text);
            }
            WsInput::Binary(_) | WsInput::Activity => cx.alive(),
            WsInput::Closed { code, reason } => match code {
                Some(CLOSE_AUTHENTICATION_FAILED) => self.refuse(cx, "OBS rejected the password"),
                Some(CLOSE_UNSUPPORTED_RPC_VERSION) => self.lost(
                    cx,
                    format!("OBS does not support RPC version {RPC_VERSION}: {reason}"),
                ),
                Some(code) => self.lost(cx, format!("closed by OBS ({code}): {reason}")),
                None => self.lost(cx, reason),
            },
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if self.refused.is_some() {
            return;
        }
        match key {
            RETRY => self.open(cx),
            HELLO => self.lost(cx, "no handshake from OBS".into()),
            POLL if self.phase == Phase::Identified => self.poll(cx),
            MEDIA if self.phase == Phase::Identified => {
                self.media_armed = false;
                for input in self.playing() {
                    self.read_input(cx, &input, InputRead::Media);
                }
            }
            FLUSH if self.phase == Phase::Identified => {
                self.flush_armed = false;
                self.flush(cx);
            }
            REQUEST => {
                let now = cx.now();
                let expired: Vec<String> = self
                    .pending
                    .iter()
                    .filter(|(_, p)| p.deadline <= now)
                    .map(|(id, _)| id.clone())
                    .collect();
                let mut missed_poll = false;
                for id in expired {
                    let p = self.pending.remove(&id).unwrap();
                    match p.purpose {
                        Purpose::Command { id, .. } => cx.complete(id, Err(CommandError::Timeout)),
                        purpose if purpose.is_poll() => missed_poll = true,
                        _ => {}
                    }
                }
                self.arm_request_timer(cx);
                if missed_poll {
                    self.strikes += 1;
                    if self.strikes >= POLL_STRIKES {
                        self.lost(cx, "OBS stopped answering".into());
                    }
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.ws_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    fn obs(password: &str) -> Obs {
        obs_with(password, true)
    }

    fn obs_with(password: &str, monitor: bool) -> Obs {
        Obs::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)),
            host_name: None,
            port: None,
            model: "obs-studio-28".into(),
            channels: None,
            settings: json!({"password": password}).as_object().unwrap().clone(),
            monitor,
        })
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

    fn feed(m: &mut Obs, now: Millis, message: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.ws(&mut cx, SOCKET, WsInput::Text(message.to_string()));
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

    /// Answer every request in `actions` from `responses` (requestType →
    /// responseData), and return what the answers caused.
    fn answer(m: &mut Obs, now: Millis, actions: &[Action], responses: &Value) -> Vec<Action> {
        let mut out = Vec::new();
        for msg in sent(actions) {
            if msg["op"] != OP_REQUEST {
                continue;
            }
            let d = &msg["d"];
            let kind = d["requestType"].as_str().unwrap();
            let (ok, data) = match responses.get(kind) {
                Some(data) => (true, data.clone()),
                None => (false, Value::Null),
            };
            out.extend(feed(
                m,
                now,
                json!({"op": 7, "d": {
                    "requestType": kind,
                    "requestId": d["requestId"],
                    "requestStatus": {"result": ok, "code": if ok { 100 } else { 600 }},
                    "responseData": data,
                }}),
            ));
        }
        out
    }

    fn event(m: &mut Obs, now: Millis, kind: &str, data: Value) -> Vec<Action> {
        feed(
            m,
            now,
            json!({"op": 5, "d": {"eventType": kind, "eventData": data}}),
        )
    }

    /// Requests in `actions` as (requestType, requestData).
    fn requests(actions: &[Action]) -> Vec<(String, Value)> {
        sent(actions)
            .into_iter()
            .filter(|m| m["op"] == OP_REQUEST)
            .map(|m| {
                (
                    m["d"]["requestType"].as_str().unwrap().to_string(),
                    m["d"]["requestData"].clone(),
                )
            })
            .collect()
    }

    type Reply = dyn Fn(&str, &Value) -> Option<Value>;

    /// Answer every request in `actions`, and every request those answers
    /// cause, from `reply` (None refuses); return everything that happened.
    fn settle(m: &mut Obs, now: Millis, actions: Vec<Action>, reply: &Reply) -> Vec<Action> {
        let mut all = actions.clone();
        let mut todo = actions;
        while !todo.is_empty() {
            let mut next = Vec::new();
            for msg in sent(&todo) {
                if msg["op"] != OP_REQUEST {
                    continue;
                }
                let d = &msg["d"];
                let kind = d["requestType"].as_str().unwrap();
                let (ok, data) = match reply(kind, &d["requestData"]) {
                    Some(data) => (true, data),
                    None => (false, Value::Null),
                };
                next.extend(feed(
                    m,
                    now,
                    json!({"op": 7, "d": {
                        "requestType": kind,
                        "requestId": d["requestId"],
                        "requestStatus": {"result": ok, "code": if ok { 100 } else { 600 }},
                        "responseData": data,
                    }}),
                ));
            }
            all.extend(next.clone());
            todo = next;
        }
        all
    }

    fn timer(m: &mut Obs, now: Millis, key: Key) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.timer(&mut cx, key);
        cx.take()
    }

    /// A small OBS: two scenes (Wide with two items), a microphone with a
    /// filter, and a playing media source.
    fn small_obs(kind: &str, data: &Value) -> Option<Value> {
        let name = |k: &str| data[k].as_str().unwrap_or("").to_string();
        Some(match kind {
            "GetSceneList" => json!({
                "currentProgramSceneName": "Wide",
                "currentPreviewSceneName": null,
                "scenes": [
                    {"sceneName": "Close", "sceneIndex": 0, "sceneUuid": "u-close"},
                    {"sceneName": "Wide", "sceneIndex": 1, "sceneUuid": "u-wide"},
                ],
            }),
            "GetGroupList" => json!({"groups": []}),
            "GetSceneItemList" if name("sceneName") == "Wide" => json!({"sceneItems": [
                {"sceneItemId": 1, "sceneItemIndex": 0, "sourceName": "Camera",
                 "sourceType": "OBS_SOURCE_TYPE_INPUT", "inputKind": "dshow_input",
                 "isGroup": null, "sceneItemEnabled": true, "sceneItemLocked": false,
                 "sceneItemBlendMode": "OBS_BLEND_NORMAL",
                 "sceneItemTransform": {"positionX": 0.0, "positionY": 0.0}},
                {"sceneItemId": 2, "sceneItemIndex": 1, "sourceName": "Mic",
                 "sourceType": "OBS_SOURCE_TYPE_INPUT", "inputKind": "wasapi_input_capture",
                 "isGroup": null, "sceneItemEnabled": true, "sceneItemLocked": true,
                 "sceneItemBlendMode": "OBS_BLEND_NORMAL",
                 "sceneItemTransform": {"positionX": 10.0, "positionY": 0.0}},
            ]}),
            "GetSceneItemList" => json!({"sceneItems": []}),
            "GetSourceFilterList" if name("sourceName") == "Mic" => json!({"filters": [
                {"filterName": "Gate", "filterKind": "noise_gate_filter", "filterEnabled": true,
                 "filterIndex": 0, "filterSettings": {"open_threshold": -26.0}},
            ]}),
            "GetSourceFilterList" => json!({"filters": []}),
            "GetSceneSceneTransitionOverride" => {
                json!({"transitionName": null, "transitionDuration": null})
            }
            "GetInputList" => json!({"inputs": [
                {"inputName": "Mic", "inputUuid": "u-mic", "inputKind": "wasapi_input_capture",
                 "unversionedInputKind": "wasapi_input_capture"},
                {"inputName": "Clip", "inputUuid": "u-clip", "inputKind": "ffmpeg_source",
                 "unversionedInputKind": "ffmpeg_source"},
            ]}),
            "GetInputMute" if name("inputName") == "Mic" => json!({"inputMuted": false}),
            "GetInputVolume" if name("inputName") == "Mic" => {
                json!({"inputVolumeDb": -6.0, "inputVolumeMul": 0.501})
            }
            "GetInputAudioTracks" if name("inputName") == "Mic" => {
                json!({"inputAudioTracks": {"1": true, "2": false}})
            }
            "GetInputSettings" => {
                json!({"inputKind": "ffmpeg_source", "inputSettings": {"looping": true}})
            }
            "GetMediaInputStatus" => json!({"mediaState": "OBS_MEDIA_STATE_PLAYING",
                "mediaCursor": 1000, "mediaDuration": 60000}),
            "GetSceneTransitionList" => json!({"currentSceneTransitionName": "Fade",
                "transitions": [{"transitionName": "Cut", "transitionKind": "cut_transition",
                    "transitionFixed": true, "transitionConfigurable": false},
                    {"transitionName": "Fade", "transitionKind": "fade_transition",
                    "transitionFixed": false, "transitionConfigurable": true}]}),
            "GetProfileList" => {
                json!({"currentProfileName": "Show", "profiles": ["Show", "Rehearsal"]})
            }
            "GetSceneCollectionList" => json!({"currentSceneCollectionName": "Main",
                "sceneCollections": ["Main", "Spare"]}),
            "GetStreamServiceSettings" => json!({"streamServiceType": "rtmp_custom",
                "streamServiceSettings": {"server": "rtmp://example.net/live", "key": "secret"}}),
            "GetOutputList" => json!({"outputs": [{"outputName": "simple_stream",
                "outputKind": "rtmp_output", "outputActive": false,
                "outputWidth": 1920, "outputHeight": 1080, "outputFlags": {}}]}),
            "GetRecordStatus" => json!({"outputActive": true, "outputPaused": false,
                "outputTimecode": "00:00:10.000", "outputDuration": 10000, "outputBytes": 5}),
            _ => return None,
        })
    }

    /// Identified and fully loaded from `small_obs`.
    fn loaded(m: &mut Obs) -> Value {
        let a = identified(m);
        state(&settle(m, 30, a, &small_obs))
    }

    fn hello(challenge: Option<(&str, &str)>) -> Value {
        let mut d = json!({"obsWebSocketVersion": "5.5.2", "rpcVersion": 1});
        if let Some((challenge, salt)) = challenge {
            d["authentication"] = json!({"challenge": challenge, "salt": salt});
        }
        json!({"op": 0, "d": d})
    }

    /// Open, handshake without a password, and return the Identified actions.
    fn identified(m: &mut Obs) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        feed(m, 10, hello(None));
        feed(m, 20, json!({"op": 2, "d": {"negotiatedRpcVersion": 1}}))
    }

    #[test]
    fn opens_with_the_json_subprotocol_on_4455() {
        let mut m = obs("");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::WsOpen {
            socket: SOCKET,
            request: WsRequest {
                url: "ws://10.0.0.7:4455".into(),
                headers: vec![("Sec-WebSocket-Protocol".into(), "obswebsocket.json".into())],
                accept_invalid_certs: false,
            },
        }));
    }

    #[test]
    fn authentication_matches_the_documented_construction() {
        // Computed independently: base64(sha256("supersecret" + salt)), then
        // base64(sha256(that + challenge)).
        let b64 = base64::engine::general_purpose::STANDARD;
        let salt = "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=";
        let challenge = "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY=";
        let secret = b64.encode(Sha256::digest(format!("supersecret{salt}").as_bytes()));
        let expected = b64.encode(Sha256::digest(format!("{secret}{challenge}").as_bytes()));
        assert_eq!(authentication("supersecret", salt, challenge), expected);
        assert_eq!(expected.len(), 44);
    }

    #[test]
    fn identifies_with_authentication_and_event_subscriptions() {
        let mut m = obs("supersecret");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let a = feed(&mut m, 10, hello(Some(("chal", "salt"))));
        let identify = &sent(&a)[0];
        assert_eq!(identify["op"], 1);
        assert_eq!(identify["d"]["rpcVersion"], 1);
        assert_eq!(identify["d"]["eventSubscriptions"], EVENTS);
        assert_eq!(
            identify["d"]["authentication"],
            authentication("supersecret", "salt", "chal")
        );
    }

    #[test]
    fn a_password_required_but_not_configured_is_terminal() {
        let mut m = obs("");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let a = feed(&mut m, 10, hello(Some(("chal", "salt"))));
        assert!(sent(&a).is_empty());
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
    }

    #[test]
    fn a_rejected_password_is_terminal() {
        let mut m = obs("wrong");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        feed(&mut m, 10, hello(Some(("chal", "salt"))));
        let mut cx = Cx::new(20);
        m.ws(
            &mut cx,
            SOCKET,
            WsInput::Closed {
                code: Some(4009),
                reason: "Authentication failed.".into(),
            },
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { .. } | Action::WsOpen { .. })));

        let mut cx = Cx::new(60_000);
        m.timer(&mut cx, RETRY);
        assert!(cx.take().is_empty());
        let mut cx = Cx::new(60_001);
        m.command(&mut cx, 4, "start_stream", &Params::new());
        assert!(matches!(
            &cx.take()[..],
            [Action::Complete {
                id: 4,
                result: Err(CommandError::Auth { .. })
            }]
        ));
    }

    #[test]
    fn identified_loads_the_initial_state() {
        let mut m = obs("");
        let a = identified(&mut m);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let answered = answer(
            &mut m,
            30,
            &a,
            &json!({
                "GetVersion": {"obsVersion": "31.0.1", "obsWebSocketVersion": "5.5.2", "platformDescription": "Windows 11"},
                "GetSceneList": {
                    "currentProgramSceneName": "Wide",
                    "currentPreviewSceneName": null,
                    "scenes": [
                        {"sceneName": "Close", "sceneIndex": 0},
                        {"sceneName": "Wide", "sceneIndex": 1},
                    ],
                },
                "GetStudioModeEnabled": {"studioModeEnabled": false},
                "GetCurrentSceneTransition": {"transitionName": "Fade", "transitionDuration": 300},
                "GetVirtualCamStatus": {"outputActive": false},
                "GetInputList": {"inputs": [{"inputName": "Mic"}, {"inputName": "Camera"}]},
                "GetStreamStatus": {"outputActive": true, "outputReconnecting": false,
                    "outputDuration": 61000, "outputCongestion": 0.02, "outputBytes": 1000,
                    "outputSkippedFrames": 1, "outputTotalFrames": 3600},
                "GetRecordStatus": {"outputActive": false, "outputPaused": false,
                    "outputDuration": 0, "outputBytes": 0},
            }),
        );
        let mut all = a.clone();
        all.extend(answered.clone());
        // The input list leads to per-input queries. The camera has no audio.
        all.extend(answer(
            &mut m,
            40,
            &answered,
            &json!({"GetInputMute": {"inputMuted": true}, "GetInputVolume": {"inputVolumeDb": -6.0}}),
        ));
        let s = state(&all);
        assert_eq!(s["program_scene"], "Wide");
        assert_eq!(s.get("preview_scene"), None);
        assert_eq!(s["scenes"], json!(["Wide", "Close"]));
        assert_eq!(s["studio_mode"], false);
        assert_eq!(s["transition"], json!({"name": "Fade", "duration_ms": 300}));
        assert_eq!(s["streaming"]["active"], true);
        assert_eq!(s["streaming"]["congestion"], 0.02);
        assert_eq!(s["device"]["obs_version"], "31.0.1");
        assert_eq!(
            s["inputs"]["Mic"],
            json!({"muted": true, "volume_db": -6.0})
        );
    }

    #[test]
    fn events_update_the_state() {
        let mut m = obs("");
        identified(&mut m);
        event(
            &mut m,
            45,
            "InputCreated",
            json!({"inputName": "Mic", "inputKind": "wasapi_input_capture",
                "unversionedInputKind": "wasapi_input_capture"}),
        );
        let mut a = feed(
            &mut m,
            50,
            json!({"op": 5, "d": {"eventType": "CurrentProgramSceneChanged", "eventIntent": 4,
                "eventData": {"sceneName": "Close"}}}),
        );
        a.extend(feed(
            &mut m,
            51,
            json!({"op": 5, "d": {"eventType": "InputMuteStateChanged", "eventIntent": 8,
                "eventData": {"inputName": "Mic", "inputMuted": false}}}),
        ));
        a.extend(feed(
            &mut m,
            52,
            json!({"op": 5, "d": {"eventType": "RecordStateChanged", "eventIntent": 64,
                "eventData": {"outputActive": true, "outputState": "OBS_WEBSOCKET_OUTPUT_PAUSED"}}}),
        ));
        let s = state(&a);
        assert_eq!(s["program_scene"], "Close");
        assert_eq!(s["inputs"]["Mic"]["muted"], false);
        assert_eq!(s["recording"], json!({"active": true, "paused": true}));
    }

    #[test]
    fn commands_send_requests_and_map_the_status() {
        let mut m = obs("");
        identified(&mut m);
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            9,
            "set_input_mute",
            &json!({"input_name": "Mic", "input_muted": true})
                .as_object()
                .unwrap()
                .clone(),
        );
        let a = cx.take();
        let request = &sent(&a)[0]["d"];
        assert_eq!(request["requestType"], "SetInputMute");
        assert_eq!(
            request["requestData"],
            json!({"inputName": "Mic", "inputMuted": true})
        );
        let a = feed(
            &mut m,
            110,
            json!({"op": 7, "d": {"requestType": "SetInputMute", "requestId": request["requestId"],
                "requestStatus": {"result": true, "code": 100}}}),
        );
        assert!(a.contains(&Action::Complete {
            id: 9,
            result: Ok(Outcome::Ack)
        }));

        let mut cx = Cx::new(120);
        m.command(
            &mut cx,
            10,
            "trigger_studio_mode_transition",
            &Params::new(),
        );
        let request = sent(&cx.take())[0]["d"].clone();
        let a = feed(
            &mut m,
            130,
            json!({"op": 7, "d": {"requestType": "TriggerStudioModeTransition",
                "requestId": request["requestId"],
                "requestStatus": {"result": false, "code": 506, "comment": "Studio mode is not active."}}}),
        );
        assert!(a.contains(&Action::Complete {
            id: 10,
            result: Err(CommandError::DeviceError {
                code: Some("506".into()),
                message: "Studio mode is not active.".into()
            })
        }));
    }

    #[test]
    fn names_convert_mechanically_to_obs_websocket() {
        assert_eq!(camel("get_scene_list", true), "GetSceneList");
        assert_eq!(camel("input_volume_db", false), "inputVolumeDb");
        let params = json!({"scene_name": "Wide", "scene_item_enabled": true})
            .as_object()
            .unwrap()
            .clone();
        let (request_type, data) = request_for("set_scene_item_enabled", &params);
        assert_eq!(request_type, "SetSceneItemEnabled");
        assert_eq!(
            data,
            Some(json!({"sceneName": "Wide", "sceneItemEnabled": true}))
        );
        assert_eq!(
            request_for("start_stream", &Params::new()),
            ("StartStream".into(), None)
        );
    }

    #[test]
    fn a_reply_with_data_is_the_value() {
        let mut m = obs("");
        identified(&mut m);
        let mut cx = Cx::new(100);
        m.command(&mut cx, 3, "get_scene_list", &Params::new());
        let request = sent(&cx.take())[0]["d"].clone();
        let a = feed(
            &mut m,
            110,
            json!({"op": 7, "d": {"requestType": "GetSceneList", "requestId": request["requestId"],
                "requestStatus": {"result": true, "code": 100},
                "responseData": {"scenes": [
                    {"sceneName": "C", "sceneIndex": 0},
                    {"sceneName": "B", "sceneIndex": 1},
                    {"sceneName": "A", "sceneIndex": 2}]}}}),
        );
        let value = a
            .iter()
            .find_map(|x| match x {
                Action::Complete {
                    id: 3,
                    result: Ok(Outcome::Value { value }),
                } => Some(value.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(value["scenes"][2]["sceneName"], "A");
    }

    #[test]
    fn unanswered_polls_drop_the_connection_and_retry_with_backoff() {
        let mut m = obs("");
        identified(&mut m);
        // Two poll rounds with no answers.
        let mut cx = Cx::new(REQUEST_TIMEOUT + 20);
        m.timer(&mut cx, REQUEST);
        assert!(cx
            .take()
            .iter()
            .all(|a| !matches!(a, Action::Connection(_))));
        let mut cx = Cx::new(POLL_EVERY + 20);
        m.timer(&mut cx, POLL);
        cx.take();
        let mut cx = Cx::new(POLL_EVERY + REQUEST_TIMEOUT + 20);
        m.timer(&mut cx, REQUEST);
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));

        // Reconnect, fail again: the retry backs off.
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, RETRY);
        assert!(cx.take().iter().any(|a| matches!(a, Action::WsOpen { .. })));
        let mut cx = Cx::new(20_100);
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
            after: RETRY_MIN * 2
        }));
    }

    #[test]
    fn a_command_in_flight_fails_when_the_connection_drops() {
        let mut m = obs("");
        identified(&mut m);
        let mut cx = Cx::new(100);
        m.command(&mut cx, 5, "start_stream", &Params::new());
        cx.take();
        let mut cx = Cx::new(110);
        m.ws(
            &mut cx,
            SOCKET,
            WsInput::Closed {
                code: Some(1001),
                reason: "going away".into(),
            },
        );
        assert!(cx.take().iter().any(|a| matches!(
            a,
            Action::Complete {
                id: 5,
                result: Err(CommandError::Transport { .. })
            }
        )));
    }

    #[test]
    fn opened_for_commands_only_it_subscribes_to_nothing_and_reads_nothing() {
        let mut m = obs_with("", false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let a = feed(&mut m, 10, hello(None));
        assert_eq!(sent(&a)[0]["d"]["eventSubscriptions"], 0);

        // Identified: only the liveness request, GetVersion.
        let a = feed(
            &mut m,
            20,
            json!({"op": 2, "d": {"negotiatedRpcVersion": 1}}),
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let requests: Vec<Value> = sent(&a)
            .iter()
            .map(|r| r["d"]["requestType"].clone())
            .collect();
        assert_eq!(requests, [json!("GetVersion")]);
        answer(
            &mut m,
            30,
            &a,
            &json!({"GetVersion": {"obsVersion": "31.0.1"}}),
        );

        // The poll is the same liveness request, never the output status.
        let mut cx = Cx::new(POLL_EVERY + 20);
        m.timer(&mut cx, POLL);
        let requests: Vec<Value> = sent(&cx.take())
            .iter()
            .map(|r| r["d"]["requestType"].clone())
            .collect();
        assert_eq!(requests, [json!("GetVersion")]);

        // Commands work as before.
        let mut cx = Cx::new(POLL_EVERY + 30);
        m.command(&mut cx, 8, "start_stream", &Params::new());
        let request = sent(&cx.take())[0]["d"].clone();
        assert_eq!(request["requestType"], "StartStream");
        let a = feed(
            &mut m,
            POLL_EVERY + 40,
            json!({"op": 7, "d": {"requestType": "StartStream", "requestId": request["requestId"],
                "requestStatus": {"result": true, "code": 100}}}),
        );
        assert!(a.contains(&Action::Complete {
            id: 8,
            result: Ok(Outcome::Ack)
        }));
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let mut m = obs("");
        identified(&mut m);
        let mut cx = Cx::new(100);
        m.command(&mut cx, 9, "start_stream", &Params::new());
        let request = sent(&cx.take())[0]["d"].clone();
        let a = feed(
            &mut m,
            142,
            json!({"op": 7, "d": {"requestType": "StartStream", "requestId": request["requestId"],
                "requestStatus": {"result": true, "code": 100}}}),
        );
        assert!(a.contains(&Action::RoundTrip(42)));
        // An event is not a reply.
        let a = feed(
            &mut m,
            150,
            json!({"op": 5, "d": {"eventType": "CurrentProgramSceneChanged",
                "eventData": {"sceneName": "Close"}}}),
        );
        assert!(!a.iter().any(|x| matches!(x, Action::RoundTrip(_))));
    }
    fn deletes(actions: &[Action], section: &str, key: &str) -> bool {
        actions.iter().any(|x| {
            matches!(x, Action::State(p)
                if p.get(section).and_then(|s| s.get(key)) == Some(&Value::Null))
        })
    }

    #[test]
    fn everything_mirrored_is_read_on_connect() {
        let mut m = obs("");
        let s = loaded(&mut m);
        assert_eq!(s["scene_items"]["Wide"]["2"]["source"], "Mic");
        assert_eq!(s["scene_items"]["Wide"]["2"]["locked"], true);
        assert_eq!(s["scene_items"]["Wide"]["1"]["transform"]["positionX"], 0.0);
        assert_eq!(s["scene_items"]["Close"], json!({}));
        assert_eq!(
            s["filters"]["Mic"]["Gate"]["settings"]["open_threshold"],
            -26.0
        );
        assert_eq!(s["inputs"]["Mic"]["volume_mul"], 0.501);
        assert_eq!(s["inputs"]["Mic"]["tracks"], json!({"1": true, "2": false}));
        assert_eq!(
            s["inputs"]["Clip"]["media"]["state"],
            "OBS_MEDIA_STATE_PLAYING"
        );
        assert_eq!(s["inputs"]["Clip"]["media"]["duration_ms"], 60000);
        assert_eq!(s["inputs"]["Clip"]["settings"], json!({"looping": true}));
        assert_eq!(s["transitions"][1]["kind"], "fade_transition");
        assert_eq!(s["profile"], "Show");
        assert_eq!(s["scene_collections"], json!(["Main", "Spare"]));
        assert_eq!(s["outputs"]["simple_stream"]["width"], 1920);
        assert_eq!(s["recording"]["timecode"], "00:00:10.000");
        // The stream key is never published.
        assert_eq!(
            s["stream_service"],
            json!({"type": "rtmp_custom", "server": "rtmp://example.net/live"})
        );
        // Only the media source is asked for media status.
        assert_eq!(s["inputs"]["Mic"].get("media"), None);
    }

    #[test]
    fn removed_and_renamed_items_filters_and_inputs_are_deleted() {
        let mut m = obs("");
        loaded(&mut m);
        // An item removed: deleted at once, then the list read again.
        let a = event(
            &mut m,
            100,
            "SceneItemRemoved",
            json!({"sceneName": "Wide", "sceneItemId": 1}),
        );
        assert!(a.iter().any(|x| matches!(x, Action::State(p)
            if p["scene_items"]["Wide"].get("1") == Some(&Value::Null))));
        assert!(requests(&a).contains(&("GetSceneItemList".into(), json!({"sceneName": "Wide"}))));

        // A filter renamed: the source's list is read and replaced.
        let a = event(
            &mut m,
            110,
            "SourceFilterNameChanged",
            json!({"sourceName": "Mic", "oldFilterName": "Gate", "filterName": "Gate 2"}),
        );
        let a = settle(&mut m, 111, a, &|kind, _| {
            (kind == "GetSourceFilterList").then(|| {
                json!({"filters": [{"filterName": "Gate 2", "filterKind": "noise_gate_filter",
                    "filterEnabled": true, "filterIndex": 0, "filterSettings": {}}]})
            })
        });
        assert_eq!(m.mirror["filters"]["Mic"].get("Gate"), None);
        assert_eq!(m.mirror["filters"]["Mic"]["Gate 2"]["enabled"], true);
        assert!(a.iter().any(|x| matches!(x, Action::State(p)
            if p["filters"]["Mic"].get("Gate") == Some(&Value::Null))));

        // An input renamed: the old name is deleted, the new one read.
        let a = event(
            &mut m,
            120,
            "InputNameChanged",
            json!({"oldInputName": "Mic", "inputName": "Lectern", "inputUuid": "u-mic"}),
        );
        assert!(deletes(&a, "inputs", "Mic"));
        assert!(deletes(&a, "filters", "Mic"));
        assert!(requests(&a).contains(&("GetInputMute".into(), json!({"inputName": "Lectern"}))));

        // A scene removed: its items and transition override go.
        let a = event(
            &mut m,
            130,
            "SceneRemoved",
            json!({"sceneName": "Close", "isGroup": false}),
        );
        assert!(deletes(&a, "scene_items", "Close"));
        assert!(deletes(&a, "scene_transitions", "Close"));
    }

    #[test]
    fn a_reconnection_reads_everything_afresh() {
        let mut m = obs("");
        loaded(&mut m);
        let mut cx = Cx::new(200);
        m.ws(
            &mut cx,
            SOCKET,
            WsInput::Closed {
                code: None,
                reason: "gone".into(),
            },
        );
        let mut cx = Cx::new(1_500);
        m.timer(&mut cx, RETRY);
        feed(&mut m, 1_510, hello(None));
        let a = feed(
            &mut m,
            1_520,
            json!({"op": 2, "d": {"negotiatedRpcVersion": 1}}),
        );
        assert!(a.iter().any(|x| matches!(x, Action::State(p)
            if p.get("inputs") == Some(&Value::Null))));
        // The media source was removed while disconnected.
        settle(&mut m, 1_530, a, &|kind, data| {
            if kind == "GetInputList" {
                return Some(json!({"inputs": [{"inputName": "Mic",
                    "inputKind": "wasapi_input_capture",
                    "unversionedInputKind": "wasapi_input_capture"}]}));
            }
            small_obs(kind, data)
        });
        assert_eq!(m.mirror["inputs"].get("Clip"), None);
        assert_eq!(m.mirror["inputs"]["Mic"]["muted"], false);
    }

    #[test]
    fn an_input_list_without_an_input_deletes_it() {
        let mut m = obs("");
        loaded(&mut m);
        let mut cx = Cx::new(100);
        m.read(&mut cx, Read::InputList);
        let a = settle(&mut m, 110, cx.take(), &|kind, data| {
            if kind == "GetInputList" {
                return Some(json!({"inputs": [{"inputName": "Mic",
                    "inputKind": "wasapi_input_capture",
                    "unversionedInputKind": "wasapi_input_capture"}]}));
            }
            small_obs(kind, data)
        });
        assert!(deletes(&a, "inputs", "Clip"));
        assert!(deletes(&a, "filters", "Clip"));
    }

    #[test]
    fn volume_meters_are_coalesced_with_peaks_held() {
        let mut m = obs("");
        loaded(&mut m);
        let meters = |mag: f64, peak: f64| {
            json!({"inputs": [{"inputName": "Mic",
                "inputLevelsMul": [[mag, peak, peak], [0.0, 0.0, 0.0]]}]})
        };
        let a = event(&mut m, 100, "InputVolumeMeters", meters(0.5, 1.0));
        assert!(state(&a).as_object().unwrap().is_empty());
        assert!(a.contains(&Action::SetTimer {
            key: FLUSH,
            after: FLUSH_EVERY
        }));
        let a = event(&mut m, 150, "InputVolumeMeters", meters(0.1, 0.25));
        // Already armed: not armed again.
        assert!(!a.iter().any(|x| matches!(x, Action::SetTimer { .. })));
        let s = state(&timer(&mut m, 300, FLUSH));
        assert_eq!(
            s["inputs"]["Mic"]["levels"],
            json!([
                {"magnitude_db": -20.0, "peak_db": 0.0, "input_peak_db": 0.0},
                {"magnitude_db": -100.0, "peak_db": -100.0, "input_peak_db": -100.0},
            ])
        );
        // The same levels again publish nothing.
        event(&mut m, 350, "InputVolumeMeters", meters(0.1, 1.0));
        assert!(state(&timer(&mut m, 550, FLUSH))
            .as_object()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn transforms_are_coalesced() {
        let mut m = obs("");
        loaded(&mut m);
        for x in [1.0, 2.0, 3.0] {
            let a = event(
                &mut m,
                100,
                "SceneItemTransformChanged",
                json!({"sceneName": "Wide", "sceneItemId": 1,
                    "sceneItemTransform": {"positionX": x, "positionY": 0.0}}),
            );
            assert!(state(&a).as_object().unwrap().is_empty());
        }
        let s = state(&timer(&mut m, 300, FLUSH));
        assert_eq!(
            s,
            json!({"scene_items": {"Wide": {"1": {"transform": {"positionX": 3.0}}}}})
        );
    }

    #[test]
    fn playing_media_is_read_every_second() {
        let mut m = obs("");
        let a = identified(&mut m);
        let a = settle(&mut m, 30, a, &small_obs);
        assert!(a.contains(&Action::SetTimer {
            key: MEDIA,
            after: MEDIA_EVERY
        }));
        let a = timer(&mut m, 1_030, MEDIA);
        assert_eq!(
            requests(&a),
            [(
                "GetMediaInputStatus".to_string(),
                json!({"inputName": "Clip"})
            )]
        );
        // Stopped: no further reads.
        let a = settle(&mut m, 1_040, a, &|_, _| {
            Some(
                json!({"mediaState": "OBS_MEDIA_STATE_STOPPED", "mediaCursor": null,
                "mediaDuration": null}),
            )
        });
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: MEDIA, .. })));
        assert_eq!(
            m.mirror["inputs"]["Clip"]["media"],
            json!({"state": "OBS_MEDIA_STATE_STOPPED"})
        );
        // A media event reads the status again.
        let a = event(
            &mut m,
            2_000,
            "MediaInputPlaybackStarted",
            json!({"inputName": "Clip"}),
        );
        assert_eq!(
            requests(&a),
            [(
                "GetMediaInputStatus".to_string(),
                json!({"inputName": "Clip"})
            )]
        );
    }

    #[test]
    fn commands_obs_sends_no_event_for_are_read_back() {
        let mut m = obs("");
        loaded(&mut m);
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            7,
            "set_scene_item_blend_mode",
            &json!({"scene_uuid": "u-wide", "scene_item_id": 1,
                "scene_item_blend_mode": "OBS_BLEND_ADDITIVE"})
            .as_object()
            .unwrap()
            .clone(),
        );
        let a = cx.take();
        // Nothing is read until OBS accepts the command.
        assert_eq!(requests(&a).len(), 1);
        let a = settle(&mut m, 110, a, &|kind, _| {
            (kind == "SetSceneItemBlendMode").then_some(Value::Null)
        });
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Ack)
        }));
        assert!(requests(&a).contains(&("GetSceneItemList".into(), json!({"sceneName": "Wide"}))));
    }

    #[test]
    fn a_scene_collection_change_reloads_everything() {
        let mut m = obs("");
        loaded(&mut m);
        let a = event(
            &mut m,
            100,
            "CurrentSceneCollectionChanged",
            json!({"sceneCollectionName": "Spare"}),
        );
        assert_eq!(state(&a)["scene_collection"], "Spare");
        for section in ["scene_items", "filters", "inputs", "outputs"] {
            assert!(
                a.iter().any(|x| matches!(x, Action::State(p)
                    if p.get(section) == Some(&Value::Null))),
                "{section}"
            );
        }
        let kinds: Vec<String> = requests(&a).into_iter().map(|r| r.0).collect();
        assert!(kinds.contains(&"GetSceneList".to_string()));
        assert!(kinds.contains(&"GetInputList".to_string()));
    }

    #[test]
    fn output_events_keep_paths_and_reconnection() {
        let mut m = obs("");
        loaded(&mut m);
        let mut a = event(
            &mut m,
            100,
            "StreamStateChanged",
            json!({"outputActive": true, "outputState": "OBS_WEBSOCKET_OUTPUT_RECONNECTING"}),
        );
        a.extend(event(
            &mut m,
            101,
            "RecordFileChanged",
            json!({"newOutputPath": "/r/2.mkv"}),
        ));
        a.extend(event(
            &mut m,
            102,
            "ReplayBufferSaved",
            json!({"savedReplayPath": "/r/replay.mkv"}),
        ));
        a.extend(event(
            &mut m,
            103,
            "SceneTransitionStarted",
            json!({"transitionName": "Fade"}),
        ));
        let s = state(&a);
        assert_eq!(s["streaming"]["reconnecting"], true);
        assert_eq!(s["recording"]["file"], "/r/2.mkv");
        assert_eq!(s["replay_buffer"]["last_replay"], "/r/replay.mkv");
        assert_eq!(s["transition"]["active"], true);
        event(
            &mut m,
            104,
            "RecordStateChanged",
            json!({"outputActive": false, "outputState": "OBS_WEBSOCKET_OUTPUT_STOPPED",
                "outputPath": "/r/2.mkv"}),
        );
        assert_eq!(m.mirror["recording"].get("file"), None);
        assert_eq!(m.mirror["recording"]["last_file"], "/r/2.mkv");
    }

    #[test]
    fn replace_publishes_only_the_difference() {
        assert_eq!(diff(&json!({"a": 1}), &json!({"a": 1})), None);
        assert_eq!(
            diff(
                &json!({"a": 1, "b": {"c": 1, "d": 2}}),
                &json!({"b": {"c": 1}, "e": 3})
            ),
            Some(json!({"a": null, "b": {"d": null}, "e": 3}))
        );
        assert_eq!(db(1.0), 0.0);
        assert_eq!(db(0.0), FLOOR_DB);
        assert_eq!(db(1e-9), FLOOR_DB);
    }
}
