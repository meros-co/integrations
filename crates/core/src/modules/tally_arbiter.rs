//! Tally Arbiter over Socket.IO: every read and change is an event on its web
//! server's port (4455), since it has had no REST API since 3.0.0.
//!
//! Socket.IO 4 runs over Engine.IO 4 on a WebSocket at `/socket.io/`: the
//! server's open packet (`0{...}`) is answered with a connect to the main
//! namespace (`40`), the server pings (`2`) and the client answers (`3`), and
//! an event is `42["name", args...]`. Tally Arbiter answers a read by emitting
//! the event of the same name back (`devices` with the device list), so a
//! reply is tied to its request by event name.
//!
//! With a username, the module signs in (`login`, answered by `login_response`
//! with an access token, sent back with `access_token`) and joins the
//! settings room (admin), or failing that the room Tally Arbiter's Companion
//! module joins (producer): both push every list and every device's busses
//! as they change, and manage events are answered in the settings room only.
//! Without signing in, or with neither role, every device's busses are read
//! every `poll_ms` and the lists every 30 s. With `device_id` set, the module
//! registers as a listener client of that device (`listenerclient_connect`),
//! so the device's busses are pushed, and takes `flash`, `messaging_client`
//! and `reassign` as Tally Arbiter's own tally page does.
//!
//! Tally per device joins two lists: `device_states` gives the busses each
//! device is active on (a bus with any source listed), and `bus_options`
//! each bus's type, so a device is program or preview while it is on a bus
//! of that type.
//!
//! A version request every 5 s is the liveness check (two unanswered drop the
//! connection, which is reopened after 1 s doubling to 30 s). Opened for
//! commands only (`monitor` false), the module signs in and joins the
//! settings room (manage events are answered there), but registers no
//! listener client, reads nothing on connecting and polls nothing; what the
//! room pushes is still applied.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    WsInput, WsRequest,
};

const PORT: u16 = 4455;
const SOCKET: Key = "tally-arbiter";

const HANDSHAKE: Key = "handshake";
const RETRY: Key = "retry";
const REQUEST: Key = "request";
const LIVENESS: Key = "liveness";
const STATES: Key = "states";
const LISTS: Key = "lists";

const HANDSHAKE_TIMEOUT: Millis = 5_000;
const REQUEST_TIMEOUT: Millis = 5_000;
const LIVENESS_EVERY: Millis = 5_000;
/// Consecutive unanswered liveness requests before the connection is dropped.
const STRIKES: u32 = 2;
/// Without a room that pushes them, the lists are read this often.
const LISTS_EVERY: Millis = 30_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

/// The lists read on connecting and, without a pushing room, every 30 s.
const LIST_READS: &[&str] = &[
    "bus_options",
    "devices",
    "sources",
    "device_sources",
    "device_actions",
    "listener_clients",
    "tsl_clients",
    "cloud_destinations",
    "cloud_keys",
    "cloud_clients",
];

/// Read once on connecting.
const SERVER_READS: &[&str] = &["version", "uiVersion", "externalAddress", "interfaces"];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    /// The WebSocket is opening, or open and waiting for Engine.IO's open
    /// packet.
    Open,
    /// The namespace connect is sent.
    Connect,
    /// Signing in.
    Login,
    Ready,
}

/// What the session is signed in for.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Access {
    None,
    /// Signed in, in no room that pushes.
    SignedIn,
    /// In the Companion module's room (producer role).
    Producer,
    /// In the settings room (admin).
    Settings,
}

impl Access {
    fn name(self) -> &'static str {
        match self {
            Access::None => "none",
            Access::SignedIn => "signed_in",
            Access::Producer => "producer",
            Access::Settings => "settings",
        }
    }

    fn pushed(self) -> bool {
        matches!(self, Access::Producer | Access::Settings)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    Liveness,
    /// A room join: success is its first answer, failure an error event.
    Join(Access),
    /// A command answered with the event's first argument.
    Read(CommandId),
    /// `get_initial_data`: the sixteen arguments of initialdata.
    InitialData(CommandId),
    /// A command whose echo confirms it.
    Echo(CommandId),
    ErrorReport(CommandId),
    ChangePassword {
        id: CommandId,
        new_password: String,
    },
    Manage {
        id: CommandId,
        then: &'static [&'static str],
        /// duplicate_device: the reply's deviceId is the value.
        value: bool,
        /// delete_user: Tally Arbiter reports an error whatever happened.
        unreliable: bool,
    },
}

