//! DMXControl 3 through Umbra, its network broker: gRPC over HTTP/2
//! (`crate::grpc`), protobuf messages (`crate::protobuf`, with the schema
//! `tools/generate_dmxcontrol3.py` writes from DMXControl's official network
//! interface repository into `dmxcontrol3_schema.rs`).
//!
//! The session follows DMXControl's reference connection client
//! (csharp/src/UmbraConnectionClient.cs) and the service definitions
//! (proto/Common/UmbraClientService.proto):
//! - when no network ID is set, `DMXCNetService.GetUmbraNetworkInfo` (which
//!   needs no login) reads Umbra's own, and Umbra's and its clients'
//!   information;
//! - `ClientService.Login` with this client's program information answers a
//!   session ID (NoError or AlreadyLoggedIn; a session ID holding anything
//!   outside printable ASCII is refused, as the reference client does);
//!   InvalidLoginData, AccessDenied and PERMISSION_DENIED are a refusal of
//!   the login (terminal: nothing is retried until the settings change);
//! - every later call carries the session ID in the metadata header the
//!   `session_header` setting names (`sessionid` by default: the repository
//!   does not name it, see the spec's quirks);
//! - `ConnectedClientService.ReportReadyToWork` once, then the bidirectional
//!   `Ping` call, kept open: a ping every 5 s with a rising counter and the
//!   client name, and every ping from another name answered with this
//!   client's name as responder. Umbra drops a client without it.
//!
//! Liveness is an HTTP/2 PING every 5 s, whose answer is the round-trip time;
//! no answer for 15 s drops the connection, and it is opened again after 1 s
//! doubling to 30 s.
//!
//! Monitored, the module reads every list the client services offer into
//! state collections keyed by ID (`COLLECTIONS`, generated with the schema),
//! opens each one's change stream, and keeps the singletons current: the
//! network's programs, the manual beat, the project's state, the global
//! functions, settings values, user contexts, command lines, power usage,
//! DMX interface states, cuelist progress, device property values (published
//! at most every 100 ms), DMX values when the settings name channels, input
//! values, procedures and programmers. Logs, notifications and broadcasts are
//! message events. What no stream reports (the global functions, the
//! monitoring counters, the project's unsaved changes and warnings) is read
//! every 5 s (every 10 s for the project). A change that carries no data is
//! read again; a stream that ends is opened again after 1 s, with its list.
//!
//! Opened for commands only, the module logs in, reports itself ready, keeps
//! the ping call (the session itself needs it) and its HTTP/2 PING, and opens
//! no stream and reads nothing else; command answers still reach the caller.
//!
//! Every command is one method (`METHODS`): its parameters are the request's
//! fields in snake_case, encoded with the schema, and its answer is decoded
//! the same way. Server-streaming and bidirectional commands collect every
//! message the server sends until it ends the call.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};

use base64::Engine;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use super::dmxcontrol3_schema::{COLLECTIONS, METHODS, SCHEMA};
use crate::catalog::Params;
use crate::grpc::{self, CallId, Client, Response, Status};
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    PollStamps, TcpInput, TlsTarget,
};

pub(crate) const PORT: u16 = 17475;
pub(crate) const TLS_PORT: u16 = 17476;
const SOCKET: Key = "umbra";
const RETRY: Key = "retry";
const TICK: Key = "tick";
const DEADLINE: Key = "deadline";
const RESTREAM: Key = "restream";
const FLUSH: Key = "flush";
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// Pings (HTTP/2 and Umbra's), and the reads no stream covers.
const TICK_EVERY: Millis = 5_000;
/// No HTTP/2 PING answered for this long drops the connection.
const SILENCE: Millis = 15_000;
/// How long the login may take, as in the reference client.
const LOGIN_TIMEOUT: Millis = 3_000;
const RESTREAM_AFTER: Millis = 1_000;
const FLUSH_EVERY: Millis = 100;

const CLIENT_SERVICE: &str = "/LumosProtobufCommon.ClientService";
const CONNECTED: &str = "/LumosProtobufCommon.ConnectedClientService";

/// One method a command calls (generated).
#[derive(Debug)]
pub(crate) struct Method {
    pub command: &'static str,
    pub path: &'static str,
    pub request: &'static str,
    pub response: &'static str,
    pub client_streaming: bool,
    pub server_streaming: bool,
}

/// One state collection (generated): `key.<id>` for each object of a list.
#[derive(Debug)]
pub(crate) struct Collection {
    pub key: &'static str,
    /// The method reading the whole list, its request and response, and
    /// the response's list field.
    pub list: Option<(&'static str, &'static str, &'static str, &'static str)>,
    /// The path of each object's ID within it.
    pub id: &'static [&'static str],
    pub changes: Option<Changes>,
}

/// How a collection's change stream reports changes.
#[derive(Debug)]
pub(crate) enum Changes {
    /// Each message names the object, carries its data, and says whether it
    /// was added, changed or removed.
    Data {
        path: &'static str,
        message: &'static str,
        id: &'static str,
        data: &'static str,
        change_type: Option<&'static str>,
    },
    /// The messages carry no data: the list is read again.
    Reread {
        path: &'static str,
        message: &'static str,
    },
}

/// A stream the module opens besides the collections', and what its
/// messages update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Special {
    ClientChanges,
    Cues,
    TimecodeState,
    ActionNodes,
    Beat,
    MainSwitches,
    InterfaceStates,
    PowerConsumption,
    SinkProxyValues,
    UserContexts,
    CommandLine,
    CommandLineHistory,
    Programmer,
    Procedures,
    Logs,
    Notifications,
    Settings,
}

/// The server-streaming change streams of the singletons: path, message.
const SPECIALS: &[(Special, &str, &str)] = &[
    (
        Special::ClientChanges,
        "/LumosProtobufCommon.ConnectedClientService/ReceiveClientChanges",
        "LumosProtobuf.UmbraClientInfoMessage",
    ),
    (
        Special::Cues,
        "/LumosProtobufClient.CueClient/ReceiveCueChanges",
        "LumosProtobuf.Cue.CueChangedMessage",
    ),
    (
        Special::TimecodeState,
        "/LumosProtobufClient.TimecodeClient/ReceiveTimecodeStateChanges",
        "LumosProtobuf.Timecode.TimecodeStateChangedMessage",
    ),
    (
        Special::ActionNodes,
        "/LumosProtobufClient.TimecodeClient/ReceiveActionNodeChanges",
        "LumosProtobuf.Timecode.ActionNodeChangedMessage",
    ),
    (
        Special::Beat,
        "/LumosProtobufClient.BeatClient/ReceiveManualBeatChanges",
        "LumosProtobuf.ManualBeatMessage",
    ),
    (
        Special::MainSwitches,
        "/LumosProtobufClient.MainSwitchClient/RecieveMainSwitchChanges",
        "LumosProtobuf.MainSwitchChangedMessage",
    ),
    (
        Special::InterfaceStates,
        "/LumosProtobufClient.DMXInterfaceManagementClient/ReceiveInterfaceStateChanges",
        "LumosProtobuf.DMXInterfaceStateChangedMessage",
    ),
    (
        Special::PowerConsumption,
        "/LumosProtobufClient.ElectricityClient/ReceivePowerSourceConsumptionChanges",
        "LumosProtobuf.PowerSourceConsumptionChangeMessage",
    ),
    (
        Special::SinkProxyValues,
        "/LumosProtobufClient.InputClient/ReceiveSinkProxyChanges",
        "LumosProtobuf.Input.InputSinkProxyValuesChangedMessage",
    ),
    (
        Special::UserContexts,
        "/LumosProtobufClient.UserClient/ReceiveUserContextChanges",
        "LumosProtobuf.User.UserContextChangedMessage",
    ),
    (
        Special::CommandLine,
        "/LumosProtobufClient.CommandLineClient/ReceiveCurrentCommandLineChanges",
        "LumosProtobuf.CurrentCommandLineChangedMessage",
    ),
    (
        Special::CommandLineHistory,
        "/LumosProtobufClient.CommandLineClient/ReceiveCommandLineHistoryChanges",
        "LumosProtobuf.CommandLineHistoryChangedMessage",
    ),
    (
        Special::Programmer,
        "/LumosProtobufClient.ProgrammerClient/ReceiveProgrammerChanges",
        "LumosProtobuf.ProgrammerChangeMessage",
    ),
    (
        Special::Procedures,
        "/LumosProtobufClient.DeviceClient/ReceiveProcedureExecutionChanges",
        "LumosProtobuf.ProcedureExecutionMessage",
    ),
    (
        Special::Logs,
        "/LumosProtobufClient.LogClient/ReceiveLogDataChanges",
        "LumosProtobuf.LogData",
    ),
    (
        Special::Notifications,
        "/LumosProtobufClient.LogClient/ReceiveUserNotifications",
        "LumosProtobuf.UserNotificationData",
    ),
    (
        Special::Settings,
        "/LumosProtobufClient.SettingsClient/ReceiveSettingChanges",
        "LumosProtobuf.SettingChangedMessage",
    ),
];

/// The bidirectional subscriptions: each is sent the IDs to watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Watch {
    CuelistProgress,
    PropertyValues,
    ConnectionSets,
    Dmx,
}

impl Watch {
    fn path(self) -> &'static str {
        match self {
            Watch::CuelistProgress => {
                "/LumosProtobufClient.CuelistClient/ReceiveCuelistProgressChanges"
            }
            Watch::PropertyValues => {
                "/LumosProtobufClient.DeviceClient/ReceiveDevicePropertyChanges"
            }
            Watch::ConnectionSets => {
                "/LumosProtobufClient.InputClient/ReceiveConnectionSetProcessedMessages"
            }
            Watch::Dmx => "/LumosProtobufClient.DMXClient/GetDmxChanges",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Watch::CuelistProgress => "LumosProtobuf.Cuelist.CuelistProgressStateChangeMessage",
            Watch::PropertyValues => "LumosProtobuf.DevicePropertyChangeMessage",
            Watch::ConnectionSets => "LumosProtobuf.Input.ConnectionSetProcessedMessage",
            Watch::Dmx => "LumosProtobuf.DMXResponse",
        }
    }
}

/// A singleton read on connecting or on a schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Read {
    NetworkInfo,
    Beat,
    BeatFactor,
    ProjectState,
    ProjectDirty,
    ProjectWarning,
    Blackout,
    Freeze,
    Fog,
    Highlight,
    DmxMapping,
    DmxStatistics,
    MonitorSlots,
    TransferStatistics,
    ItemListTypes,
    UserContext,
    CommandLine,
    CommandLineHistory,
    PowerUsage,
}

