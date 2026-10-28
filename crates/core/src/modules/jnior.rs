//! INTEG JNIOR Series 4 automation controllers (Models 410, 412, 414 and
//! 412DMX) over the JANOS Management Protocol (JMP), as the JNIOR Users
//! Manual (JANOS) documents it: JSON messages on TCP 9220, each wrapped as
//! `[length,{...}]`, or the same messages on the web server's WebSocket.
//!
//! - On connecting the client sends an empty message; a unit requiring a
//!   login answers `401 Unauthorized` with a nonce, and the client answers
//!   `Auth-Digest`, `username:MD5(username:nonce:password)`. A second 401 is
//!   a refused credential: the device is unauthorized and not retried until
//!   its settings change.
//! - The unit pushes a `Monitor` message (every input's state and counter,
//!   every relay's state, model, version, serial number) on authenticating
//!   and whenever I/O changes, `Registry Update` whenever a registry key
//!   changes (hour meters, descriptions), `Syslog Update` for each system
//!   log line and `Device Shutdown` before a reboot; all of it reaches
//!   state.
//! - Requests carry `Meta: {Id: n}`, which the unit echoes in the response,
//!   so each response is matched to its request whatever arrives meanwhile.
//!   Control messages (relays, latches, counters, usage meters) have no
//!   response: the `Monitor` that follows a change updates state, and they
//!   are reported unverified.
//! - Monitored, the client reads the device keys and each I/O channel's
//!   configuration keys once the first `Monitor` shows how many there are,
//!   enumerates the expansion modules on the sensor port and reads their
//!   data blocks every `module_poll_ms`.
//! - A clock read every 30 s of silence is the liveness check and reports
//!   the round trip; a connection that drops is opened again after 1 s,
//!   doubling to 30 s.
//!
//! Opened for commands only (`monitor` false), no registry key, module or
//! clock is read beyond the liveness check and what commands ask; the unit
//! still pushes `Monitor` on authenticating and on changes (the protocol
//! sends it unasked), and that still reaches state.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use base64::Engine as _;
use md5::{Digest, Md5};
use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, FileInput, Key, Level, Millis, Module, OpenContext,
    Outcome, TcpInput, WsInput, WsRequest,
};

pub(crate) const PORT: u16 = 9220;
const SOCKET: Key = "jmp";
const RETRY_TIMER: Key = "retry";
const IDLE_TIMER: Key = "idle";
const MODULE_TIMER: Key = "modules";
const REPLY_TIMER: Key = "reply";
const DOWNLOAD_FILE: Key = "download";
const UPLOAD_FILE: Key = "upload";
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const IDLE: Millis = 30_000;
/// Bytes read or written per file message (the manual suggests up to
/// 128 KB fit one message).
const CHUNK: usize = 65_536;

/// The configuration keys read for each input and each relay output.
const INPUT_KEYS: [&str; 14] = [
    "Desc",
    "OnDesc",
    "OffDesc",
    "Inversion",
    "Latching",
    "LatchState",
    "LatchTime",
    "Debounce",
    "Conditioning",
    "CountState",
    "UsageState",
    "Log",
    "Alarming",
    "$HourMeter",
];
const OUTPUT_KEYS: [&str; 7] = [
    "Desc",
    "ClosedDesc",
    "OpenDesc",
    "InitialState",
    "UsageState",
    "Log",
    "$HourMeter",
];
const DEVICE_KEYS: [&str; 8] = [
    "$Model",
    "$SerialNumber",
    "$Version",
    "$BootTime",
    "$LastNtpSuccess",
    "$BuildTag",
    "Device/Desc",
    "Device/Timezone",
];

/// The JMP digest: `username:MD5(username:nonce:password)` in lowercase hex.
pub(crate) fn auth_digest(username: &str, nonce: &str, password: &str) -> String {
    let hash = Md5::digest(format!("{username}:{nonce}:{password}").as_bytes());
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    format!("{username}:{hex}")
}

/// A message wrapped for TCP 9220: `[length,{...}]`.
pub(crate) fn wrap(message: &Value) -> Vec<u8> {
    let text = message.to_string();
    format!("[{},{}]", text.len(), text).into_bytes()
}

/// Reads wrapped messages from the stream: skips to a `[`, reads the
/// length, the comma, that many bytes of object and the `]`; anything that
/// breaks the format is dropped, as the server itself does.
#[derive(Debug, Default)]
struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Value> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            let Some(start) = self.buf.iter().position(|&b| b == b'[') else {
                self.buf.clear();
                break;
            };
            self.buf.drain(..start);
            let mut i = 1;
            let skip_ws = |buf: &[u8], mut i: usize| {
                while i < buf.len() && buf[i].is_ascii_whitespace() {
                    i += 1;
                }
                i
            };
            i = skip_ws(&self.buf, i);
            let digits_start = i;
            while i < self.buf.len() && self.buf[i].is_ascii_digit() {
                i += 1;
            }
            if i == self.buf.len() {
                break;
            }
            let len: Option<usize> = std::str::from_utf8(&self.buf[digits_start..i])
                .ok()
                .and_then(|d| d.parse().ok())
                .filter(|&n| n >= 2);
            i = skip_ws(&self.buf, i);
            if i == self.buf.len() {
                break;
            }
            let Some(len) = len.filter(|_| self.buf[i] == b',') else {
                self.buf.remove(0);
                continue;
            };
            i = skip_ws(&self.buf, i + 1);
            if self.buf.len() < i + len {
                break;
            }
            let object = self.buf[i..i + len].to_vec();
            let mut j = skip_ws(&self.buf, i + len);
            if j == self.buf.len() {
                break;
            }
            if self.buf[j] != b']' || object.first() != Some(&b'{') || object.last() != Some(&b'}')
            {
                self.buf.remove(0);
                continue;
            }
            j += 1;
            self.buf.drain(..j);
            if let Ok(v) = serde_json::from_slice::<Value>(&object) {
                out.push(v);
            }
        }
        out
    }
}

