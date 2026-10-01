//! Sony Alpha, FX, ZV, cinema line and pan/tilt cameras over Sony's Camera
//! Control PTP 3, carried by PTP-IP on TCP 15740.
//!
//! The session, as Sony's Camera Control PTP 3 Reference (2.02.00) lays it
//! out: a PTP-IP command connection opened with Init Command Request (this
//! initiator's GUID and friendly name), an event connection opened with Init
//! Event Request (the connection number the camera gave), OpenSession, then
//! Sony's handshake (SDIO_Connect phases 1 and 2, SDIO_GetExtDeviceInfo until
//! it returns data, SDIO_Connect phase 3). After that the camera's whole
//! property set is read with SDIO_GetAllExtDevicePropInfo, and read again,
//! changes only, when the camera signals a change and on a steady poll, which
//! the reference recommends because events alone are not guaranteed. Writes
//! are SDIO_SetExtDevicePropValue and SDIO_ControlDevice; pan/tilt cameras add
//! SDIO_ControlPTZF and SDIO_SetPresetPTZF.
//!
//! PTP allows one transaction at a time on the command connection, so every
//! operation goes through one queue: commands first, then background reads.
//!
//! A camera with pairing enabled shows the friendly name and waits for the
//! operator to approve it before answering Init Command Request. The module
//! reports that wait in the state and never retries it: a refusal, or the
//! camera closing the connection while waiting, is final until the host opens
//! the device again. A camera with SSH enabled accepts PTP-IP only through an
//! SSH tunnel to its own localhost:15740, which the session opens when the
//! `connection` setting is `ssh`.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use super::sony_camera_dataset::{
    self as ds, dt, parse_device_info, parse_ext_device_info, parse_prop_info_array, Enabled, Form,
    PropInfo, PtpValue,
};
use super::sony_camera_props::{self as props, Decode};
use super::sony_camera_ptpip::{
    request_with_data, DataPhase, FailReason, Framer, Packet, PROTOCOL_VERSION,
};
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    SshTunnel, TcpInput,
};

const PTP_IP_PORT: u16 = 15740;
const SSH_PORT: u16 = 22;

const CMD: Key = "command";
const EVT: Key = "event";

const REPLY: Key = "reply";
const INIT: Key = "init";
const RETRY: Key = "retry";
const POLL: Key = "poll";
const PAUSE: Key = "pause";
const EXT_RETRY: Key = "ext-info-retry";

const INIT_TIMEOUT: Millis = 10_000;
const REPLY_TIMEOUT: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// SDIO_GetExtDeviceInfo returns no data until the camera is ready; the
/// reference says to ask again until it does.
const EXT_INFO_RETRY: Millis = 500;
const EXT_INFO_ATTEMPTS: u32 = 40;
/// Between the down and up halves of a momentary button press.
const PRESS: Millis = 100;

const OP_GET_DEVICE_INFO: u16 = 0x1001;
const OP_OPEN_SESSION: u16 = 0x1002;
const OP_CLOSE_SESSION: u16 = 0x1003;
const OP_CONNECT: u16 = 0x9201;
const OP_EXT_DEVICE_INFO: u16 = 0x9202;
const OP_SET_PROPERTY: u16 = 0x9205;
const OP_CONTROL: u16 = 0x9207;
const OP_GET_ALL_PROPERTIES: u16 = 0x9209;
const OP_CONTROL_PTZF: u16 = 0x9245;
const OP_SET_PRESET_PTZF: u16 = 0x9246;

const RC_OK: u16 = 0x2001;
const RC_SESSION_ALREADY_OPEN: u16 = 0x201E;
const RC_AUTHENTICATION_FAILED: u16 = 0xA101;

/// The Camera Control PTP version this module speaks, 100 times 3.00.
const INITIATOR_VERSION: u32 = 0x012C;
/// Vendor code version from which the extended codes (0xE000 properties,
/// 0xF000 controls) need the option flag, 100 times 3.10.
const EXTENDED_CODES_VERSION: u32 = 310;

const UP: i128 = 1;
const DOWN: i128 = 2;

const PTZF_ABSOLUTE: u32 = 1;
const PTZF_RELATIVE: u32 = 2;
const PTZF_DIRECTION: u32 = 3;
const PTZF_HOME: u32 = 4;
const PTZF_RESET: u32 = 5;
const PTZF_CANCEL: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Plain,
    Pairing,
    Ssh,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Plain => "plain",
            Mode::Pairing => "pairing",
            Mode::Ssh => "ssh",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Nothing open; a reconnect may be pending.
    Idle,
    ConnectingCommand,
    AwaitInitAck,
    ConnectingEvent,
    AwaitEventAck,
    /// Both connections up: the handshake and then normal operation.
    Session,
    /// The camera refused this initiator. Nothing more is sent until the host
    /// opens the device again.
    Refused,
}

#[derive(Debug, Clone, PartialEq)]
enum Step {
    OpenSession,
    DeviceInfo,
    Connect(u32),
    ExtInfo {
        extended: bool,
    },
    GetAll {
        diff: bool,
    },
    /// Part of an operator command.
    Command,
    /// A wait between two parts of a command.
    Pause(Millis),
}

#[derive(Debug, Clone)]
struct Op {
    code: u16,
    params: Vec<u32>,
    data: Option<Vec<u8>>,
    step: Step,
    command: Option<CommandId>,
    /// The command completes when this operation succeeds.
    last: bool,
}

impl Op {
    fn new(code: u16, params: Vec<u32>, step: Step) -> Op {
        Op {
            code,
            params,
            data: None,
            step,
            command: None,
            last: false,
        }
    }

    fn with_data(code: u16, params: Vec<u32>, data: Vec<u8>) -> Op {
        Op {
            code,
            params,
            data: Some(data),
            step: Step::Command,
            command: None,
            last: false,
        }
    }

    fn pause(ms: Millis) -> Op {
        Op::new(0, vec![], Step::Pause(ms))
    }
}

struct InFlight {
    op: Op,
    transaction: u32,
    data: Vec<u8>,
}

pub(crate) struct SonyCamera {
    host: IpAddr,
    port: Option<u16>,
    mode: Mode,
    friendly_name: String,
    guid: [u8; 16],
    ssh_username: String,
    ssh_password: String,
    ssh_fingerprint: Option<String>,
    poll_every: Millis,

    phase: Phase,
    cmd_framer: Framer,
    evt_framer: Framer,
    connection_number: u32,
    /// The camera accepted this initiator at least once in this session.
    paired: bool,
    retry_after: Millis,
    next_transaction: u32,
    commands: VecDeque<Op>,
    background: VecDeque<Op>,
    current: Option<InFlight>,
    pausing: Option<Op>,
    ext_info_attempts: u32,

    /// The handshake is done and the property set has been read once.
    ready: bool,
    extended: bool,
    controls: Vec<u16>,
    props: HashMap<u16, PropInfo>,
    reported_raw: HashMap<u16, Value>,
    reported_named: HashMap<&'static str, Value>,
}

fn text_setting(settings: &Params, name: &str) -> String {
    settings
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// A GUID from the friendly name, so the camera recognises this initiator
/// across restarts without anything being stored.
fn derived_guid(friendly_name: &str) -> [u8; 16] {
    let digest = Sha256::digest(format!("meros-sony-camera:{friendly_name}").as_bytes());
    digest[..16].try_into().unwrap()
}

fn parse_guid(text: &str) -> Option<[u8; 16]> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn hex(code: u16) -> String {
    format!("{code:04X}")
}

/// Sets a dotted path in a merge patch.
fn put(patch: &mut Value, path: &str, value: Value) {
    let mut node = patch;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if !node.is_object() {
            *node = Value::Object(Map::new());
        }
        let map = node.as_object_mut().unwrap();
        if parts.peek().is_none() {
            map.insert(part.to_string(), value);
            return;
        }
        node = map
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn refused(code: &str, message: impl Into<String>) -> CommandError {
    CommandError::DeviceError {
        code: Some(code.into()),
        message: message.into(),
    }
}

fn param_bool(params: &Params, name: &str) -> Option<bool> {
    params.get(name).and_then(Value::as_bool)
}

fn param_int(params: &Params, name: &str) -> Result<i128, CommandError> {
    params
        .get(name)
        .and_then(|v| {
            v.as_i64()
                .map(|i| i as i128)
                .or_else(|| v.as_u64().map(|u| u as i128))
        })
        .ok_or_else(|| invalid(format!("'{name}' must be an integer")))
}

fn param_int_or(params: &Params, name: &str, default: i128) -> Result<i128, CommandError> {
    if params.contains_key(name) {
        param_int(params, name)
    } else {
        Ok(default)
    }
}

fn param_str<'a>(params: &'a Params, name: &str) -> Result<&'a str, CommandError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{name}' must be text")))
}

/// A property or control code given as `0xD21D`, `D21D` or a number.
fn param_code(params: &Params) -> Result<u16, CommandError> {
    match params.get("code") {
        Some(Value::String(s)) => {
            let digits = s.trim_start_matches("0x").trim_start_matches("0X");
            u16::from_str_radix(digits, 16).map_err(|_| invalid(format!("'{s}' is not a hex code")))
        }
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|v| u16::try_from(v).ok())
            .ok_or_else(|| invalid(format!("{n} is not a 16-bit code"))),
        _ => Err(invalid("'code' is required")),
    }
}