impl Read {
    /// The method, its request as JSON, and its response message.
    fn call(self) -> (&'static str, Value, &'static str) {
        let global = |f: &str| {
            (
                "/LumosProtobufClient.DeviceClient/GetSetGlobalFunction",
                json!({ f: if f == "highlight" { json!({}) } else if f == "fog_state" { json!(0) } else { json!(false) } }),
                "LumosProtobuf.GlobalFunctionRequestResponse",
            )
        };
        match self {
            Read::NetworkInfo => (
                "/LumosProtobufCommon.DMXCNetService/GetUmbraNetworkInfo",
                json!({}),
                "LumosProtobuf.UmbraNetworkInfoResponse",
            ),
            Read::Beat => (
                "/LumosProtobufClient.BeatClient/GetManualBeat",
                json!({}),
                "LumosProtobuf.ManualBeatIntervalRequestResponse",
            ),
            Read::BeatFactor => (
                "/LumosProtobufClient.BeatClient/GetManualBeatFactor",
                json!({}),
                "LumosProtobuf.ManualBeatFactorRequestResponse",
            ),
            Read::ProjectState => (
                "/LumosProtobufClient.ProjectClient/GetProjectState",
                json!({}),
                "LumosProtobuf.GetProjectStateRespose",
            ),
            Read::ProjectDirty => (
                "/LumosProtobufClient.ProjectClient/GetProjectDirty",
                json!({"quick_response": true}),
                "LumosProtobuf.ProjectDirtyResponse",
            ),
            Read::ProjectWarning => (
                "/LumosProtobufClient.ProjectClient/GetProjectWarning",
                json!({"quick_response": true}),
                "LumosProtobuf.ProjectWarningResponse",
            ),
            Read::Blackout => global("blackout"),
            Read::Freeze => global("freeze"),
            Read::Fog => global("fog_state"),
            Read::Highlight => global("highlight"),
            Read::DmxMapping => (
                "/LumosProtobufClient.DMXClient/GetDMXIn2OutMapping",
                json!({}),
                "LumosProtobuf.DMXIn2OutMappingRequestResponse",
            ),
            Read::DmxStatistics => (
                "/LumosProtobufClient.MonitoringClient/GetDMXStatistics",
                json!({}),
                "LumosProtobuf.DMXStatisticsResponse",
            ),
            Read::MonitorSlots => (
                "/LumosProtobufClient.MonitoringClient/GetMonitorSlots",
                json!({}),
                "LumosProtobuf.GetMonitorSlotsResponse",
            ),
            Read::TransferStatistics => (
                "/LumosProtobufClient.MonitoringClient/GetTransferStatistics",
                json!({}),
                "LumosProtobuf.TransferStatisticsResponse",
            ),
            Read::ItemListTypes => (
                "/LumosProtobufClient.ItemListClient/GetAvailableItemListTypes",
                json!({}),
                "LumosProtobuf.GetAvailableItemListTypesResponse",
            ),
            Read::UserContext => (
                "/LumosProtobufClient.UserClient/GetUserContext",
                json!({}),
                "LumosProtobuf.User.GetUserContextResponse",
            ),
            Read::CommandLine => (
                "/LumosProtobufClient.CommandLineClient/GetCurrentCommandLine",
                json!({}),
                "LumosProtobuf.GetCurrentCommandLineResponse",
            ),
            Read::CommandLineHistory => (
                "/LumosProtobufClient.CommandLineClient/GetCommandLineHistory",
                json!({}),
                "LumosProtobuf.GetCommandLineHistoryResponse",
            ),
            Read::PowerUsage => (
                "/LumosProtobufClient.ElectricityClient/GetPowerSourceUsages",
                json!({}),
                "LumosProtobuf.GetPowerSourceUsageResponse",
            ),
        }
    }
}

/// Read on connecting.
const READS_ON_CONNECT: &[Read] = &[
    Read::NetworkInfo,
    Read::Beat,
    Read::BeatFactor,
    Read::ProjectState,
    Read::ProjectDirty,
    Read::ProjectWarning,
    Read::Blackout,
    Read::Freeze,
    Read::Fog,
    Read::Highlight,
    Read::DmxMapping,
    Read::DmxStatistics,
    Read::MonitorSlots,
    Read::TransferStatistics,
    Read::ItemListTypes,
    Read::UserContext,
    Read::CommandLine,
    Read::CommandLineHistory,
    Read::PowerUsage,
];

/// Read every tick, and every other tick.
const READS_EVERY_TICK: &[Read] = &[
    Read::Blackout,
    Read::Freeze,
    Read::Fog,
    Read::Highlight,
    Read::DmxStatistics,
    Read::MonitorSlots,
    Read::TransferStatistics,
];
const READS_EVERY_OTHER_TICK: &[Read] = &[Read::ProjectDirty, Read::ProjectWarning];

/// What a successful command changes that no stream reports.
fn reads_after(command: &str) -> &'static [Read] {
    match command {
        "global_function" => &[Read::Blackout, Read::Freeze, Read::Fog, Read::Highlight],
        "set_manual_beat" | "learn_manual_beat" | "sync_manual_beat" => &[Read::Beat],
        "set_manual_beat_factor" => &[Read::BeatFactor],
        "set_dmx_in2out_mapping" => &[Read::DmxMapping],
        "load_project_version"
        | "close_project_version"
        | "save_project_version"
        | "import_project" => &[Read::ProjectState, Read::ProjectDirty, Read::ProjectWarning],
        "bind_user_context" | "destroy_user_context" => &[Read::UserContext],
        _ => &[],
    }
}

/// Collections read again after a successful command, by key.
fn lists_after(command: &str) -> &'static [&'static str] {
    match command {
        "add_dmx_interface"
        | "delete_dmx_interface"
        | "set_dmx_interface_enabled"
        | "set_dmx_interface_address"
        | "set_dmx_interface_comment"
        | "set_dmx_interface_parameter" => &["dmx_interfaces", "dmx_interfaces_available"],
        "set_plugin_enabled" => &["plugins"],
        "register_setting" => &["registered_settings"],
        "modify_attachable_preset" => &["attachables"],
        _ => &[],
    }
}