/// What a request's response becomes.
#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    /// A command answered with the response, or part of it.
    Command(CommandId, Answer),
    /// Background reads: their responses reach state only.
    Read,
    Liveness,
    /// One chunk of a file download to the host.
    Download,
    /// One chunk of a file upload from the host.
    Upload,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Answer {
    /// The response's member at this name, as the value.
    Member(&'static str),
    /// The response whole, without Message and Meta.
    Whole,
    /// A `Result` member that must read Succeed.
    Result,
}

/// A download in progress: the JNIOR file, the host file, what is read.
#[derive(Debug)]
struct Download {
    id: CommandId,
    file: String,
    offset: u64,
    size: Option<u64>,
}

/// An upload in progress.
#[derive(Debug)]
struct Upload {
    id: CommandId,
    file: String,
    data: Vec<u8>,
    offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Closed,
    Opening,
    /// The empty message sent: waiting for a nonce, Authenticated or Monitor.
    LoggingIn,
    /// The digest sent.
    Digest,
    Ready,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transport {
    Tcp,
    Ws,
    Wss,
}

pub(crate) struct Jnior {
    host: IpAddr,
    host_name: Option<String>,
    port: u16,
    transport: Transport,
    ws_path: String,
    accept_invalid_certs: bool,
    username: String,
    password: String,
    monitor: bool,
    module_poll: Millis,
    timeout: Millis,
    phase: Phase,
    deframer: Deframer,
    next_id: u64,
    pending: HashMap<u64, (Purpose, Millis)>,
    retry_after: Millis,
    /// How many inputs and outputs the last Monitor listed.
    io: Option<(usize, usize)>,
    modules: Vec<String>,
    download: Option<Download>,
    upload: Option<Upload>,
    heard_at: Millis,
}

fn text(s: &Params, key: &str) -> String {
    s.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// A registry key as a state key: without its leading slash.
fn key_of(k: &str) -> String {
    k.trim_start_matches('/').to_string()
}

/// What a registry key says of an input or output channel: (inputs or
/// outputs, channel, field).
fn channel_field(key: &str) -> Option<(&'static str, String, &'static str)> {
    let parts: Vec<&str> = key.split('/').collect();
    let [io, group, chan, field] = parts.as_slice() else {
        return None;
    };
    if !io.eq_ignore_ascii_case("IO") {
        return None;
    }
    let lower = chan.to_ascii_lowercase();
    let (which, n) = if group.eq_ignore_ascii_case("Inputs") {
        ("inputs", lower.strip_prefix("din")?)
    } else if group.eq_ignore_ascii_case("Outputs") {
        ("outputs", lower.strip_prefix("rout")?)
    } else {
        return None;
    };
    let n: usize = n.parse().ok()?;
    let field = match field.to_ascii_lowercase().as_str() {
        "desc" => "description",
        "$hourmeter" => "hour_meter",
        _ => return None,
    };
    Some((which, n.to_string(), field))
}

/// The module types the manual lists (Expansion Modules).
fn module_type_name(t: &str) -> &'static str {
    match t.to_ascii_uppercase().as_str() {
        "10" | "26" => "temperature_probe",
        "F9" => "led_dimmer",
        "FA" => "user_panel",
        "FB" => "4rout",
        "FC" => "rtd_temperature",
        "FD" => "analog_10v",
        "FE" => "analog_4_20ma",
        _ => "unknown",
    }
}

impl Jnior {
    pub(crate) fn new(ctx: OpenContext) -> Result<Jnior, String> {
        if ctx.host.is_unspecified() && ctx.host_name.is_none() {
            return Err("give the JNIOR's address as host".into());
        }
        let s = &ctx.settings;
        let transport = match s.get("transport").and_then(Value::as_str) {
            Some("ws") => Transport::Ws,
            Some("wss") => Transport::Wss,
            _ => Transport::Tcp,
        };
        let default_port = match transport {
            Transport::Tcp => PORT,
            Transport::Ws => 80,
            Transport::Wss => 443,
        };
        let mut ws_path = text(s, "ws_path");
        if !ws_path.starts_with('/') {
            ws_path.insert(0, '/');
        }
        let int = |k: &str, d: u64| s.get(k).and_then(Value::as_u64).unwrap_or(d);
        Ok(Jnior {
            host: ctx.host,
            host_name: ctx.host_name.clone(),
            port: ctx.port.unwrap_or(default_port),
            transport,
            ws_path,
            accept_invalid_certs: s
                .get("accept_invalid_certs")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            username: text(s, "username"),
            password: text(s, "password"),
            monitor: ctx.monitor,
            module_poll: int("module_poll_ms", 2000),
            timeout: int("timeout_ms", 5000).max(1),
            phase: Phase::Closed,
            deframer: Deframer::default(),
            next_id: 0,
            pending: HashMap::new(),
            retry_after: RETRY_MIN,
            io: None,
            modules: Vec::new(),
            download: None,
            upload: None,
            heard_at: 0,
        })
    }

    fn open(&mut self, cx: &mut Cx) {
        self.phase = Phase::Opening;
        self.deframer = Deframer::default();
        cx.connection(Connection::Connecting);
        match self.transport {
            Transport::Tcp => cx.tcp_open(SOCKET, SocketAddr::new(self.host, self.port)),
            Transport::Ws | Transport::Wss => {
                let host = match (&self.host_name, self.host) {
                    (Some(n), _) => n.clone(),
                    (None, IpAddr::V6(a)) => format!("[{a}]"),
                    (None, a) => a.to_string(),
                };
                let scheme = if self.transport == Transport::Ws {
                    "ws"
                } else {
                    "wss"
                };
                cx.ws_open(
                    SOCKET,
                    WsRequest {
                        url: format!("{scheme}://{host}:{}{}", self.port, self.ws_path),
                        headers: Vec::new(),
                        accept_invalid_certs: self.accept_invalid_certs,
                    },
                );
            }
        }
    }

    fn send(&self, cx: &mut Cx, message: &Value) {
        match self.transport {
            Transport::Tcp => cx.tcp_send(SOCKET, wrap(message)),
            Transport::Ws | Transport::Wss => cx.ws_send(SOCKET, message.to_string()),
        }
    }

    fn close_socket(&self, cx: &mut Cx) {
        match self.transport {
            Transport::Tcp => cx.tcp_close(SOCKET),
            Transport::Ws | Transport::Wss => cx.ws_close(SOCKET),
        }
    }

    /// Send a request whose response echoes its Meta.
    fn request(&mut self, cx: &mut Cx, mut message: Value, purpose: Purpose) {
        self.next_id += 1;
        message["Meta"] = json!({"Id": self.next_id});
        self.send(cx, &message);
        self.pending.insert(self.next_id, (purpose, cx.now()));
        cx.set_timer(REPLY_TIMER, self.timeout);
    }

    fn opened(&mut self, cx: &mut Cx) {
        self.phase = Phase::LoggingIn;
        self.heard_at = cx.now();
        self.send(cx, &json!({"Message": ""}));
        cx.set_timer(IDLE_TIMER, IDLE);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if self.phase == Phase::Refused {
            return;
        }
        self.phase = Phase::Closed;
        self.io = None;
        for (_, (purpose, _)) in self.pending.drain() {
            if let Purpose::Command(id, _) = purpose {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        if let Some(d) = self.download.take() {
            cx.file_close(DOWNLOAD_FILE);
            cx.complete(
                d.id,
                Err(CommandError::Transport {
                    message: reason.clone(),
                }),
            );
        }
        if let Some(u) = self.upload.take() {
            cx.complete(
                u.id,
                Err(CommandError::Transport {
                    message: reason.clone(),
                }),
            );
        }
        for t in [IDLE_TIMER, MODULE_TIMER, REPLY_TIMER] {
            cx.cancel_timer(t);
        }
        cx.state(json!({"console": {"open": false}}));
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn ready(&mut self, cx: &mut Cx) {
        if self.phase == Phase::Ready {
            return;
        }
        self.phase = Phase::Ready;
        self.retry_after = RETRY_MIN;
        cx.state(json!({"shutting_down": false}));
        cx.connection(Connection::Connected);
        if self.monitor {
            let keys: Vec<&str> = DEVICE_KEYS.to_vec();
            self.request(
                cx,
                json!({"Message": "Registry Read", "Keys": keys}),
                Purpose::Read,
            );
            self.request(cx, json!({"Message": "Enumerate Devices"}), Purpose::Read);
            self.request(cx, json!({"Message": "Clock Read"}), Purpose::Read);
            if self.module_poll > 0 {
                cx.set_timer(MODULE_TIMER, self.module_poll);
            }
        }
    }

    fn received(&mut self, cx: &mut Cx, msg: Value) {
        cx.alive();
        self.heard_at = cx.now();
        let kind = msg
            .get("Message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        // A response to one of our requests, by its Meta.
        let meta = msg
            .get("Meta")
            .and_then(|m| m.get("Id"))
            .and_then(Value::as_u64);
        let purpose = meta.and_then(|id| self.pending.remove(&id));
        if let Some((_, sent_at)) = &purpose {
            cx.round_trip(cx.now().saturating_sub(*sent_at));
        }
        match kind.as_str() {
            "Error" => {
                let text = msg.get("Text").and_then(Value::as_str).unwrap_or("");
                if text.starts_with("401") {
                    if let Some(nonce) = msg.get("Nonce").and_then(Value::as_str) {
                        if self.phase == Phase::Digest {
                            return self.refused(cx);
                        }
                        self.phase = Phase::Digest;
                        let digest = auth_digest(&self.username, nonce, &self.password);
                        self.send(cx, &json!({"Auth-Digest": digest}));
                        return;
                    }
                }
                if let Some((Purpose::Command(id, _), _)) = purpose {
                    cx.complete(
                        id,
                        Err(CommandError::DeviceError {
                            code: None,
                            message: text.to_string(),
                        }),
                    );
                } else {
                    cx.log(Level::Warning, format!("JNIOR error: {text}"));
                }
                return;
            }
            "Authenticated" => {
                cx.state(json!({"login": {
                    "administrator": msg.get("Administrator").and_then(Value::as_bool).unwrap_or(false),
                    "control": msg.get("Control").and_then(Value::as_bool).unwrap_or(false),
                }}));
                self.ready(cx);
            }
            "Monitor" => {
                // A unit without a login sends it at once.
                self.ready(cx);
                self.monitor_message(cx, &msg);
            }
            "Registry Update" | "Registry Response" => {
                if let Some(keys) = msg.get("Keys").and_then(Value::as_object) {
                    self.registry(cx, keys);
                }
            }
            "Enumerate Devices Response" => {
                let ids: Vec<String> = msg
                    .get("Devices")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect();
                let mut modules = Map::new();
                for gone in self.modules.iter().filter(|m| !ids.contains(m)) {
                    modules.insert(gone.clone(), Value::Null);
                }
                for id in &ids {
                    let t = id.get(id.len().saturating_sub(2)..).unwrap_or("");
                    modules.insert(
                        id.clone(),
                        json!({"type": t, "type_name": module_type_name(t)}),
                    );
                }
                self.modules = ids.clone();
                cx.state(json!({ "modules": modules }));
                if self.monitor
                    && !ids.is_empty()
                    && purpose.as_ref().is_some_and(|p| p.0 == Purpose::Read)
                {
                    self.request(
                        cx,
                        json!({"Message": "Read Devices", "Devices": ids}),
                        Purpose::Read,
                    );
                }
            }
            "Read Devices Response" => {
                let mut modules = Map::new();
                for d in msg
                    .get("Devices")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let (Some(addr), Some(hex)) = (
                        d.get("Address").and_then(Value::as_str),
                        d.get("Hex").and_then(Value::as_str),
                    ) else {
                        continue;
                    };
                    let mut entry = json!({"hex": hex});
                    // 4ROUT: the second byte is the relay status, A in bit 0.
                    if addr.to_ascii_uppercase().ends_with("FB") {
                        if let Some(status) =
                            hex.get(2..4).and_then(|b| u8::from_str_radix(b, 16).ok())
                        {
                            let mut relays = Map::new();
                            for (i, name) in ["a", "b", "c", "d"].iter().enumerate() {
                                relays.insert((*name).into(), json!(status & (1 << i) != 0));
                            }
                            entry["relays"] = Value::Object(relays);
                        }
                    }
                    modules.insert(addr.to_string(), entry);
                }
                if !modules.is_empty() {
                    cx.state(json!({ "modules": modules }));
                }
            }
            "Clock Response" => {
                cx.state(json!({"clock": {
                    "time": msg.get("Time"),
                    "date": msg.get("Date"),
                }}));
            }
            "Console Response" => {
                let status = msg.get("Status").and_then(Value::as_str).unwrap_or("");
                cx.state(json!({"console": {"open": status == "Established"}}));
            }
            "Console Stdout" => {
                let data = msg.get("Data").and_then(Value::as_str).unwrap_or("");
                cx.message(
                    "console",
                    None,
                    json!({"data": data}),
                    SocketAddr::new(self.host, self.port),
                );
            }
            "Syslog Update" => {
                let line = msg.get("Data").and_then(Value::as_str).unwrap_or("");
                cx.state(json!({"syslog": {"last": line}}));
                cx.message(
                    "syslog",
                    None,
                    json!({"line": line}),
                    SocketAddr::new(self.host, self.port),
                );
            }
            "Device Shutdown" => {
                cx.state(json!({"shutting_down": true}));
                cx.log(Level::Info, "the JNIOR is shutting down to reboot");
            }
            "File Read Response" if purpose.as_ref().is_some_and(|p| p.0 == Purpose::Download) => {
                return self.download_chunk(cx, &msg);
            }
            "File Write Response" if purpose.as_ref().is_some_and(|p| p.0 == Purpose::Upload) => {
                return self.upload_chunk(cx, &msg);
            }
            _ => {}
        }
        if let Some((Purpose::Command(id, answer), _)) = purpose {
            let mut body = msg.clone();
            if let Some(o) = body.as_object_mut() {
                o.remove("Message");
                o.remove("Meta");
            }
            let result = match answer {
                Answer::Whole => Ok(Outcome::Value { value: body }),
                Answer::Member(name) => Ok(Outcome::Value {
                    value: body.get(name).cloned().unwrap_or(Value::Null),
                }),
                Answer::Result => {
                    let ok = ["Result", "Status"]
                        .iter()
                        .any(|k| body.get(*k).and_then(Value::as_str) == Some("Succeed"));
                    if ok {
                        Ok(Outcome::Value { value: body })
                    } else {
                        Err(CommandError::DeviceError {
                            code: None,
                            message: format!("the JNIOR answered {body}"),
                        })
                    }
                }
            };
            cx.complete(id, result);
        }
    }

    fn refused(&mut self, cx: &mut Cx) {
        self.phase = Phase::Refused;
        self.close_socket(cx);
        for (_, (purpose, _)) in self.pending.drain() {
            if let Purpose::Command(id, _) = purpose {
                cx.complete(
                    id,
                    Err(CommandError::Auth {
                        message: "the JNIOR refused the user name or password".into(),
                    }),
                );
            }
        }
        cx.connection(Connection::Unauthorized {
            reason: "the JNIOR refused the user name or password".into(),
        });
    }

    fn monitor_message(&mut self, cx: &mut Cx, msg: &Value) {
        let list = |k: &str| -> Vec<Value> {
            msg.get(k)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let inputs = list("Inputs");
        let outputs = list("Outputs");
        let mut ins = Map::new();
        for (i, v) in inputs.iter().enumerate() {
            let mut e = Map::new();
            if let Some(s) = v.get("State").and_then(Value::as_i64) {
                e.insert("state".into(), json!(s != 0));
            }
            if let Some(c) = v.get("Count").and_then(Value::as_i64) {
                e.insert("count".into(), json!(c));
            }
            ins.insert((i + 1).to_string(), Value::Object(e));
        }
        let mut outs = Map::new();
        for (i, v) in outputs.iter().enumerate() {
            if let Some(s) = v.get("State").and_then(Value::as_i64) {
                outs.insert((i + 1).to_string(), json!({"state": s != 0}));
            }
        }
        let mut device = Map::new();
        for (k, name) in [
            ("Model", "model"),
            ("Version", "version"),
            ("Serial Number", "serial_number"),
            ("Timestamp", "timestamp"),
        ] {
            if let Some(v) = msg.get(k) {
                let v = match (name, v) {
                    ("serial_number", Value::Number(n)) => json!(n.to_string()),
                    _ => v.clone(),
                };
                device.insert(name.into(), v);
            }
        }
        cx.state(json!({"inputs": ins, "outputs": outs, "device": device}));
        let counts = (inputs.len(), outputs.len());
        if self.monitor && self.io != Some(counts) {
            self.io = Some(counts);
            let mut keys = Vec::new();
            for n in 1..=counts.0 {
                for k in INPUT_KEYS {
                    keys.push(format!("IO/Inputs/din{n}/{k}"));
                }
            }
            // The registry addresses the first 16 relays (ROUT1 to ROUT16).
            for n in 1..=counts.1.min(16) {
                for k in OUTPUT_KEYS {
                    keys.push(format!("IO/Outputs/rout{n}/{k}"));
                }
            }
            if !keys.is_empty() {
                self.request(
                    cx,
                    json!({"Message": "Registry Read", "Keys": keys}),
                    Purpose::Read,
                );
            }
        }
    }

    fn registry(&mut self, cx: &mut Cx, keys: &Map<String, Value>) {
        let mut registry = Map::new();
        let mut ins = Map::new();
        let mut outs = Map::new();
        let mut device = Map::new();
        for (k, v) in keys {
            let key = key_of(k);
            let text = v.as_str().unwrap_or("").to_string();
            registry.insert(key.clone(), json!(text));
            if let Some((which, n, field)) = channel_field(&key) {
                let value = if field == "hour_meter" {
                    match text.trim().parse::<f64>() {
                        Ok(h) => json!(h),
                        Err(_) => continue,
                    }
                } else {
                    json!(text)
                };
                let target = if which == "inputs" {
                    &mut ins
                } else {
                    &mut outs
                };
                let entry = target.entry(n).or_insert_with(|| json!({}));
                entry[field] = value;
            }
            let device_field = match key.as_str() {
                "$BootTime" => Some("boot_time"),
                "$LastNtpSuccess" => Some("last_ntp_success"),
                "$BuildTag" => Some("build_tag"),
                "Device/Desc" => Some("description"),
                "Device/Timezone" => Some("timezone"),
                _ => None,
            };
            if let Some(f) = device_field {
                device.insert(f.into(), json!(text));
            }
        }
        let mut patch = json!({"registry": registry});
        if !ins.is_empty() {
            patch["inputs"] = Value::Object(ins);
        }
        if !outs.is_empty() {
            patch["outputs"] = Value::Object(outs);
        }
        if !device.is_empty() {
            patch["device"] = Value::Object(device);
        }
        cx.state(patch);
    }

    fn download_chunk(&mut self, cx: &mut Cx, msg: &Value) {
        let Some(mut d) = self.download.take() else {
            return;
        };
        let status = msg.get("Status").and_then(Value::as_str).unwrap_or("");
        let data = msg
            .get("Data")
            .and_then(Value::as_str)
            .and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok());
        let (Some(data), "Succeed") = (data, status) else {
            cx.file_close(DOWNLOAD_FILE);
            cx.complete(
                d.id,
                Err(CommandError::DeviceError {
                    code: None,
                    message: format!("the JNIOR could not read {}", d.file),
                }),
            );
            return;
        };
        let size = msg.get("Size").and_then(Value::as_u64).unwrap_or(0);
        d.size = Some(size);
        d.offset += data.len() as u64;
        cx.file_write(DOWNLOAD_FILE, data.clone());
        if data.is_empty() || d.offset >= size {
            cx.file_close(DOWNLOAD_FILE);
            cx.complete(
                d.id,
                Ok(Outcome::Value {
                    value: json!({"file": d.file, "bytes": d.offset}),
                }),
            );
            return;
        }
        let message =
            json!({"Message": "File Read", "File": d.file, "Offset": d.offset, "Limit": CHUNK});
        self.download = Some(d);
        self.request(cx, message, Purpose::Download);
    }

    fn upload_chunk(&mut self, cx: &mut Cx, msg: &Value) {
        let Some(mut u) = self.upload.take() else {
            return;
        };
        let written = msg.get("NumWritten").and_then(Value::as_i64).unwrap_or(-1);
        if written < 0 || msg.get("Status").and_then(Value::as_str) != Some("Succeed") {
            cx.complete(
                u.id,
                Err(CommandError::DeviceError {
                    code: None,
                    message: format!("the JNIOR could not write {}", u.file),
                }),
            );
            return;
        }
        u.offset += written as usize;
        if u.offset >= u.data.len() {
            cx.complete(
                u.id,
                Ok(Outcome::Value {
                    value: json!({"file": u.file, "bytes": u.data.len()}),
                }),
            );
            return;
        }
        let message = self.write_message(&u);
        self.upload = Some(u);
        self.request(cx, message, Purpose::Upload);
    }

    fn write_message(&self, u: &Upload) -> Value {
        let end = (u.offset + CHUNK).min(u.data.len());
        let chunk = &u.data[u.offset..end];
        let mut m = json!({
            "Message": "File Write",
            "File": u.file,
            "Size": chunk.len(),
            "Encoding": "base64",
            "Data": base64::engine::general_purpose::STANDARD.encode(chunk),
        });
        if u.offset > 0 {
            m["Append"] = json!(true);
        }
        m
    }

    fn invalid(cx: &mut Cx, id: CommandId, message: impl Into<String>) {
        cx.complete(
            id,
            Err(CommandError::InvalidParams {
                message: message.into(),
            }),
        );
    }
}

fn list(params: &Params, key: &str) -> Vec<String> {
    text(params, key)
        .split([',', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

impl Module for Jnior {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match self.phase {
            Phase::Refused => {
                cx.complete(
                    id,
                    Err(CommandError::Auth {
                        message: "the JNIOR refused the user name or password".into(),
                    }),
                );
                return;
            }
            Phase::Ready => {}
            _ => {
                cx.complete(id, Err(CommandError::NotConnected));
                return;
            }
        }
        let int = |k: &str| params.get(k).and_then(Value::as_i64).unwrap_or(0);
        let channel = int("channel");
        let duration = int("duration_ms");
        let control = |command: &str| {
            let mut m = json!({"Message": "Control", "Command": command, "Channel": channel});
            if duration > 0 {
                m["Duration"] = json!(duration);
            }
            m
        };
        let (message, purpose) = match name {
            "close_relay" => (control("Close"), None),
            "open_relay" => (control("Open"), None),
            "toggle_relay" => (control("Toggle"), None),
            "set_relays" => {
                let mut m = json!({"Message": "Control", "Command": "Block",
                    "Mask": int("mask"), "States": int("states")});
                if duration > 0 {
                    m["Duration"] = json!(duration);
                }
                (m, None)
            }
            "reset_latch" => (control("Reset Latch"), None),
            "reset_counter" => (control("Reset Counter"), None),
            "reset_usage" => (
                json!({"Message": "Control", "Command": "Reset Usage", "Channel": int("meter")}),
                None,
            ),
            "refresh" => (json!({"Message": "Status"}), None),
            "registry_read" => {
                let keys = list(params, "keys");
                if keys.is_empty() {
                    return Jnior::invalid(cx, id, "give one or more registry keys");
                }
                (
                    json!({"Message": "Registry Read", "Keys": keys}),
                    Some(Answer::Member("Keys")),
                )
            }
            "registry_write" | "registry_write_encrypted" => {
                let key = text(params, "key");
                let value = text(params, "value");
                let message = if name == "registry_write" {
                    "Registry Write"
                } else {
                    "Registry Write Encrypted"
                };
                (
                    json!({"Message": message, "Keys": { key: value }}),
                    Some(Answer::Member("Keys")),
                )
            }
            "registry_list" => (
                json!({"Message": "Registry List", "Node": text(params, "node")}),
                Some(Answer::Member("Keys")),
            ),
            "enumerate_modules" => (
                json!({"Message": "Enumerate Devices"}),
                Some(Answer::Member("Devices")),
            ),
            "read_modules" => {
                let ids = list(params, "modules");
                let ids = if ids.is_empty() {
                    self.modules.clone()
                } else {
                    ids
                };
                if ids.is_empty() {
                    return Jnior::invalid(cx, id, "no expansion module is known: enumerate first");
                }
                (
                    json!({"Message": "Read Devices", "Devices": ids}),
                    Some(Answer::Member("Devices")),
                )
            }
            "write_module" => {
                let hex = text(params, "hex");
                if !hex.len().is_multiple_of(2) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Jnior::invalid(cx, id, "the data block is hex digits in pairs");
                }
                (
                    json!({"Message": "Write Devices",
                           "Devices": [{"Address": text(params, "module"), "Hex": hex.to_ascii_uppercase()}]}),
                    Some(Answer::Member("Devices")),
                )
            }
            "read_clock" => (json!({"Message": "Clock Read"}), Some(Answer::Whole)),
            "set_clock" => (json!({"Message": "Clock Set", "Time": int("time")}), None),
            "read_syslog" => (
                json!({"Message": "Syslog Read"}),
                Some(Answer::Member("Data")),
            ),
            "list_files" => (
                json!({"Message": "File List", "Folder": text(params, "folder")}),
                Some(Answer::Member("Content")),
            ),
            "remove_files" => {
                let files = list(params, "files");
                if files.is_empty() {
                    return Jnior::invalid(cx, id, "give one or more files");
                }
                (
                    json!({"Message": "File Remove", "Files": files}),
                    Some(Answer::Whole),
                )
            }
            "rename_file" => (
                json!({"Message": "File Rename", "Old": text(params, "old"), "New": text(params, "new")}),
                Some(Answer::Result),
            ),
            "make_folder" => (
                json!({"Message": "File Mkdir", "Folder": text(params, "folder")}),
                Some(Answer::Result),
            ),
            "download_file" => {
                if self.download.is_some() {
                    return Jnior::invalid(cx, id, "a download is already running");
                }
                let file = text(params, "file");
                cx.file_open(DOWNLOAD_FILE, text(params, "path"));
                self.download = Some(Download {
                    id,
                    file: file.clone(),
                    offset: 0,
                    size: None,
                });
                let message =
                    json!({"Message": "File Read", "File": file, "Offset": 0, "Limit": CHUNK});
                self.request(cx, message, Purpose::Download);
                return;
            }
            "upload_file" => {
                if self.upload.is_some() {
                    return Jnior::invalid(cx, id, "an upload is already running");
                }
                self.upload = Some(Upload {
                    id,
                    file: text(params, "file"),
                    data: Vec::new(),
                    offset: 0,
                });
                cx.file_read(UPLOAD_FILE, text(params, "path"));
                return;
            }
            "console_open" => (
                json!({"Message": "Console Open"}),
                Some(Answer::Member("Status")),
            ),
            "console_input" => (
                json!({"Message": "Console Stdin", "Data": text(params, "data")}),
                None,
            ),
            "console_close" => (
                json!({"Message": "Console Close"}),
                Some(Answer::Member("Status")),
            ),
            other => {
                cx.complete(
                    id,
                    Err(CommandError::UnknownCommand {
                        command: other.into(),
                    }),
                );
                return;
            }
        };
        match purpose {
            Some(answer) => self.request(cx, message, Purpose::Command(id, answer)),
            None => {
                self.send(cx, &message);
                cx.complete(id, Ok(Outcome::Unverified));
            }
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => self.opened(cx),
            TcpInput::Data(bytes) => {
                for msg in self.deframer.feed(&bytes) {
                    self.received(cx, msg);
                    if matches!(self.phase, Phase::Closed | Phase::Refused) {
                        return;
                    }
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn ws(&mut self, cx: &mut Cx, _socket: Key, input: WsInput) {
        match input {
            WsInput::Opened => self.opened(cx),
            WsInput::Text(t) => {
                if let Ok(msg) = serde_json::from_str::<Value>(&t) {
                    self.received(cx, msg);
                }
            }
            WsInput::Closed { reason, .. } => self.lost(cx, reason),
            _ => {}
        }
    }

    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        match (file, input) {
            (UPLOAD_FILE, FileInput::Read { data }) => {
                let Some(mut u) = self.upload.take() else {
                    return;
                };
                if self.phase != Phase::Ready {
                    cx.complete(u.id, Err(CommandError::NotConnected));
                    return;
                }
                u.data = data;
                let message = self.write_message(&u);
                self.upload = Some(u);
                self.request(cx, message, Purpose::Upload);
            }
            (UPLOAD_FILE, FileInput::Failed { message }) => {
                if let Some(u) = self.upload.take() {
                    cx.complete(u.id, Err(CommandError::InvalidParams { message }));
                }
            }
            (DOWNLOAD_FILE, FileInput::Failed { message }) => {
                if let Some(d) = self.download.take() {
                    cx.complete(d.id, Err(CommandError::InvalidParams { message }));
                }
            }
            _ => {}
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER if self.phase == Phase::Closed => self.open(cx),
            IDLE_TIMER if matches!(self.phase, Phase::LoggingIn | Phase::Digest | Phase::Ready) => {
                if self.phase != Phase::Ready && cx.now().saturating_sub(self.heard_at) >= IDLE {
                    self.close_socket(cx);
                    self.lost(cx, "no answer to the login".into());
                    return;
                }
                if self.phase == Phase::Ready {
                    // A clock read is the liveness check; one still pending
                    // after a whole interval means the unit is gone.
                    let stale = self.pending.values().any(|(p, at)| {
                        *p == Purpose::Liveness && cx.now().saturating_sub(*at) >= IDLE
                    });
                    if stale {
                        self.close_socket(cx);
                        self.lost(cx, "no answer from the JNIOR".into());
                        return;
                    }
                    if cx.now().saturating_sub(self.heard_at) >= IDLE {
                        self.request(cx, json!({"Message": "Clock Read"}), Purpose::Liveness);
                    }
                }
                cx.set_timer(IDLE_TIMER, IDLE);
            }
            MODULE_TIMER if self.phase == Phase::Ready && self.monitor => {
                if !self.modules.is_empty() {
                    let ids = self.modules.clone();
                    self.request(
                        cx,
                        json!({"Message": "Read Devices", "Devices": ids}),
                        Purpose::Read,
                    );
                }
                cx.set_timer(MODULE_TIMER, self.module_poll);
            }
            REPLY_TIMER => {
                let now = cx.now();
                let late: Vec<u64> = self
                    .pending
                    .iter()
                    .filter(|(_, (p, at))| {
                        *p != Purpose::Liveness && now.saturating_sub(*at) >= self.timeout
                    })
                    .map(|(k, _)| *k)
                    .collect();
                for k in late {
                    if let Some((purpose, _)) = self.pending.remove(&k) {
                        match purpose {
                            Purpose::Command(id, _) => cx.complete(id, Err(CommandError::Timeout)),
                            Purpose::Download => {
                                if let Some(d) = self.download.take() {
                                    cx.file_close(DOWNLOAD_FILE);
                                    cx.complete(d.id, Err(CommandError::Timeout));
                                }
                            }
                            Purpose::Upload => {
                                if let Some(u) = self.upload.take() {
                                    cx.complete(u.id, Err(CommandError::Timeout));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(oldest) = self
                    .pending
                    .values()
                    .filter(|(p, _)| *p != Purpose::Liveness)
                    .map(|(_, at)| *at)
                    .min()
                {
                    cx.set_timer(
                        REPLY_TIMER,
                        (oldest + self.timeout).saturating_sub(now).max(1),
                    );
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.close_socket(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::Ipv4Addr;

    fn module(monitor: bool, settings: Value) -> Jnior {
        let spec = Catalog::source_tree().devices["integ-jnior"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        Jnior::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 41)),
            host_name: None,
            port: None,
            model: "jnior-410".into(),
            channels: None,
            settings,
            monitor,
        })
        .unwrap()
    }

    /// The messages sent, unwrapped.
    fn sent(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(Deframer::default().feed(data)),
                Action::WsSend { text, .. } => Some(vec![serde_json::from_str(text).unwrap()]),
                _ => None,
            })
            .flatten()
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

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn feed(m: &mut Jnior, now: Millis, msg: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(wrap(&msg)));
        cx.take()
    }

    fn run(m: &mut Jnior, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn logged_in(m: &mut Jnior) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        feed(
            m,
            0,
            json!({"Message": "Error", "Text": "401 Unauthorized", "Nonce": "bc581a9683d3e1857218db135e4b"}),
        );
        feed(
            m,
            1,
            json!({"Message": "Authenticated", "Administrator": true, "Control": true}),
        )
    }

    #[test]
    fn the_digest_matches_the_manuals_examples() {
        assert_eq!(
            auth_digest("jnior", "bc581a9683d3e1857218db135e4b", "jnior"),
            "jnior:6b7b418f223e7e0dc600c41c7b6644b3"
        );
        assert_eq!(
            auth_digest("jnior", "5d894efb48e1c3bc074fe78e7a5f", "jnior"),
            "jnior:65f2d1cb66ef63f7d17a764f3a2f2508"
        );
    }

    #[test]
    fn messages_are_wrapped_with_their_length() {
        assert_eq!(wrap(&json!({"Message": ""})), b"[14,{\"Message\":\"\"}]");
        let mut d = Deframer::default();
        let bytes = b"junk[ 20, {\"Message\":\"Status\"} ]\r\n[14,{\"Message\":\"\"}]";
        assert_eq!(
            d.feed(&bytes[..12]),
            Vec::<Value>::new(),
            "a partial message waits"
        );
        assert_eq!(
            d.feed(&bytes[12..]),
            [json!({"Message": "Status"}), json!({"Message": ""})]
        );
        // A wrong length is dropped, and reading goes on.
        let mut d = Deframer::default();
        assert_eq!(
            d.feed(b"[5,{\"Message\":\"\"}][14,{\"Message\":\"\"}]"),
            [json!({"Message": ""})]
        );
    }

    #[test]
    fn it_logs_in_and_reads_the_unit_once_monitored() {
        let mut m = module(true, json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&cx.take()), [json!({"Message": ""})]);
        let a = feed(
            &mut m,
            0,
            json!({"Message": "Error", "Text": "401 Unauthorized", "Nonce": "bc581a9683d3e1857218db135e4b"}),
        );
        assert_eq!(
            sent(&a),
            [json!({"Auth-Digest": "jnior:6b7b418f223e7e0dc600c41c7b6644b3"})]
        );
        let a = feed(
            &mut m,
            1,
            json!({"Message": "Authenticated", "Administrator": true, "Control": true}),
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let s = sent(&a);
        assert_eq!(s[0]["Message"], "Registry Read");
        assert_eq!(s[1]["Message"], "Enumerate Devices");
        assert_eq!(s[2]["Message"], "Clock Read");
        assert_eq!(
            state(&a)["login"],
            json!({"administrator": true, "control": true})
        );

        // The Monitor message the unit pushes.
        let a = feed(
            &mut m,
            2,
            json!({"Message": "Monitor", "Model": "410", "Version": "v1.4", "Serial Number": 614070500,
                   "Inputs": [{"State": 1, "Count": 49}, {"State": 0, "Count": 360}],
                   "Outputs": [{"State": 0}, {"State": 1}], "Timestamp": 1444155435066u64}),
        );
        let st = state(&a);
        assert_eq!(st["inputs"]["1"], json!({"state": true, "count": 49}));
        assert_eq!(st["outputs"]["2"]["state"], true);
        assert_eq!(st["device"]["serial_number"], "614070500");
        // Its I/O configuration is read once the channels are known.
        let reads = sent(&a);
        assert_eq!(reads[0]["Message"], "Registry Read");
        let keys = reads[0]["Keys"].as_array().unwrap();
        assert!(keys.contains(&json!("IO/Inputs/din2/Desc")));
        assert!(keys.contains(&json!("IO/Outputs/rout2/$HourMeter")));

        // Responses, matched by their Meta, and pushed registry updates.
        let id = reads[0]["Meta"]["Id"].clone();
        let a = feed(
            &mut m,
            5,
            json!({"Message": "Registry Response", "Meta": {"Id": id},
                   "Keys": {"/IO/Inputs/din1/Desc": "Door", "IO/Outputs/rout2/$HourMeter": "43.68"}}),
        );
        assert!(a.contains(&Action::RoundTrip(3)));
        let st = state(&a);
        assert_eq!(st["inputs"]["1"]["description"], "Door");
        assert_eq!(st["outputs"]["2"]["hour_meter"], 43.68);
        assert_eq!(st["registry"]["IO/Inputs/din1/Desc"], "Door");

        // Expansion modules are read after enumerating them.
        let enumerate = s[1]["Meta"]["Id"].clone();
        let a = feed(
            &mut m,
            6,
            json!({"Message": "Enumerate Devices Response", "Meta": {"Id": enumerate},
                   "Devices": ["CD111090708109FB"]}),
        );
        assert_eq!(
            state(&a)["modules"]["CD111090708109FB"]["type_name"],
            "4rout"
        );
        assert_eq!(sent(&a)[0]["Message"], "Read Devices");
        let a = feed(
            &mut m,
            7,
            json!({"Message": "Read Devices Response",
                   "Devices": [{"Address": "CD111090708109FB", "Hex": "0F050000000000000000"}]}),
        );
        assert_eq!(
            state(&a)["modules"]["CD111090708109FB"]["relays"],
            json!({"a": true, "b": false, "c": true, "d": false})
        );
    }

    #[test]
    fn control_messages_are_unverified_and_requests_answered_by_meta() {
        let mut m = module(false, json!({}));
        let a = logged_in(&mut m);
        // Commands only: nothing read after logging in.
        assert!(sent(&a).is_empty());

        let a = run(
            &mut m,
            1,
            "close_relay",
            json!({"channel": 2, "duration_ms": 5000}),
        );
        assert_eq!(
            sent(&a),
            [json!({"Message": "Control", "Command": "Close", "Channel": 2, "Duration": 5000})]
        );
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Unverified)));

        let a = run(&mut m, 2, "set_relays", json!({"mask": 7, "states": 5}));
        assert_eq!(
            sent(&a),
            [json!({"Message": "Control", "Command": "Block", "Mask": 7, "States": 5})]
        );

        let a = run(
            &mut m,
            3,
            "registry_write",
            json!({"key": "IO/Inputs/din2/Desc", "value": "Part Produced"}),
        );
        let msg = &sent(&a)[0];
        assert_eq!(msg["Keys"], json!({"IO/Inputs/din2/Desc": "Part Produced"}));
        let meta = msg["Meta"].clone();
        // Something else arriving first is not the answer.
        let a = feed(
            &mut m,
            2,
            json!({"Message": "Registry Update", "Keys": {"x": "1"}}),
        );
        assert_eq!(completed(&a, 3), None);
        let a = feed(
            &mut m,
            3,
            json!({"Message": "Registry Response", "Meta": meta, "Keys": {"IO/Inputs/din2/Desc": "Part Produced"}}),
        );
        assert_eq!(
            completed(&a, 3),
            Some(Ok(Outcome::Value {
                value: json!({"IO/Inputs/din2/Desc": "Part Produced"})
            }))
        );

        let a = run(
            &mut m,
            4,
            "rename_file",
            json!({"old": "/flash/a", "new": "/flash/b"}),
        );
        let meta = sent(&a)[0]["Meta"].clone();
        let a = feed(
            &mut m,
            4,
            json!({"Message": "File Rename Response", "Meta": meta, "Old": "/flash/a", "New": "/flash/b", "Result": "Fail"}),
        );
        assert!(matches!(
            completed(&a, 4),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn a_second_challenge_is_a_refused_credential() {
        let mut m = module(true, json!({"password": "wrong"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let challenge = json!({"Message": "Error", "Text": "401 Unauthorized", "Nonce": "abc"});
        feed(&mut m, 0, challenge.clone());
        let a = feed(&mut m, 1, challenge);
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a.iter().any(|a| matches!(
            a,
            Action::SetTimer {
                key: RETRY_TIMER,
                ..
            }
        )));
    }

    #[test]
    fn files_are_downloaded_in_chunks() {
        let mut m = module(false, json!({}));
        logged_in(&mut m);
        let a = run(
            &mut m,
            9,
            "download_file",
            json!({"file": "/flash/x.txt", "path": "C:/tmp/x.txt"}),
        );
        assert!(a.contains(&Action::FileOpen {
            file: DOWNLOAD_FILE,
            path: "C:/tmp/x.txt".into()
        }));
        let req = &sent(&a)[0];
        assert_eq!(req["Message"], "File Read");
        assert_eq!(req["Offset"], 0);
        let a = feed(
            &mut m,
            2,
            json!({"Message": "File Read Response", "Meta": req["Meta"], "File": "/flash/x.txt",
                   "Size": 5, "Offset": 0, "NumRead": 5, "Encoding": "base64", "Data": "aGVsbG8=", "Status": "Succeed"}),
        );
        assert!(a.contains(&Action::FileWrite {
            file: DOWNLOAD_FILE,
            data: b"hello".to_vec()
        }));
        assert!(a.contains(&Action::FileClose {
            file: DOWNLOAD_FILE
        }));
        assert_eq!(
            completed(&a, 9),
            Some(Ok(Outcome::Value {
                value: json!({"file": "/flash/x.txt", "bytes": 5})
            }))
        );
    }

    #[test]
    fn the_websocket_carries_the_messages_unwrapped() {
        let mut m = module(true, json!({"transport": "wss"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::WsOpen { request, .. }
            if request.url == "wss://192.0.2.41:443/" && request.accept_invalid_certs)));
        let mut cx = Cx::new(0);
        m.ws(&mut cx, SOCKET, WsInput::Opened);
        assert!(cx.take().contains(&Action::WsSend {
            socket: SOCKET,
            text: "{\"Message\":\"\"}".into()
        }));
    }
}