/// A raw value for a given datatype: a number, a decimal or 0x-hex string
/// for integer types, any text for string types.
fn raw_value(datatype: u16, value: &Value) -> Result<PtpValue, CommandError> {
    if datatype == dt::STR {
        return match value {
            Value::String(s) => Ok(PtpValue::Str(s.clone())),
            other => Err(invalid(format!("{other} is not text"))),
        };
    }
    let parsed = match value {
        Value::Number(n) => n
            .as_i64()
            .map(|v| v as i128)
            .or_else(|| n.as_u64().map(|v| v as i128)),
        Value::String(s) => {
            let t = s.trim();
            if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                i128::from_str_radix(h, 16).ok()
            } else {
                t.parse::<i128>().ok()
            }
        }
        _ => None,
    };
    parsed
        .map(PtpValue::Int)
        .ok_or_else(|| invalid(format!("{value} is not an integer")))
}

fn ptzf_type_name(v: u32) -> &'static str {
    match v {
        PTZF_ABSOLUTE => "absolute",
        PTZF_RELATIVE => "relative",
        PTZF_DIRECTION => "direction",
        PTZF_HOME => "home",
        PTZF_RESET => "reset",
        PTZF_CANCEL => "cancel",
        _ => "unknown",
    }
}

impl SonyCamera {
    pub(crate) fn new(ctx: OpenContext) -> Result<SonyCamera, String> {
        let s = &ctx.settings;
        let mode = match text_setting(s, "connection").as_str() {
            "" | "plain" => Mode::Plain,
            "pairing" => Mode::Pairing,
            "ssh" => Mode::Ssh,
            other => return Err(format!("unknown connection mode '{other}'")),
        };
        let mut friendly_name = text_setting(s, "friendly_name");
        if friendly_name.is_empty() {
            friendly_name = "Meros".into();
        }
        let guid_text = text_setting(s, "guid");
        let guid = if guid_text.is_empty() {
            derived_guid(&friendly_name)
        } else {
            parse_guid(&guid_text).ok_or("guid must be 32 hex digits")?
        };
        let ssh_username = text_setting(s, "ssh_username");
        let ssh_password = text_setting(s, "ssh_password");
        if mode == Mode::Ssh && (ssh_username.is_empty() || ssh_password.is_empty()) {
            return Err(
                "connection 'ssh' needs ssh_username and ssh_password (the camera's Access Authentication user)"
                    .into(),
            );
        }
        let fingerprint = text_setting(s, "ssh_fingerprint");
        let poll_every = s
            .get("poll_ms")
            .and_then(Value::as_u64)
            .unwrap_or(1_000)
            .max(250);
        Ok(SonyCamera {
            host: ctx.host,
            port: ctx.port,
            mode,
            friendly_name,
            guid,
            ssh_username,
            ssh_password,
            ssh_fingerprint: (!fingerprint.is_empty()).then_some(fingerprint),
            poll_every,
            phase: Phase::Idle,
            cmd_framer: Framer::default(),
            evt_framer: Framer::default(),
            connection_number: 0,
            paired: false,
            retry_after: RETRY_MIN,
            next_transaction: 1,
            commands: VecDeque::new(),
            background: VecDeque::new(),
            current: None,
            pausing: None,
            ext_info_attempts: 0,
            ready: false,
            extended: false,
            controls: Vec::new(),
            props: HashMap::new(),
            reported_raw: HashMap::new(),
            reported_named: HashMap::new(),
        })
    }

    // ----- connections -----

    /// Opens one of the two PTP-IP connections, directly or through the SSH
    /// tunnel the camera requires when its Access Authentication is on.
    fn open(&self, cx: &mut Cx, socket: Key) {
        match self.mode {
            Mode::Ssh => cx.tcp_open_ssh(
                socket,
                SshTunnel {
                    ssh: SocketAddr::new(self.host, self.port.unwrap_or(SSH_PORT)),
                    username: self.ssh_username.clone(),
                    password: self.ssh_password.clone(),
                    fingerprint: self.ssh_fingerprint.clone(),
                    cipher: Some("aes128-ctr".into()),
                    target_host: "localhost".into(),
                    target_port: PTP_IP_PORT,
                },
            ),
            Mode::Plain | Mode::Pairing => cx.tcp_open(
                socket,
                SocketAddr::new(self.host, self.port.unwrap_or(PTP_IP_PORT)),
            ),
        }
    }

    fn connect(&mut self, cx: &mut Cx) {
        self.phase = Phase::ConnectingCommand;
        cx.connection(Connection::Connecting);
        self.open(cx, CMD);
        cx.set_timer(INIT, INIT_TIMEOUT);
    }