impl Purpose {
    fn command(&self) -> Option<CommandId> {
        match self {
            Purpose::Read(id)
            | Purpose::InitialData(id)
            | Purpose::Echo(id)
            | Purpose::ErrorReport(id)
            | Purpose::ChangePassword { id, .. }
            | Purpose::Manage { id, .. } => Some(*id),
            Purpose::Liveness | Purpose::Join(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
struct Pending {
    event: &'static str,
    purpose: Purpose,
    sent_at: Millis,
    deadline: Millis,
}

pub(crate) struct TallyArbiter {
    url: String,
    source: SocketAddr,
    username: String,
    password: String,
    device_setting: String,
    register_listener: bool,
    listener_type: String,
    chat: bool,
    poll_ms: Millis,
    monitor: bool,

    phase: Phase,
    retry_after: Millis,
    strikes: u32,
    /// Set when Tally Arbiter refuses the credential. Terminal.
    refused: Option<String>,
    /// This connection's socket id, from the namespace connect.
    sid: String,
    access: Access,
    relogin: bool,
    pending: VecDeque<Pending>,

    /// The device the listener client follows after a reassignment, which
    /// replaces the setting for later connections.
    reassigned: Option<String>,
    registered: bool,

    /// Tally Arbiter's own lists, as last received, for building state and
    /// filling edits; None until read.
    devices: Option<Vec<Value>>,
    buses: Option<Vec<Value>>,
    sources: Option<Vec<Value>>,
    device_sources: Option<Vec<Value>>,
    device_actions: Option<Vec<Value>>,
    tsl_clients: Option<Vec<Value>>,
    cloud_destinations: Option<Vec<Value>>,
    cloud_clients: Option<Vec<Value>>,
    listener_clients: Option<Vec<Value>>,
    users: Option<Vec<Value>>,
    /// Device id to bus id to active.
    device_buses: HashMap<String, Map<String, Value>>,
    /// Source id to address to bus ids (tally_data).
    source_tally: HashMap<String, Map<String, Value>>,

    flashes: u64,
    messages: u64,
    chats: u64,
    /// The state as published, to replace lists with deletions.
    mirror: Value,
}

fn str_param<'a>(params: &'a Params, name: &str) -> Option<&'a str> {
    params.get(name).and_then(Value::as_str)
}

/// Socket.IO event packet `42["name", args...]`.
fn event_packet(name: &str, args: &[Value]) -> String {
    let mut array = vec![Value::String(name.into())];
    array.extend(args.iter().cloned());
    format!("42{}", Value::Array(array))
}

/// `value` nested under `path`.
fn nest(path: &[&str], value: Value) -> Value {
    path.iter().rev().fold(value, |inner, key| {
        let mut map = Map::new();
        map.insert((*key).to_string(), inner);
        Value::Object(map)
    })
}

/// `value` without null members, recursively.
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

fn array(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

fn id_of(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// A value as text: Tally Arbiter keeps some numbers as strings and some
/// strings as numbers (TSL addresses, addresses).
fn text(v: &Value) -> Value {
    match v {
        Value::Number(n) => Value::String(n.to_string()),
        Value::String(_) => v.clone(),
        _ => Value::Null,
    }
}

/// A value as an integer, from a number or a string of digits.
fn int(v: &Value) -> Value {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .map_or(Value::Null, Value::from),
        Value::String(s) => s.trim().parse::<i64>().map_or(Value::Null, Value::from),
        _ => Value::Null,
    }
}

#[derive(Clone, Copy)]
enum Conv {
    Same,
    Text,
    Int,
    Bool,
}

/// The fields of a Tally Arbiter object under the state's names.
fn pick(obj: &Value, fields: &[(&str, &str, Conv)]) -> Value {
    let mut out = Map::new();
    for (from, to, conv) in fields {
        let Some(v) = obj.get(*from) else { continue };
        let v = match conv {
            Conv::Same => v.clone(),
            Conv::Text => text(v),
            Conv::Int => int(v),
            Conv::Bool => match v {
                Value::Bool(_) => v.clone(),
                _ => Value::Null,
            },
        };
        if !v.is_null() {
            out.insert((*to).to_string(), v);
        }
    }
    Value::Object(out)
}

/// A list keyed by each element's `key`.
fn keyed(list: &[Value], key: &str, fields: &[(&str, &str, Conv)]) -> Value {
    let mut out = Map::new();
    for item in list {
        if let Some(id) = id_of(item, key) {
            out.insert(id, pick(item, fields));
        }
    }
    Value::Object(out)
}

const SOURCE_FIELDS: &[(&str, &str, Conv)] = &[
    ("name", "name", Conv::Text),
    ("sourceTypeId", "type", Conv::Text),
    ("enabled", "enabled", Conv::Bool),
    ("connected", "connected", Conv::Bool),
    ("reconnect", "reconnect", Conv::Bool),
    ("reconnect_interval", "reconnect_interval", Conv::Int),
    ("max_reconnects", "max_reconnects", Conv::Int),
    ("data", "data", Conv::Same),
    ("cloudConnection", "cloud", Conv::Bool),
];
const DEVICE_FIELDS: &[(&str, &str, Conv)] = &[
    ("name", "name", Conv::Text),
    ("description", "description", Conv::Text),
    ("enabled", "enabled", Conv::Bool),
    ("tslAddress", "tsl_address", Conv::Text),
    ("linkedBusses", "linked_buses", Conv::Same),
    ("cameraIP", "camera_ip", Conv::Text),
    ("cameraModel", "camera_model", Conv::Text),
    (
        "cameraPreviewDisabled",
        "camera_preview_disabled",
        Conv::Bool,
    ),
    ("cloudConnection", "cloud", Conv::Bool),
];
const BUS_FIELDS: &[(&str, &str, Conv)] = &[
    ("label", "label", Conv::Text),
    ("type", "type", Conv::Text),
    ("color", "color", Conv::Text),
    ("priority", "priority", Conv::Int),
    ("visible", "visible", Conv::Bool),
];
const DEVICE_SOURCE_FIELDS: &[(&str, &str, Conv)] = &[
    ("deviceId", "device_id", Conv::Text),
    ("sourceId", "source_id", Conv::Text),
    ("address", "address", Conv::Text),
    ("bus", "bus", Conv::Text),
    ("rename", "rename", Conv::Bool),
    ("reconnect_interval", "reconnect_interval", Conv::Int),
    ("max_reconnects", "max_reconnects", Conv::Int),
    ("cloudConnection", "cloud", Conv::Bool),
];
const DEVICE_ACTION_FIELDS: &[(&str, &str, Conv)] = &[
    ("deviceId", "device_id", Conv::Text),
    ("busId", "bus_id", Conv::Text),
    ("active", "active", Conv::Bool),
    ("outputTypeId", "output_type_id", Conv::Text),
    ("data", "data", Conv::Same),
];
const LISTENER_FIELDS: &[(&str, &str, Conv)] = &[
    ("deviceId", "device_id", Conv::Text),
    ("internalId", "internal_id", Conv::Text),
    ("listenerType", "listener_type", Conv::Text),
    ("ipAddress", "ip_address", Conv::Text),
    ("datetime_connected", "connected_at", Conv::Int),
    ("inactive", "inactive", Conv::Bool),
    ("datetime_inactive", "inactive_at", Conv::Int),
    ("canBeFlashed", "can_be_flashed", Conv::Bool),
    ("canBeReassigned", "can_be_reassigned", Conv::Bool),
    ("supportsChat", "supports_chat", Conv::Bool),
    ("cloudConnection", "cloud", Conv::Bool),
];
const TSL_FIELDS: &[(&str, &str, Conv)] = &[
    ("ip", "ip", Conv::Text),
    ("port", "port", Conv::Int),
    ("transport", "transport", Conv::Text),
    ("protocol", "protocol", Conv::Text),
    ("protocolOptions", "protocol_options", Conv::Same),
    ("connected", "connected", Conv::Bool),
    ("error", "error", Conv::Bool),
];
const CLOUD_DESTINATION_FIELDS: &[(&str, &str, Conv)] = &[
    ("host", "host", Conv::Text),
    ("port", "port", Conv::Int),
    ("key", "key", Conv::Text),
    ("status", "status", Conv::Text),
    ("connected", "connected", Conv::Bool),
];
const CLOUD_CLIENT_FIELDS: &[(&str, &str, Conv)] = &[
    ("ipAddress", "ip_address", Conv::Text),
    ("key", "key", Conv::Text),
    ("datetimeConnected", "connected_at", Conv::Int),
    ("inactive", "inactive", Conv::Bool),
];

/// A command's edit or add parameter, the Tally Arbiter field it fills.
type Field = (&'static str, &'static str);

const SOURCE_EDIT: &[Field] = &[
    ("name", "name"),
    ("enabled", "enabled"),
    ("reconnect", "reconnect"),
    ("reconnect_interval", "reconnect_interval"),
    ("max_reconnects", "max_reconnects"),
    ("data", "data"),
];
const DEVICE_EDIT: &[Field] = &[
    ("name", "name"),
    ("description", "description"),
    ("enabled", "enabled"),
    ("tsl_address", "tslAddress"),
    ("linked_buses", "linkedBusses"),
    ("camera_ip", "cameraIP"),
    ("camera_model", "cameraModel"),
    ("camera_preview_disabled", "cameraPreviewDisabled"),
];
const DEVICE_SOURCE_EDIT: &[Field] = &[
    ("source_id", "sourceId"),
    ("address", "address"),
    ("bus", "bus"),
    ("rename", "rename"),
    ("reconnect_interval", "reconnect_interval"),
    ("max_reconnects", "max_reconnects"),
];
const DEVICE_ACTION_EDIT: &[Field] = &[
    ("bus_id", "busId"),
    ("active", "active"),
    ("output_type_id", "outputTypeId"),
    ("data", "data"),
];
const TSL_EDIT: &[Field] = &[
    ("ip", "ip"),
    ("port", "port"),
    ("transport", "transport"),
    ("protocol", "protocol"),
    ("protocol_options", "protocolOptions"),
];
const BUS_EDIT: &[Field] = &[
    ("label", "label"),
    ("type", "type"),
    ("color", "color"),
    ("visible", "visible"),
];
const CLOUD_DESTINATION_EDIT: &[Field] = &[("host", "host"), ("port", "port"), ("key", "key")];

/// The object `fields` make from `params`.
fn from_params(params: &Params, fields: &[Field]) -> Map<String, Value> {
    fields
        .iter()
        .filter_map(|(p, f)| params.get(*p).map(|v| ((*f).to_string(), v.clone())))
        .collect()
}

/// A read command: its event and the event it is answered with.
fn read_event(name: &str) -> Option<&'static str> {
    Some(match name {
        "get_version" => "version",
        "get_ui_version" => "uiVersion",
        "get_external_address" => "externalAddress",
        "get_interfaces" => "interfaces",
        "get_sources" => "sources",
        "get_devices" => "devices",
        "get_device_sources" => "device_sources",
        "get_device_actions" => "device_actions",
        "get_bus_options" => "bus_options",
        "get_device_states" => "device_states",
        "get_listener_clients" => "listener_clients",
        "get_tsl_clients" => "tsl_clients",
        "get_cloud_destinations" => "cloud_destinations",
        "get_cloud_keys" => "cloud_keys",
        "get_cloud_clients" => "cloud_clients",
        "get_unread_error_reports" => "unread_error_reports",
        "get_users" => "users",
        "get_default_password_users" => "default_password_users",
        _ => return None,
    })
}

/// What each read command emits when it differs from what answers it.
fn read_request(name: &str, reply: &'static str) -> &'static str {
    match name {
        "get_unread_error_reports" => "get_unread_error_reports",
        _ => reply,
    }
}

const THEN_SOURCE: &[&str] = &["sources", "devices", "device_sources"];
const THEN_DEVICE: &[&str] = &[
    "devices",
    "device_sources",
    "device_actions",
    "device_states",
    "listener_clients",
];
const THEN_DEVICE_SOURCE: &[&str] = &["device_sources", "device_states"];
const THEN_DEVICE_ACTION: &[&str] = &["device_actions"];
const THEN_TSL: &[&str] = &["tsl_clients"];
const THEN_BUS: &[&str] = &["bus_options", "device_states"];
const THEN_CLOUD_DESTINATION: &[&str] = &["cloud_destinations"];
const THEN_CLOUD_KEY: &[&str] = &["cloud_keys", "cloud_clients"];
const THEN_CLOUD_CLIENT: &[&str] = &["cloud_clients", "sources", "devices"];
const THEN_USER: &[&str] = &["users", "default_password_users"];

/// How a command is sent and answered.
enum Plan {
    /// Emit, then complete when `reply` arrives.
    Wait {
        event: &'static str,
        args: Vec<Value>,
        reply: &'static str,
        purpose: Purpose,
    },
    /// Emit and complete unverified: Tally Arbiter answers nothing.
    Unverified {
        event: &'static str,
        args: Vec<Value>,
        then: &'static [&'static str],
    },
}

impl TallyArbiter {
    pub(crate) fn new(ctx: OpenContext) -> TallyArbiter {
        let host = match ctx.host {
            std::net::IpAddr::V6(v6) => format!("[{v6}]"),
            v4 => v4.to_string(),
        };
        let port = ctx.port.unwrap_or(PORT);
        let s = |name: &str| str_param(&ctx.settings, name).unwrap_or("").to_string();
        let b = |name: &str, default: bool| {
            ctx.settings
                .get(name)
                .and_then(Value::as_bool)
                .unwrap_or(default)
        };
        TallyArbiter {
            url: format!("ws://{host}:{port}/socket.io/?EIO=4&transport=websocket"),
            source: SocketAddr::new(ctx.host, port),
            username: s("username"),
            password: s("password"),
            device_setting: s("device_id"),
            register_listener: b("register_listener", true),
            listener_type: match s("listener_type") {
                t if t.is_empty() => "meros".into(),
                t => t,
            },
            chat: b("chat", true),
            poll_ms: ctx
                .settings
                .get("poll_ms")
                .and_then(Value::as_u64)
                .unwrap_or(1_000),
            monitor: ctx.monitor,
            phase: Phase::Idle,
            retry_after: RETRY_MIN,
            strikes: 0,
            refused: None,
            sid: String::new(),
            access: Access::None,
            relogin: false,
            pending: VecDeque::new(),
            reassigned: None,
            registered: false,
            devices: None,
            buses: None,
            sources: None,
            device_sources: None,
            device_actions: None,
            tsl_clients: None,
            cloud_destinations: None,
            cloud_clients: None,
            listener_clients: None,
            users: None,
            device_buses: HashMap::new(),
            source_tally: HashMap::new(),
            flashes: 0,
            messages: 0,
            chats: 0,
            mirror: json!({}),
        }
    }

    // ---- the connection ----

    fn open(&mut self, cx: &mut Cx) {
        self.phase = Phase::Open;
        cx.ws_open(
            SOCKET,
            WsRequest {
                url: self.url.clone(),
                headers: Vec::new(),
                accept_invalid_certs: false,
            },
        );
        cx.set_timer(HANDSHAKE, HANDSHAKE_TIMEOUT);
    }

    fn emit(&self, cx: &mut Cx, event: &str, args: &[Value]) {
        cx.ws_send(SOCKET, event_packet(event, args));
    }

    fn wait(&mut self, cx: &mut Cx, event: &'static str, purpose: Purpose) {
        let now = cx.now();
        self.pending.push_back(Pending {
            event,
            purpose,
            sent_at: now,
            deadline: now + REQUEST_TIMEOUT,
        });
        self.arm_request_timer(cx);
    }

    fn arm_request_timer(&self, cx: &mut Cx) {
        match self.pending.iter().map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(REQUEST, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REQUEST),
        }
    }

    fn fail_pending(&mut self, cx: &mut Cx, error: CommandError) {
        for p in self.pending.drain(..) {
            if let Some(id) = p.purpose.command() {
                cx.complete(id, Err(error.clone()));
            }
        }
        cx.cancel_timer(REQUEST);
    }

    fn stop_everything(&mut self, cx: &mut Cx) {
        cx.ws_close(SOCKET);
        for key in [HANDSHAKE, LIVENESS, STATES, LISTS] {
            cx.cancel_timer(key);
        }
        self.phase = Phase::Idle;
        self.strikes = 0;
        self.sid.clear();
        self.relogin = false;
        self.registered = false;
        self.set_access(cx, Access::None);
        self.set(cx, &["listener", "registered"], json!(false));
    }

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

    fn refuse(&mut self, cx: &mut Cx, reason: &str) {
        self.fail_pending(
            cx,
            CommandError::Auth {
                message: reason.into(),
            },
        );
        self.stop_everything(cx);
        cx.cancel_timer(RETRY);
        self.refused = Some(reason.to_string());
        cx.log(
            Level::Warning,
            format!("{reason}; no further attempts until the device is opened again with corrected settings"),
        );
        cx.connection(Connection::Unauthorized {
            reason: reason.into(),
        });
    }

    /// Close and open again at once, keeping the state.
    fn restart(&mut self, cx: &mut Cx) {
        self.fail_pending(
            cx,
            CommandError::Transport {
                message: "reconnecting".into(),
            },
        );
        self.stop_everything(cx);
        self.retry_after = RETRY_MIN;
        cx.connection(Connection::Connecting);
        self.open(cx);
    }

    fn text(&mut self, cx: &mut Cx, text: &str) {
        let mut chars = text.chars();
        let Some(kind) = chars.next() else { return };
        let rest = chars.as_str();
        match kind {
            // Engine.IO open: join the main namespace.
            '0' => {
                if self.phase == Phase::Open {
                    self.phase = Phase::Connect;
                    cx.ws_send(SOCKET, "40");
                }
            }
            // Engine.IO ping, answered with a pong carrying the same payload.
            '2' => cx.ws_send(SOCKET, format!("3{rest}")),
            '1' => self.lost(cx, "Tally Arbiter closed the connection".into()),
            '4' => self.packet(cx, rest),
            _ => {}
        }
    }

    /// A Socket.IO packet (after Engine.IO's `4`).
    fn packet(&mut self, cx: &mut Cx, p: &str) {
        let mut chars = p.chars();
        let Some(kind) = chars.next() else { return };
        let mut rest = chars.as_str();
        // Only the main namespace is used; another's packets are not ours.
        if rest.starts_with('/') {
            return;
        }
        match kind {
            '0' => {
                let sid = serde_json::from_str::<Value>(rest)
                    .ok()
                    .and_then(|v| v.get("sid").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_default();
                self.namespace_connected(cx, sid);
            }
            '1' => self.lost(cx, "Tally Arbiter disconnected this client".into()),
            '2' => {
                // An acknowledgement id may stand before the array.
                rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
                let Ok(Value::Array(items)) = serde_json::from_str::<Value>(rest) else {
                    return;
                };
                let Some(name) = items.first().and_then(Value::as_str) else {
                    return;
                };
                let name = name.to_string();
                self.event(cx, &name, &items[1..]);
            }
            '4' => {
                let message = serde_json::from_str::<Value>(rest)
                    .ok()
                    .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_else(|| "refused".into());
                self.lost(
                    cx,
                    format!("Tally Arbiter refused the connection: {message}"),
                );
            }
            _ => {}
        }
    }

    fn namespace_connected(&mut self, cx: &mut Cx, sid: String) {
        if self.phase != Phase::Connect {
            return;
        }
        self.sid = sid;
        if self.username.is_empty() {
            self.ready(cx);
        } else {
            self.phase = Phase::Login;
            let (u, p) = (self.username.clone(), self.password.clone());
            self.emit(cx, "login", &[json!(u), json!(p)]);
            cx.set_timer(HANDSHAKE, HANDSHAKE_TIMEOUT);
        }
    }

    fn login_response(&mut self, cx: &mut Cx, r: &Value) {
        let ok = r.get("loginOk").and_then(Value::as_bool).unwrap_or(false);
        if !ok {
            let message = r
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("wrong username or password");
            self.refuse(cx, &format!("Tally Arbiter refused the sign-in: {message}"));
            return;
        }
        let token = r
            .get("accessToken")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        self.emit(cx, "access_token", &[json!(token)]);
        if self.phase == Phase::Login {
            self.set_access(cx, Access::SignedIn);
            self.ready(cx);
        } else if self.relogin {
            self.relogin = false;
            // A fresh token: join again, in case the join is what failed.
            if !self.access.pushed() {
                self.join(cx, Access::Settings);
            }
        }
    }

    fn ready(&mut self, cx: &mut Cx) {
        cx.cancel_timer(HANDSHAKE);
        self.phase = Phase::Ready;
        self.retry_after = RETRY_MIN;
        cx.connection(Connection::Connected);
        cx.alive();
        cx.set_timer(LIVENESS, LIVENESS_EVERY);
        if self.access == Access::SignedIn {
            // Reads follow the join, so that its first answer shows it worked.
            self.join(cx, Access::Settings);
        } else {
            self.after_join(cx);
        }
    }

    fn join(&mut self, cx: &mut Cx, room: Access) {
        let (event, answer) = match room {
            Access::Settings => ("settings", "initialdata"),
            _ => ("companion", "device_sources"),
        };
        self.emit(cx, event, &[]);
        self.wait(cx, answer, Purpose::Join(room));
    }

    /// A join was refused (an error event, or no answer).
    fn join_refused(&mut self, cx: &mut Cx, room: Access) {
        if room == Access::Settings && self.monitor {
            self.join(cx, Access::Producer);
        } else {
            self.after_join(cx);
        }
    }

    fn joined(&mut self, cx: &mut Cx, room: Access) {
        self.set_access(cx, room);
        self.set(cx, &["session", "must_change_password"], json!(false));
        self.after_join(cx);
    }

    /// Signed in (or not) and in whatever room the account allows: read
    /// everything, and poll what nothing pushes.
    fn after_join(&mut self, cx: &mut Cx) {
        if !self.monitor {
            return;
        }
        for event in SERVER_READS.iter().chain(LIST_READS) {
            self.emit(cx, event, &[]);
        }
        self.emit(cx, "device_states", &[]);
        self.emit(cx, "get_error_reports", &[]);
        if self.access == Access::Settings {
            self.emit(cx, "users", &[]);
            self.emit(cx, "default_password_users", &[]);
        }
        if self.access.pushed() {
            cx.cancel_timer(STATES);
            cx.cancel_timer(LISTS);
        } else {
            if self.poll_ms > 0 {
                cx.set_timer(STATES, self.poll_ms);
            }
            cx.set_timer(LISTS, LISTS_EVERY);
        }
    }

    fn set_access(&mut self, cx: &mut Cx, access: Access) {
        self.access = access;
        self.set(cx, &["session", "access"], json!(access.name()));
    }

    /// The device this connection follows as a listener client.
    fn wanted_device(&self) -> Option<String> {
        self.reassigned
            .clone()
            .or_else(|| (!self.device_setting.is_empty()).then(|| self.device_setting.clone()))
    }

    /// Register as a listener client once the device list names the device.
    fn maybe_register(&mut self, cx: &mut Cx) {
        if !self.monitor || !self.register_listener || self.registered || self.phase != Phase::Ready
        {
            return;
        }
        let Some(wanted) = self.wanted_device() else {
            return;
        };
        let devices = self.devices.clone().unwrap_or_default();
        let found = devices
            .iter()
            .find(|d| id_of(d, "id").as_deref() == Some(wanted.as_str()))
            .or_else(|| {
                devices
                    .iter()
                    .find(|d| d.get("name").and_then(Value::as_str) == Some(wanted.as_str()))
            })
            .and_then(|d| id_of(d, "id"));
        let Some(device_id) = found else {
            return;
        };
        self.registered = true;
        let connect = json!({
            "deviceId": device_id,
            "listenerType": self.listener_type,
            "canBeReassigned": true,
            "canBeFlashed": true,
            "supportsChat": self.chat,
        });
        self.emit(cx, "listenerclient_connect", &[connect]);
        self.set(cx, &["listener", "device_id"], json!(device_id));
        self.set(cx, &["listener", "registered"], json!(true));
    }

    // ---- the mirror ----

    fn publish(&mut self, cx: &mut Cx, patch: Value) {
        crate::session::merge_patch(&mut self.mirror, &patch);
        cx.state(patch);
    }

    fn at(&self, path: &[&str]) -> Option<&Value> {
        path.iter().try_fold(&self.mirror, |v, key| v.get(*key))
    }

    /// Replace the value at `path`: members it no longer has are deleted.
    fn replace(&mut self, cx: &mut Cx, path: &[&str], new: Value) {
        let new = strip_nulls(new);
        let patch = match self.at(path) {
            Some(old) => diff(old, &new),
            None if new.is_null() => None,
            None => Some(new),
        };
        if let Some(patch) = patch {
            self.publish(cx, nest(path, patch));
        }
    }

    fn set(&mut self, cx: &mut Cx, path: &[&str], value: Value) {
        self.replace(cx, path, value);
    }

    fn publish_devices(&mut self, cx: &mut Cx) {
        let types: HashMap<String, String> = self
            .buses
            .iter()
            .flatten()
            .filter_map(|b| Some((id_of(b, "id")?, b.get("type")?.as_str()?.to_string())))
            .collect();
        let mut out = Map::new();
        for (i, d) in self.devices.iter().flatten().enumerate() {
            let Some(id) = id_of(d, "id") else { continue };
            let mut obj = pick(d, DEVICE_FIELDS);
            obj["index"] = json!(i);
            out.insert(id, obj);
        }
        for (device, buses) in &self.device_buses {
            let entry = out.entry(device.clone()).or_insert_with(|| json!({}));
            entry["buses"] = Value::Object(buses.clone());
            if self.buses.is_some() {
                let on = |kind: &str| {
                    buses.iter().any(|(bus, active)| {
                        active.as_bool() == Some(true)
                            && types.get(bus).map(String::as_str) == Some(kind)
                    })
                };
                entry["program"] = json!(on("program"));
                entry["preview"] = json!(on("preview"));
            }
        }
        self.replace(cx, &["devices"], Value::Object(out));
    }

    fn publish_sources(&mut self, cx: &mut Cx) {
        let mut out = Map::new();
        for s in self.sources.iter().flatten() {
            let Some(id) = id_of(s, "id") else { continue };
            out.insert(id, pick(s, SOURCE_FIELDS));
        }
        for (source, tally) in &self.source_tally {
            if let Some(entry) = out.get_mut(source) {
                entry["tally"] = Value::Object(tally.clone());
            }
        }
        self.replace(cx, &["sources"], Value::Object(out));
    }

    fn device_states(&mut self, cx: &mut Cx, list: &[Value]) {
        let mut seen: HashMap<String, Map<String, Value>> = HashMap::new();
        for entry in list {
            let (Some(device), Some(bus)) = (id_of(entry, "deviceId"), id_of(entry, "busId"))
            else {
                continue;
            };
            let active = entry
                .get("sources")
                .and_then(Value::as_array)
                .is_some_and(|s| !s.is_empty());
            seen.entry(device).or_default().insert(bus, json!(active));
        }
        for (device, buses) in seen {
            self.device_buses.insert(device, buses);
        }
        self.publish_devices(cx);
    }

    // ---- events ----

    fn event(&mut self, cx: &mut Cx, name: &str, args: &[Value]) {
        let a0 = args.first().cloned().unwrap_or(Value::Null);
        if name == "error" {
            self.error_event(cx, a0.as_str().unwrap_or("error").to_string());
            return;
        }
        if let Some(i) = self.pending.iter().position(|p| p.event == name) {
            let p = self.pending.remove(i).unwrap();
            self.arm_request_timer(cx);
            self.answered(cx, p, args);
        }
        self.apply(cx, name, args, &a0);
    }

    fn answered(&mut self, cx: &mut Cx, p: Pending, args: &[Value]) {
        let a0 = args.first().cloned().unwrap_or(Value::Null);
        if p.purpose.command().is_some() || p.purpose == Purpose::Liveness {
            cx.round_trip(cx.now().saturating_sub(p.sent_at));
        }
        match p.purpose {
            Purpose::Liveness => self.strikes = 0,
            Purpose::Join(room) => self.joined(cx, room),
            Purpose::Read(id) => cx.complete(id, Ok(Outcome::Value { value: a0 })),
            Purpose::InitialData(id) => {
                let names = [
                    "source_types",
                    "source_type_fields",
                    "addresses",
                    "output_types",
                    "output_type_fields",
                    "bus_options",
                    "sources",
                    "devices",
                    "camera_models",
                    "device_sources",
                    "device_actions",
                    "device_states",
                    "tsl_clients",
                    "cloud_destinations",
                    "cloud_keys",
                    "cloud_clients",
                ];
                let value: Map<String, Value> = names
                    .iter()
                    .zip(args)
                    .map(|(n, v)| ((*n).to_string(), v.clone()))
                    .collect();
                cx.complete(
                    id,
                    Ok(Outcome::Value {
                        value: Value::Object(value),
                    }),
                );
            }
            Purpose::Echo(id) => cx.complete(id, Ok(Outcome::Ack)),
            Purpose::ErrorReport(id) => {
                if a0.is_object() {
                    cx.complete(id, Ok(Outcome::Value { value: a0 }));
                } else {
                    cx.complete(id, Err(device_error("no such error report")));
                }
            }
            Purpose::ChangePassword { id, new_password } => {
                let ok = a0.get("changeOk").and_then(Value::as_bool).unwrap_or(false);
                let message = a0.get("message").cloned().unwrap_or(json!(""));
                if !ok {
                    let text = message.as_str().unwrap_or("password not changed");
                    cx.complete(id, Err(device_error(text)));
                    return;
                }
                self.password = new_password;
                let token = a0.get("accessToken").and_then(Value::as_str).unwrap_or("");
                self.emit(cx, "access_token", &[json!(token)]);
                self.set(cx, &["session", "must_change_password"], json!(false));
                cx.log(
                    Level::Info,
                    "Tally Arbiter password changed; store the new password with update_settings",
                );
                cx.complete(
                    id,
                    Ok(Outcome::Value {
                        value: json!({"changeOk": true, "message": message}),
                    }),
                );
                if !self.access.pushed() {
                    self.join(cx, Access::Settings);
                }
            }
            Purpose::Manage {
                id,
                then,
                value,
                unreliable,
            } => {
                let result = a0.get("result").and_then(Value::as_str).unwrap_or("");
                let failed =
                    result.is_empty() || result == "error" || result.ends_with("-not-removed");
                if unreliable {
                    cx.complete(id, Ok(Outcome::Unverified));
                } else if failed {
                    let message = a0
                        .get("error")
                        .and_then(Value::as_str)
                        .filter(|e| !e.is_empty())
                        .or((!result.is_empty()).then_some(result))
                        .unwrap_or("Tally Arbiter gave no result for this change")
                        .to_string();
                    cx.complete(id, Err(device_error(&message)));
                } else if value {
                    let device = a0.get("deviceId").cloned().unwrap_or(Value::Null);
                    cx.complete(id, Ok(Outcome::Value { value: device }));
                } else {
                    cx.complete(id, Ok(Outcome::Ack));
                }
                if self.monitor && (unreliable || !failed) {
                    for event in then {
                        self.emit(cx, event, &[]);
                    }
                }
            }
        }
    }

    fn error_event(&mut self, cx: &mut Cx, message: String) {
        self.set(cx, &["session", "last_error"], json!(message));
        cx.log(Level::Info, format!("Tally Arbiter: {message}"));
        let join = self
            .pending
            .iter()
            .position(|p| matches!(p.purpose, Purpose::Join(_)));
        let command = self
            .pending
            .iter()
            .position(|p| p.purpose.command().is_some());
        if let Some(i) = join.or(command) {
            let p = self.pending.remove(i).unwrap();
            self.arm_request_timer(cx);
            match p.purpose {
                Purpose::Join(room) => self.join_refused(cx, room),
                purpose => {
                    if let Some(id) = purpose.command() {
                        cx.complete(id, Err(device_error(&message)));
                    }
                }
            }
        }
    }

    fn apply(&mut self, cx: &mut Cx, name: &str, args: &[Value], a0: &Value) {
        match name {
            "version" => self.set(cx, &["server", "version"], text(a0)),
            "uiVersion" => self.set(cx, &["server", "ui_version"], text(a0)),
            "externalAddress" => self.set(cx, &["server", "external_address"], text(a0)),
            "interfaces" => self.set(cx, &["server", "interfaces"], json!(array(a0))),
            "chat_enabled" if a0.is_boolean() => {
                self.set(cx, &["server", "chat_enabled"], a0.clone())
            }
            "testmode" if a0.is_boolean() => self.set(cx, &["server", "test_mode"], a0.clone()),
            "tslclients_1secupdate" if a0.is_boolean() => {
                self.set(cx, &["server", "tsl_clients_1sec_update"], a0.clone())
            }
            "remote_error_opt" if a0.is_boolean() => {
                self.set(cx, &["server", "remote_error_reporting"], a0.clone())
            }
            "PortsInUse" => self.set(cx, &["server", "ports_in_use"], json!(array(a0))),
            "networkDiscovery" => self.set(cx, &["server", "network_discovery"], json!(array(a0))),
            "vmix_clients" => self.set(cx, &["server", "vmix_clients"], json!(array(a0))),
            "addresses" if a0.is_object() => self.replace(cx, &["addresses"], a0.clone()),
            "bus_options" => {
                let list = array(a0);
                let buses = keyed(&list, "id", BUS_FIELDS);
                self.buses = Some(list);
                self.replace(cx, &["buses"], buses);
                self.publish_devices(cx);
            }
            "devices" => {
                let list = array(a0);
                let ids: Vec<String> = list.iter().filter_map(|d| id_of(d, "id")).collect();
                self.device_buses.retain(|d, _| ids.contains(d));
                self.devices = Some(list);
                self.publish_devices(cx);
                self.maybe_register(cx);
            }
            "sources" => {
                let list = array(a0);
                let ids: Vec<String> = list.iter().filter_map(|s| id_of(s, "id")).collect();
                self.source_tally.retain(|s, _| ids.contains(s));
                self.sources = Some(list);
                self.publish_sources(cx);
            }
            "device_sources" => {
                let list = array(a0);
                self.replace(
                    cx,
                    &["device_sources"],
                    keyed(&list, "id", DEVICE_SOURCE_FIELDS),
                );
                self.device_sources = Some(list);
            }
            "device_actions" => {
                let list = array(a0);
                self.replace(
                    cx,
                    &["device_actions"],
                    keyed(&list, "id", DEVICE_ACTION_FIELDS),
                );
                self.device_actions = Some(list);
            }
            "device_states" => self.device_states(cx, &array(a0)),
            "listener_clients" => {
                let list = array(a0);
                self.replace(
                    cx,
                    &["listener_clients"],
                    keyed(&list, "id", LISTENER_FIELDS),
                );
                let own = list
                    .iter()
                    .filter(|l| {
                        !self.sid.is_empty()
                            && l.get("socketId").and_then(Value::as_str) == Some(self.sid.as_str())
                            && l.get("inactive").and_then(Value::as_bool) != Some(true)
                    })
                    .find_map(|l| id_of(l, "id"));
                if let Some(own) = own {
                    self.set(cx, &["listener", "client_id"], json!(own));
                }
                self.listener_clients = Some(list);
            }
            "tsl_clients" => {
                let list = array(a0);
                self.replace(cx, &["tsl_clients"], keyed(&list, "id", TSL_FIELDS));
                self.tsl_clients = Some(list);
            }
            "cloud_destinations" => {
                let list = array(a0);
                let state = keyed(&list, "id", CLOUD_DESTINATION_FIELDS);
                self.replace(cx, &["cloud_destinations"], state);
                self.cloud_destinations = Some(list);
            }
            "cloud_keys" => self.set(cx, &["cloud_keys"], json!(array(a0))),
            "cloud_clients" => {
                let list = array(a0);
                self.replace(
                    cx,
                    &["cloud_clients"],
                    keyed(&list, "id", CLOUD_CLIENT_FIELDS),
                );
                self.cloud_clients = Some(list);
            }
            "initialdata" => self.initial_data(cx, args),
            "users" => {
                let list = array(a0);
                let mut out = Map::new();
                for u in &list {
                    let Some(name) = id_of(u, "username") else {
                        continue;
                    };
                    out.insert(
                        name,
                        json!({
                            "roles": text(u.get("roles").unwrap_or(&Value::Null)),
                            "must_change_password": u.get("mustChangePassword").and_then(Value::as_bool).unwrap_or(false),
                        }),
                    );
                }
                self.replace(cx, &["users"], Value::Object(out));
                self.users = Some(list);
            }
            "default_password_users" => self.set(cx, &["default_password_users"], json!(array(a0))),
            "error_reports" => self.set(cx, &["error_reports"], json!(array(a0))),
            "unread_error_reports" => self.set(cx, &["unread_error_reports"], json!(array(a0))),
            "tally_data" => {
                let as_text = |i: usize| {
                    args.get(i)
                        .map(text)
                        .and_then(|v| v.as_str().map(str::to_string))
                };
                let (Some(source), Some(address)) = (as_text(0), as_text(1)) else {
                    return;
                };
                let busses = args.get(2).map(array).unwrap_or_default();
                self.source_tally
                    .entry(source)
                    .or_default()
                    .insert(address, json!(busses));
                self.publish_sources(cx);
            }
            "logs" => {
                if let Some(last) = array(a0).last() {
                    self.log_item(cx, last);
                }
            }
            "log_item" => self.log_item(cx, a0),
            "messaging" => {
                self.chats += 1;
                let message = message_of(args);
                self.set(cx, &["chat", "messages"], json!(self.chats));
                self.set(cx, &["chat", "last"], message.clone());
                cx.message("messaging", None, json!(args), self.source);
            }
            "messaging_client" => {
                self.messages += 1;
                let message = message_of(args);
                self.set(cx, &["listener", "messages"], json!(self.messages));
                self.set(cx, &["listener", "last_message"], message);
                cx.message("messaging_client", None, json!(args), self.source);
            }
            "flash" => {
                self.flashes += 1;
                self.set(cx, &["listener", "flashes"], json!(self.flashes));
                cx.message("flash", None, json!(args), self.source);
            }
            "reassign" => self.reassign(cx, args),
            "must_change_password" => {
                self.set(cx, &["session", "must_change_password"], json!(true))
            }
            "invalid_access_token" => {
                if !self.username.is_empty() && !self.relogin && self.phase == Phase::Ready {
                    self.relogin = true;
                    let (u, p) = (self.username.clone(), self.password.clone());
                    self.emit(cx, "login", &[json!(u), json!(p)]);
                }
            }
            "login_response" => {
                if self.phase == Phase::Login || self.relogin {
                    self.login_response(cx, a0);
                }
            }
            "server_error" => cx.log(Level::Info, format!("Tally Arbiter reported error {a0}")),
            _ => {}
        }
    }

    fn initial_data(&mut self, cx: &mut Cx, args: &[Value]) {
        let arg = |i: usize| args.get(i).cloned().unwrap_or(Value::Null);
        // Source types (0) with their fields (1), output types (3) with
        // theirs (4).
        for (types, fields, field_key, section) in [
            (arg(0), arg(1), "sourceTypeId", "source_types"),
            (arg(3), arg(4), "outputTypeId", "output_types"),
        ] {
            let mut out = Map::new();
            for t in array(&types) {
                let Some(id) = id_of(&t, "id") else { continue };
                let mut obj = pick(
                    &t,
                    &[
                        ("label", "label", Conv::Text),
                        ("help", "help", Conv::Text),
                        ("enabled", "enabled", Conv::Bool),
                        ("busses", "buses", Conv::Same),
                    ],
                );
                if let Some(f) = array(&fields)
                    .iter()
                    .find(|f| id_of(f, field_key).as_deref() == Some(id.as_str()))
                {
                    obj["fields"] = f.get("fields").cloned().unwrap_or(json!([]));
                }
                out.insert(id, obj);
            }
            self.replace(cx, &[section], Value::Object(out));
        }
        let cameras = keyed(
            &array(&arg(8)),
            "id",
            &[
                ("label", "label", Conv::Text),
                ("visible", "visible", Conv::Bool),
            ],
        );
        self.replace(cx, &["camera_models"], cameras);
        for (i, event) in [
            (2, "addresses"),
            (5, "bus_options"),
            (6, "sources"),
            (7, "devices"),
            (9, "device_sources"),
            (10, "device_actions"),
            (11, "device_states"),
            (12, "tsl_clients"),
            (13, "cloud_destinations"),
            (14, "cloud_keys"),
            (15, "cloud_clients"),
        ] {
            let v = arg(i);
            if !v.is_null() {
                self.apply(cx, event, std::slice::from_ref(&v), &v);
            }
        }
    }

    fn log_item(&mut self, cx: &mut Cx, item: &Value) {
        let last = pick(
            item,
            &[
                ("datetime", "datetime", Conv::Text),
                ("type", "type", Conv::Text),
                ("log", "text", Conv::Text),
            ],
        );
        self.set(cx, &["log", "last"], last);
    }

    /// Tally Arbiter moved this listener client to another device.
    fn reassign(&mut self, cx: &mut Cx, args: &[Value]) {
        let old = args.first().map(text).unwrap_or(Value::Null);
        let Some(new) = args
            .get(1)
            .map(text)
            .and_then(|v| v.as_str().map(str::to_string))
        else {
            return;
        };
        self.reassigned = Some(new.clone());
        self.set(cx, &["listener", "device_id"], json!(new));
        if self.access.pushed() {
            // As Tally Arbiter's tally page answers it; it needs the
            // producer role, which this session has.
            self.emit(cx, "listener_reassign", &[old, json!(new)]);
            self.emit(cx, "device_states", &[json!(new)]);
        } else {
            // Without the role Tally Arbiter would refuse listener_reassign:
            // register again, for the new device, on a new connection.
            self.restart(cx);
        }
    }

    // ---- commands ----

    fn plan(&self, name: &str, params: &Params) -> Result<Plan, CommandError> {
        let p = |n: &str| params.get(n).cloned().unwrap_or(Value::Null);
        let unverified = |event, args| Plan::Unverified {
            event,
            args,
            then: &[],
        };
        if let Some(reply) = read_event(name) {
            let args = match (name, params.get("device_id")) {
                ("get_device_states", Some(d)) => vec![d.clone()],
                _ => Vec::new(),
            };
            return Ok(Plan::Wait {
                event: read_request(name, reply),
                args,
                reply,
                purpose: Purpose::Read(0),
            });
        }
        let manage = |kind: &str, action: &str, body: Map<String, Value>| {
            let mut obj = Map::new();
            obj.insert("type".into(), json!(kind));
            obj.insert("action".into(), json!(action));
            obj.extend(body);
            vec![Value::Object(obj)]
        };
        let one = |key: &str, v: Value| {
            let mut m = Map::new();
            m.insert(key.into(), v);
            m
        };
        let managed = |args, then, value, unreliable| Plan::Wait {
            event: "manage",
            args,
            reply: "manage_response",
            purpose: Purpose::Manage {
                id: 0,
                then,
                value,
                unreliable,
            },
        };
        Ok(match name {
            "get_initial_data" => Plan::Wait {
                event: "settings",
                args: Vec::new(),
                reply: "initialdata",
                purpose: Purpose::InitialData(0),
            },
            "get_error_reports" => Plan::Wait {
                event: "get_error_reports",
                args: Vec::new(),
                reply: "error_reports",
                purpose: Purpose::Read(0),
            },
            "get_error_report" => Plan::Wait {
                event: "get_error_report",
                args: vec![p("error_report_id")],
                reply: "error_report",
                purpose: Purpose::ErrorReport(0),
            },
            "get_config" => Plan::Wait {
                event: "get_config",
                args: Vec::new(),
                reply: "config",
                purpose: Purpose::Read(0),
            },
            "flash_listener" => unverified("flash", vec![p("listener_client_id")]),
            "send_listener_message" => unverified(
                "messaging_client",
                vec![
                    p("listener_client_id"),
                    json!("producer"),
                    json!(self.sid),
                    p("message"),
                ],
            ),
            "reassign_listener" => {
                let client = p("listener_client_id");
                let old = match params.get("old_device_id") {
                    Some(old) => old.clone(),
                    None => self
                        .listener_clients
                        .iter()
                        .flatten()
                        .find(|l| l.get("id") == Some(&client))
                        .and_then(|l| l.get("deviceId").cloned())
                        .ok_or_else(|| CommandError::InvalidParams {
                            message: "old_device_id is needed: the listener client is not in listener_clients".into(),
                        })?,
                };
                Plan::Unverified {
                    event: "reassign",
                    args: vec![client, old, p("device_id")],
                    then: &["listener_clients"],
                }
            }
            "delete_listener" => Plan::Unverified {
                event: "listener_delete",
                args: vec![p("listener_client_id")],
                then: &["listener_clients"],
            },
            "send_chat" => unverified("messaging", vec![p("sender"), p("message")]),
            "set_test_mode" => {
                let args = vec![p("enabled"), p("interval_ms")];
                if self.access == Access::Settings {
                    Plan::Wait {
                        event: "testmode",
                        args,
                        reply: "testmode",
                        purpose: Purpose::Echo(0),
                    }
                } else {
                    unverified("testmode", args)
                }
            }
            "set_tsl_clients_1sec_update" => {
                unverified("tslclients_1secupdate", vec![p("enabled")])
            }
            "set_chat_enabled" => Plan::Wait {
                event: "chat_enabled",
                args: vec![p("enabled")],
                reply: "chat_enabled",
                purpose: Purpose::Echo(0),
            },
            "set_remote_error_reporting" => Plan::Wait {
                event: "remote_error_opt",
                args: vec![p("enabled")],
                reply: "remote_error_opt",
                purpose: Purpose::Echo(0),
            },
            "set_config" => unverified("set_config", vec![p("config")]),
            "mark_error_reports_read" => Plan::Unverified {
                event: "mark_error_reports_as_read",
                args: Vec::new(),
                then: &["get_error_reports"],
            },
            "delete_all_error_reports" => Plan::Unverified {
                event: "delete_every_error_report",
                args: Vec::new(),
                then: &["get_error_reports"],
            },
            "change_password" => Plan::Wait {
                event: "change_password",
                args: vec![p("current_password"), p("new_password")],
                reply: "change_password_response",
                purpose: Purpose::ChangePassword {
                    id: 0,
                    new_password: str_param(params, "new_password").unwrap_or("").to_string(),
                },
            },
            "reconnect_source" => Plan::Unverified {
                event: "reconnect_source",
                args: vec![p("source_id")],
                then: &["sources"],
            },
            "reconnect_cloud_destination" => Plan::Unverified {
                event: "cloud_destination_reconnect",
                args: vec![p("cloud_destination_id")],
                then: &["cloud_destinations"],
            },
            "disconnect_cloud_destination" => Plan::Unverified {
                event: "cloud_destination_disconnect",
                args: vec![p("cloud_destination_id")],
                then: &["cloud_destinations"],
            },

            "add_source" => {
                let mut source = from_params(params, SOURCE_EDIT);
                source.insert("sourceTypeId".into(), p("source_type_id"));
                managed(
                    manage("source", "add", one("source", Value::Object(source))),
                    THEN_SOURCE,
                    false,
                    false,
                )
            }
            "edit_source" => {
                let source = edited(&self.sources, "source_id", params, SOURCE_EDIT, "a source")?;
                managed(
                    manage("source", "edit", one("source", source)),
                    THEN_SOURCE,
                    false,
                    false,
                )
            }
            "delete_source" => {
                known(&self.sources, &p("source_id"), "source")?;
                managed(
                    manage("source", "delete", one("sourceId", p("source_id"))),
                    THEN_SOURCE,
                    false,
                    false,
                )
            }
            "add_device" => managed(
                manage(
                    "device",
                    "add",
                    one("device", Value::Object(from_params(params, DEVICE_EDIT))),
                ),
                THEN_DEVICE,
                false,
                false,
            ),
            "edit_device" => {
                let device = edited(&self.devices, "device_id", params, DEVICE_EDIT, "a device")?;
                managed(
                    manage("device", "edit", one("device", device)),
                    THEN_DEVICE,
                    false,
                    false,
                )
            }
            "delete_device" => {
                known(&self.devices, &p("device_id"), "device")?;
                managed(
                    manage("device", "delete", one("deviceId", p("device_id"))),
                    THEN_DEVICE,
                    false,
                    false,
                )
            }
            "duplicate_device" => {
                known(&self.devices, &p("device_id"), "device")?;
                managed(
                    manage("device", "duplicate", one("deviceId", p("device_id"))),
                    THEN_DEVICE,
                    true,
                    false,
                )
            }
            "add_device_source" => {
                let mut ds = from_params(params, DEVICE_SOURCE_EDIT);
                ds.insert("deviceId".into(), p("device_id"));
                managed(
                    manage(
                        "device_source",
                        "add",
                        one("device_source", Value::Object(ds)),
                    ),
                    THEN_DEVICE_SOURCE,
                    false,
                    false,
                )
            }
            "edit_device_source" => {
                let ds = edited(
                    &self.device_sources,
                    "device_source_id",
                    params,
                    DEVICE_SOURCE_EDIT,
                    "a device source",
                )?;
                managed(
                    manage("device_source", "edit", one("device_source", ds)),
                    THEN_DEVICE_SOURCE,
                    false,
                    false,
                )
            }
            "delete_device_source" => {
                known(
                    &self.device_sources,
                    &p("device_source_id"),
                    "device source",
                )?;
                managed(
                    manage(
                        "device_source",
                        "delete",
                        one("device_source", json!({"id": p("device_source_id")})),
                    ),
                    THEN_DEVICE_SOURCE,
                    false,
                    false,
                )
            }
            "add_device_action" => {
                let mut da = from_params(params, DEVICE_ACTION_EDIT);
                da.insert("deviceId".into(), p("device_id"));
                managed(
                    manage(
                        "device_action",
                        "add",
                        one("device_action", Value::Object(da)),
                    ),
                    THEN_DEVICE_ACTION,
                    false,
                    false,
                )
            }
            "edit_device_action" => {
                let da = edited(
                    &self.device_actions,
                    "device_action_id",
                    params,
                    DEVICE_ACTION_EDIT,
                    "a device action",
                )?;
                managed(
                    manage("device_action", "edit", one("device_action", da)),
                    THEN_DEVICE_ACTION,
                    false,
                    false,
                )
            }
            "delete_device_action" | "duplicate_device_action" => {
                known(
                    &self.device_actions,
                    &p("device_action_id"),
                    "device action",
                )?;
                let action = if name == "delete_device_action" {
                    "delete"
                } else {
                    "duplicate"
                };
                managed(
                    manage(
                        "device_action",
                        action,
                        one("device_action", json!({"id": p("device_action_id")})),
                    ),
                    THEN_DEVICE_ACTION,
                    false,
                    false,
                )
            }
            "add_tsl_client" => managed(
                manage(
                    "tsl_client",
                    "add",
                    one("tslClient", Value::Object(from_params(params, TSL_EDIT))),
                ),
                THEN_TSL,
                false,
                false,
            ),
            "edit_tsl_client" => {
                let c = edited(
                    &self.tsl_clients,
                    "tsl_client_id",
                    params,
                    TSL_EDIT,
                    "a TSL client",
                )?;
                managed(
                    manage("tsl_client", "edit", one("tslClient", c)),
                    THEN_TSL,
                    false,
                    false,
                )
            }
            "delete_tsl_client" => {
                known(&self.tsl_clients, &p("tsl_client_id"), "TSL client")?;
                managed(
                    manage(
                        "tsl_client",
                        "delete",
                        one("tslClientId", p("tsl_client_id")),
                    ),
                    THEN_TSL,
                    false,
                    false,
                )
            }
            "add_bus_option" => {
                let mut bus = from_params(params, BUS_EDIT);
                bus.insert("priority".into(), p("priority"));
                managed(
                    manage("bus_option", "add", one("busOption", Value::Object(bus))),
                    THEN_BUS,
                    false,
                    false,
                )
            }
            "edit_bus_option" => {
                let bus = edited(&self.buses, "bus_id", params, BUS_EDIT, "a bus")?;
                managed(
                    manage("bus_option", "edit", one("busOption", bus)),
                    THEN_BUS,
                    false,
                    false,
                )
            }
            "delete_bus_option" => {
                known(&self.buses, &p("bus_id"), "bus")?;
                managed(
                    manage("bus_option", "delete", one("busOptionId", p("bus_id"))),
                    THEN_BUS,
                    false,
                    false,
                )
            }
            "add_cloud_destination" => managed(
                manage(
                    "cloud_destination",
                    "add",
                    one(
                        "cloudDestination",
                        Value::Object(from_params(params, CLOUD_DESTINATION_EDIT)),
                    ),
                ),
                THEN_CLOUD_DESTINATION,
                false,
                false,
            ),
            "edit_cloud_destination" => {
                let d = edited(
                    &self.cloud_destinations,
                    "cloud_destination_id",
                    params,
                    CLOUD_DESTINATION_EDIT,
                    "a cloud destination",
                )?;
                managed(
                    manage("cloud_destination", "edit", one("cloudDestination", d)),
                    THEN_CLOUD_DESTINATION,
                    false,
                    false,
                )
            }
            "delete_cloud_destination" => {
                known(
                    &self.cloud_destinations,
                    &p("cloud_destination_id"),
                    "cloud destination",
                )?;
                managed(
                    manage(
                        "cloud_destination",
                        "delete",
                        one("cloudId", p("cloud_destination_id")),
                    ),
                    THEN_CLOUD_DESTINATION,
                    false,
                    false,
                )
            }
            "add_cloud_key" | "delete_cloud_key" => managed(
                manage(
                    "cloud_key",
                    if name == "add_cloud_key" {
                        "add"
                    } else {
                        "delete"
                    },
                    one("key", p("key")),
                ),
                THEN_CLOUD_KEY,
                false,
                false,
            ),
            "remove_cloud_client" => managed(
                manage("cloud_client", "remove", one("id", p("cloud_client_id"))),
                THEN_CLOUD_CLIENT,
                false,
                false,
            ),
            "add_user" => managed(
                manage(
                    "user",
                    "add",
                    one(
                        "user",
                        json!({"username": p("username"), "password": p("password"), "roles": p("roles")}),
                    ),
                ),
                THEN_USER,
                false,
                false,
            ),
            "edit_user" => {
                let username = p("username");
                let roles = match params.get("roles") {
                    Some(r) => r.clone(),
                    None => self
                        .users
                        .iter()
                        .flatten()
                        .find(|u| u.get("username") == Some(&username))
                        .and_then(|u| u.get("roles").cloned())
                        .ok_or_else(|| CommandError::InvalidParams {
                            message:
                                "roles is needed: the user is not in users (read it with get_users)"
                                    .into(),
                        })?,
                };
                let mut user = json!({"username": username, "roles": roles});
                if let Some(password) = params.get("password") {
                    user["password"] = password.clone();
                }
                managed(
                    manage("user", "edit", one("user", user)),
                    THEN_USER,
                    false,
                    false,
                )
            }
            "delete_user" => managed(
                manage(
                    "user",
                    "delete",
                    one("user", json!({"username": p("username")})),
                ),
                THEN_USER,
                false,
                true,
            ),
            other => {
                return Err(CommandError::UnknownCommand {
                    command: other.to_string(),
                })
            }
        })
    }
}

fn device_error(message: &str) -> CommandError {
    CommandError::DeviceError {
        code: None,
        message: message.to_string(),
    }
}

/// `{type, from, text}` from a messaging event's (type, socketid, message).
fn message_of(args: &[Value]) -> Value {
    json!({
        "type": args.first().map(text).unwrap_or(Value::Null),
        "from": args.get(1).map(text).unwrap_or(Value::Null),
        "text": args.get(2).map(text).unwrap_or(Value::Null),
    })
}

/// Refuse an id a list that has been read does not hold: Tally Arbiter's
/// handlers throw on some unknown ids and never answer.
fn known(list: &Option<Vec<Value>>, id: &Value, what: &str) -> Result<(), CommandError> {
    match list {
        Some(items) if !items.iter().any(|i| i.get("id") == Some(id)) => {
            Err(CommandError::InvalidParams {
                message: format!("Tally Arbiter has no {what} with id {id}"),
            })
        }
        _ => Ok(()),
    }
}

/// The object an edit sends: the current one, as last read, with the given
/// fields replaced. Tally Arbiter replaces every field, so one that was never
/// read must be given.
fn edited(
    list: &Option<Vec<Value>>,
    id_param: &str,
    params: &Params,
    fields: &[Field],
    what: &str,
) -> Result<Value, CommandError> {
    let id = params.get(id_param).cloned().unwrap_or(Value::Null);
    let current = list
        .iter()
        .flatten()
        .find(|i| i.get("id") == Some(&id))
        .cloned();
    if current.is_none() && list.is_some() {
        known(list, &id, what.trim_start_matches("a "))?;
    }
    let mut obj = match current {
        Some(Value::Object(o)) => o,
        _ => {
            let missing: Vec<&str> = fields
                .iter()
                .map(|(p, _)| *p)
                .filter(|p| !params.contains_key(*p) && *p != "bus")
                .collect();
            if !missing.is_empty() {
                return Err(CommandError::InvalidParams {
                    message: format!(
                        "{what} not read yet needs every field; missing {}",
                        missing.join(", ")
                    ),
                });
            }
            Map::new()
        }
    };
    obj.insert("id".into(), id);
    obj.extend(from_params(params, fields));
    Ok(Value::Object(obj))
}

impl Module for TallyArbiter {
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
        if self.phase != Phase::Ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let plan = match self.plan(name, params) {
            Ok(plan) => plan,
            Err(e) => {
                cx.complete(id, Err(e));
                return;
            }
        };
        match plan {
            Plan::Wait {
                event,
                args,
                reply,
                purpose,
            } => {
                if event == "manage" && self.access != Access::Settings {
                    cx.complete(
                        id,
                        Err(device_error(
                            "Tally Arbiter answers changes in its settings room only: sign in with an admin account",
                        )),
                    );
                    return;
                }
                let purpose = match purpose {
                    Purpose::Read(_) => Purpose::Read(id),
                    Purpose::InitialData(_) => Purpose::InitialData(id),
                    Purpose::Echo(_) => Purpose::Echo(id),
                    Purpose::ErrorReport(_) => Purpose::ErrorReport(id),
                    Purpose::ChangePassword { new_password, .. } => {
                        Purpose::ChangePassword { id, new_password }
                    }
                    Purpose::Manage {
                        then,
                        value,
                        unreliable,
                        ..
                    } => Purpose::Manage {
                        id,
                        then,
                        value,
                        unreliable,
                    },
                    other => other,
                };
                self.emit(cx, event, &args);
                self.wait(cx, reply, purpose);
            }
            Plan::Unverified { event, args, then } => {
                self.emit(cx, event, &args);
                cx.complete(id, Ok(Outcome::Unverified));
                if self.monitor {
                    for event in then {
                        self.emit(cx, event, &[]);
                    }
                }
            }
        }
    }

    fn ws(&mut self, cx: &mut Cx, _socket: Key, input: WsInput) {
        if self.refused.is_some() {
            return;
        }
        match input {
            WsInput::Opened => {}
            WsInput::Text(text) => {
                cx.alive();
                self.text(cx, &text);
            }
            WsInput::Binary(_) | WsInput::Activity => cx.alive(),
            WsInput::Closed { code, reason } => match code {
                Some(code) => self.lost(cx, format!("closed by Tally Arbiter ({code}): {reason}")),
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
            HANDSHAKE if self.phase != Phase::Ready && self.phase != Phase::Idle => {
                self.lost(cx, "no Socket.IO handshake from Tally Arbiter".into())
            }
            LIVENESS if self.phase == Phase::Ready => {
                self.emit(cx, "version", &[]);
                self.wait(cx, "version", Purpose::Liveness);
                cx.set_timer(LIVENESS, LIVENESS_EVERY);
            }
            STATES if self.phase == Phase::Ready && self.monitor && !self.access.pushed() => {
                self.emit(cx, "device_states", &[]);
                if self.poll_ms > 0 {
                    cx.set_timer(STATES, self.poll_ms);
                }
            }
            LISTS if self.phase == Phase::Ready && self.monitor && !self.access.pushed() => {
                for event in LIST_READS {
                    self.emit(cx, event, &[]);
                }
                cx.set_timer(LISTS, LISTS_EVERY);
            }
            REQUEST => {
                let now = cx.now();
                let mut missed = false;
                let mut refused_joins = Vec::new();
                let mut kept = VecDeque::new();
                for p in self.pending.drain(..) {
                    if p.deadline > now {
                        kept.push_back(p);
                        continue;
                    }
                    match p.purpose {
                        Purpose::Liveness => missed = true,
                        Purpose::Join(room) => refused_joins.push(room),
                        purpose => {
                            if let Some(id) = purpose.command() {
                                cx.complete(id, Err(CommandError::Timeout));
                            }
                        }
                    }
                }
                self.pending = kept;
                self.arm_request_timer(cx);
                for room in refused_joins {
                    self.join_refused(cx, room);
                }
                if missed {
                    self.strikes += 1;
                    if self.strikes >= STRIKES {
                        self.lost(cx, "Tally Arbiter stopped answering".into());
                    }
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.ws_close(SOCKET);
    }

    fn reconnect(&mut self, cx: &mut Cx) -> bool {
        // A refused credential stays refused until the settings change.
        if self.refused.is_none() {
            cx.cancel_timer(RETRY);
            self.restart(cx);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    fn ta(settings: Value, monitor: bool) -> TallyArbiter {
        TallyArbiter::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)),
            host_name: None,
            port: None,
            model: "tally-arbiter-3".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        })
    }

    /// Socket.IO events sent, as [name, args...].
    fn sent(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::WsSend { text, .. } => text
                    .strip_prefix("42")
                    .map(|t| serde_json::from_str(t).unwrap()),
                _ => None,
            })
            .collect()
    }

    fn names(actions: &[Action]) -> Vec<String> {
        sent(actions)
            .iter()
            .map(|e| e[0].as_str().unwrap().to_string())
            .collect()
    }

    fn raw(m: &mut TallyArbiter, now: Millis, text: &str) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.ws(&mut cx, SOCKET, WsInput::Text(text.into()));
        cx.take()
    }