/// What a call is for.
#[derive(Debug)]
enum Purpose {
    /// Umbra's information before logging in, for its network ID.
    Discover,
    Login {
        sent_at: Millis,
    },
    Ready,
    Logoff,
    Ping,
    Broadcast,
    List {
        collection: usize,
        filtered: bool,
    },
    Changes {
        collection: usize,
    },
    Special(Special),
    Watch(Watch),
    Read(Read),
    Setting {
        path: String,
    },
    Command {
        id: CommandId,
        name: &'static str,
        response: &'static str,
        streaming: bool,
        collected: Vec<Value>,
        sent_at: Millis,
        deadline: Option<Millis>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Closed,
    /// Connecting, or logging in.
    Opening,
    Ready,
    /// The login was refused: nothing is sent until the settings change.
    Refused,
}

pub(crate) struct Dmx3 {
    host: IpAddr,
    host_name: Option<String>,
    port: u16,
    tls: bool,
    accept_invalid_certs: bool,
    client_name: String,
    network_id: String,
    session_header: String,
    timeout: Millis,
    monitor: bool,
    /// When each poll was last answered (`polled.<group>`, SPEC.md §8).
    stamps: PollStamps,
    watch_properties: bool,
    dmx_out: (i64, i64),
    dmx_in: (i64, i64),
    runtime_id: String,
    phase: Phase,
    client: Option<Client>,
    session: Option<String>,
    calls: HashMap<CallId, Purpose>,
    retry_after: Millis,
    last_pong: Millis,
    ping_counter: i64,
    ping_call: Option<CallId>,
    broadcast_call: Option<CallId>,
    watch_calls: HashMap<Watch, CallId>,
    ticks: u64,
    /// Each collection's objects as last published, by ID.
    mirror: Vec<BTreeMap<String, Value>>,
    /// Collections being read in full, and those to read again once that ends.
    reading: HashSet<usize>,
    reread: HashSet<usize>,
    /// Singletons published whole, by state path, to work out removals.
    published: HashMap<String, Value>,
    /// Streams to open again.
    restream: Vec<Purpose>,
    /// Streams the server does not offer (UNIMPLEMENTED), not opened again.
    unimplemented: HashSet<&'static str>,
    /// Property values waiting for the next flush.
    pending_values: Map<String, Value>,
    flush_armed: bool,
    next_request: u64,
}

fn text(s: &Params, key: &str, default: &str) -> String {
    s.get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn int(s: &Params, key: &str, default: i64) -> i64 {
    s.get(key).and_then(Value::as_i64).unwrap_or(default)
}

impl Dmx3 {
    pub(crate) fn new(ctx: OpenContext) -> Dmx3 {
        let s = &ctx.settings;
        let tls = s.get("scheme").and_then(Value::as_str) == Some("tls");
        let port = ctx.port.unwrap_or(if tls { TLS_PORT } else { PORT });
        let client_name = text(s, "client_name", "Meros");
        Dmx3 {
            host: ctx.host,
            host_name: ctx.host_name.clone(),
            port,
            tls,
            accept_invalid_certs: s
                .get("accept_invalid_certs")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            client_name,
            network_id: text(s, "network_id", ""),
            session_header: text(s, "session_header", "sessionid").to_ascii_lowercase(),
            timeout: int(s, "timeout_ms", 10_000).max(1) as Millis,
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            watch_properties: s
                .get("watch_property_values")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            dmx_out: (int(s, "dmx_output_start", 0), int(s, "dmx_output_stop", 0)),
            dmx_in: (int(s, "dmx_input_start", 0), int(s, "dmx_input_stop", 0)),
            runtime_id: String::new(),
            phase: Phase::Closed,
            client: None,
            session: None,
            calls: HashMap::new(),
            retry_after: RETRY_MIN,
            last_pong: 0,
            ping_counter: 0,
            ping_call: None,
            broadcast_call: None,
            watch_calls: HashMap::new(),
            ticks: 0,
            mirror: COLLECTIONS.iter().map(|_| BTreeMap::new()).collect(),
            reading: HashSet::new(),
            reread: HashSet::new(),
            published: HashMap::new(),
            restream: Vec::new(),
            unimplemented: HashSet::new(),
            pending_values: Map::new(),
            flush_armed: false,
            next_request: 0,
        }
    }

    fn authority(&self) -> String {
        let host = match (&self.host_name, self.host) {
            (Some(n), _) => n.clone(),
            (None, IpAddr::V6(a)) => format!("[{a}]"),
            (None, a) => a.to_string(),
        };
        format!("{host}:{}", self.port)
    }

    /// This client's program information, as Login and Logoff take it.
    fn program_info(&self) -> Value {
        json!({
            "program_info": {
                "program_name": "Meros integrations",
                "vendor": "Meros",
                "program_version": env!("CARGO_PKG_VERSION"),
            },
            "client_info": {
                "hostname": self.client_name,
                "type": "ExternalTool",
                "clientname": self.client_name,
                "networkid": self.network_id,
                "runtimeid": self.runtime_id,
                "state": { "ready_to_work": true, "receive_and_display_messages": false },
            },
        })
    }

    fn metadata(&self) -> Vec<(String, String)> {
        match &self.session {
            Some(id) => vec![(self.session_header.clone(), id.clone())],
            None => Vec::new(),
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        self.phase = Phase::Opening;
        self.session = None;
        self.client = Some(Client::new(self.authority(), self.tls));
        cx.connection(Connection::Connecting);
        let to = SocketAddr::new(self.host, self.port);
        if self.tls {
            cx.tcp_open_tls(
                SOCKET,
                TlsTarget {
                    to,
                    server_name: self
                        .host_name
                        .clone()
                        .unwrap_or_else(|| self.host.to_string()),
                    accept_invalid_certs: self.accept_invalid_certs,
                    preamble: None,
                    alpn: vec![grpc::ALPN.to_string()],
                },
            );
        } else {
            cx.tcp_open(SOCKET, to);
        }
    }

    fn flush(&mut self, cx: &mut Cx) {
        if let Some(client) = &mut self.client {
            let out = client.take_output();
            if !out.is_empty() {
                cx.tcp_send(SOCKET, out);
            }
        }
    }

    /// Start a call sending one message and closing its side.
    fn call(
        &mut self,
        path: &str,
        request: &str,
        value: &Value,
        purpose: Purpose,
    ) -> Option<CallId> {
        let bytes = match SCHEMA.encode(request, value) {
            Ok(b) => b,
            Err(e) => {
                // The module's own requests are fixed; this is a defect.
                debug_assert!(false, "{path}: {e}");
                return None;
            }
        };
        let metadata = self.metadata();
        let client = self.client.as_mut()?;
        let call = client.unary(path, &metadata, bytes);
        self.calls.insert(call, purpose);
        Some(call)
    }

    /// Open a bidirectional call, sending nothing yet.
    fn open_call(&mut self, path: &str, purpose: Purpose) -> Option<CallId> {
        let metadata = self.metadata();
        let client = self.client.as_mut()?;
        let call = client.start(path, &metadata);
        self.calls.insert(call, purpose);
        Some(call)
    }

    fn send_on(&mut self, call: CallId, message: &str, value: &Value) {
        if let (Ok(bytes), Some(client)) = (SCHEMA.encode(message, value), self.client.as_mut()) {
            client.send(call, bytes);
        }
    }

    fn connected(&mut self, cx: &mut Cx) {
        self.last_pong = cx.now();
        if self.network_id.is_empty() {
            let (path, request, _) = Read::NetworkInfo.call();
            self.call(
                path,
                "LumosProtobuf.GetRequest",
                &request,
                Purpose::Discover,
            );
        } else {
            self.login(cx);
        }
        cx.set_timer(TICK, TICK_EVERY);
        self.flush(cx);
    }

    fn login(&mut self, cx: &mut Cx) {
        let request = json!({ "client": self.program_info() });
        let path = format!("{CLIENT_SERVICE}/Login");
        self.call(
            &path,
            "LumosProtobuf.UmbraLoginRequest",
            &request,
            Purpose::Login { sent_at: cx.now() },
        );
    }

    /// The login was answered with a session.
    fn logged_in(&mut self, cx: &mut Cx, session: String, umbra: Option<&Value>) {
        self.session = Some(session);
        self.phase = Phase::Ready;
        self.retry_after = RETRY_MIN;
        cx.connection(Connection::Connected);
        cx.alive();
        let mut patch = json!({
            "session": {
                "client_name": self.client_name,
                "runtime_id": self.runtime_id,
                "network_id": self.network_id,
            }
        });
        if let Some(info) = umbra {
            patch["umbra"] = umbra_state(info);
        }
        cx.state(patch);
        let ready =
            json!({ "state": { "ready_to_work": true, "receive_and_display_messages": false } });
        self.call(
            &format!("{CONNECTED}/ReportReadyToWork"),
            "LumosProtobuf.UmbraClientReadyToWorkNotification",
            &ready,
            Purpose::Ready,
        );
        self.ping_call = self.open_call(&format!("{CONNECTED}/Ping"), Purpose::Ping);
        self.send_ping();
        if self.monitor {
            self.monitor_all();
        }
    }

    fn send_ping(&mut self) {
        if let Some(call) = self.ping_call {
            self.ping_counter += 1;
            let ping =
                json!({ "request_counter": self.ping_counter, "clientname": self.client_name });
            self.send_on(call, "LumosProtobuf.PingPong", &ping);
        }
    }

    /// Read everything and open every stream.
    fn monitor_all(&mut self) {
        self.broadcast_call =
            self.open_call(&format!("{CONNECTED}/SendBroadcast"), Purpose::Broadcast);
        for index in 0..COLLECTIONS.len() {
            self.read_list(index);
            self.open_changes(index);
        }
        for (special, _, _) in SPECIALS {
            self.open_special(*special);
        }
        for read in READS_ON_CONNECT {
            self.read(*read);
        }
        if self.dmx_out.1 > 0 || self.dmx_in.1 > 0 {
            self.subscribe(Watch::Dmx);
        }
    }

    fn read(&mut self, read: Read) {
        let (path, request, _) = read.call();
        let message = request_of(path);
        self.call(path, message, &request, Purpose::Read(read));
    }

    fn read_list(&mut self, index: usize) {
        let Some((path, request, _, _)) = COLLECTIONS[index].list else {
            return;
        };
        if !self.reading.insert(index) {
            self.reread.insert(index);
            return;
        }
        self.call(
            path,
            request,
            &json!({}),
            Purpose::List {
                collection: index,
                filtered: false,
            },
        );
    }

    /// Read some objects of a collection again, or the whole list when its
    /// request takes no filter.
    fn read_ids(&mut self, index: usize, ids: Vec<String>) {
        let Some((path, request, _, _)) = COLLECTIONS[index].list else {
            return;
        };
        let filters = SCHEMA
            .message(request)
            .is_some_and(|m| m.fields.iter().any(|f| f.json == "id_filter"));
        if !filters || ids.is_empty() {
            self.read_list(index);
            return;
        }
        self.call(
            path,
            request,
            &json!({ "id_filter": ids }),
            Purpose::List {
                collection: index,
                filtered: true,
            },
        );
    }

    fn open_changes(&mut self, index: usize) {
        let path = match &COLLECTIONS[index].changes {
            Some(Changes::Data { path, .. }) | Some(Changes::Reread { path, .. }) => *path,
            None => return,
        };
        if self.unimplemented.contains(path) {
            return;
        }
        self.call(
            path,
            request_of(path),
            &json!({}),
            Purpose::Changes { collection: index },
        );
    }

    fn open_special(&mut self, special: Special) {
        let (_, path, _) = SPECIALS
            .iter()
            .find(|(s, _, _)| *s == special)
            .copied()
            .expect("listed");
        if self.unimplemented.contains(path) {
            return;
        }
        self.call(
            path,
            request_of(path),
            &json!({}),
            Purpose::Special(special),
        );
    }

    /// Send a bidirectional subscription its current IDs, opening it first.
    fn subscribe(&mut self, watch: Watch) {
        if !self.monitor || self.phase != Phase::Ready || self.unimplemented.contains(watch.path())
        {
            return;
        }
        let (message, value) = match watch {
            Watch::CuelistProgress => (
                "LumosProtobuf.Cuelist.CuelistProgressStateChangeRequest",
                json!({ "cuelist_ids": self.ids_of("cuelists") }),
            ),
            Watch::ConnectionSets => (
                "LumosProtobuf.Input.ConnectionSetProcessedRequest",
                json!({ "connection_set_ids": self.ids_of("connection_sets") }),
            ),
            Watch::PropertyValues => {
                if !self.watch_properties {
                    return;
                }
                (
                    "LumosProtobuf.DevicePropertyChangeRequest",
                    json!({ "device_property_ids": self.property_ids() }),
                )
            }
            Watch::Dmx => (
                "LumosProtobuf.GetDMXRequest",
                json!({ "dmx_out_start": self.dmx_out.0, "dmx_out_stop": self.dmx_out.1,
                        "dmx_in_start": self.dmx_in.0, "dmx_in_stop": self.dmx_in.1 }),
            ),
        };
        let call = match self.watch_calls.get(&watch) {
            Some(call) => *call,
            None => match self.open_call(watch.path(), Purpose::Watch(watch)) {
                Some(call) => {
                    self.watch_calls.insert(watch, call);
                    call
                }
                None => return,
            },
        };
        self.send_on(call, message, &value);
    }

    fn collection(key: &str) -> usize {
        COLLECTIONS
            .iter()
            .position(|c| c.key == key)
            .expect("a generated collection")
    }

    fn ids_of(&self, key: &str) -> Vec<String> {
        self.mirror[Self::collection(key)].keys().cloned().collect()
    }

    /// The IDs of every property of every device's beams.
    fn property_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for device in self.mirror[Self::collection("devices")].values() {
            for beam in device["beams"].as_array().into_iter().flatten() {
                for property in beam["properties"].as_array().into_iter().flatten() {
                    if let Some(id) = property["id"].as_str().filter(|id| !id.is_empty()) {
                        ids.push(id.to_string());
                    }
                }
            }
        }
        ids.sort();
        ids.dedup();
        ids
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if self.phase == Phase::Closed || self.phase == Phase::Refused {
            return;
        }
        cx.tcp_close(SOCKET);
        self.fail_calls(cx, "the connection to Umbra was lost");
        self.phase = Phase::Closed;
        self.session = None;
        cx.cancel_timer(TICK);
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn refuse(&mut self, cx: &mut Cx, reason: String) {
        cx.tcp_close(SOCKET);
        self.fail_calls(cx, &reason);
        self.phase = Phase::Refused;
        self.session = None;
        cx.cancel_timer(TICK);
        cx.cancel_timer(RETRY);
        cx.connection(Connection::Unauthorized { reason });
    }

    fn fail_calls(&mut self, cx: &mut Cx, message: &str) {
        if let Some(mut client) = self.client.take() {
            client.fail_all(message);
        }
        let mut commands: Vec<CommandId> = self
            .calls
            .drain()
            .filter_map(|(_, p)| match p {
                Purpose::Command { id, .. } => Some(id),
                _ => None,
            })
            .collect();
        commands.sort_unstable();
        for id in commands {
            cx.complete(
                id,
                Err(CommandError::Transport {
                    message: message.to_string(),
                }),
            );
        }
        self.ping_call = None;
        self.broadcast_call = None;
        self.watch_calls.clear();
        self.reading.clear();
        self.reread.clear();
        self.restream.clear();
        self.pending_values.clear();
        self.flush_armed = false;
    }

    fn arm_deadline(&self, cx: &mut Cx) {
        let next = self
            .calls
            .values()
            .filter_map(|p| match p {
                Purpose::Command { deadline, .. } => *deadline,
                Purpose::Login { sent_at } => Some(sent_at + LOGIN_TIMEOUT),
                _ => None,
            })
            .min();
        match next {
            Some(at) => cx.set_timer(DEADLINE, at.saturating_sub(cx.now()).max(1)),
            None => cx.cancel_timer(DEADLINE),
        }
    }

    fn deadlines(&mut self, cx: &mut Cx) {
        let now = cx.now();
        let due: Vec<CallId> = self
            .calls
            .iter()
            .filter(|(_, p)| match p {
                Purpose::Command {
                    deadline: Some(at), ..
                } => *at <= now,
                Purpose::Login { sent_at } => sent_at + LOGIN_TIMEOUT <= now,
                _ => false,
            })
            .map(|(c, _)| *c)
            .collect();
        for call in due {
            if let Some(client) = &mut self.client {
                client.cancel(call);
            }
            match self.calls.remove(&call) {
                Some(Purpose::Command { id, .. }) => cx.complete(id, Err(CommandError::Timeout)),
                Some(Purpose::Login { .. }) => {
                    self.lost(cx, "Umbra did not answer the login".into());
                    return;
                }
                _ => {}
            }
        }
        self.arm_deadline(cx);
        self.flush(cx);
    }

    fn received(&mut self, cx: &mut Cx, data: &[u8]) {
        let responses = match self.client.as_mut().map(|c| c.receive(data)) {
            Some(Ok(r)) => r,
            Some(Err(e)) => {
                self.lost(cx, format!("HTTP/2: {e}"));
                return;
            }
            None => return,
        };
        cx.alive();
        for response in responses {
            if self.client.is_none() {
                break;
            }
            match response {
                Response::Ready | Response::Headers { .. } => {}
                Response::Pong(payload) => {
                    let sent = u64::from_be_bytes(payload);
                    self.last_pong = cx.now();
                    cx.round_trip(cx.now().saturating_sub(sent));
                }
                Response::GoingAway { debug, .. } => {
                    self.lost(
                        cx,
                        format!("Umbra is closing the connection {debug}")
                            .trim()
                            .to_string(),
                    );
                    return;
                }
                Response::Message { call, message } => self.message(cx, call, &message),
                Response::Closed { call, status } => self.closed(cx, call, status),
            }
        }
        self.arm_deadline(cx);
        self.flush(cx);
    }

    fn message(&mut self, cx: &mut Cx, call: CallId, bytes: &[u8]) {
        let Some(purpose) = self.calls.get_mut(&call) else {
            return;
        };
        let message = match purpose_message(purpose) {
            Some(m) => m,
            None => return,
        };
        let value = match SCHEMA.decode(message, bytes) {
            Ok(v) => v,
            Err(e) => {
                cx.log(
                    Level::Warning,
                    format!("a {message} that does not decode: {e}"),
                );
                return;
            }
        };
        match purpose {
            Purpose::Command { collected, .. } => {
                collected.push(value);
            }
            Purpose::Discover => {
                let id = value["umbra_server"]["client_info"]["networkid"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                self.network_id = id;
                self.network_info(cx, &value);
            }
            Purpose::Login { sent_at } => {
                let sent_at = *sent_at;
                self.calls.remove(&call);
                self.login_answer(cx, sent_at, &value);
            }
            Purpose::Ping => {
                if value["clientname"].as_str() != Some(self.client_name.as_str()) {
                    let mut pong = value.clone();
                    pong["responder"] = json!(self.client_name);
                    self.send_on(call, "LumosProtobuf.PingPong", &pong);
                }
            }
            Purpose::Broadcast => cx.message(
                "broadcast",
                None,
                value,
                SocketAddr::new(self.host, self.port),
            ),
            Purpose::List {
                collection,
                filtered,
            } => {
                let (index, filtered) = (*collection, *filtered);
                self.list_answer(cx, index, filtered, &value);
            }
            Purpose::Changes { collection } => {
                let index = *collection;
                self.change(cx, index, &value);
            }
            Purpose::Special(special) => {
                let special = *special;
                self.special(cx, special, value);
            }
            Purpose::Watch(watch) => {
                let watch = *watch;
                self.watched(cx, watch, value);
            }
            Purpose::Read(read) => {
                let read = *read;
                self.read_answer(cx, read, &value);
                if self.monitor
                    && (READS_EVERY_TICK.contains(&read) || READS_EVERY_OTHER_TICK.contains(&read))
                {
                    self.stamps.answered(cx, "status");
                }
            }
            Purpose::Setting { path } => {
                let path = path.clone();
                if let Some(v) = setting_value(&value) {
                    self.set_whole(
                        cx,
                        &["settings_values", &path],
                        json!({ "value": v, "registered": value["registered"] }),
                    );
                }
            }
            Purpose::Ready | Purpose::Logoff => {}
        }
    }

    fn closed(&mut self, cx: &mut Cx, call: CallId, status: Status) {
        let Some(purpose) = self.calls.remove(&call) else {
            return;
        };
        match purpose {
            Purpose::Command {
                id,
                name,
                response,
                streaming,
                collected,
                sent_at,
                ..
            } => {
                if !streaming {
                    cx.round_trip(cx.now().saturating_sub(sent_at));
                }
                let result = command_result(response, streaming, collected, &status);
                if result.is_ok() {
                    for read in reads_after(name) {
                        self.read(*read);
                    }
                    for key in lists_after(name) {
                        self.read_list(Self::collection(key));
                    }
                }
                if !status.ok()
                    && matches!(
                        status.code,
                        grpc::code::UNAUTHENTICATED | grpc::code::PERMISSION_DENIED
                    )
                {
                    cx.log(
                        Level::Warning,
                        format!(
                            "Umbra refused {name} ({status}); if every call is refused, check the session_header setting ('{}')",
                            self.session_header
                        ),
                    );
                }
                cx.complete(id, result);
            }
            Purpose::Discover => {
                if !status.ok() {
                    cx.log(Level::Info, format!("Umbra's network information could not be read ({status}); logging in with an empty network ID"));
                }
                self.login(cx);
            }
            Purpose::Login { .. } => {
                if matches!(
                    status.code,
                    grpc::code::PERMISSION_DENIED | grpc::code::UNAUTHENTICATED
                ) {
                    self.refuse(cx, format!("Umbra refused the login: {status}"));
                } else {
                    self.lost(cx, format!("the login failed: {status}"));
                }
            }
            Purpose::Ready => {
                if !status.ok() {
                    cx.log(
                        Level::Warning,
                        format!("ReportReadyToWork failed: {status}"),
                    );
                }
            }
            Purpose::Logoff => {}
            Purpose::List {
                collection,
                filtered,
            } => {
                if !filtered {
                    self.reading.remove(&collection);
                    if self.reread.remove(&collection) {
                        self.read_list(collection);
                    }
                }
                if !status.ok() && status.code != grpc::code::UNIMPLEMENTED {
                    cx.log(
                        Level::Info,
                        format!("reading {} failed: {status}", COLLECTIONS[collection].key),
                    );
                }
            }
            Purpose::Read(read) => {
                if !status.ok() && status.code != grpc::code::UNIMPLEMENTED {
                    cx.log(Level::Debug, format!("reading {read:?} failed: {status}"));
                }
            }
            Purpose::Setting { .. } => {}
            Purpose::Ping => {
                self.ping_call = None;
                self.stream_ended(
                    cx,
                    Purpose::Ping,
                    "/LumosProtobufCommon.ConnectedClientService/Ping",
                    &status,
                );
            }
            Purpose::Broadcast => {
                self.broadcast_call = None;
                self.stream_ended(
                    cx,
                    Purpose::Broadcast,
                    "/LumosProtobufCommon.ConnectedClientService/SendBroadcast",
                    &status,
                );
            }
            Purpose::Changes { collection } => {
                let path = match &COLLECTIONS[collection].changes {
                    Some(Changes::Data { path, .. }) | Some(Changes::Reread { path, .. }) => *path,
                    None => "",
                };
                self.stream_ended(cx, Purpose::Changes { collection }, path, &status);
            }
            Purpose::Special(special) => {
                let (_, path, _) = SPECIALS
                    .iter()
                    .find(|(s, _, _)| *s == special)
                    .copied()
                    .expect("listed");
                self.stream_ended(cx, Purpose::Special(special), path, &status);
            }
            Purpose::Watch(watch) => {
                self.watch_calls.remove(&watch);
                self.stream_ended(cx, Purpose::Watch(watch), watch.path(), &status);
            }
        }
    }

    /// A stream ended while the session is up: open it again in a second,
    /// unless the server does not offer it.
    fn stream_ended(&mut self, cx: &mut Cx, purpose: Purpose, path: &'static str, status: &Status) {
        if self.phase != Phase::Ready {
            return;
        }
        if status.code == grpc::code::UNIMPLEMENTED {
            if self.unimplemented.insert(path) {
                cx.log(
                    Level::Info,
                    format!("Umbra does not offer {path}; it is not opened again"),
                );
            }
            return;
        }
        if self.restream.is_empty() {
            cx.set_timer(RESTREAM, RESTREAM_AFTER);
        }
        self.restream.push(purpose);
    }

    fn reopen(&mut self) {
        for purpose in std::mem::take(&mut self.restream) {
            match purpose {
                Purpose::Ping => {
                    self.ping_call = self.open_call(&format!("{CONNECTED}/Ping"), Purpose::Ping);
                    self.send_ping();
                }
                Purpose::Broadcast => {
                    self.broadcast_call =
                        self.open_call(&format!("{CONNECTED}/SendBroadcast"), Purpose::Broadcast);
                }
                Purpose::Changes { collection } => {
                    self.open_changes(collection);
                    // What changed meanwhile is read with the list.
                    self.read_list(collection);
                }
                Purpose::Special(special) => self.open_special(special),
                Purpose::Watch(watch) => self.subscribe(watch),
                _ => {}
            }
        }
    }

    fn login_answer(&mut self, cx: &mut Cx, sent_at: Millis, value: &Value) {
        cx.round_trip(cx.now().saturating_sub(sent_at));
        let code = value["return_code"].as_str().unwrap_or("Error");
        let message = value["message"].as_str().unwrap_or("").to_string();
        let session = value["session_id"].as_str().unwrap_or("").to_string();
        match code {
            "NoError" | "AlreadyLoggedIn" => {
                if session.is_empty() || !session.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
                    self.lost(
                        cx,
                        "Umbra answered the login with an invalid session ID".into(),
                    );
                    return;
                }
                self.logged_in(cx, session, value.get("umbra_server"));
            }
            "InvalidLoginData" | "AccessDenied" => {
                self.refuse(
                    cx,
                    format!("Umbra refused the login ({code}){}", suffix(&message)),
                );
            }
            other => self.lost(
                cx,
                format!(
                    "Umbra did not log this client in ({other}){}",
                    suffix(&message)
                ),
            ),
        }
    }

    /// Umbra's information and its clients'.
    fn network_info(&mut self, cx: &mut Cx, value: &Value) {
        let mut clients = Map::new();
        for (list, connected) in [("known_clients", false), ("connected_clients", true)] {
            for info in value[list].as_array().into_iter().flatten() {
                if let Some((id, mut state)) = client_state(info) {
                    state["connected"] = json!(connected);
                    if connected {
                        state["logoff_reason"] = json!("none");
                    }
                    clients.insert(id, state);
                }
            }
        }
        let mut patch = json!({ "umbra": umbra_state(&value["umbra_server"]) });
        if !value["umbra_server"].is_object() {
            patch = json!({});
        }
        cx.state(patch);
        self.set_whole(cx, &["clients"], Value::Object(clients));
    }

    fn list_answer(&mut self, cx: &mut Cx, index: usize, filtered: bool, value: &Value) {
        let c = &COLLECTIONS[index];
        let Some((_, _, _, field)) = c.list else {
            return;
        };
        let mut items = BTreeMap::new();
        for item in value[field].as_array().into_iter().flatten() {
            if let Some(id) = id_of(item, c.id) {
                items.insert(id, item.clone());
            }
        }
        if c.key == "projects" {
            if let Some(current) = value.get("current_loaded_project_version") {
                self.set_whole(cx, &["project", "current_version"], current.clone());
            }
        }
        let mut patch = Map::new();
        if filtered {
            for (id, item) in items {
                if let Some(d) = diff(self.mirror[index].get(&id), &item) {
                    patch.insert(id.clone(), d);
                }
                self.mirror[index].insert(id, item);
            }
        } else {
            let old = std::mem::take(&mut self.mirror[index]);
            for id in old.keys() {
                if !items.contains_key(id) {
                    patch.insert(id.clone(), Value::Null);
                }
            }
            for (id, item) in &items {
                if let Some(d) = diff(old.get(id), item) {
                    patch.insert(id.clone(), d);
                }
            }
            self.mirror[index] = items;
        }
        let first = !self.published.contains_key(c.key);
        if !patch.is_empty() || first {
            self.published.insert(c.key.to_string(), Value::Bool(true));
            if patch.is_empty() {
                cx.state(json!({ c.key: {} }));
            } else {
                cx.state(json!({ c.key: patch }));
            }
        }
        match c.key {
            "cuelists" => self.subscribe(Watch::CuelistProgress),
            "connection_sets" => self.subscribe(Watch::ConnectionSets),
            "devices" => self.subscribe(Watch::PropertyValues),
            "registered_settings" if !filtered => {
                let paths: Vec<String> = self.mirror[index].keys().cloned().collect();
                for path in paths {
                    let request = json!({ "path": path, "registered": true });
                    self.call(
                        "/LumosProtobufClient.SettingsClient/GetSetting",
                        "LumosProtobuf.GetSettingRequest",
                        &request,
                        Purpose::Setting { path },
                    );
                }
            }
            _ => {}
        }
    }

    fn set_item(&mut self, cx: &mut Cx, index: usize, id: String, item: Option<Value>) {
        let key = COLLECTIONS[index].key;
        match item {
            None => {
                if self.mirror[index].remove(&id).is_some() {
                    cx.state(json!({ key: { id: null } }));
                }
            }
            Some(item) => {
                if let Some(d) = diff(self.mirror[index].get(&id), &item) {
                    cx.state(json!({ key: { id.clone(): d } }));
                }
                self.mirror[index].insert(id, item);
            }
        }
        match key {
            "cuelists" => self.subscribe(Watch::CuelistProgress),
            "connection_sets" => self.subscribe(Watch::ConnectionSets),
            "devices" => self.subscribe(Watch::PropertyValues),
            _ => {}
        }
    }

    fn change(&mut self, cx: &mut Cx, index: usize, value: &Value) {
        match &COLLECTIONS[index].changes {
            Some(Changes::Data {
                id,
                data,
                change_type,
                ..
            }) => {
                let Some(object) = value[*id]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                else {
                    return;
                };
                let removed = change_type.is_some_and(|t| value[t].as_str() == Some("Removed"));
                if removed {
                    self.set_item(cx, index, object, None);
                } else if value[*data].is_object() {
                    self.set_item(cx, index, object, Some(value[*data].clone()));
                } else {
                    // A change without the object's data: read it again.
                    self.read_ids(index, vec![object]);
                }
            }
            Some(Changes::Reread { .. }) => self.read_list(index),
            None => {}
        }
    }

    fn special(&mut self, cx: &mut Cx, special: Special, value: Value) {
        let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();
        match special {
            Special::ClientChanges => {
                if let Some((id, mut state)) = client_state(&value["client"]) {
                    match (value.get("login"), value.get("logoff")) {
                        (Some(Value::Bool(true)), _) => {
                            state["connected"] = json!(true);
                            state["logoff_reason"] = json!("none");
                        }
                        (_, Some(reason)) if reason.as_str() != Some("ELogoffReason_Unknown") => {
                            state["connected"] = json!(false);
                            state["logoff_reason"] = reason.clone();
                        }
                        _ => {}
                    }
                    self.merge_whole(cx, &["clients", &id], state);
                }
            }
            Special::Cues => {
                let parent = s(&value, "parent_container_id");
                if !parent.is_empty() {
                    self.read_ids(Self::collection("cuelists"), vec![parent]);
                }
            }
            Special::ActionNodes => {
                let track = s(&value, "track_id");
                if !track.is_empty() {
                    self.read_ids(Self::collection("tracks"), vec![track]);
                }
            }
            Special::TimecodeState => {
                let id = s(&value, "id");
                let index = Self::collection("timecodes");
                if let Some(item) = self.mirror[index].get(&id).cloned() {
                    let mut item = item;
                    item["time_elapsed"] = value["time_elapsed"].clone();
                    item["timecode_state"] = value["state"].clone();
                    self.set_item(cx, index, id, Some(item));
                }
            }
            Special::Beat => cx.state(json!({ "beat": {
                "count": value["beat"], "interval": value["interval"], "factor": value["factor"] } })),
            Special::MainSwitches => {
                let id = s(&value, "main_switch_id");
                let index = Self::collection("main_switches");
                let mut item = self.mirror[index].get(&id).cloned().unwrap_or_else(|| json!({ "main_switch_id": id }));
                item["enabled_state"] = value["enabled_state"].clone();
                self.set_item(cx, index, id, Some(item));
            }
            Special::InterfaceStates => {
                let id = s(&value, "interface_id");
                if id.is_empty() {
                    return;
                }
                let mut fields = Map::new();
                for k in ["working", "enabled", "available"] {
                    if let Some(v) = value.get(k) {
                        fields.insert(k.into(), v.clone());
                    }
                }
                let port = value["port"].as_i64().unwrap_or(0).to_string();
                let mut patch = json!({ "rdm": value["rdm"] });
                match value["type"].as_str() {
                    Some("Input") => patch["inputs"] = json!({ port: fields }),
                    Some("Output") => patch["outputs"] = json!({ port: fields }),
                    _ => patch["interface"] = Value::Object(fields),
                }
                cx.state(json!({ "dmx_interface_states": { id: patch } }));
            }
            Special::PowerConsumption => cx.state(json!({ "power_usage": {
                "amps": value["changed_consumption_amps"], "watts": value["changed_consumption_watts"] } })),
            Special::SinkProxyValues => {
                let mut patch = Map::new();
                for v in value["changed_values"].as_array().into_iter().flatten() {
                    if let Some(id) = v["input_sink_id"].as_str() {
                        patch.insert(id.to_string(), v["new_value"].clone());
                    }
                }
                if !patch.is_empty() {
                    cx.state(json!({ "input_sink_values": patch }));
                }
            }
            Special::UserContexts => {
                let data = &value["user_context_data"];
                let id = s(data, "id");
                if !id.is_empty() {
                    let mut data = data.clone();
                    if let Some(o) = data.as_object_mut() {
                        o.remove("id");
                    }
                    self.set_whole(cx, &["user_contexts", &id], data);
                }
            }
            Special::CommandLine => {
                let id = s(&value, "user_context_id");
                cx.state(json!({ "command_lines": { id: { "current": value["new_current_command_line"] } } }));
            }
            Special::CommandLineHistory => {
                let id = s(&value, "user_context_id");
                cx.state(json!({ "command_lines": { id: { "history": value["new_history"] } } }));
            }
            Special::Programmer => {
                let id = s(&value, "user_context_id");
                let mut patch = json!({ "last_change": value });
                match value["type"].as_str() {
                    Some("OutputMode") => patch["output_mode"] = value["mode"].clone(),
                    Some("Snapshots") => patch["snapshots"] = value["snapshots"].clone(),
                    _ => {}
                }
                self.merge_whole(cx, &["programmers", &id], patch);
                cx.message("programmer_change", None, value, SocketAddr::new(self.host, self.port));
            }
            Special::Procedures => {
                let id = s(&value["procedure"], "id");
                if !id.is_empty() {
                    self.set_whole(cx, &["procedure_executions", &id], value);
                }
            }
            Special::Logs => cx.message("log", None, value, SocketAddr::new(self.host, self.port)),
            Special::Notifications => cx.message("notification", None, value, SocketAddr::new(self.host, self.port)),
            Special::Settings => {
                let path = s(&value, "settings_path");
                if !path.is_empty() {
                    let v = object_value(&value["new_value"]);
                    self.set_whole(cx, &["settings_values", &path], json!({
                        "value": v, "type": value["type"], "registered": value["registered"] }));
                }
            }
        }
    }

    fn watched(&mut self, cx: &mut Cx, watch: Watch, value: Value) {
        match watch {
            Watch::CuelistProgress => {
                let id = value["cuelist_id"].as_str().unwrap_or("").to_string();
                if !id.is_empty() {
                    self.set_whole(cx, &["cuelist_progress", &id], json!({
                        "cuelist_state": value["cuelist_state"], "cues": value["cue_progress_state"] }));
                }
            }
            Watch::ConnectionSets => {
                let id = value["connection_set_id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if !id.is_empty() {
                    cx.state(json!({ "connection_set_values": { id: value["values"] } }));
                }
            }
            Watch::PropertyValues => {
                for change in value["changes"].as_array().into_iter().flatten() {
                    let (Some(device), Some(property)) =
                        (change["device_id"].as_str(), change["property_id"].as_str())
                    else {
                        continue;
                    };
                    let mut v = change.clone();
                    if let Some(o) = v.as_object_mut() {
                        o.remove("device_id");
                        o.remove("property_id");
                    }
                    let entry = self
                        .pending_values
                        .entry(device.to_string())
                        .or_insert_with(|| Value::Object(Map::new()));
                    entry[property] = v;
                }
                if !self.flush_armed && !self.pending_values.is_empty() {
                    self.flush_armed = true;
                    cx.set_timer(FLUSH, FLUSH_EVERY);
                }
            }
            Watch::Dmx => {
                let bytes = value["dmx_values"]
                    .as_str()
                    .and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok())
                    .unwrap_or_default();
                let mut channels = Map::new();
                for (channel, v) in value["dmx_channels"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .zip(bytes)
                {
                    channels.insert(channel.to_string(), json!(v));
                }
                let key = if value["in_values"].as_bool() == Some(true) {
                    "input"
                } else {
                    "output"
                };
                if !channels.is_empty() {
                    cx.state(json!({ "dmx": { key: channels } }));
                }
            }
        }
    }

    fn read_answer(&mut self, cx: &mut Cx, read: Read, value: &Value) {
        match read {
            Read::NetworkInfo => self.network_info(cx, value),
            Read::Beat => {
                let mut beat = json!({ "interval": value["interval"] });
                if value["enabled_set"].as_bool() != Some(false) {
                    beat["enabled"] = value["enabled"].clone();
                }
                cx.state(json!({ "beat": beat }));
            }
            Read::BeatFactor => cx.state(json!({ "beat": { "factor": value["factor"] } })),
            Read::ProjectState => cx.state(json!({ "project": {
                "loaded": value["is_loaded"], "closed": value["is_closed"],
                "state": value["state"], "file_version": value["project_file_version"] } })),
            Read::ProjectDirty => {
                cx.state(json!({ "project": { "dirty": value["dirty"] } }));
                self.set_whole(cx, &["project", "dirty_where"], value["dirty_where"].clone());
            }
            Read::ProjectWarning => {
                cx.state(json!({ "project": { "warning": value["warning"] } }));
                self.set_whole(cx, &["project", "warning_where"], value["warning_where"].clone());
            }
            Read::Blackout => cx.state(json!({ "global": { "blackout": value.get("blackout").cloned().unwrap_or(json!(false)) } })),
            Read::Freeze => cx.state(json!({ "global": { "freeze": value.get("freeze").cloned().unwrap_or(json!(false)) } })),
            Read::Fog => cx.state(json!({ "global": { "fog_state": value.get("fog_state").cloned().unwrap_or(json!(0)) } })),
            Read::Highlight => {
                if let Some(h) = value.get("highlight") {
                    self.set_whole(cx, &["global", "highlight"], h.clone());
                }
            }
            Read::DmxMapping => self.set_whole(cx, &["dmx", "in2out_mapping"], value["mapping"].clone()),
            Read::DmxStatistics => cx.state(json!({ "dmx": {
                "values_per_second": value["dmx_values_per_second"], "frame_rate": value["dmx_frame_rate"] } })),
            Read::MonitorSlots => self.set_whole(cx, &["monitoring", "slots"], value["slots"].clone()),
            Read::TransferStatistics => self.set_whole(cx, &["monitoring", "transfer_statistics"], value["statistics"].clone()),
            Read::ItemListTypes => self.set_whole(cx, &["item_list_types"], value["available_item_list_types"].clone()),
            Read::UserContext => {
                let data = &value["user_context"];
                if let Some(id) = data["id"].as_str().filter(|s| !s.is_empty()) {
                    let mut data = data.clone();
                    if let Some(o) = data.as_object_mut() {
                        o.remove("id");
                    }
                    let id = id.to_string();
                    self.set_whole(cx, &["user_contexts", &id], data);
                }
            }
            Read::CommandLine => {
                let id = value["user_context_id"].as_str().unwrap_or("").to_string();
                cx.state(json!({ "command_lines": { id: { "current": value["current_command_line"] } } }));
            }
            Read::CommandLineHistory => {
                let id = value["user_context_id"].as_str().unwrap_or("").to_string();
                cx.state(json!({ "command_lines": { id: { "history": value["history"] } } }));
            }
            Read::PowerUsage => self.set_whole(cx, &["power_usage", "amps"], value["sum_usage_amps"].clone()),
        }
    }

    /// Publish a value at a path whole: what it no longer holds is removed.
    fn set_whole(&mut self, cx: &mut Cx, path: &[&str], value: Value) {
        let key = path.join("\u{1f}");
        let old = self.published.get(&key);
        let Some(d) = diff(old, &value) else {
            if old.is_none() {
                self.published.insert(key, value.clone());
                cx.state(nest(path, value));
            }
            return;
        };
        self.published.insert(key, value);
        cx.state(nest(path, d));
    }

    /// Merge fields into a value published whole.
    fn merge_whole(&mut self, cx: &mut Cx, path: &[&str], fields: Value) {
        let key = path.join("\u{1f}");
        let mut value = self
            .published
            .get(&key)
            .cloned()
            .unwrap_or_else(|| json!({}));
        if let (Some(o), Some(f)) = (value.as_object_mut(), fields.as_object()) {
            for (k, v) in f {
                o.insert(k.clone(), v.clone());
            }
        }
        self.set_whole(cx, path, value);
    }

    fn tick(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Ready && self.phase != Phase::Opening {
            return;
        }
        if cx.now().saturating_sub(self.last_pong) > SILENCE {
            self.lost(cx, "Umbra stopped answering".into());
            return;
        }
        cx.set_timer(TICK, TICK_EVERY);
        if let Some(client) = &mut self.client {
            client.ping(cx.now().to_be_bytes());
        }
        if self.phase == Phase::Ready {
            self.send_ping();
            if self.monitor {
                self.ticks += 1;
                for read in READS_EVERY_TICK {
                    self.read(*read);
                }
                if self.ticks.is_multiple_of(2) {
                    for read in READS_EVERY_OTHER_TICK {
                        self.read(*read);
                    }
                }
            }
        }
        self.arm_deadline(cx);
        self.flush(cx);
    }

    fn send_broadcast(&mut self, cx: &mut Cx, id: CommandId, params: &Params) {
        if self.broadcast_call.is_none() {
            self.broadcast_call =
                self.open_call(&format!("{CONNECTED}/SendBroadcast"), Purpose::Broadcast);
        }
        let Some(call) = self.broadcast_call else {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        };
        self.next_request += 1;
        let mut message = json!({
            "request_id": self.next_request.to_string(),
            "sender_name": self.client_name,
            "message_type": params.get("message_type").cloned().unwrap_or(json!("")),
            "receiver_type": params.get("receiver_type").cloned().unwrap_or(json!("EClientType_Unknown")),
            "receiver_hostnames": params.get("receiver_hostnames").cloned().unwrap_or(json!([])),
        });
        if let Some(text) = params
            .get("string_content")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
        {
            message["string_content"] = json!(text);
        }
        match SCHEMA.encode("LumosProtobuf.BroadcastMessage", &message) {
            Ok(bytes) => {
                if let Some(client) = &mut self.client {
                    client.send(call, bytes);
                }
                cx.complete(id, Ok(Outcome::Unverified));
            }
            Err(e) => cx.complete(id, Err(CommandError::InvalidParams { message: e })),
        }
        self.flush(cx);
    }
}

impl Module for Dmx3 {
    fn start(&mut self, cx: &mut Cx) {
        // A runtime ID of the UUID shape, new for each start.
        let digest = Sha256::digest(
            format!(
                "{}/{}/{}/{}",
                cx.unix_millis(),
                self.host,
                self.port,
                self.client_name
            )
            .as_bytes(),
        );
        let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
        self.runtime_id = format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        );
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.phase == Phase::Refused {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: "Umbra refused the login".into(),
                }),
            );
            return;
        }
        if self.phase != Phase::Ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        if name == "send_broadcast" {
            self.send_broadcast(cx, id, params);
            return;
        }
        let Ok(index) = METHODS.binary_search_by(|m| m.command.cmp(name)) else {
            cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.to_string(),
                }),
            );
            return;
        };
        let method = &METHODS[index];
        let request = match request_value(method.request, params) {
            Ok(v) => v,
            Err(message) => {
                cx.complete(id, Err(CommandError::InvalidParams { message }));
                return;
            }
        };
        let bytes = match SCHEMA.encode(method.request, &request) {
            Ok(b) => b,
            Err(message) => {
                cx.complete(id, Err(CommandError::InvalidParams { message }));
                return;
            }
        };
        let metadata = self.metadata();
        let Some(client) = self.client.as_mut() else {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        };
        let call = if method.client_streaming {
            let call = client.start(method.path, &metadata);
            client.send(call, bytes);
            client.close_send(call);
            call
        } else {
            client.unary(method.path, &metadata, bytes)
        };
        let streaming = method.server_streaming || method.client_streaming;
        self.calls.insert(
            call,
            Purpose::Command {
                id,
                name: method.command,
                response: method.response,
                streaming,
                collected: Vec::new(),
                sent_at: cx.now(),
                deadline: (!streaming).then(|| cx.now() + self.timeout),
            },
        );
        self.arm_deadline(cx);
        self.flush(cx);
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        if self.phase == Phase::Refused || self.phase == Phase::Closed {
            return;
        }
        match input {
            TcpInput::Connected => self.connected(cx),
            TcpInput::Data(data) => self.received(cx, &data),
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY if self.phase == Phase::Closed => self.open(cx),
            TICK => self.tick(cx),
            DEADLINE => self.deadlines(cx),
            RESTREAM if self.phase == Phase::Ready => {
                self.reopen();
                self.arm_deadline(cx);
                self.flush(cx);
            }
            FLUSH => {
                self.flush_armed = false;
                if !self.pending_values.is_empty() {
                    let values = std::mem::take(&mut self.pending_values);
                    cx.state(json!({ "property_values": values }));
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.phase == Phase::Ready {
            let request = json!({ "client": self.program_info() });
            self.call(
                &format!("{CLIENT_SERVICE}/Logoff"),
                "LumosProtobuf.UmbraLogoffRequest",
                &request,
                Purpose::Logoff,
            );
            if let Some(client) = &mut self.client {
                client.goaway();
            }
            self.flush(cx);
        }
        cx.tcp_close(SOCKET);
        self.phase = Phase::Closed;
    }
}

/// The request message of a module-made call, by its method.
fn request_of(path: &str) -> &'static str {
    match path {
        p if p.ends_with("/GetProjectDirty") => "LumosProtobuf.ProjectDirtyRequest",
        p if p.ends_with("/GetProjectWarning") => "LumosProtobuf.ProjectWarningRequest",
        p if p.ends_with("/GetSetGlobalFunction") => "LumosProtobuf.GlobalFunctionRequestResponse",
        p if p.ends_with("/GetUserContext") || p.ends_with("/GetPowerSourceUsages") => {
            "LumosProtobuf.GetMultipleRequest"
        }
        _ => "LumosProtobuf.GetRequest",
    }
}