    /// Clears the session; the caller decides whether to try again.
    fn teardown(&mut self, cx: &mut Cx, error: CommandError) {
        let mut failed = Vec::new();
        if let Some(inflight) = self.current.take() {
            failed.extend(inflight.op.command);
        }
        failed.extend(self.pausing.take().and_then(|op| op.command));
        failed.extend(self.commands.drain(..).filter_map(|op| op.command));
        failed.extend(self.background.drain(..).filter_map(|op| op.command));
        failed.dedup();
        for id in failed {
            cx.complete(id, Err(error.clone()));
        }
        cx.tcp_close(CMD);
        cx.tcp_close(EVT);
        for key in [REPLY, INIT, POLL, PAUSE, EXT_RETRY] {
            cx.cancel_timer(key);
        }
        self.cmd_framer = Framer::default();
        self.evt_framer = Framer::default();
        self.ready = false;
        self.ext_info_attempts = 0;
        self.props.clear();
        self.reported_raw.clear();
        self.reported_named.clear();
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if matches!(self.phase, Phase::Idle | Phase::Refused) {
            return;
        }
        self.teardown(
            cx,
            CommandError::Transport {
                message: reason.clone(),
            },
        );
        self.phase = Phase::Idle;
        cx.log(
            Level::Warning,
            format!("Sony camera connection lost: {reason}"),
        );
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// Final until the host opens the device again: retrying a refused
    /// pairing or login would only prompt or lock out the camera.
    fn refuse(&mut self, cx: &mut Cx, reason: String) {
        if self.phase == Phase::Refused {
            return;
        }
        self.teardown(
            cx,
            CommandError::Auth {
                message: reason.clone(),
            },
        );
        cx.cancel_timer(RETRY);
        self.phase = Phase::Refused;
        if self.mode == Mode::Pairing && !self.paired {
            cx.state(json!({"session": {"pairing": "refused"}}));
        }
        cx.log(
            Level::Warning,
            format!("Sony camera refused the connection: {reason}"),
        );
        cx.connection(Connection::Unauthorized { reason });
    }

    fn closed(&mut self, cx: &mut Cx, socket: Key, reason: String) {
        if matches!(self.phase, Phase::Idle | Phase::Refused) {
            return;
        }
        if reason.starts_with(crate::ssh::REFUSED) {
            return self.refuse(cx, reason);
        }
        let which = if socket == EVT { "event" } else { "command" };
        let reason = format!("{which} connection: {reason}");
        if self.mode == Mode::Pairing && self.phase == Phase::AwaitInitAck && !self.paired {
            return self.refuse(
                cx,
                format!(
                    "the camera closed the connection before pairing was approved ({reason}); open the device again with the camera's pairing menu showing"
                ),
            );
        }
        self.lost(cx, reason);
    }

    // ----- the operation queue -----

    fn transaction(&mut self, code: u16) -> u32 {
        if code == OP_OPEN_SESSION {
            // OpenSession is transaction 0; numbering restarts after it.
            self.next_transaction = 1;
            return 0;
        }
        let t = self.next_transaction;
        self.next_transaction = match t.wrapping_add(1) {
            0 | 0xFFFF_FFFF => 1,
            n => n,
        };
        t
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Session || self.current.is_some() || self.pausing.is_some() {
            return;
        }
        let Some(op) = self
            .commands
            .pop_front()
            .or_else(|| self.background.pop_front())
        else {
            return;
        };
        if let Step::Pause(ms) = op.step {
            self.pausing = Some(op);
            cx.set_timer(PAUSE, ms);
            return;
        }
        let transaction = self.transaction(op.code);
        let bytes = match &op.data {
            Some(data) => request_with_data(op.code, transaction, &op.params, data),
            None => Packet::OperationRequest {
                phase: DataPhase::NoneOrIn,
                code: op.code,
                transaction,
                params: op.params.clone(),
            }
            .encode(),
        };
        cx.tcp_send(CMD, bytes);
        cx.set_timer(REPLY, REPLY_TIMEOUT);
        self.current = Some(InFlight {
            op,
            transaction,
            data: Vec::new(),
        });
    }

    fn flag(&self) -> u32 {
        self.extended as u32
    }

    /// Reads the property set again: changes only unless `full`.
    fn want_refresh(&mut self, cx: &mut Cx, full: bool) {
        if !self.ready {
            return;
        }
        let queued = self
            .background
            .iter_mut()
            .find(|op| matches!(op.step, Step::GetAll { .. }));
        match queued {
            Some(op) => {
                if full {
                    op.step = Step::GetAll { diff: false };
                    op.params[0] = 0;
                }
            }
            None => {
                let op = self.get_all(!full);
                self.background.push_back(op);
            }
        }
        self.pump(cx);
    }

    fn get_all(&self, diff: bool) -> Op {
        Op::new(
            OP_GET_ALL_PROPERTIES,
            vec![diff as u32, self.flag()],
            Step::GetAll { diff },
        )
    }

    fn start_session(&mut self, cx: &mut Cx) {
        self.phase = Phase::Session;
        self.background.extend([
            Op::new(OP_OPEN_SESSION, vec![1], Step::OpenSession),
            Op::new(OP_GET_DEVICE_INFO, vec![], Step::DeviceInfo),
            Op::new(OP_CONNECT, vec![1, 0, 0], Step::Connect(1)),
            Op::new(OP_CONNECT, vec![2, 0, 0], Step::Connect(2)),
            Op::new(
                OP_EXT_DEVICE_INFO,
                vec![INITIATOR_VERSION, 0],
                Step::ExtInfo { extended: false },
            ),
        ]);
        self.pump(cx);
    }

    fn response(&mut self, cx: &mut Cx, code: u16, transaction: u32, params: Vec<u32>) {
        let matches = self
            .current
            .as_ref()
            .is_some_and(|c| c.transaction == transaction);
        if !matches {
            cx.log(
                Level::Debug,
                format!("response 0x{code:04X} to transaction {transaction}, which is not pending"),
            );
            return;
        }
        cx.cancel_timer(REPLY);
        let inflight = self.current.take().unwrap();
        self.finish(cx, inflight.op, code, &params, inflight.data);
        self.pump(cx);
    }

    fn finish(&mut self, cx: &mut Cx, op: Op, code: u16, params: &[u32], data: Vec<u8>) {
        let ok = code == RC_OK;
        match op.step {
            Step::OpenSession => {
                if !ok && code != RC_SESSION_ALREADY_OPEN {
                    self.lost(
                        cx,
                        format!("OpenSession failed: {}", props::response_name(code)),
                    );
                }
            }
            Step::DeviceInfo => match parse_device_info(&data) {
                Ok(info) if ok => cx.state(json!({"device": {
                    "manufacturer": info.manufacturer,
                    "model": info.model,
                    "version": info.version,
                    "serial": info.serial,
                }})),
                _ => cx.log(Level::Debug, "the camera's DeviceInfo could not be read"),
            },
            Step::Connect(phase) => {
                if code == RC_AUTHENTICATION_FAILED {
                    self.refuse(
                        cx,
                        "the camera refused Sony's authentication handshake".into(),
                    );
                } else if !ok {
                    self.lost(
                        cx,
                        format!(
                            "SDIO_Connect phase {phase} failed: {}",
                            props::response_name(code)
                        ),
                    );
                }
            }
            Step::ExtInfo { extended } => self.ext_info(cx, extended, code, params, &data),
            Step::GetAll { diff } => {
                if ok {
                    match parse_prop_info_array(&data) {
                        Ok(list) => self.apply(cx, list),
                        Err(e) => cx.log(Level::Warning, format!("unreadable property set: {e}")),
                    }
                    if !self.ready {
                        self.ready = true;
                        self.retry_after = RETRY_MIN;
                        cx.connection(Connection::Connected);
                        cx.set_timer(POLL, self.poll_every);
                    }
                } else if !self.ready {
                    self.lost(
                        cx,
                        format!(
                            "reading the camera's properties failed: {}",
                            props::response_name(code)
                        ),
                    );
                } else if !diff {
                    cx.log(
                        Level::Warning,
                        format!("property read failed: {}", props::response_name(code)),
                    );
                }
                if let Some(id) = op.command {
                    let result = if ok {
                        Ok(Outcome::Ack)
                    } else {
                        Err(refused(
                            &format!("0x{code:04X}"),
                            props::response_name(code),
                        ))
                    };
                    cx.complete(id, result);
                }
            }
            Step::Pause(_) => {}
            Step::Command => {
                let Some(id) = op.command else { return };
                if ok {
                    if op.last {
                        cx.complete(id, Ok(Outcome::Ack));
                    }
                } else {
                    // The rest of this command is not sent.
                    self.commands.retain(|o| o.command != Some(id));
                    cx.complete(
                        id,
                        Err(refused(
                            &format!("0x{code:04X}"),
                            props::response_name(code),
                        )),
                    );
                }
                self.want_refresh(cx, false);
            }
        }
    }

    fn ext_info(&mut self, cx: &mut Cx, extended: bool, code: u16, params: &[u32], data: &[u8]) {
        if code == RC_AUTHENTICATION_FAILED {
            return self.refuse(
                cx,
                "the camera refused this initiator's protocol version (Camera Control PTP 3.00)"
                    .into(),
            );
        }
        if code != RC_OK {
            if extended {
                cx.log(
                    Level::Warning,
                    format!(
                        "extended code list unavailable: {}",
                        props::response_name(code)
                    ),
                );
                return;
            }
            return self.lost(
                cx,
                format!(
                    "SDIO_GetExtDeviceInfo failed: {}",
                    props::response_name(code)
                ),
            );
        }
        if data.is_empty() {
            if extended {
                return;
            }
            self.ext_info_attempts += 1;
            if self.ext_info_attempts >= EXT_INFO_ATTEMPTS {
                return self.lost(cx, "the camera never returned its extension info".into());
            }
            cx.set_timer(EXT_RETRY, EXT_INFO_RETRY);
            return;
        }
        let info = match parse_ext_device_info(data) {
            Ok(info) => info,
            Err(e) => return self.lost(cx, format!("unreadable extension info: {e}")),
        };
        let vendor = params.first().copied().unwrap_or(0);
        if info.version / 100 < 3 {
            return self.refuse(
                cx,
                format!(
                    "the camera speaks Camera Control PTP {}.{:02}, not 3",
                    info.version / 100,
                    info.version % 100
                ),
            );
        }
        if info.version != INITIATOR_VERSION as u16 {
            cx.log(
                Level::Info,
                format!(
                    "camera extension version {:#06X}; this module speaks 0x012C",
                    info.version
                ),
            );
        }
        self.controls = info.controls.clone();
        cx.state(json!({
            "session": {
                "protocol_version": info.version,
                "vendor_code_version": vendor,
                "extended_codes": extended,
            },
            "controls": info.controls.iter().map(|c| hex(*c)).collect::<Vec<_>>(),
        }));
        if !extended {
            self.background
                .push_back(Op::new(OP_CONNECT, vec![3, 0, 0], Step::Connect(3)));
            if vendor >= EXTENDED_CODES_VERSION {
                self.extended = true;
                self.background.push_back(Op::new(
                    OP_EXT_DEVICE_INFO,
                    vec![INITIATOR_VERSION, 1],
                    Step::ExtInfo { extended: true },
                ));
            } else {
                self.extended = false;
            }
            let op = self.get_all(false);
            self.background.push_back(op);
        }
    }

    // ----- state -----

    fn apply(&mut self, cx: &mut Cx, list: Vec<PropInfo>) {
        let mut patch = json!({});
        let mut changed = false;
        for info in list {
            let raw = info.to_json();
            if self.reported_raw.get(&info.code) != Some(&raw) {
                put(
                    &mut patch,
                    &format!("properties.{}", hex(info.code)),
                    raw.clone(),
                );
                self.reported_raw.insert(info.code, raw);
                changed = true;
            }
            if let Some(prop) = props::by_code(info.code) {
                // A disabled property's value is not guaranteed.
                let named = if info.enabled == Enabled::No {
                    Value::Null
                } else {
                    props::decode(prop.decode, &info.current)
                };
                if self.reported_named.get(prop.path) != Some(&named) {
                    put(&mut patch, prop.path, named.clone());
                    self.reported_named.insert(prop.path, named);
                    changed = true;
                }
            }
            self.props.insert(info.code, info);
        }
        if changed {
            cx.state(patch);
        }
    }

    fn event(&mut self, cx: &mut Cx, code: u16, params: &[u32]) {
        let p = |i: usize| params.get(i).copied().unwrap_or(0);
        let mut patch = json!({});
        match code {
            0xC222 => {
                let result = props::result_name(p(1));
                put(
                    &mut patch,
                    "operation.last",
                    json!({
                        "operation": hex((p(0) >> 16) as u16),
                        "target": hex((p(0) & 0xFFFF) as u16),
                        "result": result,
                    }),
                );
                if p(1) != 1 {
                    cx.log(
                        Level::Warning,
                        format!(
                            "camera reports {result} for operation 0x{:04X} on 0x{:04X}",
                            p(0) >> 16,
                            p(0) & 0xFFFF
                        ),
                    );
                }
            }
            0xC224 => put(
                &mut patch,
                "recording.last_result",
                json!(props::drive_result_name(p(0))),
            ),
            0xC217 => put(
                &mut patch,
                "zoom.last_position_result",
                json!(props::drive_result_name(p(0))),
            ),
            0xC218 => put(
                &mut patch,
                "focus.last_position_result",
                json!(props::drive_result_name(p(0))),
            ),
            0xC223 => put(
                &mut patch,
                "focus.indication",
                props::label(props::FOCUS_INDICATION, p(0) as i128),
            ),
            0xC238 => put(
                &mut patch,
                "pan_tilt.last_result",
                json!({"result": props::drive_result_name(p(0)), "control": ptzf_type_name(p(1))}),
            ),
            0xC239 => put(
                &mut patch,
                "pan_tilt.preset_result",
                json!(match p(0) {
                    1 => "completed",
                    2 => "interrupted",
                    3 => "error",
                    _ => "invalid",
                }),
            ),
            0xC21F => put(
                &mut patch,
                &format!("streaming.streams.{}", p(0)),
                json!(match p(1) {
                    1 => "inactive",
                    2 => "idle",
                    3 => "streaming",
                    4 => "error",
                    _ => "unknown",
                }),
            ),
            0xC20B => put(
                &mut patch,
                "media.last_format_result",
                json!(props::drive_result_name(p(0))),
            ),
            0xC208 => put(
                &mut patch,
                "white_balance.custom_capture_result",
                json!(props::drive_result_name(p(0))),
            ),
            0xC203 | 0xC201 | 0xC202 | 0xC206 | 0xC21B | 0xC228 | 0x4004 | 0x4005 => {}
            other => cx.log(
                Level::Debug,
                format!("camera event 0x{other:04X} {params:?}"),
            ),
        }
        if patch.as_object().is_some_and(|m| !m.is_empty()) {
            cx.state(patch);
        }
        self.want_refresh(cx, false);
    }

    fn packet(&mut self, cx: &mut Cx, socket: Key, packet: Packet) {
        match packet {
            Packet::InitCommandAck {
                connection, name, ..
            } if socket == CMD && self.phase == Phase::AwaitInitAck => {
                cx.cancel_timer(INIT);
                self.connection_number = connection;
                self.paired = true;
                let mut patch = json!({"device": {"friendly_name": name}});
                if self.mode == Mode::Pairing {
                    put(&mut patch, "session.pairing", json!("approved"));
                }
                cx.state(patch);
                self.phase = Phase::ConnectingEvent;
                self.open(cx, EVT);
                cx.set_timer(INIT, INIT_TIMEOUT);
            }
            Packet::InitEventAck if socket == EVT && self.phase == Phase::AwaitEventAck => {
                cx.cancel_timer(INIT);
                self.start_session(cx);
            }
            Packet::InitFail { reason } => match reason {
                FailReason::Rejected => self.refuse(
                    cx,
                    match self.mode {
                        Mode::Pairing => "pairing was refused on the camera; open the device again to retry".into(),
                        _ => "the camera rejected this initiator; if it requires pairing, set connection to pairing and approve it on the camera".into(),
                    },
                ),
                FailReason::Busy => self.lost(cx, "the camera is busy with another controller".into()),
                FailReason::Other(code) => self.lost(cx, format!("the camera refused the connection (reason {code})")),
            },
            Packet::OperationResponse {
                code,
                transaction,
                params,
            } => self.response(cx, code, transaction, params),
            Packet::StartData { transaction, .. } => {
                if let Some(c) = self.current.as_mut().filter(|c| c.transaction == transaction) {
                    c.data.clear();
                }
            }
            Packet::Data {
                transaction,
                payload,
            }
            | Packet::EndData {
                transaction,
                payload,
            } => {
                if let Some(c) = self.current.as_mut().filter(|c| c.transaction == transaction) {
                    c.data.extend_from_slice(&payload);
                }
            }
            Packet::Event { code, params, .. } => {
                if self.ready {
                    self.event(cx, code, &params);
                }
            }
            Packet::ProbeRequest => cx.tcp_send(socket, Packet::ProbeResponse.encode()),
            other => cx.log(Level::Debug, format!("unexpected PTP-IP packet {other:?}")),
        }
    }

    // ----- commands -----

    fn prop(&self, code: u16) -> Result<&PropInfo, CommandError> {
        self.props.get(&code).ok_or_else(|| {
            refused(
                "not_reported",
                format!("the camera does not report property 0x{code:04X} (not on this model, firmware or setting)"),
            )
        })
    }

    /// A property write, checked against what the camera says it accepts.
    fn set_prop(&self, code: u16, value: PtpValue) -> Result<Op, CommandError> {
        let info = self.prop(code)?;
        if !info.writable {
            return Err(refused(
                "read_only",
                format!("property 0x{code:04X} is read-only"),
            ));
        }
        if info.enabled != Enabled::Yes {
            return Err(refused(
                "not_available",
                format!("property 0x{code:04X} cannot be changed in the camera's current mode"),
            ));
        }
        match (&info.form, &value) {
            (Form::Enum { settable, .. }, _)
                if !settable.is_empty() && !settable.contains(&value) =>
            {
                let options: Vec<String> = settable
                    .iter()
                    .take(40)
                    .map(|v| v.to_json().to_string())
                    .collect();
                return Err(invalid(format!(
                    "{} is not accepted by property 0x{code:04X} now; it accepts {}",
                    value.to_json(),
                    options.join(", ")
                )));
            }
            (Form::Range { min, max, .. }, PtpValue::Int(v)) => {
                if let (Some(lo), Some(hi)) = (min.as_int(), max.as_int()) {
                    if *v < lo || *v > hi {
                        return Err(invalid(format!(
                            "{v} is outside property 0x{code:04X}'s range {lo} to {hi}"
                        )));
                    }
                }
            }
            _ => {}
        }
        let data = ds::encode(info.datatype, &value).map_err(invalid)?;
        Ok(Op::with_data(
            OP_SET_PROPERTY,
            vec![code as u32, self.flag()],
            data,
        ))
    }

    fn control_op(&self, code: u16, value: PtpValue, datatype: u16) -> Result<Op, CommandError> {
        if !self.controls.is_empty() && !self.controls.contains(&code) {
            return Err(refused(
                "not_reported",
                format!("the camera does not offer control 0x{code:04X}"),
            ));
        }
        let data = ds::encode(datatype, &value).map_err(invalid)?;
        Ok(Op::with_data(
            OP_CONTROL,
            vec![code as u32, self.flag()],
            data,
        ))
    }

    fn control(&self, code: u16, value: i128) -> Result<Op, CommandError> {
        let datatype = props::control_type(code).unwrap_or(dt::UINT16);
        self.control_op(code, PtpValue::Int(value), datatype)
    }

    /// A button: held or released when `pressed` is given, otherwise pressed
    /// and released.
    fn button(&self, code: u16, pressed: Option<bool>) -> Result<Vec<Op>, CommandError> {
        Ok(match pressed {
            Some(down) => vec![self.control(code, if down { DOWN } else { UP })?],
            None => vec![
                self.control(code, DOWN)?,
                Op::pause(PRESS),
                self.control(code, UP)?,
            ],
        })
    }

    /// A property that behaves as a button (the push-auto functions).
    fn prop_button(&self, code: u16, pressed: Option<bool>) -> Result<Vec<Op>, CommandError> {
        Ok(match pressed {
            Some(down) => vec![self.set_prop_unchecked(code, if down { DOWN } else { UP })?],
            None => vec![
                self.set_prop_unchecked(code, DOWN)?,
                Op::pause(PRESS),
                self.set_prop_unchecked(code, UP)?,
            ],
        })
    }

    /// A property write without the enumeration check, for button-like
    /// properties whose current value is the button state.
    fn set_prop_unchecked(&self, code: u16, value: i128) -> Result<Op, CommandError> {
        let info = self.prop(code)?;
        if info.enabled == Enabled::No {
            return Err(refused(
                "not_available",
                format!("property 0x{code:04X} cannot be used in the camera's current mode"),
            ));
        }
        let data = ds::encode(info.datatype, &PtpValue::Int(value)).map_err(invalid)?;
        Ok(Op::with_data(
            OP_SET_PROPERTY,
            vec![code as u32, self.flag()],
            data,
        ))
    }

    /// The range a speed property reports, for scaling a ±32767 speed.
    fn speed_range(&self, code: u16) -> i128 {
        match self.props.get(&code).map(|p| &p.form) {
            Some(Form::Range { max, .. }) => max.as_int().unwrap_or(1).max(1),
            _ => 1,
        }
    }

    /// Zoom or focus at a speed from -32767 to 32767, through the 16-bit
    /// control where the camera offers it and the coarse one otherwise.
    fn drive(&self, fine: u16, coarse: u16, range: u16, speed: i128) -> Result<Op, CommandError> {
        if self.controls.contains(&fine) {
            return self.control(fine, speed.clamp(-32767, 32767));
        }
        let max = self.speed_range(range);
        let scaled = if speed == 0 {
            0
        } else {
            let s = ((speed.abs() as f64 / 32767.0) * max as f64)
                .round()
                .max(1.0) as i128;
            s.min(max) * speed.signum()
        };
        self.control(coarse, scaled)
    }

    fn ptzf_header(&self, version_code: u16) -> Result<Vec<u8>, CommandError> {
        let info = self.prop(version_code)?;
        if info.enabled == Enabled::No {
            return Err(refused(
                "not_available",
                "pan/tilt control is not available in the camera's current state",
            ));
        }
        let version = info.current.as_int().unwrap_or(100) as u16;
        let mut b = version.to_le_bytes().to_vec();
        b.extend_from_slice(&[0, 0]);
        Ok(b)
    }

    fn ptzf(&self, kind: u32, params: &Params) -> Result<Op, CommandError> {
        let mut data = self.ptzf_header(0xE0C0)?;
        let degrees = |name: &str| -> Result<Option<i32>, CommandError> {
            match params.get(name) {
                None | Some(Value::Null) => Ok(None),
                Some(v) => {
                    let d = v
                        .as_f64()
                        .ok_or_else(|| invalid(format!("'{name}' must be a number")))?;
                    let micro = (d * 1_000_000.0).round();
                    if micro.abs() > i32::MAX as f64 {
                        return Err(invalid(format!("'{name}' is out of range")));
                    }
                    Ok(Some(micro as i32))
                }
            }
        };
        let speed = |name: &str| -> Result<i32, CommandError> {
            Ok(param_int_or(params, name, 12)? as i32)
        };
        match kind {
            PTZF_ABSOLUTE | PTZF_RELATIVE => {
                let pan = degrees("pan")?;
                let tilt = degrees("tilt")?;
                if pan.is_none() && tilt.is_none() {
                    return Err(invalid("give pan, tilt or both"));
                }
                for (position, speed_name) in [(pan, "pan_speed"), (tilt, "tilt_speed")] {
                    match position {
                        Some(p) => {
                            data.push(1);
                            data.extend_from_slice(&p.to_le_bytes());
                            data.extend_from_slice(&speed(speed_name)?.to_le_bytes());
                        }
                        None => data.push(0),
                    }
                }
            }
            PTZF_DIRECTION => {
                for name in ["pan_speed", "tilt_speed"] {
                    data.push(1);
                    data.extend_from_slice(&(param_int_or(params, name, 0)? as i32).to_le_bytes());
                }
            }
            _ => {}
        }
        Ok(Op::with_data(OP_CONTROL_PTZF, vec![kind], data))
    }

    fn preset_ptzf(&self, set: bool, params: &Params) -> Result<Op, CommandError> {
        let mut data = self.ptzf_header(0xE0CC)?;
        let preset = param_int(params, "preset")?;
        let preset = u16::try_from(preset).map_err(|_| invalid("'preset' is out of range"))?;
        data.extend_from_slice(&preset.to_le_bytes());
        if set {
            data.push(0x01); // from the current position
            data.push(if param_bool(params, "thumbnail").unwrap_or(true) {
                0x01
            } else {
                0x02
            });
        } else {
            data.extend_from_slice(&[0, 0]);
        }
        Ok(Op::with_data(
            OP_SET_PRESET_PTZF,
            vec![if set { 1 } else { 2 }],
            data,
        ))
    }

    fn channel(params: &Params) -> Result<usize, CommandError> {
        let ch = param_int(params, "channel")?;
        if !(1..=4).contains(&ch) {
            return Err(invalid("'channel' must be 1 to 4"));
        }
        Ok(ch as usize - 1)
    }

    /// The operations for a command, or its immediate result.
    fn plan(&self, name: &str, params: &Params) -> Result<Plan, CommandError> {
        let pressed = param_bool(params, "pressed");
        let ops = |v: Result<Op, CommandError>| v.map(|op| Plan::Ops(vec![op]));
        if let Some(prop) = props::by_command(name) {
            let value = params
                .get("value")
                .ok_or_else(|| invalid("'value' is required"))?;
            let wire = props::encode(prop.decode, value).map_err(invalid)?;
            return ops(self.set_prop(prop.code, wire));
        }
        match name {
            "set_property" => {
                let code = param_code(params)?;
                let info = self.prop(code)?;
                let value = params
                    .get("value")
                    .ok_or_else(|| invalid("'value' is required"))?;
                let wire = raw_value(info.datatype, value)?;
                ops(self.set_prop(code, wire))
            }
            "get_property" => {
                let code = param_code(params)?;
                Ok(Plan::Value(self.prop(code)?.to_json()))
            }
            "control" => {
                let code = param_code(params)?;
                let datatype = match params.get("type").and_then(Value::as_str) {
                    Some(t) => ds::type_from_name(t)
                        .ok_or_else(|| invalid(format!("unknown type '{t}'")))?,
                    None => props::control_type(code).unwrap_or(dt::UINT16),
                };
                let value = params
                    .get("value")
                    .ok_or_else(|| invalid("'value' is required"))?;
                ops(self.control_op(code, raw_value(datatype, value)?, datatype))
            }
            "refresh" => Ok(Plan::Refresh),
            "set_shutter_speed" => {
                let value = params
                    .get("value")
                    .ok_or_else(|| invalid("'value' is required"))?;
                let (code, decode) = if self.props.contains_key(&0xD20D) {
                    (0xD20D, Decode::Shutter32)
                } else {
                    (0xD016, Decode::Shutter64)
                };
                ops(self.set_prop(code, props::encode(decode, value).map_err(invalid)?))
            }
            "set_tally" => {
                let lamp = param_str(params, "lamp")?;
                let code = props::TALLY
                    .iter()
                    .find(|(n, _)| *n == lamp)
                    .map(|(_, c)| *c)
                    .ok_or_else(|| invalid(format!("unknown lamp '{lamp}'")))?;
                let lit = param_bool(params, "lit").ok_or_else(|| invalid("'lit' is required"))?;
                ops(self.set_prop(code, PtpValue::Int(if lit { 2 } else { 1 })))
            }
            "set_audio_level" => {
                let ch = Self::channel(params)?;
                ops(self.set_prop(
                    props::AUDIO_LEVEL[ch],
                    PtpValue::Int(param_int(params, "level")?),
                ))
            }
            "set_audio_level_control" => {
                let ch = Self::channel(params)?;
                let mode = match param_str(params, "mode")? {
                    "auto" => 1,
                    "manual" => 2,
                    other => return Err(invalid(format!("unknown mode '{other}'"))),
                };
                ops(self.set_prop(props::AUDIO_LEVEL_CONTROL[ch], PtpValue::Int(mode)))
            }
            "set_audio_input" => {
                let ch = Self::channel(params)?;
                let input = params.get("input").cloned().unwrap_or(Value::Null);
                let wire =
                    props::encode(Decode::Labels(props::AUDIO_INPUT), &input).map_err(invalid)?;
                ops(self.set_prop(props::AUDIO_INPUT_SELECT[ch], wire))
            }
            "set_zoom_position" => {
                ops(self.set_prop(0xE040, PtpValue::Int(param_int(params, "position")?)))
            }
            "set_focus_position" => {
                ops(self.set_prop(0xE042, PtpValue::Int(param_int(params, "position")?)))
            }
            "shutter_half_press" => Ok(Plan::Ops(self.button(0xD2C1, pressed)?)),
            "shutter_full_press" => Ok(Plan::Ops(self.button(0xD2C2, pressed)?)),
            "take_photo" => Ok(Plan::Ops(self.button(0xD2E6, None)?)),
            "record" => {
                let on = param_bool(params, "recording")
                    .ok_or_else(|| invalid("'recording' is required"))?;
                ops(self.control(0xD2C8, if on { DOWN } else { UP }))
            }
            "record_toggle" => Ok(Plan::Ops(self.button(0xF001, None)?)),
            "ae_lock" => Ok(Plan::Ops(self.button(0xD2C3, pressed)?)),
            "awb_lock" => Ok(Plan::Ops(self.button(0xD2D9, pressed)?)),
            "fe_lock" => Ok(Plan::Ops(self.button(0xD2C9, pressed)?)),
            "tracking_af_on" => Ok(Plan::Ops(self.button(0xD30D, pressed)?)),
            "push_auto_focus" => Ok(Plan::Ops(self.prop_button(0xD05E, pressed)?)),
            "push_auto_iris" => Ok(Plan::Ops(self.prop_button(0xD05B, pressed)?)),
            "push_agc" => Ok(Plan::Ops(self.prop_button(0xD05C, pressed)?)),
            "push_auto_nd" => Ok(Plan::Ops(self.prop_button(0xD05D, pressed)?)),
            "one_push_awb" => Ok(Plan::Ops(self.prop_button(0xD03B, pressed)?)),
            "zoom" => ops(self.drive(0xF003, 0xD2DD, 0xD25E, param_int(params, "speed")?)),
            "focus" => ops(self.drive(0xF004, 0xD2EF, 0xD008, param_int(params, "speed")?)),
            "focus_near_far" => ops(self.control(0xD2D1, param_int(params, "step")?)),
            "cancel_zoom_position" => Ok(Plan::Ops(self.button(0xF00C, None)?)),
            "cancel_focus_position" => Ok(Plan::Ops(self.button(0xF002, None)?)),
            "save_zoom_focus_position" => ops(self.control(0xD2E9, param_int(params, "slot")?)),
            "load_zoom_focus_position" => ops(self.control(0xD2EA, param_int(params, "slot")?)),
            "af_area_position" => {
                let (x, y) = (param_int(params, "x")?, param_int(params, "y")?);
                ops(self.control(0xD2DC, (x << 16) | y))
            }
            "color_temperature_step" => ops(self.control(0xD2EC, param_int(params, "step")?)),
            "tint_step" => ops(self.control(0xD2ED, param_int(params, "step")?)),
            "custom_wb_capture_standby" => Ok(Plan::Ops(self.button(0xD2DF, None)?)),
            "custom_wb_capture_cancel" => Ok(Plan::Ops(self.button(0xD2E0, None)?)),
            "custom_wb_capture" => {
                let (x, y) = (param_int(params, "x")?, param_int(params, "y")?);
                ops(self.control(0xD2E1, (x << 16) | y))
            }
            "flicker_scan" => Ok(Plan::Ops(self.button(0xD2F1, None)?)),
            "format_media" => {
                let slot = param_int(params, "slot")?;
                let quick = param_bool(params, "quick").unwrap_or(false);
                ops(self.control(0xD2E2, slot + if quick { 0x10 } else { 0 }))
            }
            "cancel_media_format" => Ok(Plan::Ops(self.button(0xD2E7, None)?)),
            "power_off" => Ok(Plan::Ops(self.button(0xD301, None)?)),
            "camera_standby" => Ok(Plan::Ops(self.button(0xD315, None)?)),
            "power_on" => Ok(Plan::Ops(self.button(0xD316, None)?)),
            "stream" => {
                let on = param_bool(params, "streaming")
                    .ok_or_else(|| invalid("'streaming' is required"))?;
                ops(self.control(0xD307, if on { DOWN } else { UP }))
            }
            "timecode_preset_reset" => Ok(Plan::Ops(self.button(0xD302, None)?)),
            "user_bits_preset_reset" => Ok(Plan::Ops(self.button(0xD303, None)?)),
            "remote_key" => {
                let key = param_str(params, "key")?;
                let code = props::REMOTE_KEYS
                    .iter()
                    .find(|(n, _)| *n == key)
                    .map(|(_, c)| *c)
                    .ok_or_else(|| invalid(format!("unknown key '{key}'")))?;
                Ok(Plan::Ops(self.button(code, pressed)?))
            }
            "pan_tilt_absolute" => ops(self.ptzf(PTZF_ABSOLUTE, params)),
            "pan_tilt_relative" => ops(self.ptzf(PTZF_RELATIVE, params)),
            "pan_tilt_drive" => {
                let still = param_int_or(params, "pan_speed", 0)? == 0
                    && param_int_or(params, "tilt_speed", 0)? == 0;
                ops(self.ptzf(if still { PTZF_CANCEL } else { PTZF_DIRECTION }, params))
            }
            "pan_tilt_stop" => ops(self.ptzf(PTZF_CANCEL, params)),
            "pan_tilt_home" => ops(self.ptzf(PTZF_HOME, params)),
            "pan_tilt_reset" => ops(self.ptzf(PTZF_RESET, params)),
            "preset_recall" => {
                let preset = param_int(params, "preset")?;
                if let Ok(PropInfo {
                    form: Form::Range { min, max, .. },
                    ..
                }) = self.prop(0xE0CB)
                {
                    if let (Some(lo), Some(hi)) = (min.as_int(), max.as_int()) {
                        if preset < lo || preset > hi {
                            return Err(invalid(format!("the camera has presets {lo} to {hi}")));
                        }
                    }
                }
                ops(self.control(0xF015, preset))
            }
            "preset_set" => ops(self.preset_ptzf(true, params)),
            "preset_clear" => ops(self.preset_ptzf(false, params)),
            other => Err(CommandError::UnknownCommand {
                command: other.into(),
            }),
        }
    }
}

enum Plan {
    Ops(Vec<Op>),
    Value(Value),
    Refresh,
}

impl Module for SonyCamera {
    fn start(&mut self, cx: &mut Cx) {
        let mut session = json!({"mode": self.mode.name()});
        if self.mode == Mode::Pairing {
            session["pairing"] = Value::Null;
        }
        cx.state(json!({"session": session}));
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.plan(name, params) {
            Ok(Plan::Ops(ops)) => {
                let count = ops.len();
                for (i, mut op) in ops.into_iter().enumerate() {
                    op.command = Some(id);
                    op.last = i + 1 == count;
                    self.commands.push_back(op);
                }
                self.pump(cx);
            }
            Ok(Plan::Value(value)) => cx.complete(id, Ok(Outcome::Value { value })),
            Ok(Plan::Refresh) => {
                let mut op = self.get_all(false);
                op.command = Some(id);
                op.last = true;
                self.commands.push_back(op);
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => match socket {
                CMD if self.phase == Phase::ConnectingCommand => {
                    self.phase = Phase::AwaitInitAck;
                    cx.tcp_send(
                        CMD,
                        Packet::InitCommandRequest {
                            guid: self.guid,
                            name: self.friendly_name.clone(),
                            version: PROTOCOL_VERSION,
                        }
                        .encode(),
                    );
                    if self.mode == Mode::Pairing && !self.paired {
                        // The camera waits for the operator; there is no
                        // timeout and no retry while it does.
                        cx.cancel_timer(INIT);
                        cx.state(json!({"session": {"pairing": "waiting_for_approval"}}));
                        cx.log(
                            Level::Info,
                            format!(
                                "waiting for '{}' to be approved on the camera's pairing prompt",
                                self.friendly_name
                            ),
                        );
                    }
                }
                EVT if self.phase == Phase::ConnectingEvent => {
                    self.phase = Phase::AwaitEventAck;
                    cx.tcp_send(
                        EVT,
                        Packet::InitEventRequest {
                            connection: self.connection_number,
                        }
                        .encode(),
                    );
                }
                _ => {}
            },
            TcpInput::Data(data) => {
                cx.alive();
                let framer = if socket == EVT {
                    &mut self.evt_framer
                } else {
                    &mut self.cmd_framer
                };
                match framer.feed(&data) {
                    Ok(packets) => {
                        for packet in packets {
                            if matches!(self.phase, Phase::Idle | Phase::Refused) {
                                break;
                            }
                            self.packet(cx, socket, packet);
                        }
                    }
                    Err(e) => self.lost(cx, e),
                }
            }
            TcpInput::Closed { reason } => self.closed(cx, socket, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => {
                if self.phase == Phase::Idle {
                    self.connect(cx);
                }
            }
            INIT => {
                if matches!(
                    self.phase,
                    Phase::ConnectingCommand
                        | Phase::AwaitInitAck
                        | Phase::ConnectingEvent
                        | Phase::AwaitEventAck
                ) {
                    self.lost(
                        cx,
                        "the camera did not complete the PTP-IP connection within 10 s".into(),
                    );
                }
            }
            REPLY => {
                let code = self.current.as_ref().map(|c| c.op.code).unwrap_or(0);
                self.lost(
                    cx,
                    format!("no reply to operation 0x{code:04X} within 10 s"),
                );
            }
            PAUSE => {
                if let Some(op) = self.pausing.take() {
                    if let (Some(id), true) = (op.command, op.last) {
                        cx.complete(id, Ok(Outcome::Ack));
                    }
                }
                self.pump(cx);
            }
            EXT_RETRY => {
                if self.phase == Phase::Session {
                    self.background.push_front(Op::new(
                        OP_EXT_DEVICE_INFO,
                        vec![INITIATOR_VERSION, 0],
                        Step::ExtInfo { extended: false },
                    ));
                    self.pump(cx);
                }
            }
            POLL => {
                self.want_refresh(cx, false);
                if self.ready {
                    cx.set_timer(POLL, self.poll_every);
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.phase == Phase::Session && self.current.is_none() {
            let transaction = self.transaction(OP_CLOSE_SESSION);
            cx.tcp_send(
                CMD,
                Packet::OperationRequest {
                    phase: DataPhase::NoneOrIn,
                    code: OP_CLOSE_SESSION,
                    transaction,
                    params: vec![],
                }
                .encode(),
            );
        }
        cx.tcp_close(EVT);
        cx.tcp_close(CMD);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use ds::build;
    use std::net::Ipv4Addr;

    fn settings(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn camera(s: Value) -> SonyCamera {
        SonyCamera::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)),
            port: None,
            model: "ilce-7sm3".into(),
            channels: None,
            settings: settings(s),
        })
        .unwrap()
    }

    fn sent(actions: &[Action], key: Key) -> Vec<Packet> {
        let mut framer = Framer::default();
        let mut out = Vec::new();
        for a in actions {
            if let Action::TcpSend { socket, data } = a {
                if *socket == key {
                    out.extend(framer.feed(data).unwrap());
                }
            }
        }
        out
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

    fn feed(m: &mut SonyCamera, socket: Key, packets: &[Packet]) -> Vec<Action> {
        let mut bytes = Vec::new();
        for p in packets {
            bytes.extend(p.encode());
        }
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, socket, TcpInput::Data(bytes));
        cx.take()
    }

    fn ok(transaction: u32, params: Vec<u32>) -> Packet {
        Packet::OperationResponse {
            code: RC_OK,
            transaction,
            params,
        }
    }

    fn data_in(transaction: u32, payload: Vec<u8>) -> Vec<Packet> {
        vec![
            Packet::StartData {
                transaction,
                total: payload.len() as u64,
            },
            Packet::EndData {
                transaction,
                payload,
            },
        ]
    }

    fn last_request(actions: &[Action]) -> (u16, u32, Vec<u32>) {
        sent(actions, CMD)
            .into_iter()
            .rev()
            .find_map(|p| match p {
                Packet::OperationRequest {
                    code,
                    transaction,
                    params,
                    ..
                } => Some((code, transaction, params)),
                _ => None,
            })
            .expect("an operation request")
    }

    fn props_dataset() -> Vec<u8> {
        build::prop_array(&[
            build::enum_prop(
                0x5005,
                dt::UINT16,
                true,
                1,
                0x0002,
                &[0x0002, 0x0004, 0x8012],
            ),
            build::enum_prop(0x5007, dt::UINT16, true, 1, 280, &[280, 400, 560]),
            build::enum_prop(0xD21D, dt::UINT8, false, 1, 0, &[]),
            build::enum_prop(0xE0D2, dt::UINT8, true, 1, 1, &[1, 2]),
            build::range_prop(0xD218, dt::INT8, false, 76, (-1, 100, 1)),
            build::range_prop(0xD25E, dt::INT8, false, 0, (-8, 8, 1)),
            build::enum_prop(0xD20D, dt::UINT32, true, 0, 0x0001_0032, &[]),
            build::string_prop(0xD07B, "FE 24-70mm F2.8 GM II"),
        ])
    }

    /// Runs the whole connection sequence and returns the actions.
    fn connected(m: &mut SonyCamera, vendor: u32) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, CMD, TcpInput::Connected);
        let mut all = cx.take();
        all.extend(feed(
            m,
            CMD,
            &[Packet::InitCommandAck {
                connection: 5,
                guid: [9; 16],
                name: "ILCE-7SM3".into(),
                version: PROTOCOL_VERSION,
            }],
        ));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, EVT, TcpInput::Connected);
        all.extend(cx.take());
        all.extend(feed(m, EVT, &[Packet::InitEventAck]));
        // OpenSession, DeviceInfo, Connect 1 and 2, ExtInfo, Connect 3.
        let mut replies: Vec<(u16, Vec<Packet>)> = Vec::new();
        loop {
            let (code, t, _) = last_request(&all);
            let packets = match code {
                OP_GET_DEVICE_INFO => {
                    let mut v = data_in(
                        t,
                        build::device_info("Sony Corporation", "ILCE-7SM3", "3.00", "5001"),
                    );
                    v.push(ok(t, vec![]));
                    v
                }
                OP_CONNECT => {
                    let mut v = data_in(t, vec![0; 8]);
                    v.push(ok(t, vec![]));
                    v
                }
                OP_EXT_DEVICE_INFO => {
                    let mut v = data_in(
                        t,
                        build::ext_device_info(
                            0x012C,
                            &[0x5005, 0x5007, 0xD21D],
                            &[0xD2C1, 0xD2C2, 0xD2C8, 0xD2DD],
                        ),
                    );
                    v.push(ok(t, vec![vendor]));
                    v
                }
                OP_GET_ALL_PROPERTIES => {
                    let mut v = data_in(t, props_dataset());
                    v.push(ok(t, vec![]));
                    replies.push((code, v.clone()));
                    all.extend(feed(m, CMD, &v));
                    return all;
                }
                _ => vec![ok(t, vec![])],
            };
            replies.push((code, packets.clone()));
            all.extend(feed(m, CMD, &packets));
            assert!(replies.len() < 20, "the handshake does not end");
        }
    }