    fn event(m: &mut TallyArbiter, now: Millis, name: &str, args: Value) -> Vec<Action> {
        let mut items = vec![json!(name)];
        items.extend(args.as_array().unwrap().iter().cloned());
        raw(m, now, &format!("42{}", Value::Array(items)))
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

    fn command(
        m: &mut TallyArbiter,
        now: Millis,
        id: CommandId,
        name: &str,
        params: Value,
    ) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, params.as_object().unwrap());
        cx.take()
    }

    /// Open, Engine.IO open and the namespace connect.
    fn connect(m: &mut TallyArbiter) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(x, Action::WsOpen { request, .. }
            if request.url == "ws://10.0.0.9:4455/socket.io/?EIO=4&transport=websocket")));
        let a = raw(
            m,
            1,
            r#"0{"sid":"e1","upgrades":[],"pingInterval":25000,"pingTimeout":20000,"maxPayload":1000000}"#,
        );
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::WsSend { text, .. } if text == "40")));
        raw(m, 2, r#"40{"sid":"s1"}"#)
    }

    const BUSES: &str = r##"[{"id":"pvw","label":"Preview","type":"preview","color":"#3fe481","priority":50,"visible":true},
        {"id":"pgm","label":"Program","type":"program","color":"#e43f5a","priority":200,"visible":true},
        {"id":"aux","label":"Aux 1","type":"aux","color":"#0000FF","priority":100,"visible":true}]"##;
    const DEVICES: &str = r#"[{"id":"cam1","name":"Camera 1","description":"","enabled":true,"tslAddress":"1","linkedBusses":[]},
        {"id":"cam2","name":"Camera 2","description":"wide","enabled":true,"tslAddress":2,"linkedBusses":["pgm"]}]"#;

    fn admin() -> (TallyArbiter, Vec<Action>) {
        let mut m = ta(
            json!({"username": "admin", "password": "pw", "device_id": "cam1"}),
            true,
        );
        let a = connect(&mut m);
        assert_eq!(sent(&a), [json!(["login", "admin", "pw"])]);
        let a = event(
            &mut m,
            3,
            "login_response",
            json!([{"loginOk": true, "message": "", "accessToken": "tok"}]),
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(
            sent(&a),
            [json!(["access_token", "tok"]), json!(["settings"])]
        );
        let mut init = vec![
            json!([{"id": "ATEM", "label": "Blackmagic ATEM", "help": "", "enabled": true, "busses": []}]),
            json!([{"sourceTypeId": "ATEM", "fields": [{"fieldName": "ip", "fieldLabel": "IP Address", "fieldType": "text"}]}]),
            json!({}),
            json!([]),
            json!([]),
            serde_json::from_str(BUSES).unwrap(),
            json!([]),
            serde_json::from_str(DEVICES).unwrap(),
            json!([{"id": "canon_xc", "label": "Canon", "visible": true}]),
            json!([]),
            json!([]),
            json!([]),
            json!([]),
            json!([]),
            json!([]),
            json!([]),
        ];
        init.truncate(16);
        let a = event(&mut m, 4, "initialdata", Value::Array(init));
        (m, a)
    }

    #[test]
    fn signs_in_joins_the_settings_room_and_registers_the_listener() {
        let (m, a) = admin();
        let s = state(&a);
        assert_eq!(s["session"]["access"], "settings");
        assert_eq!(s["source_types"]["ATEM"]["label"], "Blackmagic ATEM");
        assert_eq!(s["source_types"]["ATEM"]["fields"][0]["fieldName"], "ip");
        assert_eq!(s["camera_models"]["canon_xc"]["label"], "Canon");
        assert_eq!(s["devices"]["cam2"]["tsl_address"], "2");
        assert_eq!(s["buses"]["pgm"]["type"], "program");
        let n = names(&a);
        assert!(n.contains(&"listenerclient_connect".to_string()));
        assert!(n.contains(&"users".to_string()));
        assert!(n.contains(&"version".to_string()));
        let connect = sent(&a)
            .into_iter()
            .find(|e| e[0] == "listenerclient_connect")
            .unwrap();
        assert_eq!(
            connect[1],
            json!({"deviceId": "cam1", "listenerType": "meros",
            "canBeReassigned": true, "canBeFlashed": true, "supportsChat": true})
        );
        assert_eq!(m.access, Access::Settings);
    }

    #[test]
    fn device_states_give_program_preview_and_every_bus() {
        let (mut m, _) = admin();
        let a = event(
            &mut m,
            10,
            "device_states",
            json!([[
                {"busId": "pvw", "deviceId": "cam1", "sources": []},
                {"busId": "pgm", "deviceId": "cam1", "sources": ["ds1"]},
                {"busId": "aux", "deviceId": "cam1", "sources": []},
                {"busId": "pvw", "deviceId": "cam2", "sources": ["ds2"]},
                {"busId": "pgm", "deviceId": "cam2", "sources": []},
                {"busId": "aux", "deviceId": "cam2", "sources": ["ds3"]}
            ]]),
        );
        let s = state(&a);
        assert_eq!(s["devices"]["cam1"]["program"], true);
        assert_eq!(s["devices"]["cam1"]["preview"], false);
        assert_eq!(
            s["devices"]["cam1"]["buses"],
            json!({"pvw": false, "pgm": true, "aux": false})
        );
        assert_eq!(s["devices"]["cam2"]["preview"], true);
        assert_eq!(s["devices"]["cam2"]["buses"]["aux"], true);

        // A push for one device leaves the other as it was.
        let a = event(
            &mut m,
            11,
            "device_states",
            json!([[
                {"busId": "pvw", "deviceId": "cam1", "sources": ["ds1"]},
                {"busId": "pgm", "deviceId": "cam1", "sources": []},
                {"busId": "aux", "deviceId": "cam1", "sources": []}
            ]]),
        );
        let s = state(&a);
        assert_eq!(s["devices"]["cam1"]["program"], false);
        assert_eq!(s["devices"]["cam1"]["preview"], true);
        assert!(s["devices"].get("cam2").is_none());
        assert_eq!(m.mirror["devices"]["cam2"]["preview"], true);
        assert_eq!(m.mirror["devices"]["cam1"]["name"], "Camera 1");

        // Changing a bus's type changes what program means.
        let a = event(
            &mut m,
            12,
            "bus_options",
            json!([[
                {"id": "pvw", "label": "Preview", "type": "program", "color": "#3fe481", "priority": 50, "visible": true}
            ]]),
        );
        let s = state(&a);
        assert_eq!(s["devices"]["cam1"]["program"], true);
        assert_eq!(s["buses"]["pgm"], Value::Null);
    }

    #[test]
    fn lists_are_replaced_whole() {
        let (mut m, _) = admin();
        event(
            &mut m,
            10,
            "tsl_clients",
            json!([[
                {"id": "t1", "ip": "10.0.0.20", "port": "5727", "transport": "udp", "protocol": "3.1", "protocolOptions": {"tally1": "pvw"}, "connected": false},
                {"id": "t2", "ip": "10.0.0.21", "port": 8900, "transport": "tcp", "protocol": "5.0", "protocolOptions": {}, "connected": true}
            ]]),
        );
        assert_eq!(m.mirror["tsl_clients"]["t1"]["port"], 5727);
        let a = event(
            &mut m,
            11,
            "tsl_clients",
            json!([[
                {"id": "t2", "ip": "10.0.0.21", "port": 8900, "transport": "tcp", "protocol": "5.0", "protocolOptions": {}, "connected": true}
            ]]),
        );
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::State(p) if p["tsl_clients"] == json!({"t1": null}))));
        let a = event(
            &mut m,
            12,
            "listener_clients",
            json!([[
                {"id": "L1", "socketId": "s1", "deviceId": "cam1", "internalId": "L1", "listenerType": "meros", "ipAddress": "::ffff:10.0.0.5",
                 "datetime_connected": 1790000000000_i64, "canBeReassigned": true, "canBeFlashed": true, "supportsChat": true, "inactive": false}
            ]]),
        );
        let s = state(&a);
        assert_eq!(
            s["listener_clients"]["L1"]["connected_at"],
            1790000000000_i64
        );
        assert_eq!(s["listener"]["client_id"], "L1");
        let a = event(
            &mut m,
            13,
            "users",
            json!([[{"username": "admin", "roles": "admin"}, {"username": "pat", "roles": "producer", "mustChangePassword": true}]]),
        );
        let s = state(&a);
        assert_eq!(
            s["users"]["pat"],
            json!({"roles": "producer", "must_change_password": true})
        );
        let a = event(&mut m, 14, "tally_data", json!(["src1", "3", ["pgm"]]));
        assert!(state(&a).get("sources").is_none());
        event(
            &mut m,
            15,
            "sources",
            json!([[{"id": "src1", "name": "ATEM", "sourceTypeId": "ATEM", "enabled": true, "connected": true, "reconnect": true, "data": {"ip": "10.0.0.2"}}]]),
        );
        assert_eq!(m.mirror["sources"]["src1"]["tally"]["3"], json!(["pgm"]));
        assert_eq!(m.mirror["sources"]["src1"]["data"]["ip"], "10.0.0.2");
    }

    /// Every command: its params, and the event it emits.
    fn table() -> Vec<(&'static str, Value, Value)> {
        vec![
            ("get_version", json!({}), json!(["version"])),
            ("get_ui_version", json!({}), json!(["uiVersion"])),
            (
                "get_external_address",
                json!({}),
                json!(["externalAddress"]),
            ),
            ("get_interfaces", json!({}), json!(["interfaces"])),
            ("get_sources", json!({}), json!(["sources"])),
            ("get_devices", json!({}), json!(["devices"])),
            ("get_device_sources", json!({}), json!(["device_sources"])),
            ("get_device_actions", json!({}), json!(["device_actions"])),
            ("get_bus_options", json!({}), json!(["bus_options"])),
            (
                "get_device_states",
                json!({"device_id": "cam1"}),
                json!(["device_states", "cam1"]),
            ),
            (
                "get_listener_clients",
                json!({}),
                json!(["listener_clients"]),
            ),
            ("get_tsl_clients", json!({}), json!(["tsl_clients"])),
            (
                "get_cloud_destinations",
                json!({}),
                json!(["cloud_destinations"]),
            ),
            ("get_cloud_keys", json!({}), json!(["cloud_keys"])),
            ("get_cloud_clients", json!({}), json!(["cloud_clients"])),
            ("get_initial_data", json!({}), json!(["settings"])),
            ("get_error_reports", json!({}), json!(["get_error_reports"])),
            (
                "get_unread_error_reports",
                json!({}),
                json!(["get_unread_error_reports"]),
            ),
            (
                "get_error_report",
                json!({"error_report_id": "r1"}),
                json!(["get_error_report", "r1"]),
            ),
            ("get_users", json!({}), json!(["users"])),
            (
                "get_default_password_users",
                json!({}),
                json!(["default_password_users"]),
            ),
            ("get_config", json!({}), json!(["get_config"])),
            (
                "flash_listener",
                json!({"listener_client_id": "L1"}),
                json!(["flash", "L1"]),
            ),
            (
                "send_listener_message",
                json!({"listener_client_id": "L1", "message": "Stand by"}),
                json!(["messaging_client", "L1", "producer", "s1", "Stand by"]),
            ),
            (
                "reassign_listener",
                json!({"listener_client_id": "L1", "device_id": "cam2", "old_device_id": "cam1"}),
                json!(["reassign", "L1", "cam1", "cam2"]),
            ),
            (
                "delete_listener",
                json!({"listener_client_id": "L1"}),
                json!(["listener_delete", "L1"]),
            ),
            (
                "send_chat",
                json!({"message": "Hi", "sender": "producer"}),
                json!(["messaging", "producer", "Hi"]),
            ),
            (
                "set_test_mode",
                json!({"enabled": true, "interval_ms": 1000}),
                json!(["testmode", true, 1000]),
            ),
            (
                "set_tsl_clients_1sec_update",
                json!({"enabled": true}),
                json!(["tslclients_1secupdate", true]),
            ),
            (
                "set_chat_enabled",
                json!({"enabled": false}),
                json!(["chat_enabled", false]),
            ),
            (
                "set_remote_error_reporting",
                json!({"enabled": false}),
                json!(["remote_error_opt", false]),
            ),
            (
                "set_config",
                json!({"config": {"uuid": "x"}}),
                json!(["set_config", {"uuid": "x"}]),
            ),
            (
                "mark_error_reports_read",
                json!({}),
                json!(["mark_error_reports_as_read"]),
            ),
            (
                "delete_all_error_reports",
                json!({}),
                json!(["delete_every_error_report"]),
            ),
            (
                "change_password",
                json!({"current_password": "a", "new_password": "b"}),
                json!(["change_password", "a", "b"]),
            ),
            (
                "reconnect_source",
                json!({"source_id": "src1"}),
                json!(["reconnect_source", "src1"]),
            ),
            (
                "add_source",
                json!({"name": "ATEM", "source_type_id": "ATEM", "enabled": true, "reconnect": true,
                "reconnect_interval": 5000, "max_reconnects": 5, "data": {"ip": "10.0.0.2"}}),
                json!(["manage", {"type": "source", "action": "add", "source": {"name": "ATEM", "sourceTypeId": "ATEM",
                    "enabled": true, "reconnect": true, "reconnect_interval": 5000, "max_reconnects": 5, "data": {"ip": "10.0.0.2"}}}]),
            ),
            (
                "edit_source",
                json!({"source_id": "src1", "name": "ATEM 2", "enabled": true, "reconnect": false,
                "reconnect_interval": 1000, "max_reconnects": 3, "data": {}}),
                json!(["manage", {"type": "source", "action": "edit", "source": {"id": "src1", "name": "ATEM 2", "sourceTypeId": "ATEM",
                    "enabled": true, "reconnect": false, "reconnect_interval": 1000, "max_reconnects": 3, "data": {}}}]),
            ),
            (
                "delete_source",
                json!({"source_id": "src1"}),
                json!(["manage", {"type": "source", "action": "delete", "sourceId": "src1"}]),
            ),
            (
                "add_device",
                json!({"name": "Cam 3", "description": "", "enabled": true, "tsl_address": "3", "linked_buses": [],
                "camera_ip": "", "camera_model": "", "camera_preview_disabled": false}),
                json!(["manage", {"type": "device", "action": "add", "device": {"name": "Cam 3", "description": "", "enabled": true,
                    "tslAddress": "3", "linkedBusses": [], "cameraIP": "", "cameraModel": "", "cameraPreviewDisabled": false}}]),
            ),
            (
                "edit_device",
                json!({"device_id": "cam1", "name": "Cam One"}),
                json!(["manage", {"type": "device", "action": "edit", "device": {"id": "cam1", "name": "Cam One", "description": "",
                    "enabled": true, "tslAddress": "1", "linkedBusses": []}}]),
            ),
            (
                "delete_device",
                json!({"device_id": "cam1"}),
                json!(["manage", {"type": "device", "action": "delete", "deviceId": "cam1"}]),
            ),
            (
                "duplicate_device",
                json!({"device_id": "cam1"}),
                json!(["manage", {"type": "device", "action": "duplicate", "deviceId": "cam1"}]),
            ),
            (
                "add_device_source",
                json!({"device_id": "cam1", "source_id": "src1", "address": "1", "bus": "", "rename": false,
                "reconnect_interval": 5000, "max_reconnects": 5}),
                json!(["manage", {"type": "device_source", "action": "add", "device_source": {"deviceId": "cam1", "sourceId": "src1",
                    "address": "1", "bus": "", "rename": false, "reconnect_interval": 5000, "max_reconnects": 5}}]),
            ),
            (
                "edit_device_source",
                json!({"device_source_id": "ds1", "source_id": "src1", "address": "2", "rename": false,
                "reconnect_interval": 5000, "max_reconnects": 5}),
                json!(["manage", {"type": "device_source", "action": "edit", "device_source": {"id": "ds1", "deviceId": "cam1", "sourceId": "src1",
                    "address": "2", "rename": false, "reconnect_interval": 5000, "max_reconnects": 5}}]),
            ),
            (
                "delete_device_source",
                json!({"device_source_id": "ds1"}),
                json!(["manage", {"type": "device_source", "action": "delete", "device_source": {"id": "ds1"}}]),
            ),
            (
                "add_device_action",
                json!({"device_id": "cam1", "bus_id": "pgm", "active": true, "output_type_id": "osc", "data": {}}),
                json!(["manage", {"type": "device_action", "action": "add", "device_action": {"deviceId": "cam1", "busId": "pgm",
                    "active": true, "outputTypeId": "osc", "data": {}}}]),
            ),
            (
                "edit_device_action",
                json!({"device_action_id": "da1", "bus_id": "pvw", "active": false, "output_type_id": "osc", "data": {}}),
                json!(["manage", {"type": "device_action", "action": "edit", "device_action": {"id": "da1", "deviceId": "cam1", "busId": "pvw",
                    "active": false, "outputTypeId": "osc", "data": {}}}]),
            ),
            (
                "delete_device_action",
                json!({"device_action_id": "da1"}),
                json!(["manage", {"type": "device_action", "action": "delete", "device_action": {"id": "da1"}}]),
            ),
            (
                "duplicate_device_action",
                json!({"device_action_id": "da1"}),
                json!(["manage", {"type": "device_action", "action": "duplicate", "device_action": {"id": "da1"}}]),
            ),
            (
                "add_tsl_client",
                json!({"ip": "10.0.0.20", "port": 5727, "transport": "udp", "protocol": "3.1", "protocol_options": {}}),
                json!(["manage", {"type": "tsl_client", "action": "add", "tslClient": {"ip": "10.0.0.20", "port": 5727,
                    "transport": "udp", "protocol": "3.1", "protocolOptions": {}}}]),
            ),
            (
                "edit_tsl_client",
                json!({"tsl_client_id": "t1", "ip": "10.0.0.21", "port": 5727, "transport": "tcp", "protocol": "5.0", "protocol_options": {}}),
                json!(["manage", {"type": "tsl_client", "action": "edit", "tslClient": {"id": "t1", "ip": "10.0.0.21", "port": 5727,
                    "transport": "tcp", "protocol": "5.0", "protocolOptions": {}}}]),
            ),
            (
                "delete_tsl_client",
                json!({"tsl_client_id": "t1"}),
                json!(["manage", {"type": "tsl_client", "action": "delete", "tslClientId": "t1"}]),
            ),
            (
                "add_bus_option",
                json!({"label": "Aux 9", "type": "aux", "color": "#0000FF", "priority": 100, "visible": true}),
                json!(["manage", {"type": "bus_option", "action": "add", "busOption": {"label": "Aux 9", "type": "aux",
                    "color": "#0000FF", "visible": true, "priority": 100}}]),
            ),
            (
                "edit_bus_option",
                json!({"bus_id": "aux", "color": "#00FF00"}),
                json!(["manage", {"type": "bus_option", "action": "edit", "busOption": {"id": "aux", "label": "Aux 1", "type": "aux",
                    "color": "#00FF00", "priority": 100, "visible": true}}]),
            ),
            (
                "delete_bus_option",
                json!({"bus_id": "aux"}),
                json!(["manage", {"type": "bus_option", "action": "delete", "busOptionId": "aux"}]),
            ),
            (
                "add_cloud_destination",
                json!({"host": "10.0.0.50", "port": 4455, "key": "k"}),
                json!(["manage", {"type": "cloud_destination", "action": "add", "cloudDestination": {"host": "10.0.0.50", "port": 4455, "key": "k"}}]),
            ),
            (
                "edit_cloud_destination",
                json!({"cloud_destination_id": "c1", "host": "10.0.0.51", "port": 4455, "key": "k"}),
                json!(["manage", {"type": "cloud_destination", "action": "edit", "cloudDestination": {"id": "c1", "host": "10.0.0.51", "port": 4455, "key": "k"}}]),
            ),
            (
                "delete_cloud_destination",
                json!({"cloud_destination_id": "c1"}),
                json!(["manage", {"type": "cloud_destination", "action": "delete", "cloudId": "c1"}]),
            ),
            (
                "reconnect_cloud_destination",
                json!({"cloud_destination_id": "c1"}),
                json!(["cloud_destination_reconnect", "c1"]),
            ),
            (
                "disconnect_cloud_destination",
                json!({"cloud_destination_id": "c1"}),
                json!(["cloud_destination_disconnect", "c1"]),
            ),
            (
                "add_cloud_key",
                json!({"key": "k"}),
                json!(["manage", {"type": "cloud_key", "action": "add", "key": "k"}]),
            ),
            (
                "delete_cloud_key",
                json!({"key": "k"}),
                json!(["manage", {"type": "cloud_key", "action": "delete", "key": "k"}]),
            ),
            (
                "remove_cloud_client",
                json!({"cloud_client_id": "cc1"}),
                json!(["manage", {"type": "cloud_client", "action": "remove", "id": "cc1"}]),
            ),
            (
                "add_user",
                json!({"username": "pat", "password": "secret", "roles": "producer"}),
                json!(["manage", {"type": "user", "action": "add", "user": {"username": "pat", "password": "secret", "roles": "producer"}}]),
            ),
            (
                "edit_user",
                json!({"username": "pat", "roles": "producer;settings:testing"}),
                json!(["manage", {"type": "user", "action": "edit", "user": {"username": "pat", "roles": "producer;settings:testing"}}]),
            ),
            (
                "delete_user",
                json!({"username": "pat"}),
                json!(["manage", {"type": "user", "action": "delete", "user": {"username": "pat"}}]),
            ),
        ]
    }

    #[test]
    fn every_command_emits_its_documented_event() {
        let spec = crate::catalog::Catalog::source_tree();
        let spec = &spec.devices["tally-arbiter"];
        let table = table();
        let mut covered: Vec<&str> = table.iter().map(|(n, _, _)| *n).collect();
        covered.sort();
        let mut declared: Vec<&str> = spec.commands.keys().map(String::as_str).collect();
        declared.sort();
        assert_eq!(
            covered, declared,
            "the table covers every command in the spec"
        );
        for (i, (name, params, expected)) in table.into_iter().enumerate() {
            let (mut m, _) = admin();
            // Lists read in, as an edit fills from them.
            event(
                &mut m,
                5,
                "tsl_clients",
                json!([[{"id": "t1", "ip": "10.0.0.20", "port": 5727, "transport": "udp"}]]),
            );
            event(
                &mut m,
                5,
                "device_actions",
                json!([[{"id": "da1", "deviceId": "cam1", "busId": "pgm", "active": true, "outputTypeId": "osc", "data": {}}]]),
            );
            event(
                &mut m,
                5,
                "device_sources",
                json!([[{"id": "ds1", "deviceId": "cam1", "sourceId": "src1", "address": "1", "rename": false,
                "reconnect_interval": 5000, "max_reconnects": 5}]]),
            );
            event(
                &mut m,
                5,
                "sources",
                json!([[{"id": "src1", "name": "ATEM", "sourceTypeId": "ATEM", "enabled": true}]]),
            );
            event(
                &mut m,
                5,
                "cloud_destinations",
                json!([[{"id": "c1", "host": "10.0.0.50", "port": 4455, "key": "k"}]]),
            );
            let a = command(&mut m, 100, i as u64 + 1, name, params);
            assert!(
                !a.iter()
                    .any(|x| matches!(x, Action::Complete { result: Err(_), .. })),
                "{name}: {a:?}"
            );
            let first = sent(&a).into_iter().next().unwrap_or(Value::Null);
            assert_eq!(first, expected, "{name}");
        }
    }

    #[test]
    fn replies_complete_commands_with_their_value() {
        let (mut m, _) = admin();
        command(&mut m, 100, 7, "get_devices", json!({}));
        let a = event(
            &mut m,
            130,
            "devices",
            json!([[{"id": "cam1", "name": "Camera 1"}]]),
        );
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Value {
                value: json!([{"id": "cam1", "name": "Camera 1"}])
            })
        }));
        assert!(a.contains(&Action::RoundTrip(30)));

        // A manage change is answered in the settings room.
        command(
            &mut m,
            200,
            8,
            "duplicate_device",
            json!({"device_id": "cam1"}),
        );
        let a = event(
            &mut m,
            210,
            "manage_response",
            json!([{"result": "device-duplicated-successfully", "deviceId": "cam9"}]),
        );
        assert!(a.contains(&Action::Complete {
            id: 8,
            result: Ok(Outcome::Value {
                value: json!("cam9")
            })
        }));
        // And the changed lists are read back.
        assert_eq!(
            names(&a)[..2],
            ["devices".to_string(), "device_sources".to_string()]
        );

        command(
            &mut m,
            300,
            9,
            "add_bus_option",
            json!({"label": "A", "type": "aux", "color": "#000000", "priority": 1, "visible": true}),
        );
        let a = event(
            &mut m,
            310,
            "manage_response",
            json!([{"result": "error", "error": "Access denied. You are not authorized to do this."}]),
        );
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                id: 9,
                result: Err(CommandError::DeviceError { .. })
            }
        )));

        // delete_user is unconfirmed whatever Tally Arbiter says.
        command(&mut m, 400, 10, "delete_user", json!({"username": "pat"}));
        let a = event(
            &mut m,
            410,
            "manage_response",
            json!([{"result": "error", "error": "User not found."}]),
        );
        assert!(a.contains(&Action::Complete {
            id: 10,
            result: Ok(Outcome::Unverified)
        }));
        assert!(names(&a).contains(&"users".to_string()));

        // An error event fails the command waiting.
        command(&mut m, 500, 11, "get_users", json!({}));
        let a = event(
            &mut m,
            510,
            "error",
            json!(["Access denied. You are not authorized to do this."]),
        );
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                id: 11,
                result: Err(CommandError::DeviceError { .. })
            }
        )));
        assert_eq!(
            state(&a)["session"]["last_error"],
            "Access denied. You are not authorized to do this."
        );

        // Unconfirmed commands complete at once.
        let a = command(
            &mut m,
            600,
            12,
            "flash_listener",
            json!({"listener_client_id": "L1"}),
        );
        assert!(a.contains(&Action::Complete {
            id: 12,
            result: Ok(Outcome::Unverified)
        }));
    }

    #[test]
    fn an_unknown_id_in_a_read_list_is_refused_before_sending() {
        let (mut m, _) = admin();
        let a = command(
            &mut m,
            100,
            1,
            "delete_device",
            json!({"device_id": "nope"}),
        );
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                id: 1,
                result: Err(CommandError::InvalidParams { .. })
            }
        )));
        assert!(sent(&a).is_empty());
    }

    #[test]
    fn without_signing_in_tally_is_polled_and_the_listener_registered_by_name() {
        let mut m = ta(json!({"device_id": "Camera 2", "poll_ms": 1000}), true);
        let a = connect(&mut m);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let n = names(&a);
        assert!(n.contains(&"devices".to_string()));
        assert!(n.contains(&"device_states".to_string()));
        assert!(a.iter().any(|x| matches!(
            x,
            Action::SetTimer {
                key: STATES,
                after: 1000
            }
        )));
        let a = event(
            &mut m,
            5,
            "devices",
            json!([serde_json::from_str::<Value>(DEVICES).unwrap()]),
        );
        let connect = sent(&a)
            .into_iter()
            .find(|e| e[0] == "listenerclient_connect")
            .unwrap();
        assert_eq!(connect[1]["deviceId"], "cam2");
        assert_eq!(state(&a)["listener"]["device_id"], "cam2");
        let mut cx = Cx::new(1000);
        m.timer(&mut cx, STATES);
        assert_eq!(sent(&cx.take()), [json!(["device_states"])]);
        // Manage changes need the settings room.
        let a = command(
            &mut m,
            1100,
            3,
            "delete_device",
            json!({"device_id": "cam1"}),
        );
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                id: 3,
                result: Err(CommandError::DeviceError { .. })
            }
        )));
    }

    #[test]
    fn a_device_tally_arbiter_lacks_is_not_registered() {
        let mut m = ta(json!({"device_id": "cam7"}), true);
        connect(&mut m);
        let a = event(
            &mut m,
            5,
            "devices",
            json!([serde_json::from_str::<Value>(DEVICES).unwrap()]),
        );
        assert!(!names(&a).contains(&"listenerclient_connect".to_string()));
    }

    #[test]
    fn a_refused_settings_join_falls_back_to_the_producer_room() {
        let mut m = ta(json!({"username": "producer", "password": "pw"}), true);
        connect(&mut m);
        event(
            &mut m,
            3,
            "login_response",
            json!([{"loginOk": true, "message": "", "accessToken": "tok"}]),
        );
        let a = event(
            &mut m,
            4,
            "error",
            json!(["Access denied. You are not authorized to do this."]),
        );
        assert_eq!(sent(&a), [json!(["companion"])]);
        let a = event(&mut m, 5, "device_sources", json!([[]]));
        assert_eq!(state(&a)["session"]["access"], "producer");
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::CancelTimer { key: STATES })));
    }

    #[test]
    fn listener_events_are_kept_and_reported() {
        let (mut m, _) = admin();
        let a = event(&mut m, 10, "flash", json!(["L1"]));
        assert_eq!(state(&a)["listener"]["flashes"], 1);
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Message { address, .. } if address == "flash")));
        let a = event(
            &mut m,
            11,
            "messaging_client",
            json!(["producer", "p1", "Stand by"]),
        );
        let s = state(&a);
        assert_eq!(
            s["listener"]["last_message"],
            json!({"type": "producer", "from": "p1", "text": "Stand by"})
        );
        let a = event(&mut m, 12, "messaging", json!(["client", "x", "Ready"]));
        assert_eq!(state(&a)["chat"]["last"]["text"], "Ready");
        // Reassigned while in a room with the producer role.
        let a = event(&mut m, 13, "reassign", json!(["cam1", "cam2", "L1"]));
        assert_eq!(
            sent(&a),
            [
                json!(["listener_reassign", "cam1", "cam2"]),
                json!(["device_states", "cam2"])
            ]
        );
        assert_eq!(state(&a)["listener"]["device_id"], "cam2");
    }

    #[test]
    fn reassigned_without_a_role_registers_again_on_a_new_connection() {
        let mut m = ta(json!({"device_id": "cam1"}), true);
        connect(&mut m);
        event(
            &mut m,
            5,
            "devices",
            json!([serde_json::from_str::<Value>(DEVICES).unwrap()]),
        );
        let a = event(&mut m, 6, "reassign", json!(["cam1", "cam2", "x"]));
        assert!(a.iter().any(|x| matches!(x, Action::WsClose { .. })));
        assert!(a.iter().any(|x| matches!(x, Action::WsOpen { .. })));
        raw(&mut m, 7, r#"0{"sid":"e2"}"#);
        raw(&mut m, 8, r#"40{"sid":"s2"}"#);
        let a = event(
            &mut m,
            9,
            "devices",
            json!([serde_json::from_str::<Value>(DEVICES).unwrap()]),
        );
        let connect = sent(&a)
            .into_iter()
            .find(|e| e[0] == "listenerclient_connect")
            .unwrap();
        assert_eq!(connect[1]["deviceId"], "cam2");
    }

    #[test]
    fn a_refused_sign_in_is_terminal() {
        let mut m = ta(json!({"username": "admin", "password": "bad"}), true);
        connect(&mut m);
        let a = event(
            &mut m,
            3,
            "login_response",
            json!([{"loginOk": false, "message": "Wrong username or password!", "access_token": ""}]),
        );
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: RETRY, .. })));
        let a = command(&mut m, 4, 1, "get_version", json!({}));
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                result: Err(CommandError::Auth { .. }),
                ..
            }
        )));
        let mut cx = Cx::new(5);
        assert!(m.reconnect(&mut cx));
        assert!(!cx.take().iter().any(|x| matches!(x, Action::WsOpen { .. })));
    }

    #[test]
    fn an_expired_token_signs_in_again() {
        let (mut m, _) = admin();
        let a = event(&mut m, 10, "invalid_access_token", json!([]));
        assert_eq!(sent(&a), [json!(["login", "admin", "pw"])]);
        let a = event(
            &mut m,
            11,
            "login_response",
            json!([{"loginOk": true, "message": "", "accessToken": "tok2"}]),
        );
        assert_eq!(sent(&a), [json!(["access_token", "tok2"])]);
    }

    #[test]
    fn change_password_carries_on_with_the_new_token() {
        let mut m = ta(json!({"username": "admin", "password": "12345"}), true);
        connect(&mut m);
        event(
            &mut m,
            3,
            "login_response",
            json!([{"loginOk": true, "message": "", "accessToken": "tok"}]),
        );
        event(
            &mut m,
            4,
            "error",
            json!(["You must change your password before you can use this feature."]),
        );
        event(&mut m, 4, "must_change_password", json!([]));
        let a = event(
            &mut m,
            5,
            "error",
            json!(["You must change your password before you can use this feature."]),
        );
        assert_eq!(state(&a)["session"]["access"], Value::Null);
        assert_eq!(m.mirror["session"]["must_change_password"], true);
        command(
            &mut m,
            10,
            1,
            "change_password",
            json!({"current_password": "12345", "new_password": "longer-secret"}),
        );
        let a = event(
            &mut m,
            20,
            "change_password_response",
            json!([{"changeOk": true, "message": "", "accessToken": "tok3"}]),
        );
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                id: 1,
                result: Ok(Outcome::Value { .. })
            }
        )));
        assert_eq!(
            sent(&a),
            [json!(["access_token", "tok3"]), json!(["settings"])]
        );
        assert_eq!(m.password, "longer-secret");
    }

    #[test]
    fn engine_io_pings_are_answered_and_silence_drops_the_connection() {
        let (mut m, _) = admin();
        let a = raw(&mut m, 10, "2");
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::WsSend { text, .. } if text == "3")));
        // An answered liveness request resets the count.
        let mut cx = Cx::new(LIVENESS_EVERY);
        m.timer(&mut cx, LIVENESS);
        assert_eq!(sent(&cx.take()), [json!(["version"])]);
        let a = event(&mut m, LIVENESS_EVERY + 20, "version", json!(["3.3.0"]));
        assert!(a.contains(&Action::RoundTrip(20)));
        // Two unanswered in a row drop the connection.
        let mut at = 2 * LIVENESS_EVERY;
        for strike in 1..=STRIKES {
            let mut cx = Cx::new(at);
            m.timer(&mut cx, LIVENESS);
            at += REQUEST_TIMEOUT;
            let mut cx = Cx::new(at);
            m.timer(&mut cx, REQUEST);
            let a = cx.take();
            let dropped = a
                .iter()
                .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. })));
            assert_eq!(dropped, strike == STRIKES);
            if dropped {
                assert!(a.iter().any(|x| matches!(
                    x,
                    Action::SetTimer {
                        key: RETRY,
                        after: RETRY_MIN
                    }
                )));
            }
        }
    }

    #[test]
    fn opened_for_commands_only_it_registers_nothing_and_reads_nothing() {
        let mut m = ta(
            json!({"username": "admin", "password": "pw", "device_id": "cam1"}),
            false,
        );
        connect(&mut m);
        let a = event(
            &mut m,
            3,
            "login_response",
            json!([{"loginOk": true, "message": "", "accessToken": "tok"}]),
        );
        // The settings room is joined: changes are answered there.
        assert_eq!(
            sent(&a),
            [json!(["access_token", "tok"]), json!(["settings"])]
        );
        let a = event(
            &mut m,
            4,
            "initialdata",
            json!([
                [],
                [],
                {},
                [],
                [],
                serde_json::from_str::<Value>(BUSES).unwrap(),
                [],
                serde_json::from_str::<Value>(DEVICES).unwrap(),
                [],
                [],
                [],
                [],
                [],
                [],
                [],
                []
            ]),
        );
        assert!(sent(&a).is_empty(), "{:?}", names(&a));
        // What the room pushes is still applied.
        assert_eq!(state(&a)["devices"]["cam1"]["name"], "Camera 1");
        // Only the liveness request is ever sent unasked.
        let mut cx = Cx::new(LIVENESS_EVERY);
        m.timer(&mut cx, LIVENESS);
        assert_eq!(sent(&cx.take()), [json!(["version"])]);
        let mut cx = Cx::new(LIVENESS_EVERY + 1);
        m.timer(&mut cx, STATES);
        m.timer(&mut cx, LISTS);
        assert!(sent(&cx.take()).is_empty());
        // Commands work, with no read-back.
        command(
            &mut m,
            6000,
            4,
            "delete_device",
            json!({"device_id": "cam2"}),
        );
        let a = event(
            &mut m,
            6010,
            "manage_response",
            json!([{"result": "device-deleted-successfully"}]),
        );
        assert!(a.contains(&Action::Complete {
            id: 4,
            result: Ok(Outcome::Ack)
        }));
        assert!(sent(&a).is_empty());
    }

    #[test]
    fn reconnect_reopens_at_once() {
        let (mut m, _) = admin();
        let mut cx = Cx::new(50);
        assert!(m.reconnect(&mut cx));
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(x, Action::WsClose { .. })));
        assert!(a.iter().any(|x| matches!(x, Action::WsOpen { .. })));
        assert!(a.contains(&Action::Connection(Connection::Connecting)));
    }
}
