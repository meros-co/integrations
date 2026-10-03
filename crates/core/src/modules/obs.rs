//! OBS Studio over obs-websocket 5: JSON messages on a WebSocket.
//!
//! Protocol from obs-websocket's protocol documentation (docs/generated/
//! protocol.md): the server sends Hello (op 0), the client answers Identify
//! (op 1) with the authentication string when Hello carries a challenge, and
//! the server confirms with Identified (op 2). After that the client sends
//! Request (op 6) messages, matched to RequestResponse (op 7) by the client's
//! requestId, and receives Event (op 5) messages for the subscribed categories.
//!
//! Opened for commands only (`monitor` false), the module identifies with no
//! event categories and reads no state; a `GetVersion` request every 5 s is
//! its liveness check, since an open WebSocket alone does not show OBS is
//! answering.

use std::collections::HashMap;

use base64::Engine;
use serde_json::{json, Value};
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

/// EventSubscription flags: General, Scenes, Inputs, Transitions, Outputs, Ui.
/// Not the high-volume categories (volume meters and the like).
const EVENTS: u64 = (1 << 0) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 6) | (1 << 10);

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
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

const HELLO: Key = "hello";
const RETRY: Key = "retry";
const POLL: Key = "poll";
const REQUEST: Key = "request";

/// Why a request was sent.
#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    Command {
        id: CommandId,
    },
    Version,
    SceneList,
    StudioMode,
    Transition,
    StreamStatus,
    RecordStatus,
    VirtualCam,
    InputList,
    InputMute {
        input: String,
    },
    InputVolume {
        input: String,
    },
    /// Commands only: GetVersion in place of the output-status poll.
    Liveness,
}

impl Purpose {
    fn is_poll(&self) -> bool {
        matches!(
            self,
            Purpose::StreamStatus | Purpose::RecordStatus | Purpose::Liveness
        )
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
        cx.cancel_timer(HELLO);
        cx.cancel_timer(POLL);
        cx.cancel_timer(RETRY);
        self.phase = Phase::Idle;
        self.connected = false;
        self.strikes = 0;
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
        self.request(cx, Purpose::Version, "GetVersion", None);
        self.request(cx, Purpose::SceneList, "GetSceneList", None);
        self.request(cx, Purpose::StudioMode, "GetStudioModeEnabled", None);
        self.request(cx, Purpose::Transition, "GetCurrentSceneTransition", None);
        self.request(cx, Purpose::VirtualCam, "GetVirtualCamStatus", None);
        self.request(cx, Purpose::InputList, "GetInputList", None);
        self.poll(cx);
    }

    fn poll(&mut self, cx: &mut Cx) {
        if self.monitor {
            self.request(cx, Purpose::StreamStatus, "GetStreamStatus", None);
            self.request(cx, Purpose::RecordStatus, "GetRecordStatus", None);
        } else {
            // Commands only: the cheapest request OBS answers, for liveness.
            self.request(cx, Purpose::Liveness, "GetVersion", None);
        }
        cx.set_timer(POLL, POLL_EVERY);
    }