    #[test]
    fn the_handshake_follows_the_reference() {
        let mut m = camera(json!({"friendly_name": "Studio A"}));
        let all = connected(&mut m, 300);
        let first = sent(&all, CMD);
        match &first[0] {
            Packet::InitCommandRequest {
                guid,
                name,
                version,
            } => {
                assert_eq!(name, "Studio A");
                assert_eq!(*guid, derived_guid("Studio A"));
                assert_eq!(*version, PROTOCOL_VERSION);
            }
            other => panic!("expected Init Command Request, got {other:?}"),
        }
        assert_eq!(
            sent(&all, EVT),
            [Packet::InitEventRequest { connection: 5 }]
        );
        let ops: Vec<(u16, u32, Vec<u32>)> = first
            .iter()
            .filter_map(|p| match p {
                Packet::OperationRequest {
                    code,
                    transaction,
                    params,
                    ..
                } => Some((*code, *transaction, params.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            ops,
            [
                (OP_OPEN_SESSION, 0, vec![1]),
                (OP_GET_DEVICE_INFO, 1, vec![]),
                (OP_CONNECT, 2, vec![1, 0, 0]),
                (OP_CONNECT, 3, vec![2, 0, 0]),
                (OP_EXT_DEVICE_INFO, 4, vec![0x012C, 0]),
                (OP_CONNECT, 5, vec![3, 0, 0]),
                (OP_GET_ALL_PROPERTIES, 6, vec![0, 0]),
            ]
        );
        assert!(all.contains(&Action::Connection(Connection::Connected)));
        let s = state(&all);
        assert_eq!(s["device"]["model"], "ILCE-7SM3");
        assert_eq!(s["device"]["friendly_name"], "ILCE-7SM3");
        assert_eq!(s["session"]["protocol_version"], 0x012C);
        assert_eq!(s["controls"], json!(["D2C1", "D2C2", "D2C8", "D2DD"]));
        assert_eq!(s["white_balance"]["mode"], "auto");
        assert_eq!(s["exposure"]["iris"], 2.8);
        assert_eq!(s["recording"]["state"], "not_recording");
        assert_eq!(s["tally"]["red"], false);
        assert_eq!(s["power"]["battery"]["percent"], 76);
        assert_eq!(s["lens"]["model"], "FE 24-70mm F2.8 GM II");
        // Disabled: its value is not meaningful.
        assert_eq!(s["exposure"].get("shutter_speed"), None);
        assert_eq!(s["properties"]["D20D"]["enabled"], "disabled");
        assert_eq!(
            s["properties"]["5005"],
            json!({"value": 2, "type": "uint16", "writable": true, "enabled": "enabled", "options": [2, 4, 0x8012]})
        );
    }

    #[test]
    fn newer_cameras_get_the_extended_code_flag() {
        let mut m = camera(json!({}));
        let all = connected(&mut m, 320);
        let ops: Vec<(u16, Vec<u32>)> = sent(&all, CMD)
            .into_iter()
            .filter_map(|p| match p {
                Packet::OperationRequest { code, params, .. } => Some((code, params)),
                _ => None,
            })
            .collect();
        assert!(ops.contains(&(OP_EXT_DEVICE_INFO, vec![0x012C, 1])));
        assert_eq!(ops.last().unwrap(), &(OP_GET_ALL_PROPERTIES, vec![0, 1]));
        assert_eq!(state(&all)["session"]["extended_codes"], true);
    }

    #[test]
    fn ext_info_without_data_is_asked_again() {
        let mut m = camera(json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, CMD, TcpInput::Connected);
        cx.take();
        feed(
            &mut m,
            CMD,
            &[Packet::InitCommandAck {
                connection: 1,
                guid: [0; 16],
                name: "x".into(),
                version: PROTOCOL_VERSION,
            }],
        );
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, EVT, TcpInput::Connected);
        cx.take();
        feed(&mut m, EVT, &[Packet::InitEventAck]);
        for t in 0..4 {
            feed(&mut m, CMD, &[ok(t, vec![])]);
        }
        // ExtInfo answered with no data.
        let a = feed(&mut m, CMD, &[ok(4, vec![300])]);
        assert!(a.contains(&Action::SetTimer {
            key: EXT_RETRY,
            after: EXT_INFO_RETRY
        }));
        let mut cx = Cx::new(600);
        m.timer(&mut cx, EXT_RETRY);
        let (code, t, params) = last_request(&cx.take());
        assert_eq!((code, t, params), (OP_EXT_DEVICE_INFO, 5, vec![0x012C, 0]));
    }

    #[test]
    fn pairing_waits_without_retrying_and_a_refusal_is_final() {
        let mut m = camera(json!({"connection": "pairing"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, CMD, TcpInput::Connected);
        let a = cx.take();
        assert_eq!(state(&a)["session"]["pairing"], "waiting_for_approval");
        assert!(a.contains(&Action::CancelTimer { key: INIT }));
        // The operator declines.
        let a = feed(
            &mut m,
            CMD,
            &[Packet::InitFail {
                reason: FailReason::Rejected,
            }],
        );
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: RETRY, .. })));
        assert_eq!(state(&a)["session"]["pairing"], "refused");
        // Nothing is retried, even on a stray timer.
        let mut cx = Cx::new(60_000);
        m.timer(&mut cx, RETRY);
        assert!(cx.take().is_empty());
    }