/// The message a call's answers decode as.
fn purpose_message(purpose: &Purpose) -> Option<&'static str> {
    Some(match purpose {
        Purpose::Discover => "LumosProtobuf.UmbraNetworkInfoResponse",
        Purpose::Login { .. } => "LumosProtobuf.UmbraLoginResponse",
        Purpose::Ready => "LumosProtobuf.ConfirmedResponse",
        Purpose::Logoff => "LumosProtobuf.UmbraLogoffResponse",
        Purpose::Ping => "LumosProtobuf.PingPong",
        Purpose::Broadcast => "LumosProtobuf.BroadcastMessage",
        Purpose::List { collection, .. } => COLLECTIONS[*collection].list?.2,
        Purpose::Changes { collection } => match &COLLECTIONS[*collection].changes {
            Some(Changes::Data { message, .. }) | Some(Changes::Reread { message, .. }) => message,
            None => return None,
        },
        Purpose::Special(special) => SPECIALS.iter().find(|(s, _, _)| s == special)?.2,
        Purpose::Watch(watch) => watch.message(),
        Purpose::Read(read) => read.call().2,
        Purpose::Setting { .. } => "LumosProtobuf.GetSettingResponse",
        Purpose::Command { response, .. } => response,
    })
}

/// A command's parameters as its request: each the field of its name; `id`
/// fills requestId where the request has no field of its own by that name
/// (the Get-style requests).
fn request_value(message: &str, params: &Params) -> Result<Value, String> {
    let def = SCHEMA
        .message(message)
        .ok_or_else(|| format!("no message {message}"))?;
    let mut request = Map::new();
    for (name, value) in params {
        let field = if name == "id" && !def.fields.iter().any(|f| f.json == "id") {
            "request_id"
        } else {
            name.as_str()
        };
        // A JSON parameter given as text holding JSON is read as JSON.
        let value = match (value, def.fields.iter().find(|f| f.json == field)) {
            (Value::String(s), Some(f))
                if !matches!(
                    f.kind,
                    crate::protobuf::Kind::String
                        | crate::protobuf::Kind::Bytes
                        | crate::protobuf::Kind::Enum(_)
                ) || !matches!(f.label, crate::protobuf::Label::Single) =>
            {
                serde_json::from_str(s).map_err(|e| format!("{name} is not JSON: {e}"))?
            }
            _ => value.clone(),
        };
        request.insert(field.to_string(), value);
    }
    Ok(Value::Object(request))
}