    fn query_input(&mut self, cx: &mut Cx, input: &str) {
        let data = json!({"inputName": input});
        self.request(
            cx,
            Purpose::InputMute {
                input: input.into(),
            },
            "GetInputMute",
            Some(data.clone()),
        );
        self.request(
            cx,
            Purpose::InputVolume {
                input: input.into(),
            },
            "GetInputVolume",
            Some(data),
        );
    }

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
            Purpose::Command { id } => {
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
                cx.complete(id, result);
            }
            // A failed internal query leaves that part of the state unknown;
            // it is not a reason to drop the connection.
            _ if !ok => {
                if let Purpose::InputMute { .. } | Purpose::InputVolume { .. } = pending.purpose {
                    // Inputs without audio have neither; expected.
                } else {
                    cx.log(
                        Level::Warning,
                        format!("{:?} failed: {}", pending.purpose, status),
                    );
                }
            }
            Purpose::Version | Purpose::Liveness => cx.state(json!({"device": {
                "obs_version": data["obsVersion"],
                "websocket_version": data["obsWebSocketVersion"],
                "platform": data["platformDescription"],
            }})),
            Purpose::SceneList => cx.state(json!({
                "scenes": scene_names(&data),
                "program_scene": data["currentProgramSceneName"],
                // null when studio mode is off, which removes it.
                "preview_scene": data["currentPreviewSceneName"],
            })),
            Purpose::StudioMode => cx.state(json!({"studio_mode": data["studioModeEnabled"]})),
            Purpose::Transition => cx.state(json!({"transition": {
                "name": data["transitionName"],
                "duration_ms": data["transitionDuration"],
            }})),
            Purpose::StreamStatus => cx.state(json!({"streaming": {
                "active": data["outputActive"],
                "reconnecting": data["outputReconnecting"],
                "duration_ms": data["outputDuration"],
                "congestion": data["outputCongestion"],
                "bytes": data["outputBytes"],
                "skipped_frames": data["outputSkippedFrames"],
                "total_frames": data["outputTotalFrames"],
            }})),
            Purpose::RecordStatus => cx.state(json!({"recording": {
                "active": data["outputActive"],
                "paused": data["outputPaused"],
                "duration_ms": data["outputDuration"],
                "bytes": data["outputBytes"],
            }})),
            Purpose::VirtualCam => {
                cx.state(json!({"virtualcam": {"active": data["outputActive"]}}))
            }
            Purpose::InputList => {
                let names: Vec<String> = data["inputs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|i| i["inputName"].as_str().map(str::to_string))
                    .collect();
                for name in names {
                    self.query_input(cx, &name);
                }
            }
            Purpose::InputMute { input } => {
                cx.state(json!({"inputs": {input: {"muted": data["inputMuted"]}}}))
            }
            Purpose::InputVolume { input } => {
                cx.state(json!({"inputs": {input: {"volume_db": data["inputVolumeDb"]}}}))
            }
        }
    }

    fn event(&mut self, cx: &mut Cx, d: &Value) {
        let data = &d["eventData"];
        match d.get("eventType").and_then(Value::as_str).unwrap_or("") {
            "CurrentProgramSceneChanged" => cx.state(json!({"program_scene": data["sceneName"]})),
            "CurrentPreviewSceneChanged" => cx.state(json!({"preview_scene": data["sceneName"]})),
            "StudioModeStateChanged" => {
                let enabled = data["studioModeEnabled"].as_bool() == Some(true);
                let mut patch = json!({"studio_mode": enabled});
                if !enabled {
                    patch["preview_scene"] = Value::Null;
                }
                cx.state(patch);
                if enabled {
                    // Studio mode opens with a preview scene; the event does
                    // not say which.
                    self.request(cx, Purpose::SceneList, "GetSceneList", None);
                }
            }
            "SceneListChanged" => cx.state(json!({"scenes": scene_names(data)})),
            "SceneNameChanged" | "SceneCreated" | "SceneRemoved" => {
                self.request(cx, Purpose::SceneList, "GetSceneList", None);
            }
            "CurrentSceneTransitionChanged" => {
                cx.state(json!({"transition": {"name": data["transitionName"]}}));
                self.request(cx, Purpose::Transition, "GetCurrentSceneTransition", None);
            }
            "CurrentSceneTransitionDurationChanged" => {
                cx.state(json!({"transition": {"duration_ms": data["transitionDuration"]}}))
            }
            "StreamStateChanged" => {
                cx.state(json!({"streaming": {"active": data["outputActive"]}}))
            }
            "RecordStateChanged" => cx.state(json!({"recording": {
                "active": data["outputActive"],
                "paused": data["outputState"] == "OBS_WEBSOCKET_OUTPUT_PAUSED",
            }})),
            "VirtualcamStateChanged" => {
                cx.state(json!({"virtualcam": {"active": data["outputActive"]}}))
            }
            "InputMuteStateChanged" => {
                if let Some(input) = data["inputName"].as_str() {
                    cx.state(json!({"inputs": {input: {"muted": data["inputMuted"]}}}));
                }
            }
            "InputVolumeChanged" => {
                if let Some(input) = data["inputName"].as_str() {
                    cx.state(json!({"inputs": {input: {"volume_db": data["inputVolumeDb"]}}}));
                }
            }
            "InputCreated" => {
                if let Some(input) = data["inputName"].as_str() {
                    self.query_input(cx, input);
                }
            }
            "InputRemoved" => {
                if let Some(input) = data["inputName"].as_str() {
                    cx.state(json!({"inputs": {input: null}}));
                }
            }
            "InputNameChanged" => {
                if let (Some(old), Some(new)) =
                    (data["oldInputName"].as_str(), data["inputName"].as_str())
                {
                    cx.state(json!({"inputs": {old: null}}));
                    self.query_input(cx, new);
                }
            }
            // OBS is shutting down; the close follows.
            "ExitStarted" => cx.log(Level::Info, "OBS is exiting"),
            _ => {}
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
        self.request(cx, Purpose::Command { id }, &request_type, data);
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
}