    #[test]
    fn pairing_closed_while_waiting_is_final_but_plain_loss_retries() {
        let mut m = camera(json!({"connection": "pairing"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, CMD, TcpInput::Connected);
        m.tcp(
            &mut cx,
            CMD,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));

        let mut m = camera(json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, CMD, TcpInput::Connected);
        m.tcp(
            &mut cx,
            CMD,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        let a = cx.take();
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
    }

    #[test]
    fn ssh_mode_tunnels_both_connections_and_a_refused_login_is_final() {
        let mut m = camera(
            json!({"connection": "ssh", "ssh_username": "admin", "ssh_password": "pw",
                                  "ssh_fingerprint": "SHA256:abc"}),
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        let tunnel = a
            .iter()
            .find_map(|x| match x {
                Action::TcpOpenSsh { socket, tunnel } if *socket == CMD => Some(tunnel.clone()),
                _ => None,
            })
            .expect("a tunnelled command connection");
        assert_eq!(
            tunnel.ssh,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)), 22)
        );
        assert_eq!(tunnel.target_host, "localhost");
        assert_eq!(tunnel.target_port, 15740);
        assert_eq!(tunnel.cipher.as_deref(), Some("aes128-ctr"));
        assert_eq!(tunnel.fingerprint.as_deref(), Some("SHA256:abc"));
        let mut cx = Cx::new(0);
        m.tcp(
            &mut cx,
            CMD,
            TcpInput::Closed {
                reason: "ssh refused: host key mismatch".into(),
            },
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: RETRY, .. })));

        // Without credentials the device cannot be opened in this mode.
        assert!(SonyCamera::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: None,
            model: "x".into(),
            channels: None,
            settings: settings(json!({"connection": "ssh"})),
        })
        .is_err());
    }

    #[test]
    fn typed_commands_write_the_property_in_its_datatype() {
        let mut m = camera(json!({}));
        connected(&mut m, 300);
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "set_white_balance",
            &settings(json!({"value": "daylight"})),
        );
        let a = cx.take();
        let packets = sent(&a, CMD);
        assert_eq!(
            packets,
            [
                Packet::OperationRequest {
                    phase: DataPhase::Out,
                    code: OP_SET_PROPERTY,
                    transaction: 7,
                    params: vec![0x5005, 0]
                },
                Packet::StartData {
                    transaction: 7,
                    total: 2
                },
                Packet::EndData {
                    transaction: 7,
                    payload: vec![0x04, 0x00]
                },
            ]
        );
        let a = feed(&mut m, CMD, &[ok(7, vec![])]);
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));
        // A changes-only read follows the write.
        assert_eq!(last_request(&a), (OP_GET_ALL_PROPERTIES, 8, vec![1, 0]));
    }

    #[test]
    fn writes_the_camera_would_not_accept_are_refused_before_sending() {
        let mut m = camera(json!({}));
        connected(&mut m, 300);
        let mut run = |name: &str, p: Value| {
            let mut cx = Cx::new(10);
            m.command(&mut cx, 2, name, &settings(p));
            cx.take()
                .into_iter()
                .find_map(|a| match a {
                    Action::Complete { result, .. } => Some(result),
                    _ => None,
                })
                .unwrap()
        };
        // Not among the values the camera lists.
        assert!(matches!(
            run("set_white_balance", json!({"value": "shade"})),
            Err(CommandError::InvalidParams { .. })
        ));
        // Read-only.
        assert!(matches!(
            run("set_property", json!({"code": "D21D", "value": 1})),
            Err(CommandError::DeviceError { code: Some(c), .. }) if c == "read_only"
        ));
        // Greyed out in the current mode.
        assert!(matches!(
            run("set_shutter_speed", json!({"value": "1/100"})),
            Err(CommandError::DeviceError { code: Some(c), .. }) if c == "not_available"
        ));
        // Not reported by this camera.
        assert!(matches!(
            run("set_iso", json!({"value": "auto"})),
            Err(CommandError::DeviceError { code: Some(c), .. }) if c == "not_reported"
        ));
        // A control the camera does not offer.
        assert!(matches!(
            run("take_photo", json!({})),
            Err(CommandError::DeviceError { code: Some(c), .. }) if c == "not_reported"
        ));
        assert_eq!(
            run("get_property", json!({"code": "0x5007"})),
            Ok(Outcome::Value {
                value: json!({"value": 280, "type": "uint16", "writable": true, "enabled": "enabled", "options": [280, 400, 560]})
            })
        );
    }

    #[test]
    fn buttons_press_and_release_and_a_failed_step_ends_the_command() {
        let mut m = camera(json!({}));
        connected(&mut m, 300);
        let mut cx = Cx::new(10);
        m.command(&mut cx, 3, "shutter_half_press", &settings(json!({})));
        let (code, t, params) = last_request(&cx.take());
        assert_eq!((code, params.clone()), (OP_CONTROL, vec![0xD2C1, 0]));
        let a = feed(&mut m, CMD, &[ok(t, vec![])]);
        assert!(a.contains(&Action::SetTimer {
            key: PAUSE,
            after: PRESS
        }));
        let mut cx = Cx::new(200);
        m.timer(&mut cx, PAUSE);
        let a = cx.take();
        let up = sent(&a, CMD);
        assert!(matches!(&up[2], Packet::EndData { payload, .. } if payload == &vec![1, 0]));
        let (_, t, _) = last_request(&a);
        let a = feed(&mut m, CMD, &[ok(t, vec![])]);
        assert!(a.contains(&Action::Complete {
            id: 3,
            result: Ok(Outcome::Ack)
        }));
        // The read that follows a command finds nothing changed.
        let (code, t, _) = last_request(&a);
        assert_eq!(code, OP_GET_ALL_PROPERTIES);
        let mut packets = data_in(t, build::prop_array(&[]));
        packets.push(ok(t, vec![]));
        feed(&mut m, CMD, &packets);

        // Held down only, and the camera refuses.
        let mut cx = Cx::new(300);
        m.command(&mut cx, 4, "record", &settings(json!({"recording": true})));
        let a = cx.take();
        assert!(
            matches!(&sent(&a, CMD)[2], Packet::EndData { payload, .. } if payload == &vec![2, 0])
        );
        let (_, t, _) = last_request(&a);
        let a = feed(
            &mut m,
            CMD,
            &[Packet::OperationResponse {
                code: 0x2019,
                transaction: t,
                params: vec![],
            }],
        );
        assert!(a.contains(&Action::Complete {
            id: 4,
            result: Err(CommandError::DeviceError {
                code: Some("0x2019".into()),
                message: "device busy".into()
            })
        }));
    }

    #[test]
    fn zoom_uses_the_coarse_control_scaled_to_the_camera_range() {
        let mut m = camera(json!({}));
        connected(&mut m, 300);
        let mut cx = Cx::new(10);
        m.command(&mut cx, 5, "zoom", &settings(json!({"speed": -32767})));
        let a = cx.take();
        let packets = sent(&a, CMD);
        assert!(
            matches!(&packets[0], Packet::OperationRequest { params, .. } if params[0] == 0xD2DD)
        );
        assert!(
            matches!(&packets[2], Packet::EndData { payload, .. } if payload == &vec![(-8i8) as u8])
        );
    }

    #[test]
    fn events_trigger_a_changes_only_read_and_report_results() {
        let mut m = camera(json!({}));
        connected(&mut m, 300);
        let a = feed(
            &mut m,
            EVT,
            &[Packet::Event {
                code: 0xC222,
                transaction: 0,
                params: vec![0x9207_D2C8, 4],
            }],
        );
        assert_eq!(last_request(&a), (OP_GET_ALL_PROPERTIES, 7, vec![1, 0]));
        assert_eq!(
            state(&a)["operation"]["last"],
            json!({"operation": "9207", "target": "D2C8", "result": "camera_status_error"})
        );
        // While that read is pending, a second event does not queue another.
        let a = feed(
            &mut m,
            EVT,
            &[Packet::Event {
                code: 0xC203,
                transaction: 0,
                params: vec![],
            }],
        );
        assert!(sent(&a, CMD).is_empty());
        // The read returns only what changed: recording started.
        let changed = build::prop_array(&[build::enum_prop(0xD21D, dt::UINT8, false, 1, 1, &[])]);
        let mut packets = data_in(7, changed);
        packets.push(ok(7, vec![]));
        let a = feed(&mut m, CMD, &packets);
        assert_eq!(
            state(&a),
            json!({"properties": {"D21D": {"value": 1, "type": "uint8", "writable": false, "enabled": "enabled", "options": []}},
                   "recording": {"state": "recording"}})
        );
        // The queued read goes next, and an unchanged property is not reported.
        let (code, t, _) = last_request(&a);
        assert_eq!(code, OP_GET_ALL_PROPERTIES);
        let mut packets = data_in(
            t,
            build::prop_array(&[build::enum_prop(0xD21D, dt::UINT8, false, 1, 1, &[])]),
        );
        packets.push(ok(t, vec![]));
        let a = feed(&mut m, CMD, &packets);
        assert!(!a.iter().any(|x| matches!(x, Action::State(_))));
    }

    #[test]
    fn a_missing_reply_drops_the_session_and_fails_pending_commands() {
        let mut m = camera(json!({}));
        connected(&mut m, 300);
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            6,
            "set_tally",
            &settings(json!({"lamp": "red", "lit": true})),
        );
        cx.take();
        let mut cx = Cx::new(10_100);
        m.timer(&mut cx, REPLY);
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                id: 6,
                result: Err(CommandError::Transport { .. })
            }
        )));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        // Commands are refused until the camera is back.
        let mut cx = Cx::new(10_200);
        m.command(
            &mut cx,
            7,
            "set_tally",
            &settings(json!({"lamp": "red", "lit": false})),
        );
        assert!(cx.take().contains(&Action::Complete {
            id: 7,
            result: Err(CommandError::NotConnected)
        }));
    }

    #[test]
    fn pan_tilt_absolute_drive_dataset() {
        let mut m = camera(json!({}));
        connected(&mut m, 300);
        let version =
            build::prop_array(&[build::enum_prop(0xE0C0, dt::UINT16, false, 1, 100, &[])]);
        let mut cx = Cx::new(0);
        m.apply(&mut cx, parse_prop_info_array(&version).unwrap());
        let op = m
            .ptzf(
                PTZF_ABSOLUTE,
                &settings(json!({"pan": 17.5, "pan_speed": 1, "tilt_speed": 50})),
            )
            .unwrap();
        assert_eq!(op.code, OP_CONTROL_PTZF);
        assert_eq!(op.params, vec![1]);
        let mut expected = vec![100, 0, 0, 0, 1];
        expected.extend_from_slice(&17_500_000i32.to_le_bytes());
        expected.extend_from_slice(&1i32.to_le_bytes());
        expected.push(0); // no tilt
        assert_eq!(op.data.unwrap(), expected);
        // Stop carries the header only.
        let stop = m.ptzf(PTZF_CANCEL, &Params::new()).unwrap();
        assert_eq!(
            (stop.params, stop.data.unwrap()),
            (vec![6], vec![100, 0, 0, 0])
        );
    }

    #[test]
    fn the_spec_and_the_module_agree() {
        let catalog = crate::catalog::Catalog::embedded();
        let spec = catalog.device("sony-camera").expect("the sony-camera spec");
        let m = camera(json!({}));
        for (name, command) in &spec.commands {
            if let Err(CommandError::UnknownCommand { .. }) = m.plan(name, &Params::new()) {
                panic!("the spec's '{name}' has no implementation");
            }
            if let Some(prop) = props::by_command(name) {
                if let Some(labels) = props::labels_of(prop.decode) {
                    assert_eq!(
                        command.params["value"].values.as_ref(),
                        Some(&labels),
                        "{name}'s values"
                    );
                }
            }
        }
        for prop in props::PROPS {
            if let Some(c) = prop.command {
                assert!(spec.commands.contains_key(c), "{c} is not in the spec");
            }
            assert!(
                spec.state.contains_key(prop.path),
                "{} is not in the spec's state",
                prop.path
            );
        }
        for model in &spec.models {
            assert!(model.supports.iter().any(|c| c == "set_property"));
        }
    }

    #[test]
    fn guid_setting_and_put() {
        assert_eq!(
            parse_guid("00112233-4455-6677-8899-aabbccddeeff").unwrap()[15],
            0xFF
        );
        assert!(parse_guid("0011").is_none());
        let mut v = json!({});
        put(&mut v, "a.b.c", json!(1));
        put(&mut v, "a.d", json!(2));
        assert_eq!(v, json!({"a": {"b": {"c": 1}, "d": 2}}));
    }
}