/// How a command's call ended, as its result.
fn command_result(
    response: &str,
    streaming: bool,
    collected: Vec<Value>,
    status: &Status,
) -> Result<Outcome, CommandError> {
    if !status.ok() {
        return Err(CommandError::DeviceError {
            code: Some(grpc::code_name(status.code).to_string()),
            message: if status.message.is_empty() {
                grpc::code_name(status.code).to_string()
            } else {
                status.message.clone()
            },
        });
    }
    if streaming {
        return Ok(Outcome::Value {
            value: Value::Array(collected),
        });
    }
    let Some(answer) = collected.into_iter().last() else {
        return Err(CommandError::DeviceError {
            code: None,
            message: "Umbra ended the call without an answer".into(),
        });
    };
    if response == "LumosProtobuf.ConfirmedResponse" {
        return if answer["ok"].as_bool() == Some(true) {
            Ok(Outcome::Ack)
        } else {
            Err(CommandError::DeviceError {
                code: None,
                message: or_refused(translate(&answer["message"])),
            })
        };
    }
    // A choice of answers whose `error` is set is a refusal.
    match answer.get("error") {
        Some(Value::String(e)) if !e.is_empty() => {
            return Err(CommandError::DeviceError {
                code: None,
                message: e.clone(),
            });
        }
        Some(e @ Value::Object(_)) if is_oneof_member(response, "error") => {
            return Err(CommandError::DeviceError {
                code: None,
                message: or_refused(translate(e)),
            });
        }
        _ => {}
    }
    Ok(Outcome::Value { value: answer })
}

fn is_oneof_member(message: &str, field: &str) -> bool {
    SCHEMA.message(message).is_some_and(|m| {
        m.fields
            .iter()
            .any(|f| f.json == field && f.oneof.is_some())
    })
}

fn or_refused(text: String) -> String {
    if text.is_empty() {
        "DMXControl refused the command".into()
    } else {
        text
    }
}

fn suffix(message: &str) -> String {
    if message.is_empty() {
        String::new()
    } else {
        format!(": {message}")
    }
}

/// A TranslateableData as text: its format string with {0}, {1}, ...
/// replaced by its parameters.
fn translate(data: &Value) -> String {
    let mut text = data["format_string"].as_str().unwrap_or("").to_string();
    for (i, p) in data["parameters"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let value = match (p.get("translateable"), p.get("object_data")) {
            (Some(t), _) => translate(t),
            (_, Some(o)) => match object_value(o) {
                Value::String(s) => s,
                Value::Null => String::new(),
                other => other.to_string(),
            },
            _ => String::new(),
        };
        text = text.replace(&format!("{{{i}}}", i = i), &value);
    }
    text
}

/// An ObjectData's value as plain JSON.
fn object_value(data: &Value) -> Value {
    const KINDS: [&str; 12] = [
        "string_value",
        "double_value",
        "float_value",
        "int_value",
        "long_value",
        "uint_value",
        "ulong_value",
        "bool_value",
        "short_value",
        "sbyte_value",
        "ushort_value",
        "byte_value",
    ];
    for k in KINDS {
        if let Some(v) = data.get(k) {
            return v.clone();
        }
    }
    if let Some(a) = data.get("array_value") {
        return Value::Array(
            a["values"]
                .as_array()
                .into_iter()
                .flatten()
                .map(object_value)
                .collect(),
        );
    }
    Value::Null
}

/// A GetSettingResponse's value, whichever kind it is.
fn setting_value(data: &Value) -> Option<Value> {
    ["string_value", "bool_value", "int_value", "double_value"]
        .iter()
        .find_map(|k| data.get(*k).cloned())
}

fn id_of(item: &Value, path: &[&str]) -> Option<String> {
    let mut v = item;
    for p in path {
        v = v.get(*p)?;
    }
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// A client's state from its ClientProgramInfo: its runtime ID and fields.
fn client_state(info: &Value) -> Option<(String, Value)> {
    let c = &info["client_info"];
    let id = c["runtimeid"]
        .as_str()
        .filter(|s| !s.is_empty())?
        .to_string();
    let p = &info["program_info"];
    Some((
        id,
        json!({
            "client_name": c["clientname"],
            "hostname": c["hostname"],
            "type": c["type"],
            "ips": c["ips"],
            "project_name": c["project_name"],
            "network_id": c["networkid"],
            "ready_to_work": c["state"]["ready_to_work"].as_bool().unwrap_or(false),
            "program_name": p["program_name"].as_str().unwrap_or(""),
            "program_version": p["program_version"].as_str().unwrap_or(""),
            "vendor": p["vendor"].as_str().unwrap_or(""),
            "latency_us": info["connection_latency_micro_seconds"],
        }),
    ))
}

fn umbra_state(info: &Value) -> Value {
    let c = &info["client_info"];
    let p = &info["program_info"];
    json!({
        "client_name": c["clientname"].as_str().unwrap_or(""),
        "hostname": c["hostname"].as_str().unwrap_or(""),
        "ips": c["ips"].as_array().cloned().unwrap_or_default(),
        "network_id": c["networkid"].as_str().unwrap_or(""),
        "runtime_id": c["runtimeid"].as_str().unwrap_or(""),
        "project_name": c["project_name"].as_str().unwrap_or(""),
        "port": c["umbra_port"].as_i64().unwrap_or(0),
        "secure_port": c["umbra_secure_port"].as_i64().unwrap_or(0),
        "program_name": p["program_name"].as_str().unwrap_or(""),
        "program_version": p["program_version"].as_str().unwrap_or(""),
        "vendor": p["vendor"].as_str().unwrap_or(""),
    })
}

/// The merge patch (RFC 7386) taking `old` to `new`, or `None` when they
/// are equal. A value that was absent is `new` whole.
fn diff(old: Option<&Value>, new: &Value) -> Option<Value> {
    let Some(old) = old else {
        return Some(new.clone());
    };
    match (old, new) {
        (Value::Object(o), Value::Object(n)) => {
            let mut patch = Map::new();
            for k in o.keys() {
                if !n.contains_key(k) {
                    patch.insert(k.clone(), Value::Null);
                }
            }
            for (k, v) in n {
                if let Some(d) = diff(o.get(k), v) {
                    patch.insert(k.clone(), d);
                }
            }
            (!patch.is_empty()).then_some(Value::Object(patch))
        }
        _ => (old != new).then(|| new.clone()),
    }
}

/// `value` at `path` as a patch.
fn nest(path: &[&str], value: Value) -> Value {
    path.iter()
        .rev()
        .fold(value, |inner, key| json!({ *key: inner }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::{Request, Server};
    use crate::module::Action;

    fn module(settings: Value, monitor: bool) -> Dmx3 {
        Dmx3::new(OpenContext {
            host: "10.0.0.5".parse().unwrap(),
            host_name: None,
            port: None,
            model: "dmxcontrol3".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        })
    }

    fn sent(actions: &[Action]) -> Vec<u8> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// A module and a server standing in for Umbra, logged in.
    struct Rig {
        m: Dmx3,
        s: Server,
        now: Millis,
        /// Every call the server saw: stream, path, session header.
        calls: Vec<(u32, String, Option<String>)>,
        messages: HashMap<u32, Vec<Vec<u8>>>,
        actions: Vec<Action>,
    }

    impl Rig {
        fn new(settings: Value, monitor: bool) -> Rig {
            let mut m = module(settings, monitor);
            let mut cx = Cx::new(0);
            m.start(&mut cx);
            let actions = cx.take();
            assert!(matches!(&actions[1], Action::TcpOpen { to, .. } if to.port() == PORT));
            let mut rig = Rig {
                m,
                s: Server::new(),
                now: 0,
                calls: Vec::new(),
                messages: HashMap::new(),
                actions: Vec::new(),
            };
            let mut cx = Cx::new(0);
            rig.m.tcp(&mut cx, SOCKET, TcpInput::Connected);
            rig.feed_server(cx.take());
            rig
        }

        /// Give the server what the module sent, and record the calls.
        fn feed_server(&mut self, actions: Vec<Action>) -> Vec<Request> {
            let bytes = sent(&actions);
            self.actions.extend(actions);
            let requests = self.s.receive(&bytes).unwrap();
            for r in &requests {
                match r {
                    Request::Start {
                        stream,
                        path,
                        metadata,
                    } => {
                        let session = metadata
                            .iter()
                            .find(|(k, _)| k == "sessionid")
                            .map(|(_, v)| v.clone());
                        self.calls.push((*stream, path.clone(), session));
                    }
                    Request::Message { stream, message } => self
                        .messages
                        .entry(*stream)
                        .or_default()
                        .push(message.clone()),
                    _ => {}
                }
            }
            requests
        }

        /// Give the module what the server sent.
        fn feed_module(&mut self) -> Vec<Action> {
            let bytes = self.s.take_output();
            let mut cx = Cx::new(self.now);
            self.m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes));
            let actions = cx.take();
            self.feed_server(actions.clone());
            actions
        }

        fn stream_of(&self, path: &str) -> u32 {
            self.calls
                .iter()
                .rev()
                .find(|(_, p, _)| p.ends_with(path))
                .unwrap_or_else(|| panic!("no call to {path}"))
                .0
        }

        fn answer(&mut self, path: &str, message: &str, value: Value) -> Vec<Action> {
            let stream = self.stream_of(path);
            let bytes = SCHEMA.encode(message, &value).unwrap();
            self.s.headers(stream);
            self.s.message(stream, &bytes);
            self.s.close(stream, 0, "");
            self.feed_module()
        }

        fn login(&mut self) -> Vec<Action> {
            // The server's settings, then the network information.
            self.feed_module();
            self.answer(
                "/GetUmbraNetworkInfo",
                "LumosProtobuf.UmbraNetworkInfoResponse",
                json!({"umbra_server": {"client_info": {"networkid": "LIGHT-Net", "clientname": "LIGHT-PC-1",
                        "runtimeid": "u-1", "umbra_port": 17475, "type": "Umbra"},
                        "program_info": {"program_version": "3.3.2"}},
                       "connected_clients": [{"client_info": {"runtimeid": "k-1", "clientname": "Kernel", "type": "Kernel"}}]}),
            );
            self.answer(
                "/Login",
                "LumosProtobuf.UmbraLoginResponse",
                json!({"return_code": "NoError", "session_id": "S3ss10n",
                       "umbra_server": {"client_info": {"clientname": "LIGHT-PC-1", "networkid": "LIGHT-Net"}}}),
            )
        }

        fn state(&self) -> Value {
            let mut state = json!({});
            for a in &self.actions {
                if let Action::State(patch) = a {
                    merge(&mut state, patch);
                }
            }
            state
        }
    }

    fn merge(target: &mut Value, patch: &Value) {
        if let Value::Object(p) = patch {
            if !target.is_object() {
                *target = json!({});
            }
            let t = target.as_object_mut().unwrap();
            for (k, v) in p {
                if v.is_null() {
                    t.remove(k);
                } else {
                    merge(t.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
        } else {
            *target = patch.clone();
        }
    }

    #[test]
    fn the_session_reads_the_network_id_logs_in_and_reports_ready() {
        let mut rig = Rig::new(json!({}), false);
        let actions = rig.login();
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        let paths: Vec<&str> = rig.calls.iter().map(|(_, p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            [
                "/LumosProtobufCommon.DMXCNetService/GetUmbraNetworkInfo",
                "/LumosProtobufCommon.ClientService/Login",
                "/LumosProtobufCommon.ConnectedClientService/ReportReadyToWork",
                "/LumosProtobufCommon.ConnectedClientService/Ping",
            ],
            "commands only: no stream, no read"
        );
        // The login carries no session; the calls after it do.
        assert_eq!(rig.calls[1].2, None);
        assert_eq!(rig.calls[2].2.as_deref(), Some("S3ss10n"));
        assert_eq!(rig.calls[3].2.as_deref(), Some("S3ss10n"));
        // The login names this client with the network ID Umbra gave.
        let login = SCHEMA
            .decode(
                "LumosProtobuf.UmbraLoginRequest",
                &rig.messages[&rig.stream_of("/Login")][0],
            )
            .unwrap();
        assert_eq!(login["client"]["client_info"]["networkid"], "LIGHT-Net");
        assert_eq!(login["client"]["client_info"]["type"], "ExternalTool");
        assert_eq!(login["client"]["client_info"]["clientname"], "Meros");
        let ready = SCHEMA
            .decode(
                "LumosProtobuf.UmbraClientReadyToWorkNotification",
                &rig.messages[&rig.stream_of("/ReportReadyToWork")][0],
            )
            .unwrap();
        assert_eq!(ready["state"]["ready_to_work"], true);
        // The first ping went out on the ping call.
        let ping = SCHEMA
            .decode(
                "LumosProtobuf.PingPong",
                &rig.messages[&rig.stream_of("/Ping")][0],
            )
            .unwrap();
        assert_eq!(
            (
                ping["request_counter"].as_i64(),
                ping["clientname"].as_str()
            ),
            (Some(1), Some("Meros"))
        );
        let state = rig.state();
        assert_eq!(state["umbra"]["network_id"], "LIGHT-Net");
        assert_eq!(state["session"]["network_id"], "LIGHT-Net");
    }

    #[test]
    fn umbras_pings_are_answered_with_this_clients_name() {
        let mut rig = Rig::new(json!({"network_id": "N"}), false);
        rig.feed_module();
        rig.answer(
            "/Login",
            "LumosProtobuf.UmbraLoginResponse",
            json!({"return_code": "AlreadyLoggedIn", "session_id": "abc"}),
        );
        let stream = rig.stream_of("/Ping");
        rig.s.headers(stream);
        let ping = SCHEMA
            .encode(
                "LumosProtobuf.PingPong",
                &json!({"request_counter": 7, "clientname": "Umbra"}),
            )
            .unwrap();
        rig.s.message(stream, &ping);
        rig.feed_module();
        let pong = SCHEMA
            .decode(
                "LumosProtobuf.PingPong",
                rig.messages[&stream].last().unwrap(),
            )
            .unwrap();
        assert_eq!(
            (pong["request_counter"].as_i64(), pong["responder"].as_str()),
            (Some(7), Some("Meros"))
        );
        // This client's own ping echoed back is not answered.
        let count = rig.messages[&stream].len();
        let own = SCHEMA
            .encode(
                "LumosProtobuf.PingPong",
                &json!({"request_counter": 1, "clientname": "Meros"}),
            )
            .unwrap();
        rig.s.message(stream, &own);
        rig.feed_module();
        assert_eq!(rig.messages[&stream].len(), count);
    }

    #[test]
    fn a_refused_login_is_terminal_and_a_full_umbra_is_retried() {
        let mut rig = Rig::new(json!({"network_id": "N"}), true);
        rig.feed_module();
        let actions = rig.answer(
            "/Login",
            "LumosProtobuf.UmbraLoginResponse",
            json!({"return_code": "AccessDenied", "message": "no"}),
        );
        assert!(actions.iter().any(|a| matches!(a, Action::Connection(Connection::Unauthorized { reason }) if reason.contains("AccessDenied"))));
        assert!(!actions
            .iter()
            .any(|a| matches!(a, Action::SetTimer { key: RETRY, .. })));
        let mut cx = Cx::new(10);
        rig.m.command(&mut cx, 1, "get_cuelists", &Params::new());
        assert!(matches!(
            &cx.take()[0],
            Action::Complete {
                result: Err(CommandError::Auth { .. }),
                ..
            }
        ));

        let mut rig = Rig::new(json!({"network_id": "N"}), true);
        rig.feed_module();
        let actions = rig.answer(
            "/Login",
            "LumosProtobuf.UmbraLoginResponse",
            json!({"return_code": "ClientLimitReached"}),
        );
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        assert!(actions.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
    }

    #[test]
    fn commands_encode_their_parameters_and_map_the_answer() {
        let mut rig = Rig::new(json!({}), false);
        rig.login();
        let mut cx = Cx::new(rig.now);
        rig.m.command(
            &mut cx,
            9,
            "cuelist_action",
            &json!({"cuelist_id": "c-1", "action": "GO"})
                .as_object()
                .unwrap()
                .clone(),
        );
        rig.feed_server(cx.take());
        let stream = rig.stream_of("/LumosProtobufClient.CuelistClient/CuelistAction");
        let request = SCHEMA
            .decode(
                "LumosProtobuf.Cuelist.CuelistActionRequest",
                &rig.messages[&stream][0],
            )
            .unwrap();
        assert_eq!(
            (request["cuelist_id"].as_str(), request["action"].as_str()),
            (Some("c-1"), Some("GO"))
        );
        rig.now = 40;
        let actions = rig.answer(
            "/CuelistAction",
            "LumosProtobuf.ConfirmedResponse",
            json!({"ok": true}),
        );
        assert!(actions.contains(&Action::Complete {
            id: 9,
            result: Ok(Outcome::Ack)
        }));
        assert!(actions.contains(&Action::RoundTrip(40)));

        // A refusal carries its message, its {0} filled in.
        let mut cx = Cx::new(rig.now);
        rig.m.command(
            &mut cx,
            10,
            "set_main_switch",
            &json!({"main_switch_id": "m", "toggle": true})
                .as_object()
                .unwrap()
                .clone(),
        );
        rig.feed_server(cx.take());
        let actions = rig.answer(
            "/SetMainSwitch",
            "LumosProtobuf.ConfirmedResponse",
            json!({"ok": false, "message": {"format_string": "No switch {0}", "parameters": [{"object_data": {"string_value": "m"}}]}}),
        );
        assert!(actions.contains(&Action::Complete {
            id: 10,
            result: Err(CommandError::DeviceError {
                code: None,
                message: "No switch m".into()
            })
        }));

        // A Get-style command's id goes in requestId, and the answer is the message.
        let mut cx = Cx::new(rig.now);
        rig.m.command(
            &mut cx,
            11,
            "get_client_program_info",
            &json!({"id": "k-1"}).as_object().unwrap().clone(),
        );
        rig.feed_server(cx.take());
        let stream = rig.stream_of("/GetClientProgramInfo");
        let request = SCHEMA
            .decode("LumosProtobuf.GetRequest", &rig.messages[&stream][0])
            .unwrap();
        assert_eq!(request["request_id"], "k-1");
        let actions = rig.answer(
            "/GetClientProgramInfo",
            "LumosProtobuf.UmbraClientProgramInfoResponse",
            json!({"clientname": "Kernel"}),
        );
        let Some(Action::Complete {
            result: Ok(Outcome::Value { value }),
            ..
        }) = actions
            .iter()
            .find(|a| matches!(a, Action::Complete { id: 11, .. }))
        else {
            panic!("{actions:?}")
        };
        assert_eq!(value["clientname"], "Kernel");

        // A deletion refused with its error choice fails.
        let mut cx = Cx::new(rig.now);
        rig.m.command(
            &mut cx,
            12,
            "delete_cuelist",
            &json!({"cuelist_id": "c-1"}).as_object().unwrap().clone(),
        );
        rig.feed_server(cx.take());
        let actions = rig.answer(
            "/DeleteCuelist",
            "LumosProtobuf.Cuelist.CuelistDeletionResponse",
            json!({"error": "in use"}),
        );
        assert!(actions.contains(&Action::Complete {
            id: 12,
            result: Err(CommandError::DeviceError {
                code: None,
                message: "in use".into()
            })
        }));

        // A status other than OK fails with its code.
        let mut cx = Cx::new(rig.now);
        rig.m.command(&mut cx, 13, "get_cuelists", &Params::new());
        rig.feed_server(cx.take());
        let stream = rig.stream_of("/GetCuelists");
        rig.s.headers(stream);
        rig.s.close(stream, 12, "nope");
        let actions = rig.feed_module();
        assert!(actions.contains(&Action::Complete {
            id: 13,
            result: Err(CommandError::DeviceError {
                code: Some("UNIMPLEMENTED".into()),
                message: "nope".into()
            })
        }));
    }

    #[test]
    fn a_streaming_command_collects_every_message_and_a_unary_one_times_out() {
        let mut rig = Rig::new(json!({"timeout_ms": 500}), false);
        rig.login();
        let mut cx = Cx::new(rig.now);
        rig.m
            .command(&mut cx, 1, "save_project_version", &Params::new());
        rig.feed_server(cx.take());
        let stream = rig.stream_of("/SaveProjectVersion");
        rig.s.headers(stream);
        for status in ["WORKING", "DONE"] {
            let m = SCHEMA
                .encode(
                    "LumosProtobuf.ProjectChangeStatusResponse",
                    &json!({"status": status}),
                )
                .unwrap();
            rig.s.message(stream, &m);
        }
        rig.s.close(stream, 0, "");
        let actions = rig.feed_module();
        let Some(Action::Complete {
            result: Ok(Outcome::Value { value }),
            ..
        }) = actions
            .iter()
            .find(|a| matches!(a, Action::Complete { id: 1, .. }))
        else {
            panic!("{actions:?}")
        };
        assert_eq!(value.as_array().unwrap().len(), 2);
        assert_eq!(value[1]["status"], "DONE");

        let mut cx = Cx::new(1_000);
        rig.m.command(&mut cx, 2, "get_cuelists", &Params::new());
        let actions = cx.take();
        assert!(actions.contains(&Action::SetTimer {
            key: DEADLINE,
            after: 500
        }));
        rig.feed_server(actions);
        let mut cx = Cx::new(1_500);
        rig.m.timer(&mut cx, DEADLINE);
        let actions = cx.take();
        assert!(actions.contains(&Action::Complete {
            id: 2,
            result: Err(CommandError::Timeout)
        }));
        let requests = rig.feed_server(actions);
        assert!(
            requests.iter().any(|r| matches!(r, Request::Cancel { .. })),
            "the call is cancelled"
        );
    }

    #[test]
    fn monitoring_reads_lists_opens_streams_and_applies_changes() {
        let mut rig = Rig::new(json!({}), true);
        rig.login();
        let paths: HashSet<String> = rig.calls.iter().map(|(_, p, _)| p.clone()).collect();
        for c in COLLECTIONS {
            if let Some((path, ..)) = c.list {
                assert!(paths.contains(path), "{} is read", c.key);
            }
        }
        assert!(paths.contains("/LumosProtobufClient.ExecutorClient/ReceiveExecutorChanges"));
        assert!(paths.contains("/LumosProtobufClient.CueClient/ReceiveCueChanges"));
        assert!(paths.contains("/LumosProtobufCommon.ConnectedClientService/SendBroadcast"));
        assert!(
            !paths.contains("/LumosProtobufClient.DMXClient/GetDmxChanges"),
            "no DMX channels watched"
        );

        rig.answer(
            "/GetExecutors",
            "LumosProtobuf.Executor.GetExecutorsResponse",
            json!({"executors": [{"id": "e1", "name": "Front", "fader_position": 0.5}, {"id": "e2", "name": "Back"}]}),
        );
        let state = rig.state();
        assert_eq!(state["executors"]["e1"]["name"], "Front");
        assert_eq!(state["executors"]["e1"]["fader_position"], 0.5);
        assert_eq!(
            state["executors"]["e2"]["button1_active"], false,
            "proto3 defaults are written"
        );

        // A change with data replaces the executor; removed deletes it.
        let stream = rig.stream_of("/ReceiveExecutorChanges");
        rig.s.headers(stream);
        let change = SCHEMA
            .encode("LumosProtobuf.Executor.ExecutorChangedMessage", &json!({"change_type": "Changed", "executor_id": "e1",
                "executor_data": {"id": "e1", "name": "Front", "fader_position": 1.0, "button1_active": true}}))
            .unwrap();
        rig.s.message(stream, &change);
        let removed = SCHEMA
            .encode(
                "LumosProtobuf.Executor.ExecutorChangedMessage",
                &json!({"change_type": "Removed", "executor_id": "e2"}),
            )
            .unwrap();
        rig.s.message(stream, &removed);
        let actions = rig.feed_module();
        assert!(actions.contains(&Action::State(
            json!({"executors": {"e1": {"fader_position": 1.0, "button1_active": true}}})
        )));
        assert!(actions.contains(&Action::State(json!({"executors": {"e2": null}}))));

        // A change without data reads the object again, by its ID.
        let before = rig.calls.len();
        let bare = SCHEMA
            .encode(
                "LumosProtobuf.Executor.ExecutorChangedMessage",
                &json!({"change_type": "Changed", "executor_id": "e1"}),
            )
            .unwrap();
        rig.s.message(stream, &bare);
        rig.feed_module();
        let (stream2, path, _) = rig.calls[before].clone();
        assert_eq!(path, "/LumosProtobufClient.ExecutorClient/GetExecutors");
        let request = SCHEMA
            .decode(
                "LumosProtobuf.GetMultipleRequest",
                &rig.messages[&stream2][0],
            )
            .unwrap();
        assert_eq!(request["id_filter"], json!(["e1"]));

        // Cuelists subscribe their progress; progress replaces the cue map.
        rig.answer(
            "/GetCuelists",
            "LumosProtobuf.Cuelist.GetCuelistsResponse",
            json!({"cuelists": [{"id": "c1", "name": "Main"}]}),
        );
        let watch = rig.stream_of("/ReceiveCuelistProgressChanges");
        let subscription = SCHEMA
            .decode(
                "LumosProtobuf.Cuelist.CuelistProgressStateChangeRequest",
                rig.messages[&watch].last().unwrap(),
            )
            .unwrap();
        assert_eq!(subscription["cuelist_ids"], json!(["c1"]));
        rig.s.headers(watch);
        let progress = SCHEMA
            .encode(
                "LumosProtobuf.Cuelist.CuelistProgressStateChangeMessage",
                &json!({"cuelist_id": "c1", "cuelist_state": "RUNNING",
                "cue_progress_state": {"0": {"cue_id": "q1", "progress": 0.25}}}),
            )
            .unwrap();
        rig.s.message(watch, &progress);
        rig.feed_module();
        let state = rig.state();
        assert_eq!(state["cuelist_progress"]["c1"]["cuelist_state"], "RUNNING");
        assert_eq!(
            state["cuelist_progress"]["c1"]["cues"]["0"]["progress"],
            0.25
        );

        // A cue change reads its cuelist again.
        let cues = rig.stream_of("/ReceiveCueChanges");
        rig.s.headers(cues);
        let cue = SCHEMA
            .encode(
                "LumosProtobuf.Cue.CueChangedMessage",
                &json!({"parent_container_id": "c1", "cue_id": "q1"}),
            )
            .unwrap();
        rig.s.message(cues, &cue);
        let before = rig.calls.len();
        rig.feed_module();
        assert!(rig.calls[before..]
            .iter()
            .any(|(_, p, _)| p.ends_with("/GetCuelists")));

        // The global functions are read on each tick, with an HTTP/2 PING.
        rig.now = TICK_EVERY;
        let mut cx = Cx::new(rig.now);
        rig.m.timer(&mut cx, TICK);
        let before = rig.calls.len();
        rig.feed_server(cx.take());
        assert!(rig.calls[before..]
            .iter()
            .any(|(_, p, _)| p.ends_with("/GetSetGlobalFunction")));
    }

    #[test]
    fn an_ended_stream_is_opened_again_and_an_unoffered_one_is_not() {
        let mut rig = Rig::new(json!({}), true);
        rig.login();
        rig.answer(
            "/GetMacros",
            "LumosProtobuf.Macro.GetMacrosResponse",
            json!({"macros": []}),
        );
        let stream = rig.stream_of("/ReceiveMacroChanges");
        rig.s.headers(stream);
        rig.s.close(stream, 14, "restart");
        let actions = rig.feed_module();
        assert!(actions.contains(&Action::SetTimer {
            key: RESTREAM,
            after: RESTREAM_AFTER
        }));
        let stream = rig.stream_of("/ReceiveStageViewChanges");
        rig.s.headers(stream);
        rig.s.close(stream, 12, "");
        rig.feed_module();
        let before = rig.calls.len();
        let mut cx = Cx::new(rig.now + RESTREAM_AFTER);
        rig.m.timer(&mut cx, RESTREAM);
        rig.feed_server(cx.take());
        let reopened: Vec<&str> = rig.calls[before..]
            .iter()
            .map(|(_, p, _)| p.as_str())
            .collect();
        assert!(reopened.contains(&"/LumosProtobufClient.MacroClient/ReceiveMacroChanges"));
        assert!(
            reopened.contains(&"/LumosProtobufClient.MacroClient/GetMacros"),
            "with its list"
        );
        assert!(!reopened
            .iter()
            .any(|p| p.ends_with("/ReceiveStageViewChanges")));
    }

    #[test]
    fn a_silent_umbra_is_dropped_and_pongs_give_the_round_trip() {
        let mut rig = Rig::new(json!({}), false);
        rig.login();
        rig.now = 5_000;
        let mut cx = Cx::new(rig.now);
        rig.m.timer(&mut cx, TICK);
        rig.feed_server(cx.take());
        rig.now = 5_030;
        let actions = rig.feed_module();
        assert!(actions.contains(&Action::RoundTrip(30)), "{actions:?}");
        let mut cx = Cx::new(25_000);
        rig.m.timer(&mut cx, TICK);
        let actions = cx.take();
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        assert!(actions.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
    }

    #[test]
    fn closing_logs_off() {
        let mut rig = Rig::new(json!({}), false);
        rig.login();
        let mut cx = Cx::new(rig.now);
        rig.m.stop(&mut cx);
        let actions = cx.take();
        assert!(matches!(actions.last(), Some(Action::TcpClose { .. })));
        rig.feed_server(actions);
        assert!(rig
            .calls
            .iter()
            .any(|(_, p, s)| p.ends_with("/Logoff") && s.as_deref() == Some("S3ss10n")));
    }

    #[test]
    fn tls_offers_h2_on_the_tls_port() {
        let mut m = module(json!({"scheme": "tls"}), true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let actions = cx.take();
        let Some(Action::TcpOpenTls { target, .. }) = actions
            .iter()
            .find(|a| matches!(a, Action::TcpOpenTls { .. }))
        else {
            panic!("{actions:?}")
        };
        assert_eq!(target.to.port(), TLS_PORT);
        assert_eq!(target.alpn, ["h2"]);
        assert!(target.accept_invalid_certs);
    }

    #[test]
    fn every_method_and_collection_names_messages_in_the_schema() {
        for m in METHODS {
            assert!(SCHEMA.message(m.request).is_some(), "{}", m.request);
            assert!(SCHEMA.message(m.response).is_some(), "{}", m.response);
        }
        assert!(
            METHODS.windows(2).all(|w| w[0].command < w[1].command),
            "sorted for the lookup"
        );
        for c in COLLECTIONS {
            if let Some((_, req, resp, field)) = c.list {
                assert!(SCHEMA.message(req).is_some());
                assert!(
                    SCHEMA
                        .message(resp)
                        .unwrap()
                        .fields
                        .iter()
                        .any(|f| f.json == field),
                    "{}",
                    c.key
                );
            }
        }
        for (_, path, message) in SPECIALS {
            assert!(SCHEMA.message(message).is_some(), "{path}");
        }
    }

    #[test]
    fn the_diff_removes_what_is_gone_and_sends_what_changed() {
        assert_eq!(diff(None, &json!({"a": 1})), Some(json!({"a": 1})));
        assert_eq!(
            diff(Some(&json!({"a": 1, "b": 2})), &json!({"a": 1, "b": 2})),
            None
        );
        assert_eq!(
            diff(
                Some(&json!({"a": 1, "b": {"c": 1, "d": 2}})),
                &json!({"a": 2, "b": {"c": 1}})
            ),
            Some(json!({"a": 2, "b": {"d": null}}))
        );
        assert_eq!(
            diff(Some(&json!({"a": [1, 2]})), &json!({"a": [1]})),
            Some(json!({"a": [1]}))
        );
    }
}
