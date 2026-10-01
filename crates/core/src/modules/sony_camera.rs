//! Sony Alpha, FX, ZV, cinema line and pan/tilt cameras over Sony's Camera
//! Control PTP 3, and the earlier bodies over Camera Control PTP 2, carried by
//! PTP-IP on TCP 15740.
//!
//! The session, as Sony's Camera Control PTP 3 and PTP 2 References
//! (Camera Remote Command 2.02.00) lay it out: a PTP-IP command connection
//! opened with Init Command Request (this initiator's GUID and friendly name),
//! an event connection opened with Init Event Request (the connection number
//! the camera gave), OpenSession (or SDIO_OpenSession with a function mode),
//! then Sony's handshake (SDIO_Connect phases 1 and 2, SDIO_GetExtDeviceInfo
//! until it returns data, SDIO_Connect phase 3). After that the camera's whole
//! property set is read with SDIO_GetAllExtDevicePropInfo, and read again when
//! the camera signals a change and on a steady poll, which the references
//! recommend because events alone are not guaranteed. PTP 3 reads only what
//! changed; PTP 2 has no such option and reads everything. Writes are
//! SDIO_SetExtDevicePropValue and SDIO_ControlDevice; pan/tilt cameras add
//! SDIO_ControlPTZF and SDIO_SetPresetPTZF.
//!
//! The two protocols share the transport, the handshake and the property
//! dataset; they differ in the version announced (3.00 or 2.00), in the
//! option parameters PTP 3 adds, and in the property set. Where PTP 2 only
//! reports a value (iris, shutter speed, ISO, exposure and flash
//! compensation), it is changed by steps through a control of the same code,
//! and the module converts a target value into those steps.
//!
//! PTP allows one transaction at a time on the command connection, so every
//! operation goes through one queue: commands first, then background reads.
//! Live view frames, content lists and downloads, and FTP settings are
//! operations on that queue too. Downloads stream into a file on the host as
//! the data arrives. Pan/tilt cameras serve live view, and video-only cameras
//! their clips, over HTTP on the camera's own localhost; the module opens
//! those through the SSH tunnel and speaks HTTP itself.
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

use base64::Engine as _;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use super::sony_camera_content as content;
use super::sony_camera_dataset::{
    self as ds, dt, parse_device_info, parse_ext_device_info, parse_prop_info_array, Enabled, Form,
    PropInfo, PtpValue,
};
use super::sony_camera_ftp as ftp;
use super::sony_camera_http::{parse_url, HttpEvent, HttpReader, Url};
use super::sony_camera_props::{self as props, Decode};
use super::sony_camera_ptpip::{
    request_with_data, DataPhase, FailReason, Framer, Packet, PROTOCOL_VERSION,
};
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, FileInput, Key, Level, Millis, Module, OpenContext,
    Outcome, SshTunnel, TcpInput,
};

#[path = "sony_camera_files.rs"]
mod files;

use files::{Deletion, Upload};

#[path = "sony_camera_info.rs"]
mod info;

use info::Info;

const PTP_IP_PORT: u16 = 15740;
const SSH_PORT: u16 = 22;

const CMD: Key = "command";
const EVT: Key = "event";
/// The HTTP stream to a pan/tilt camera's live view or a video-only
/// camera's MediaProfile, through the SSH tunnel.
const HTTP: Key = "http";
const FILE: Key = "download";
/// The partial file a resumed download continues.
const RESUME: Key = "resume";

const REPLY: Key = "reply";
const INIT: Key = "init";
const RETRY: Key = "retry";
const POLL: Key = "poll";
const PAUSE: Key = "pause";
const EXT_RETRY: Key = "ext-info-retry";
const HTTP_TIMEOUT: Key = "http-timeout";

const INIT_TIMEOUT: Millis = 10_000;
const REPLY_TIMEOUT: Millis = 10_000;
const HTTP_TIMEOUT_MS: Millis = 15_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// SDIO_GetExtDeviceInfo returns no data until the camera is ready; the
/// reference says to ask again until it does.
const EXT_INFO_RETRY: Millis = 500;
const EXT_INFO_ATTEMPTS: u32 = 40;
/// Between the down and up halves of a momentary button press.
const PRESS: Millis = 100;
/// Live view: at most 30 frames a second, and a frame asked for too soon
/// comes back empty and is asked for again.
const LIVE_VIEW_RETRY: Millis = 40;
const LIVE_VIEW_ATTEMPTS: u32 = 15;
/// Bytes asked for per partial read when downloading.
const CHUNK: u32 = 4 * 1024 * 1024;
/// A MediaProfile larger than this is not a MediaProfile.
const MAX_PROFILE: usize = 16 * 1024 * 1024;
/// The largest partial file a download resumes from. The host's file API
/// has no append, so the part already on disk is read and written again.
const MAX_RESUME: u64 = 1024 * 1024 * 1024;

const OP_GET_DEVICE_INFO: u16 = 0x1001;
const OP_OPEN_SESSION: u16 = 0x1002;
const OP_CLOSE_SESSION: u16 = 0x1003;
const OP_GET_OBJECT_HANDLES: u16 = 0x1007;
const OP_GET_OBJECT_INFO: u16 = 0x1008;
const OP_GET_OBJECT: u16 = 0x1009;
const OP_CONNECT: u16 = 0x9201;
const OP_EXT_DEVICE_INFO: u16 = 0x9202;
const OP_SET_PROPERTY: u16 = 0x9205;
const OP_CONTROL: u16 = 0x9207;
const OP_GET_ALL_PROPERTIES: u16 = 0x9209;
const OP_SDIO_OPEN_SESSION: u16 = 0x9210;
const OP_GET_PARTIAL_LARGE_OBJECT: u16 = 0x9211;
const OP_SET_CONTENTS_TRANSFER_MODE: u16 = 0x9212;
const OP_GET_FTP_JOB_LIST: u16 = 0x9217;
const OP_CONTROL_FTP_JOB_LIST: u16 = 0x9218;
const OP_GET_FTP_SETTING_LIST: u16 = 0x921F;
const OP_SET_FTP_SETTING_LIST: u16 = 0x9220;
const OP_GET_DISPLAY_FTP_RESULT: u16 = 0x9233;
const OP_GET_CONTENT_INFO_LIST: u16 = 0x923C;
const OP_GET_CONTENT_DATA: u16 = 0x923D;
const OP_GET_CONTENT_COMPRESSED_DATA: u16 = 0x923E;
const OP_CONTROL_PTZF: u16 = 0x9245;
const OP_SET_PRESET_PTZF: u16 = 0x9246;

const RC_OK: u16 = 0x2001;
const RC_ACCESS_DENIED: u16 = 0x200F;
const RC_SESSION_ALREADY_OPEN: u16 = 0x201E;
const RC_AUTHENTICATION_FAILED: u16 = 0xA101;

const LIVE_VIEW_HANDLE: u32 = 0xFFFF_C002;
const CAPTURED_HANDLE: u32 = 0xFFFF_C001;

/// The Camera Control PTP versions this module speaks, 100 times 3.00 and
/// 2.00.
const PTP3_VERSION: u32 = 0x012C;
const PTP2_VERSION: u32 = 0x00C8;
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

/// Models that speak only Camera Control PTP 2 over IP (README protocol
/// and interface tables).
const PTP2_ONLY: &[&str] = &["ilce-7m3"];

/// Models whose compatibility tables list the Remote Control with Transfer
/// Mode content operations, opened in that mode unless the setting says
/// otherwise.
const TRANSFER_WITH_REMOTE: &[&str] = &[
    "ilce-1m2",
    "ilce-1",
    "ilce-9m3",
    "ilce-7rm6",
    "ilce-7rm5",
    "ilce-7m5",
    "ilce-7m4",
    "ilce-7sm3",
    "ilce-7cm2",
    "ilce-7cr",
    "ilce-6700",
    "ilx-lr1",
    "ilme-fx3",
    "ilme-fx30",
    "ilme-fx2",
    "zv-e1",
    "dsc-rx1rm3",
];

/// What the module may do in Content Transfer Mode, where the camera takes
/// no remote control.
const CONTENT_TRANSFER_COMMANDS: &[&str] = &[
    "list_content",
    "download_content",
    "get_property",
    "refresh",
    "switch_session_mode",
];

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
enum Protocol {
    Ptp3,
    Ptp2,
}

impl Protocol {
    fn name(self) -> &'static str {
        match self {
            Protocol::Ptp3 => "ptp3",
            Protocol::Ptp2 => "ptp2",
        }
    }

    fn version(self) -> u32 {
        match self {
            Protocol::Ptp3 => PTP3_VERSION,
            Protocol::Ptp2 => PTP2_VERSION,
        }
    }
}

/// The function mode a session is opened in (SDIO_OpenSession).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Function {
    Remote,
    RemoteWithTransfer,
    ContentTransfer,
}

impl Function {
    fn name(self) -> &'static str {
        match self {
            Function::Remote => "remote",
            Function::RemoteWithTransfer => "remote_with_transfer",
            Function::ContentTransfer => "content_transfer",
        }
    }

    fn parse(text: &str) -> Option<Function> {
        Some(match text {
            "remote" => Function::Remote,
            "remote_with_transfer" => Function::RemoteWithTransfer,
            "content_transfer" => Function::ContentTransfer,
            _ => return None,
        })
    }

    fn sdio_mode(self) -> u32 {
        match self {
            Function::Remote => 0,
            Function::ContentTransfer => 1,
            Function::RemoteWithTransfer => 2,
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

/// What a command's data turns into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parse {
    ContentList,
    Thumbnail,
    FtpSettings,
    FtpJobs,
    FtpResult,
}

#[derive(Debug, Clone, PartialEq)]
enum Step {
    OpenSession {
        sdio: bool,
    },
    DeviceInfo,
    Connect(u32),
    ExtInfo {
        extended: bool,
    },
    GetAll {
        diff: bool,
    },
    TransferMode,
    /// Part of an operator command.
    Command,
    /// A wait between two parts of a command.
    Pause(Millis),
    LiveView {
        attempt: u32,
    },
    /// The data becomes the command's value.
    Value(Parse),
    ListHandles,
    ListInfo(u32),
    DownloadInfo,
    DownloadChunk,
    DownloadWhole,
    /// Close the session to reopen it in another function mode.
    Switch(Function),
    /// Background reads of the FTP lists into the state.
    FtpRefresh,
    JobsRefresh,
    /// Part of an upload; the last one is followed by a result event.
    Upload {
        awaits: bool,
    },
    /// A step that must succeed for the rest of its command to be sent.
    Prelude,
    UploadResultFile,
    DownloadDataset,
    Delete,
    /// A read of what the camera reports about itself, or a write whose
    /// response carries a result.
    Info(Info),
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

    fn for_command(mut self, id: CommandId) -> Op {
        self.command = Some(id);
        self
    }
}

struct InFlight {
    op: Op,
    transaction: u32,
    data: Vec<u8>,
    /// Bytes written straight to the download file.
    streamed: u64,
}

/// Where a download comes from.
#[derive(Debug, Clone, PartialEq)]
enum Source {
    /// A PTP object in Content Transfer Mode.
    Object(u32),
    /// A file of a listed content in Remote Control with Transfer Mode.
    Content { slot: u32, content: u32, file: u32 },
    /// A whole object by handle: the image just shot, or a setting file.
    Handle(u32),
    /// A file inside an SDIO_DownloadData dataset (a scene file).
    Dataset,
    /// A clip of a video-only camera, over HTTP.
    Http,
}

struct Download {
    id: CommandId,
    path: String,
    source: Source,
    offset: u64,
    total: Option<u64>,
    name: Option<String>,
    error: Option<String>,
    /// The file has been asked to close; its result completes the command.
    closing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum HttpPurpose {
    LiveView,
    Profile { slot: u32 },
    Download,
}

/// Who a live view frame is for. Today each frame answers one
/// `get_live_view_image` command. A continuous mode adds a stream target
/// here: a timer asks for the next frame (the same GetObject, or the next
/// chunk of the HTTP stream) once the previous one is delivered, and
/// `deliver_frame` hands frames to the host's media stream instead of
/// completing a command.
#[derive(Debug, Clone, Copy, PartialEq)]
enum FrameTarget {
    Command(CommandId),
}

struct HttpJob {
    id: CommandId,
    purpose: HttpPurpose,
    url: Url,
    reader: HttpReader,
    body: Vec<u8>,
    chunked: bool,
}

struct Listing {
    id: CommandId,
    slot: u32,
    handles: VecDeque<u32>,
    items: Vec<Value>,
    total: usize,
}

/// What a command turns into.
enum Plan {
    Ops(Vec<Op>),
    Value(Value),
    Refresh,
    Http(HttpPurpose, Url),
    Download(Download, Vec<Op>),
    HttpDownload(Download, Url),
    /// An upload: the host file to read, and the most it may be.
    Upload(Upload, String, u64),
    Delete(Deletion, Op),
    Listing(u32, u32, Op),
    Switch(Function),
    /// A download that continues the file already on the host.
    Resume(Download),
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

    protocol: Protocol,
    function: Function,

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
    operations: Vec<u16>,
    controls: Vec<u16>,
    props: HashMap<u16, PropInfo>,
    reported_raw: HashMap<u16, Value>,
    reported_named: HashMap<&'static str, Value>,

    live_view_enabled: bool,
    download: Option<Download>,
    listing: Option<Listing>,
    http: Option<HttpJob>,
    upload: Option<Upload>,
    deletion: Option<Deletion>,
    last_delete_at: Option<Millis>,
    /// Sizes of listed files, by download id.
    sizes: HashMap<String, u64>,
    /// Download ids of listed files, by UMID.
    umids: HashMap<String, String>,
    /// When each slot's content list was generated, as last listed.
    list_times: HashMap<u32, u64>,
    ftp_setting_version: u16,
    ftp_job_version: u16,
    ftp_servers: Vec<String>,
    ftp_jobs: Vec<String>,
    /// Content in Content Transfer Mode is chosen on the camera, not here.
    select_on_camera: bool,
    /// The camera's display string lists, by list type: value to name.
    display_lists: HashMap<u32, Map<String, Value>>,
    stream_version: u16,
    captures: u64,
    cautions: u64,
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

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
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

fn rejected(code: u16) -> CommandError {
    refused(&format!("0x{code:04X}"), props::response_name(code))
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

fn param_u32(params: &Params, name: &str) -> Result<u32, CommandError> {
    u32::try_from(param_int(params, name)?)
        .map_err(|_| invalid(format!("'{name}' is out of range")))
}

fn param_str<'a>(params: &'a Params, name: &str) -> Result<&'a str, CommandError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{name}' must be text")))
}

fn param_str_or<'a>(params: &'a Params, name: &str) -> &'a str {
    params.get(name).and_then(Value::as_str).unwrap_or("")
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

/// A live view frame as a command's value.
fn frame_value(frame: &content::LiveFrame) -> Value {
    let mut v = json!({
        "format": "jpeg",
        "bytes": frame.jpeg.len(),
        "image": b64(&frame.jpeg),
    });
    if let Some((w, h)) = content::jpeg_size(&frame.jpeg) {
        v["width"] = json!(w);
        v["height"] = json!(h);
    }
    if let Some(ffi) = &frame.focal_frame_info {
        v["focal_frame_info"] = json!(b64(ffi));
    }
    v
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
        let protocol = match text_setting(s, "protocol").as_str() {
            "" | "auto" => {
                if PTP2_ONLY.contains(&ctx.model.as_str()) {
                    Protocol::Ptp2
                } else {
                    Protocol::Ptp3
                }
            }
            "ptp3" => Protocol::Ptp3,
            "ptp2" => Protocol::Ptp2,
            other => return Err(format!("unknown protocol '{other}'")),
        };
        let function = match text_setting(s, "session_mode").as_str() {
            "" | "auto" => {
                if protocol == Protocol::Ptp3 && TRANSFER_WITH_REMOTE.contains(&ctx.model.as_str())
                {
                    Function::RemoteWithTransfer
                } else {
                    Function::Remote
                }
            }
            other => Function::parse(other).ok_or(format!("unknown session_mode '{other}'"))?,
        };
        if protocol == Protocol::Ptp2 && function != Function::Remote {
            return Err("Camera Control PTP 2 has only the remote control session mode".into());
        }
        let select_on_camera = match text_setting(s, "content_selection").as_str() {
            "" | "remote" => false,
            "camera" => true,
            other => return Err(format!("unknown content_selection '{other}'")),
        };
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
            protocol,
            function,
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
            operations: Vec::new(),
            controls: Vec::new(),
            props: HashMap::new(),
            reported_raw: HashMap::new(),
            reported_named: HashMap::new(),
            live_view_enabled: false,
            download: None,
            listing: None,
            http: None,
            upload: None,
            deletion: None,
            last_delete_at: None,
            sizes: HashMap::new(),
            umids: HashMap::new(),
            list_times: HashMap::new(),
            ftp_setting_version: 100,
            ftp_job_version: 100,
            ftp_servers: Vec::new(),
            ftp_jobs: Vec::new(),
            select_on_camera,
            display_lists: HashMap::new(),
            stream_version: 100,
            captures: 0,
            cautions: 0,
        })
    }

    // ----- connections -----

    fn tunnel(&self, target_host: &str, target_port: u16) -> SshTunnel {
        SshTunnel {
            ssh: SocketAddr::new(self.host, self.port.unwrap_or(SSH_PORT)),
            username: self.ssh_username.clone(),
            password: self.ssh_password.clone(),
            fingerprint: self.ssh_fingerprint.clone(),
            cipher: Some("aes128-ctr".into()),
            target_host: target_host.into(),
            target_port,
        }
    }

    /// Opens one of the two PTP-IP connections, directly or through the SSH
    /// tunnel the camera requires when its Access Authentication is on.
    fn open(&self, cx: &mut Cx, socket: Key) {
        match self.mode {
            Mode::Ssh => cx.tcp_open_ssh(socket, self.tunnel("localhost", PTP_IP_PORT)),
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

    /// Clears the session; the caller decides whether to try again. HTTP
    /// streams through the tunnel are separate and carry on.
    fn teardown(&mut self, cx: &mut Cx, error: CommandError) {
        let mut failed = Vec::new();
        if let Some(inflight) = self.current.take() {
            failed.extend(inflight.op.command);
        }
        failed.extend(self.pausing.take().and_then(|op| op.command));
        failed.extend(self.commands.drain(..).filter_map(|op| op.command));
        failed.extend(self.background.drain(..).filter_map(|op| op.command));
        if let Some(listing) = self.listing.take() {
            failed.push(listing.id);
        }
        failed.extend(self.upload.take().map(|u| u.id));
        failed.extend(self.deletion.take().map(|d| d.id));
        if self
            .download
            .as_ref()
            .is_some_and(|d| d.source != Source::Http)
        {
            let d = self.download.take().unwrap();
            failed.push(d.id);
            cx.file_close(FILE);
        }
        failed.sort_unstable();
        failed.dedup();
        for id in failed {
            cx.complete(id, Err(error.clone()));
        }
        cx.tcp_close(CMD);
        cx.tcp_close(EVT);
        for key in [
            REPLY,
            INIT,
            POLL,
            PAUSE,
            EXT_RETRY,
            files::UPLOAD_WAIT,
            files::DELETE_WAIT,
        ] {
            cx.cancel_timer(key);
        }
        self.cmd_framer = Framer::default();
        self.evt_framer = Framer::default();
        self.ready = false;
        self.live_view_enabled = false;
        self.ext_info_attempts = 0;
        self.props.clear();
        self.reported_raw.clear();
        self.reported_named.clear();
        self.display_lists.clear();
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
        if code == OP_OPEN_SESSION || code == OP_SDIO_OPEN_SESSION {
            // Opening a session is transaction 0; numbering restarts after it.
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
            streamed: 0,
        });
    }

    fn flag(&self) -> u32 {
        self.extended as u32
    }

    /// SDIO_SetContentsTransferMode's selection parameter: 2 when content
    /// is chosen from this side, 1 when the operator chooses it on the
    /// camera.
    fn selection(&self) -> u32 {
        if self.select_on_camera {
            1
        } else {
            2
        }
    }

    /// The parameters that name a property or control: PTP 3 adds the
    /// extended-codes option flag.
    fn code_params(&self, code: u16) -> Vec<u32> {
        match self.protocol {
            Protocol::Ptp3 => vec![code as u32, self.flag()],
            Protocol::Ptp2 => vec![code as u32],
        }
    }

    /// Reads the property set again: changes only unless `full` (PTP 2
    /// always reads everything).
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
                if full && !op.params.is_empty() {
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
        match self.protocol {
            Protocol::Ptp3 => Op::new(
                OP_GET_ALL_PROPERTIES,
                vec![diff as u32, self.flag()],
                Step::GetAll { diff },
            ),
            Protocol::Ptp2 => Op::new(OP_GET_ALL_PROPERTIES, vec![], Step::GetAll { diff: false }),
        }
    }

    fn ext_info_op(&self, extended: bool) -> Op {
        let params = match self.protocol {
            Protocol::Ptp3 => vec![PTP3_VERSION, extended as u32],
            Protocol::Ptp2 => vec![PTP2_VERSION],
        };
        Op::new(OP_EXT_DEVICE_INFO, params, Step::ExtInfo { extended })
    }

    fn start_session(&mut self, cx: &mut Cx) {
        self.phase = Phase::Session;
        let open = if self.function == Function::Remote {
            Op::new(OP_OPEN_SESSION, vec![1], Step::OpenSession { sdio: false })
        } else {
            Op::new(
                OP_SDIO_OPEN_SESSION,
                vec![1, self.function.sdio_mode()],
                Step::OpenSession { sdio: true },
            )
        };
        let ext = self.ext_info_op(false);
        self.background.extend([
            open,
            Op::new(OP_GET_DEVICE_INFO, vec![], Step::DeviceInfo),
            Op::new(OP_CONNECT, vec![1, 0, 0], Step::Connect(1)),
            Op::new(OP_CONNECT, vec![2, 0, 0], Step::Connect(2)),
            ext,
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
        self.finish(
            cx,
            inflight.op,
            code,
            &params,
            inflight.data,
            inflight.streamed,
        );
        self.pump(cx);
    }

    fn complete_op(&mut self, cx: &mut Cx, op: &Op, code: u16) {
        let Some(id) = op.command else { return };
        if code == RC_OK {
            if op.last {
                cx.complete(id, Ok(Outcome::Ack));
            }
        } else {
            // The rest of this command is not sent.
            self.commands.retain(|o| o.command != Some(id));
            cx.complete(id, Err(rejected(code)));
        }
    }

    fn finish(
        &mut self,
        cx: &mut Cx,
        op: Op,
        code: u16,
        params: &[u32],
        data: Vec<u8>,
        streamed: u64,
    ) {
        let ok = code == RC_OK;
        match op.step {
            Step::OpenSession { sdio } => {
                if ok || code == RC_SESSION_ALREADY_OPEN {
                    cx.state(json!({"session": {"function_mode": self.function.name()}}));
                } else if sdio {
                    // Not every firmware takes every function mode: fall
                    // back to plain remote control.
                    cx.log(
                        Level::Warning,
                        format!(
                            "the camera refused session mode {} ({}); using remote control only",
                            self.function.name(),
                            props::response_name(code)
                        ),
                    );
                    self.function = Function::Remote;
                    self.background.push_front(Op::new(
                        OP_OPEN_SESSION,
                        vec![1],
                        Step::OpenSession { sdio: false },
                    ));
                } else {
                    self.lost(
                        cx,
                        format!("OpenSession failed: {}", props::response_name(code)),
                    );
                }
            }
            Step::DeviceInfo => match parse_device_info(&data) {
                Ok(info) if ok => {
                    self.operations = info.operations.clone();
                    cx.state(json!({"device": {
                        "manufacturer": info.manufacturer,
                        "model": info.model,
                        "version": info.version,
                        "serial": info.serial,
                    }}))
                }
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
            Step::TransferMode => {
                if !ok {
                    cx.log(
                        Level::Warning,
                        format!(
                            "the camera refused content transfer mode: {}",
                            props::response_name(code)
                        ),
                    );
                }
            }
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
                        self.queue_info_reads();
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
                    cx.complete(
                        id,
                        if ok {
                            Ok(Outcome::Ack)
                        } else {
                            Err(rejected(code))
                        },
                    );
                }
            }
            Step::Pause(_) => {}
            Step::Command => {
                if ok && op.code == OP_CONTROL && op.params.first() == Some(&0xD313) {
                    self.live_view_enabled = true;
                }
                self.complete_op(cx, &op, code);
                if ok && op.code == OP_SET_FTP_SETTING_LIST {
                    self.background.push_back(Op::new(
                        OP_GET_FTP_SETTING_LIST,
                        vec![],
                        Step::FtpRefresh,
                    ));
                }
                if ok && op.code == OP_CONTROL_FTP_JOB_LIST {
                    self.queue_jobs_refresh();
                }
                self.want_refresh(cx, false);
            }
            Step::LiveView { attempt } => self.live_view(cx, op, code, &data, attempt),
            Step::Value(parse) => {
                let Some(id) = op.command else { return };
                if !ok {
                    cx.complete(id, Err(rejected(code)));
                    return;
                }
                let result = self.parse_value(cx, parse, &data);
                cx.complete(
                    id,
                    result
                        .map(|value| Outcome::Value { value })
                        .map_err(|e| refused("unreadable", e)),
                );
            }
            Step::ListHandles => self.listed_handles(cx, code, &data),
            Step::ListInfo(handle) => self.listed_info(cx, handle, code, &data),
            Step::DownloadInfo => self.download_info(cx, code, &data),
            Step::DownloadChunk | Step::DownloadWhole => {
                self.download_chunk(cx, op.step == Step::DownloadWhole, code, streamed)
            }
            Step::Switch(function) => {
                if let Some(id) = op.command {
                    cx.complete(id, Ok(Outcome::Ack));
                }
                self.reopen(cx, function);
            }
            Step::FtpRefresh => {
                if ok {
                    let _ = self.parse_value(cx, Parse::FtpSettings, &data);
                }
            }
            Step::JobsRefresh => {
                if ok {
                    let _ = self.parse_value(cx, Parse::FtpJobs, &data);
                }
            }
            Step::Upload { awaits } => self.upload_step(cx, &op, code, awaits),
            Step::Prelude => self.prelude(cx, &op, code),
            Step::UploadResultFile => self.upload_result_file(cx, code, &data),
            Step::DownloadDataset => self.download_dataset(cx, code, &data),
            Step::Delete => self.delete_step(cx, code),
            Step::Info(info) => self.info_done(cx, info, &op, code, params, &data),
        }
    }

    fn ext_info(&mut self, cx: &mut Cx, extended: bool, code: u16, params: &[u32], data: &[u8]) {
        if code == RC_AUTHENTICATION_FAILED {
            return self.refuse(
                cx,
                format!(
                    "the camera refused this initiator's protocol version (Camera Control PTP {})",
                    if self.protocol == Protocol::Ptp3 {
                        "3.00"
                    } else {
                        "2.00"
                    }
                ),
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
        let major = info.version / 100;
        if major < 2 {
            return self.refuse(
                cx,
                format!(
                    "the camera speaks Camera Control PTP {}.{:02}, not 2 or 3",
                    major,
                    info.version % 100
                ),
            );
        }
        if major == 2 && self.protocol == Protocol::Ptp3 {
            // A camera that answers 3.00 with a 2.xx extension speaks PTP 2.
            cx.log(
                Level::Info,
                "the camera answers with Camera Control PTP 2; using PTP 2",
            );
            self.protocol = Protocol::Ptp2;
        }
        if info.version as u32 != self.protocol.version() {
            cx.log(
                Level::Info,
                format!(
                    "camera extension version {:#06X}; this module speaks {:#06X}",
                    info.version,
                    self.protocol.version()
                ),
            );
        }
        let vendor = match self.protocol {
            Protocol::Ptp3 => params.first().copied().unwrap_or(0),
            Protocol::Ptp2 => 0,
        };
        self.controls = info.controls.clone();
        cx.state(json!({
            "session": {
                "protocol": self.protocol.name(),
                "protocol_version": info.version,
                "vendor_code_version": vendor,
                "extended_codes": extended,
            },
            "controls": info.controls.iter().map(|c| hex(*c)).collect::<Vec<_>>(),
        }));
        if !extended {
            self.background
                .push_back(Op::new(OP_CONNECT, vec![3, 0, 0], Step::Connect(3)));
            if self.protocol == Protocol::Ptp3 && vendor >= EXTENDED_CODES_VERSION {
                self.extended = true;
                let op = self.ext_info_op(true);
                self.background.push_back(op);
            } else {
                self.extended = false;
            }
            if self.function == Function::ContentTransfer {
                // Content selected from this side (or on the camera, when
                // set so), transfer on.
                self.background.push_back(Op::new(
                    OP_SET_CONTENTS_TRANSFER_MODE,
                    vec![self.selection(), 1, 0],
                    Step::TransferMode,
                ));
            }
            let op = self.get_all(false);
            self.background.push_back(op);
        }
    }

    /// Closes the session and opens it again in another function mode.
    fn reopen(&mut self, cx: &mut Cx, function: Function) {
        self.teardown(
            cx,
            CommandError::Transport {
                message: "the session was closed to change its mode".into(),
            },
        );
        self.function = function;
        self.phase = Phase::Idle;
        cx.cancel_timer(RETRY);
        cx.log(
            Level::Info,
            format!("reopening the session in {} mode", function.name()),
        );
        self.connect(cx);
    }

    // ----- state -----

    fn apply(&mut self, cx: &mut Cx, list: Vec<PropInfo>) {
        let mut patch = json!({});
        let mut changed = false;
        let mut jobs_changed = false;
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
                    jobs_changed |= info.code == 0xD02A;
                }
            }
            self.capabilities(cx, &info);
            if self.props.get(&info.code).map(|p| &p.current) != Some(&info.current) {
                self.info_property(cx, info.code, &info.current);
            }
            self.props.insert(info.code, info);
        }
        if changed {
            cx.state(patch);
        }
        self.refresh_labels(cx);
        // The camera's FTP job list moved on: read it again.
        if jobs_changed && self.operations.contains(&OP_GET_FTP_JOB_LIST) {
            self.queue_jobs_refresh();
        }
    }

    fn queue_jobs_refresh(&mut self) {
        if !self
            .background
            .iter()
            .any(|op| op.step == Step::JobsRefresh)
        {
            self.background
                .push_back(Op::new(OP_GET_FTP_JOB_LIST, vec![0], Step::JobsRefresh));
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
            0xC211 => {
                put(
                    &mut patch,
                    "ftp.last_job_result",
                    json!(props::drive_result_name(p(0))),
                );
                self.queue_jobs_refresh();
            }
            0xC234 => put(
                &mut patch,
                "content.last_change",
                json!({"slot": p(0), "change": match p(1) { 1 => "added", 2 => "deleted", 3 => "changed", _ => "unknown" }}),
            ),
            0xC210 => put(
                &mut patch,
                "content.last_change",
                json!({"slot": p(0), "change": "media_profile"}),
            ),
            0xC214 | 0xC209 | 0xC20A | 0xC21A | 0xC240 => self.file_event(cx, code, &p),
            0xC20F | 0xC21B | 0xC226 | 0xC205 | 0xC206 | 0xC201 | 0xC202 | 0xC228 | 0x4004
            | 0x4005 => self.info_event(cx, code, &p),
            // Property change: the read below picks it up.
            0xC203 => {}
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
                let streaming = self.download.is_some();
                if let Some(c) = self.current.as_mut().filter(|c| c.transaction == transaction) {
                    if streaming
                        && matches!(c.op.step, Step::DownloadChunk | Step::DownloadWhole)
                    {
                        // Downloads go to the file as they arrive.
                        c.streamed += payload.len() as u64;
                        cx.file_write(FILE, payload);
                        cx.set_timer(REPLY, REPLY_TIMEOUT);
                    } else {
                        c.data.extend_from_slice(&payload);
                    }
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

    // ----- live view -----

    /// Hands a frame, or the reason there is none, to whoever asked.
    fn deliver_frame(
        &mut self,
        cx: &mut Cx,
        target: FrameTarget,
        frame: Result<content::LiveFrame, CommandError>,
    ) {
        match target {
            FrameTarget::Command(id) => cx.complete(
                id,
                frame.map(|frame| Outcome::Value {
                    value: frame_value(&frame),
                }),
            ),
        }
    }

    fn live_view(&mut self, cx: &mut Cx, op: Op, code: u16, data: &[u8], attempt: u32) {
        let Some(id) = op.command else { return };
        let target = FrameTarget::Command(id);
        let frame = if code == RC_OK {
            match content::parse_live_view(data) {
                Ok(frame) => frame,
                Err(e) => {
                    return self.deliver_frame(cx, target, Err(refused("unreadable", e)));
                }
            }
        } else if code == RC_ACCESS_DENIED {
            None
        } else {
            return self.deliver_frame(cx, target, Err(rejected(code)));
        };
        match frame {
            Some(frame) => self.deliver_frame(cx, target, Ok(frame)),
            None if attempt + 1 < LIVE_VIEW_ATTEMPTS => {
                // Asked too soon after the last frame: ask again shortly.
                let mut again = Op::new(
                    OP_GET_OBJECT,
                    vec![LIVE_VIEW_HANDLE],
                    Step::LiveView {
                        attempt: attempt + 1,
                    },
                )
                .for_command(id);
                again.last = true;
                self.commands.push_front(again);
                self.commands
                    .push_front(Op::pause(LIVE_VIEW_RETRY).for_command(id));
            }
            None => self.deliver_frame(
                cx,
                target,
                Err(refused(
                    "no_frame",
                    "the camera returned no live view image",
                )),
            ),
        }
    }

    // ----- values -----

    fn parse_value(&mut self, cx: &mut Cx, parse: Parse, data: &[u8]) -> Result<Value, String> {
        match parse {
            Parse::ContentList => {
                let (value, files) = content::parse_content_info_list(data)?;
                if let (Some(slot), Some(time)) =
                    (value["slot"].as_u64(), value["list_time"].as_u64())
                {
                    self.list_times.insert(slot as u32, time);
                }
                for f in files {
                    self.sizes.insert(f.id.clone(), f.size);
                    self.umids.insert(f.umid, f.id);
                }
                Ok(value)
            }
            Parse::Thumbnail => {
                let jpeg = content::find_jpeg(data).unwrap_or(data);
                let mut v = json!({"format": "jpeg", "bytes": jpeg.len(), "image": b64(jpeg)});
                if let Some((w, h)) = content::jpeg_size(jpeg) {
                    v["width"] = json!(w);
                    v["height"] = json!(h);
                }
                Ok(v)
            }
            Parse::FtpSettings => {
                let (version, servers) = ftp::parse_setting_list(data)?;
                self.ftp_setting_version = version;
                let mut patch = json!({});
                let mut ids = Vec::new();
                for s in &servers {
                    let id = s.id.to_string();
                    put(&mut patch, &format!("ftp.servers.{id}"), s.to_json());
                    ids.push(id);
                }
                for gone in self.ftp_servers.iter().filter(|g| !ids.contains(g)) {
                    put(&mut patch, &format!("ftp.servers.{gone}"), Value::Null);
                }
                self.ftp_servers = ids;
                cx.state(patch);
                Ok(json!({
                    "version": version,
                    "servers": servers.iter().map(ftp::FtpServer::to_json).collect::<Vec<_>>(),
                }))
            }
            Parse::FtpJobs => {
                let (version, sync, jobs) = ftp::parse_job_list(data)?;
                self.ftp_job_version = version;
                let mut patch = json!({});
                let mut ids = Vec::new();
                for j in &jobs {
                    let id = j["job_id"].to_string();
                    put(&mut patch, &format!("ftp.jobs.{id}"), j.clone());
                    ids.push(id);
                }
                for gone in self.ftp_jobs.iter().filter(|g| !ids.contains(g)) {
                    put(&mut patch, &format!("ftp.jobs.{gone}"), Value::Null);
                }
                self.ftp_jobs = ids;
                cx.state(patch);
                Ok(json!({"version": version, "sync_id": sync, "jobs": jobs}))
            }
            Parse::FtpResult => {
                let (succeeded, failed) = ftp::parse_ftp_result(data)?;
                Ok(json!({"succeeded": succeeded, "failed": failed}))
            }
        }
    }

    // ----- content listing (Content Transfer Mode) -----

    fn listed_handles(&mut self, cx: &mut Cx, code: u16, data: &[u8]) {
        let Some(listing) = self.listing.as_mut() else {
            return;
        };
        let id = listing.id;
        if code != RC_OK {
            self.listing = None;
            cx.complete(id, Err(rejected(code)));
            return;
        }
        match content::parse_u32_array(data) {
            Ok(handles) => {
                listing.total = handles.len();
                listing.handles = handles.into_iter().collect();
                self.next_listed(cx);
            }
            Err(e) => {
                self.listing = None;
                cx.complete(id, Err(refused("unreadable", e)));
            }
        }
    }

    fn listed_info(&mut self, cx: &mut Cx, handle: u32, code: u16, data: &[u8]) {
        let Some(listing) = self.listing.as_mut() else {
            return;
        };
        if code == RC_OK {
            if let Ok(info) = content::parse_object_info(data) {
                // Folders are not content.
                if info.format != 0x3001 {
                    if info.size != u32::MAX {
                        self.sizes.insert(format!("o:{handle}"), info.size as u64);
                    }
                    listing.items.push(info.to_json(handle));
                }
            }
        }
        self.next_listed(cx);
    }

    fn next_listed(&mut self, cx: &mut Cx) {
        let Some(listing) = self.listing.as_mut() else {
            return;
        };
        match listing.handles.pop_front() {
            Some(h) => {
                let op =
                    Op::new(OP_GET_OBJECT_INFO, vec![h], Step::ListInfo(h)).for_command(listing.id);
                self.commands.push_front(op);
            }
            None => {
                let listing = self.listing.take().unwrap();
                cx.complete(
                    listing.id,
                    Ok(Outcome::Value {
                        value: json!({
                            "source": "ptp_objects",
                            "slot": listing.slot,
                            "total": listing.total,
                            "items": listing.items,
                        }),
                    }),
                );
            }
        }
    }

    // ----- downloads -----

    fn chunk_op(&self, d: &Download) -> Option<Op> {
        let lo = d.offset as u32;
        let hi = (d.offset >> 32) as u32;
        let op = match d.source {
            Source::Object(h) => Op::new(
                OP_GET_PARTIAL_LARGE_OBJECT,
                vec![h, lo, hi, CHUNK],
                Step::DownloadChunk,
            ),
            Source::Content {
                slot,
                content,
                file,
            } => Op::new(
                OP_GET_CONTENT_DATA,
                vec![content, (slot << 24) | file, lo, hi, CHUNK],
                Step::DownloadChunk,
            ),
            Source::Handle(h) => Op::new(OP_GET_OBJECT, vec![h], Step::DownloadWhole),
            Source::Http | Source::Dataset => return None,
        };
        Some(op.for_command(d.id))
    }

    fn download_info(&mut self, cx: &mut Cx, code: u16, data: &[u8]) {
        let Some(d) = self.download.as_mut() else {
            return;
        };
        let info = if code == RC_OK {
            content::parse_object_info(data).map_err(|e| refused("unreadable", e))
        } else {
            Err(rejected(code))
        };
        match info {
            Ok(info) => {
                if info.size != u32::MAX {
                    d.total = Some(info.size as u64);
                }
                d.name = Some(info.filename);
                cx.file_open(FILE, d.path.clone());
                let d = self.download.as_ref().unwrap();
                if let Some(op) = self.chunk_op(d) {
                    self.commands.push_front(op);
                }
            }
            Err(e) => {
                let d = self.download.take().unwrap();
                cx.complete(d.id, Err(e));
            }
        }
    }

    fn download_chunk(&mut self, cx: &mut Cx, whole: bool, code: u16, received: u64) {
        let Some(d) = self.download.as_mut() else {
            // The file failed and the command has already ended.
            return;
        };
        if code != RC_OK {
            d.error = Some(format!(
                "the camera stopped the transfer: {}",
                props::response_name(code)
            ));
            self.close_download(cx);
            return;
        }
        d.offset += received;
        let done = whole
            || received == 0
            || match d.total {
                Some(total) => d.offset >= total,
                None => received < CHUNK as u64,
            };
        if done {
            self.close_download(cx);
        } else {
            let d = self.download.as_ref().unwrap();
            if let Some(op) = self.chunk_op(d) {
                self.commands.push_front(op);
            }
        }
    }

    fn close_download(&mut self, cx: &mut Cx) {
        if let Some(d) = self.download.as_mut() {
            if !d.closing {
                d.closing = true;
                cx.file_close(FILE);
            }
        }
    }

    // ----- HTTP through the tunnel -----

    fn start_http(&mut self, cx: &mut Cx, id: CommandId, purpose: HttpPurpose, url: Url) {
        cx.tcp_open_ssh(HTTP, self.tunnel(&url.host, url.port));
        cx.set_timer(HTTP_TIMEOUT, HTTP_TIMEOUT_MS);
        self.http = Some(HttpJob {
            id,
            purpose,
            url,
            reader: HttpReader::default(),
            body: Vec::new(),
            chunked: false,
        });
    }

    fn end_http(&mut self, cx: &mut Cx) {
        cx.tcp_close(HTTP);
        cx.cancel_timer(HTTP_TIMEOUT);
        self.http = None;
    }

    fn fail_http(&mut self, cx: &mut Cx, error: CommandError) {
        let Some(job) = self.http.take() else { return };
        cx.tcp_close(HTTP);
        cx.cancel_timer(HTTP_TIMEOUT);
        if job.purpose != HttpPurpose::Download {
            cx.complete(job.id, Err(error));
            return;
        }
        let opened = self.download_file_open();
        let closing = self.download.as_ref().is_some_and(|d| d.closing);
        if self.download.is_none() || closing {
            return;
        }
        if opened {
            if let Some(d) = self.download.as_mut() {
                d.error = Some(error.to_string());
            }
            self.close_download(cx);
        } else {
            let d = self.download.take().unwrap();
            cx.complete(d.id, Err(error));
        }
    }

    /// Whether the file of an HTTP download has been opened (on the
    /// response head).
    fn download_file_open(&self) -> bool {
        self.download
            .as_ref()
            .is_some_and(|d| d.total.is_some() || d.name.is_some())
    }

    fn http_events(&mut self, cx: &mut Cx, events: Vec<HttpEvent>) {
        for event in events {
            let Some(job) = self.http.as_mut() else {
                return;
            };
            match event {
                HttpEvent::Head { status, length, .. } => {
                    if status != 200 {
                        let message =
                            format!("the camera answered HTTP {status} for {}", job.url.path);
                        return self.fail_http(cx, refused("http_status", message));
                    }
                    job.chunked = length.is_none();
                    if job.purpose == HttpPurpose::Download {
                        if let Some(d) = self.download.as_mut() {
                            d.total = length;
                            d.name =
                                Some(job.url.path.rsplit('/').next().unwrap_or("").to_string());
                            cx.file_open(FILE, d.path.clone());
                        }
                    }
                }
                HttpEvent::Body(bytes) => match job.purpose {
                    HttpPurpose::Download => {
                        if let Some(d) = self.download.as_mut() {
                            d.offset += bytes.len() as u64;
                            cx.file_write(FILE, bytes);
                        }
                        cx.set_timer(HTTP_TIMEOUT, HTTP_TIMEOUT_MS);
                    }
                    HttpPurpose::Profile { .. } => {
                        job.body.extend_from_slice(&bytes);
                        if job.body.len() > MAX_PROFILE {
                            return self
                                .fail_http(cx, refused("unreadable", "MediaProfile too large"));
                        }
                    }
                    HttpPurpose::LiveView => {
                        job.body.extend_from_slice(&bytes);
                        if job.body.len() > MAX_PROFILE {
                            return self
                                .fail_http(cx, refused("no_frame", "no live view frame found"));
                        }
                    }
                },
                HttpEvent::ChunkEnd => {
                    if job.purpose == HttpPurpose::LiveView {
                        // One live view dataset per chunk.
                        let body = std::mem::take(&mut job.body);
                        if let Ok(Some(frame)) = content::parse_live_view(&body) {
                            return self.http_frame(cx, frame);
                        }
                        if let Some(jpeg) = content::find_jpeg(&body) {
                            let frame = content::LiveFrame {
                                jpeg: jpeg.to_vec(),
                                focal_frame_info: None,
                            };
                            return self.http_frame(cx, frame);
                        }
                    }
                }
                HttpEvent::End => return self.http_done(cx),
            }
            // A stream that is not chunked: take the first whole JPEG.
            if let Some(job) = self.http.as_ref() {
                if job.purpose == HttpPurpose::LiveView && !job.chunked {
                    if let Some(jpeg) = content::find_jpeg(&job.body) {
                        let frame = content::LiveFrame {
                            jpeg: jpeg.to_vec(),
                            focal_frame_info: None,
                        };
                        return self.http_frame(cx, frame);
                    }
                }
            }
        }
    }

    fn http_frame(&mut self, cx: &mut Cx, frame: content::LiveFrame) {
        let target = self.http.as_ref().map(|job| FrameTarget::Command(job.id));
        if let Some(target) = target {
            self.deliver_frame(cx, target, Ok(frame));
        }
        self.end_http(cx);
    }

    fn http_done(&mut self, cx: &mut Cx) {
        let Some(job) = self.http.as_mut() else {
            return;
        };
        match job.purpose {
            HttpPurpose::Download => {
                self.end_http(cx);
                self.close_download(cx);
            }
            HttpPurpose::Profile { slot } => {
                let text = String::from_utf8_lossy(&job.body).into_owned();
                let id = job.id;
                let result = content::parse_media_profile(&text, slot);
                self.end_http(cx);
                cx.complete(
                    id,
                    result
                        .map(|value| Outcome::Value { value })
                        .map_err(|e| refused("unreadable", e)),
                );
            }
            HttpPurpose::LiveView => {
                let body = std::mem::take(&mut job.body);
                let frame = content::parse_live_view(&body).ok().flatten().or_else(|| {
                    content::find_jpeg(&body).map(|j| content::LiveFrame {
                        jpeg: j.to_vec(),
                        focal_frame_info: None,
                    })
                });
                match frame {
                    Some(frame) => self.http_frame(cx, frame),
                    None => self.fail_http(cx, refused("no_frame", "no live view frame found")),
                }
            }
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

    fn has_control(&self, code: u16) -> bool {
        self.controls.contains(&code)
    }

    /// A text property's value, when the camera reports one.
    fn text_prop(&self, code: u16) -> Option<String> {
        match self.props.get(&code).map(|p| &p.current) {
            Some(PtpValue::Str(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        }
    }

    fn need_ssh(&self, what: &str) -> Result<(), CommandError> {
        if self.mode == Mode::Ssh {
            Ok(())
        } else {
            Err(refused(
                "needs_ssh",
                format!("{what} is served over HTTP on the camera's own localhost, reached only through SSH: open the device with connection ssh"),
            ))
        }
    }

    /// The MediaProfile URL of a video-only camera's slot.
    fn profile_url(&self, slot: u32) -> Option<Url> {
        let code = match slot {
            1 => 0xD031,
            2 => 0xD032,
            3 => 0xD191,
            _ => return None,
        };
        parse_url(&self.text_prop(code)?)
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
        Ok(Op::with_data(OP_SET_PROPERTY, self.code_params(code), data))
    }

    /// Sets a value: directly, or, where the camera only reports it and
    /// offers a step control of the same code (Camera Control PTP 2), by the
    /// number of steps between the current and the target value in the
    /// camera's own list.
    fn set_value(&self, code: u16, value: PtpValue) -> Result<Vec<Op>, CommandError> {
        let stepped = props::STEPPED.iter().any(|(_, c)| *c == code);
        match self.props.get(&code) {
            Some(info) if !info.writable && stepped && self.has_control(code) => {
                let list = match &info.form {
                    Form::Enum { values, settable } => {
                        if values.is_empty() {
                            settable
                        } else {
                            values
                        }
                    }
                    _ => {
                        return Err(refused(
                            "not_available",
                            format!("property 0x{code:04X} lists no values to step through"),
                        ))
                    }
                };
                let target = list.iter().position(|v| *v == value).ok_or_else(|| {
                    invalid(format!(
                        "{} is not among the values the camera lists for 0x{code:04X}",
                        value.to_json()
                    ))
                })?;
                let current = list
                    .iter()
                    .position(|v| *v == info.current)
                    .ok_or_else(|| {
                        refused(
                            "not_available",
                            "the current value is not in the camera's list; use exposure_step",
                        )
                    })?;
                let steps = (target as i128 - current as i128).clamp(-127, 127);
                if steps == 0 {
                    return Ok(vec![]);
                }
                Ok(vec![self.control(code, steps)?])
            }
            _ => Ok(vec![self.set_prop(code, value)?]),
        }
    }

    fn control_op(&self, code: u16, value: PtpValue, datatype: u16) -> Result<Op, CommandError> {
        if !self.controls.is_empty() && !self.controls.contains(&code) {
            return Err(refused(
                "not_reported",
                format!("the camera does not offer control 0x{code:04X}"),
            ));
        }
        let data = ds::encode(datatype, &value).map_err(invalid)?;
        Ok(Op::with_data(OP_CONTROL, self.code_params(code), data))
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

    /// The first of these buttons the camera offers (PTP 3 code first).
    fn first_button(&self, codes: &[u16]) -> Result<Vec<Op>, CommandError> {
        let code = codes
            .iter()
            .copied()
            .find(|c| self.has_control(*c))
            .unwrap_or(codes[0]);
        self.button(code, None)
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
        Ok(Op::with_data(OP_SET_PROPERTY, self.code_params(code), data))
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

    fn get_live_view(&self) -> Result<Plan, CommandError> {
        if let Some(url) = self.text_prop(0xD278).and_then(|u| parse_url(&u)) {
            self.need_ssh("this camera's live view")?;
            return Ok(Plan::Http(HttpPurpose::LiveView, url));
        }
        if let Some(info) = self.props.get(&0xD221) {
            if info.current != PtpValue::Int(1) {
                return Err(refused(
                    "live_view_unavailable",
                    "the camera reports live view unavailable now (Live View Status)",
                ));
            }
        }
        let mut ops = Vec::new();
        if self.function == Function::RemoteWithTransfer
            && !self.live_view_enabled
            && self.has_control(0xD313)
        {
            ops.push(self.control(0xD313, DOWN)?);
        }
        let mut frame = Op::new(
            OP_GET_OBJECT,
            vec![LIVE_VIEW_HANDLE],
            Step::LiveView { attempt: 0 },
        );
        frame.last = true;
        ops.push(frame);
        Ok(Plan::Ops(ops))
    }

    fn list_content(&self, params: &Params) -> Result<Plan, CommandError> {
        let slot = param_int_or(params, "slot", 1)? as u32;
        let limit = param_int_or(params, "limit", 100)? as u32;
        if let Some(url) = self.profile_url(slot) {
            self.need_ssh("this camera's clip list")?;
            return Ok(Plan::Http(HttpPurpose::Profile { slot }, url));
        }
        match self.function {
            Function::RemoteWithTransfer => {
                let since = param_int_or(params, "since", 0)? as u64;
                let mut op = Op::new(
                    OP_GET_CONTENT_INFO_LIST,
                    vec![since as u32, (since >> 32) as u32, limit, slot, 0],
                    Step::Value(Parse::ContentList),
                );
                op.last = true;
                Ok(Plan::Ops(vec![op]))
            }
            Function::ContentTransfer => {
                let storage = (slot << 16) | 1;
                Ok(Plan::Listing(
                    slot,
                    limit,
                    Op::new(OP_GET_OBJECT_HANDLES, vec![storage, 0, 0], Step::ListHandles),
                ))
            }
            Function::Remote => Err(refused(
                "session_mode",
                "the session is in remote control mode; switch_session_mode to remote_with_transfer or content_transfer to reach the card",
            )),
        }
    }

    fn download_content(&self, params: &Params, id: CommandId) -> Result<Plan, CommandError> {
        if self.download.is_some() {
            return Err(refused("busy", "another download is in progress"));
        }
        let resume = param_bool(params, "resume").unwrap_or(false);
        let content_id = match params.get("umid").and_then(Value::as_str) {
            Some(umid) => self.content_by_umid(umid)?,
            None => param_str(params, "id")?,
        };
        let path = param_str(params, "path")?.to_string();
        let bad = || invalid(format!("'{content_id}' is not an id from list_content"));
        if resume && !content_id.starts_with("c:") {
            return Err(refused(
                "not_resumable",
                "only content list downloads (remote_with_transfer mode) can be resumed",
            ));
        }
        let mut parts = content_id.splitn(4, ':');
        let new = |source, total| Download {
            id,
            path: path.clone(),
            source,
            offset: 0,
            total,
            name: None,
            error: None,
            closing: false,
        };
        match parts.next() {
            Some("c") => {
                if self.function != Function::RemoteWithTransfer {
                    return Err(refused(
                        "session_mode",
                        "content list ids need remote_with_transfer mode",
                    ));
                }
                let mut n = || {
                    parts
                        .next()
                        .and_then(|p| p.parse::<u32>().ok())
                        .ok_or_else(bad)
                };
                let (slot, content, file) = (n()?, n()?, n()?);
                let total = self.sizes.get(content_id).copied();
                let d = new(
                    Source::Content {
                        slot,
                        content,
                        file,
                    },
                    total,
                );
                if resume {
                    return Ok(Plan::Resume(d));
                }
                let op = self.chunk_op(&d);
                Ok(Plan::Download(d, op.into_iter().collect()))
            }
            Some("o") => {
                if self.function != Function::ContentTransfer {
                    return Err(refused(
                        "session_mode",
                        "object ids need content_transfer mode",
                    ));
                }
                let handle: u32 = parts.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
                let d = new(Source::Object(handle), None);
                let op =
                    Op::new(OP_GET_OBJECT_INFO, vec![handle], Step::DownloadInfo).for_command(id);
                Ok(Plan::Download(d, vec![op]))
            }
            Some("m") => {
                let slot: u32 = parts.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
                let uri = parts.next().ok_or_else(bad)?;
                self.need_ssh("this camera's clips")?;
                if self.http.is_some() {
                    return Err(refused("busy", "another HTTP transfer is in progress"));
                }
                let url = self
                    .profile_url(slot)
                    .ok_or_else(|| {
                        refused("not_reported", format!("no MediaProfile for slot {slot}"))
                    })?
                    .join(uri);
                Ok(Plan::Download(new(Source::Http, None), vec![]).with_http(url))
            }
            _ => Err(bad()),
        }
    }

    /// The current download id of a listed file, by its UMID. Content
    /// numbers change when the camera regenerates its list (after an edit or
    /// deletion); the UMID does not, so a list older than the camera's is
    /// refused rather than trusted.
    fn content_by_umid(&self, umid: &str) -> Result<&str, CommandError> {
        let umid = umid.to_ascii_lowercase();
        let id = self.umids.get(&umid).ok_or_else(|| {
            refused(
                "not_listed",
                "no listed file has that UMID: list_content first",
            )
        })?;
        let slot: u32 = id
            .split(':')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        let regenerated = match slot {
            1 => self.props.get(&0xD1D6),
            2 => self.props.get(&0xD1D7),
            _ => None,
        }
        .and_then(|p| p.current.as_int());
        let listed = self.list_times.get(&slot).copied();
        if let (Some(now), Some(listed)) = (regenerated, listed) {
            if now as u64 > listed {
                return Err(refused(
                    "list_changed",
                    format!("the camera regenerated slot {slot}'s content list since it was listed: list_content again, then download by UMID"),
                ));
            }
        }
        Ok(id)
    }

    /// The part of a resumed download already on the host arrived: write it
    /// back and continue from where it ends.
    fn resume_read(&mut self, cx: &mut Cx, data: Vec<u8>) {
        let Some(d) = self.download.as_mut() else {
            return;
        };
        let have = data.len() as u64;
        if d.total.is_some_and(|total| have > total) {
            let d = self.download.take().unwrap();
            cx.complete(
                d.id,
                Err(refused(
                    "not_resumable",
                    "the file on the host is larger than the camera's file: it is another file",
                )),
            );
            return;
        }
        cx.file_open(FILE, d.path.clone());
        if have > 0 {
            cx.file_write(FILE, data);
        }
        d.offset = have;
        if d.total.is_some_and(|total| have >= total) {
            return self.close_download(cx);
        }
        let d = self.download.as_ref().unwrap();
        if let Some(op) = self.chunk_op(d) {
            self.commands.push_back(op);
        }
        self.pump(cx);
    }

    /// The operations for a command, or its immediate result.
    fn plan(&self, name: &str, params: &Params, id: CommandId) -> Result<Plan, CommandError> {
        if let Some(plan) = self.plan_files(name, params, id) {
            return plan;
        }
        if let Some(plan) = self.plan_info(name, params, id) {
            return plan;
        }
        let pressed = param_bool(params, "pressed");
        let ops = |v: Result<Op, CommandError>| v.map(|op| Plan::Ops(vec![op]));
        if let Some(prop) = props::by_command(name) {
            let value = params
                .get("value")
                .ok_or_else(|| invalid("'value' is required"))?;
            let wire = props::encode(prop.decode, value).map_err(invalid)?;
            return Ok(Plan::Ops(self.set_value(prop.code, wire)?));
        }
        match name {
            "set_property" => {
                let code = param_code(params)?;
                let info = self.prop(code)?;
                let value = params
                    .get("value")
                    .ok_or_else(|| invalid("'value' is required"))?;
                let wire = raw_value(info.datatype, value)?;
                Ok(Plan::Ops(self.set_value(code, wire)?))
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
                Ok(Plan::Ops(self.set_value(
                    code,
                    props::encode(decode, value).map_err(invalid)?,
                )?))
            }
            "exposure_step" => {
                let target = param_str(params, "target")?;
                let code = props::STEPPED
                    .iter()
                    .find(|(n, _)| *n == target)
                    .map(|(_, c)| *c)
                    .ok_or_else(|| invalid(format!("unknown target '{target}'")))?;
                ops(self.control(code, param_int(params, "steps")?))
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
            "take_photo" => Ok(Plan::Ops(self.first_button(&[0xD2E6, 0xD2C7])?)),
            "record" => {
                let on = param_bool(params, "recording")
                    .ok_or_else(|| invalid("'recording' is required"))?;
                ops(self.control(0xD2C8, if on { DOWN } else { UP }))
            }
            "record_toggle" => Ok(Plan::Ops(self.button(0xF001, None)?)),
            "ae_lock" => Ok(Plan::Ops(self.button(0xD2C3, pressed)?)),
            "af_lock" => Ok(Plan::Ops(self.button(0xD2C4, pressed)?)),
            "awb_lock" => Ok(Plan::Ops(self.button(0xD2D9, pressed)?)),
            "fe_lock" => Ok(Plan::Ops(self.button(0xD2C9, pressed)?)),
            "tracking_af_on" => Ok(Plan::Ops(self.button(0xD30D, pressed)?)),
            "af_mf_hold" => Ok(Plan::Ops(self.button(0xD2D2, pressed)?)),
            "release_lock" => {
                let locked =
                    param_bool(params, "locked").ok_or_else(|| invalid("'locked' is required"))?;
                ops(self.control(0xD2C5, if locked { DOWN } else { UP }))
            }
            "focus_magnifier" => Ok(Plan::Ops(self.button(0xD2CB, None)?)),
            "focus_magnifier_cancel" => Ok(Plan::Ops(self.button(0xD2CC, None)?)),
            "focus_step" => {
                let code = match param_str(params, "direction")? {
                    "near" => 0xD2D7,
                    "far" => 0xD2D8,
                    other => return Err(invalid(format!("unknown direction '{other}'"))),
                };
                Ok(Plan::Ops(self.button(code, None)?))
            }
            "hfr_standby" => Ok(Plan::Ops(self.button(0xD2D5, None)?)),
            "hfr_recording_cancel" => Ok(Plan::Ops(self.button(0xD2D6, None)?)),
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
                if !self.has_control(0xD2E2) && self.has_control(0xD2CA) {
                    // Camera Control PTP 2: one card, formatted by a press.
                    return Ok(Plan::Ops(self.button(0xD2CA, None)?));
                }
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
            // Live view
            "get_live_view_image" => self.get_live_view(),
            // Content
            "list_content" => self.list_content(params),
            "download_content" => self.download_content(params, id),
            "download_captured_image" => {
                if self.download.is_some() {
                    return Err(refused("busy", "another download is in progress"));
                }
                let d = Download {
                    id,
                    path: param_str(params, "path")?.to_string(),
                    source: Source::Handle(CAPTURED_HANDLE),
                    offset: 0,
                    total: None,
                    name: None,
                    error: None,
                    closing: false,
                };
                let op = Op::new(
                    OP_GET_OBJECT_INFO,
                    vec![CAPTURED_HANDLE],
                    Step::DownloadInfo,
                )
                .for_command(id);
                Ok(Plan::Download(d, vec![op]))
            }
            "get_thumbnail" => {
                let text = param_str(params, "id")?;
                let parts: Vec<u32> = text
                    .strip_prefix("c:")
                    .map(|r| r.split(':').filter_map(|p| p.parse().ok()).collect())
                    .unwrap_or_default();
                let [slot, content, file] = parts[..] else {
                    return Err(invalid(format!(
                        "'{text}' is not a content list id (c:slot:content:file)"
                    )));
                };
                let mut op = Op::new(
                    OP_GET_CONTENT_COMPRESSED_DATA,
                    vec![content, (slot << 24) | file, 1],
                    Step::Value(Parse::Thumbnail),
                );
                op.last = true;
                Ok(Plan::Ops(vec![op]))
            }
            "switch_session_mode" => {
                let mode = param_str(params, "mode")?;
                let function = Function::parse(mode)
                    .ok_or_else(|| invalid(format!("unknown mode '{mode}'")))?;
                if self.protocol == Protocol::Ptp2 && function != Function::Remote {
                    return Err(refused(
                        "session_mode",
                        "Camera Control PTP 2 has only the remote control session mode",
                    ));
                }
                Ok(Plan::Switch(function))
            }
            // FTP
            "get_ftp_settings" => {
                let mut op = Op::new(
                    OP_GET_FTP_SETTING_LIST,
                    vec![],
                    Step::Value(Parse::FtpSettings),
                );
                op.last = true;
                Ok(Plan::Ops(vec![op]))
            }
            "set_ftp_server" => {
                if let Some(info) = self.props.get(&0xD09A) {
                    if info.current != PtpValue::Int(1) {
                        return Err(refused(
                            "not_available",
                            "the camera does not accept FTP settings now (FTPSettingList Operation Enable Status)",
                        ));
                    }
                }
                let server = ftp::FtpServerWrite {
                    id: u16::try_from(param_int(params, "server_id")?)
                        .map_err(|_| invalid("'server_id' is out of range"))?,
                    name: param_str_or(params, "name").into(),
                    host: param_str(params, "host")?.into(),
                    port: u16::try_from(param_int_or(params, "port", 21)?)
                        .map_err(|_| invalid("'port' is out of range"))?,
                    user: param_str_or(params, "username").into(),
                    password: params
                        .get("password")
                        .and_then(Value::as_str)
                        .map(String::from),
                    passive: param_bool(params, "passive").unwrap_or(true),
                    directory: param_str_or(params, "directory").into(),
                    secure: match params
                        .get("secure")
                        .and_then(Value::as_str)
                        .unwrap_or("off")
                    {
                        "ftps" => 2,
                        "sftp" => 3,
                        _ => 1,
                    },
                    hierarchy: match param_str_or(params, "directory_hierarchy") {
                        "same_as_camera" => 2,
                        _ => 1,
                    },
                    overwrite: if param_bool(params, "overwrite").unwrap_or(false) {
                        1
                    } else {
                        2
                    },
                    certificate_error: match param_str_or(params, "certificate_error") {
                        "connect" => 1,
                        _ => 2,
                    },
                };
                let data = ftp::encode_setting_list(self.ftp_setting_version, &server);
                let mut op = Op::with_data(OP_SET_FTP_SETTING_LIST, vec![], data);
                op.last = true;
                Ok(Plan::Ops(vec![op]))
            }
            "get_ftp_jobs" => {
                let mut op = Op::new(OP_GET_FTP_JOB_LIST, vec![0], Step::Value(Parse::FtpJobs));
                op.last = true;
                Ok(Plan::Ops(vec![op]))
            }
            "add_ftp_job" => {
                let job = ftp::NewJob {
                    server: param_u32(params, "server_id")?,
                    slot: param_u32(params, "slot")?,
                    clip_path: param_str(params, "clip_path")?.into(),
                    directory: param_str_or(params, "directory").into(),
                    name: param_str_or(params, "name").into(),
                };
                let data = ftp::encode_job_add(self.ftp_job_version, &job).map_err(invalid)?;
                ops(Ok(Op::with_data(OP_CONTROL_FTP_JOB_LIST, vec![1, 0], data)))
            }
            "ftp_job" => {
                let action = param_str(params, "action")?;
                let job =
                    || -> Result<Vec<u32>, CommandError> { Ok(vec![param_u32(params, "job_id")?]) };
                let (control, target, ids) = match action {
                    "suspend" => (3, 0, Some(job()?)),
                    "resume" => (4, 0, Some(job()?)),
                    "delete" => (2, 1, Some(job()?)),
                    "delete_all" => (2, 2, None),
                    "delete_finished" => (2, 3, None),
                    other => return Err(invalid(format!("unknown action '{other}'"))),
                };
                let data = ftp::encode_job_ids(self.ftp_job_version, ids.as_deref());
                ops(Ok(Op::with_data(
                    OP_CONTROL_FTP_JOB_LIST,
                    vec![control, target],
                    data,
                )))
            }
            "get_ftp_result" => {
                let mut op = Op::new(
                    OP_GET_DISPLAY_FTP_RESULT,
                    vec![param_u32(params, "slot")?],
                    Step::Value(Parse::FtpResult),
                );
                op.last = true;
                Ok(Plan::Ops(vec![op]))
            }
            other => Err(CommandError::UnknownCommand {
                command: other.into(),
            }),
        }
    }
}

impl Plan {
    /// An HTTP download: the download plus where it comes from.
    fn with_http(self, url: Url) -> Plan {
        match self {
            Plan::Download(d, _) => Plan::HttpDownload(d, url),
            other => other,
        }
    }
}

impl Module for SonyCamera {
    fn start(&mut self, cx: &mut Cx) {
        let mut session = json!({
            "mode": self.mode.name(),
            "protocol": self.protocol.name(),
            "function_mode": self.function.name(),
        });
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
        if self.function == Function::ContentTransfer && !CONTENT_TRANSFER_COMMANDS.contains(&name)
        {
            cx.complete(
                id,
                Err(refused(
                    "session_mode",
                    "the camera is in content transfer mode and takes no remote control; switch_session_mode to remote or remote_with_transfer",
                )),
            );
            return;
        }
        match self.plan(name, params, id) {
            Ok(Plan::Ops(ops)) => {
                if ops.is_empty() {
                    cx.complete(id, Ok(Outcome::Ack));
                    return;
                }
                let count = ops.len();
                for (i, mut op) in ops.into_iter().enumerate() {
                    op.command = Some(id);
                    op.last = op.last || i + 1 == count;
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
            Ok(Plan::Http(purpose, url)) => {
                if self.http.is_some() {
                    cx.complete(
                        id,
                        Err(refused("busy", "another HTTP transfer is in progress")),
                    );
                    return;
                }
                self.start_http(cx, id, purpose, url);
            }
            Ok(Plan::HttpDownload(d, url)) => {
                self.download = Some(d);
                self.start_http(cx, id, HttpPurpose::Download, url);
            }
            Ok(Plan::Download(d, ops)) => {
                // Opened now when the data comes straight away; otherwise
                // once the object's size, or the dataset, is known.
                let open_now = matches!(
                    ops.first().map(|o| &o.step),
                    Some(Step::DownloadChunk | Step::DownloadWhole)
                );
                if open_now {
                    cx.file_open(FILE, d.path.clone());
                }
                self.download = Some(d);
                self.commands.extend(ops);
                self.pump(cx);
            }
            Ok(Plan::Upload(upload, path, max)) => self.begin_upload(cx, upload, path, max),
            Ok(Plan::Resume(d)) => {
                let max = d.total.unwrap_or(MAX_RESUME).min(MAX_RESUME);
                cx.file_read(RESUME, d.path.clone(), max);
                self.download = Some(d);
            }
            Ok(Plan::Delete(deletion, op)) => {
                if let Some(wait) = self.delete_spacing(cx.now()) {
                    self.commands.push_back(Op::pause(wait).for_command(id));
                }
                self.deletion = Some(deletion);
                self.commands.push_back(op);
                self.pump(cx);
            }
            Ok(Plan::Listing(slot, limit, op)) => {
                if self.listing.is_some() {
                    cx.complete(id, Err(refused("busy", "another listing is in progress")));
                    return;
                }
                self.listing = Some(Listing {
                    id,
                    slot,
                    handles: VecDeque::new(),
                    items: Vec::new(),
                    total: limit as usize,
                });
                self.commands.push_back(op.for_command(id));
                self.pump(cx);
            }
            Ok(Plan::Switch(function)) => {
                if function == self.function {
                    cx.complete(id, Ok(Outcome::Ack));
                    return;
                }
                if self.function == Function::ContentTransfer {
                    self.commands.push_back(
                        Op::new(
                            OP_SET_CONTENTS_TRANSFER_MODE,
                            vec![self.selection(), 0, 0],
                            Step::TransferMode,
                        )
                        .for_command(id),
                    );
                }
                self.commands.push_back(
                    Op::new(OP_CLOSE_SESSION, vec![], Step::Switch(function)).for_command(id),
                );
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        if socket == HTTP {
            return self.http_input(cx, input);
        }
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

    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        if file == files::READ {
            match input {
                FileInput::Read { data } => self.upload_read(cx, data),
                FileInput::Failed { message } => {
                    if let Some(upload) = self.upload.take() {
                        cx.complete(upload.id, Err(refused("file", message)));
                    }
                }
                FileInput::Closed { .. } => {}
            }
            return;
        }
        if file == RESUME {
            match input {
                FileInput::Read { data } => self.resume_read(cx, data),
                FileInput::Failed { message } => {
                    if let Some(d) = self.download.take() {
                        cx.complete(
                            d.id,
                            Err(refused(
                                "not_resumable",
                                format!("the partial file could not be read ({message}); download without resume to start again"),
                            )),
                        );
                    }
                }
                FileInput::Closed { .. } => {}
            }
            return;
        }
        if file != FILE {
            return;
        }
        let Some(d) = self.download.take() else {
            return;
        };
        match input {
            // Reads are reported on their own key.
            FileInput::Read { .. } => self.download = Some(d),
            FileInput::Closed { bytes } => {
                let result = match d.error {
                    Some(e) => Err(refused("download_failed", e)),
                    None => {
                        let mut v = json!({"path": d.path, "bytes": bytes});
                        if let Some(name) = d.name {
                            v["name"] = json!(name);
                        }
                        Ok(Outcome::Value { value: v })
                    }
                };
                cx.complete(d.id, result);
            }
            FileInput::Failed { message } => {
                // Later chunks find no download and stop.
                cx.complete(d.id, Err(refused("file", message)));
                if self
                    .http
                    .as_ref()
                    .is_some_and(|j| j.purpose == HttpPurpose::Download)
                {
                    self.end_http(cx);
                }
            }
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
                    let op = self.ext_info_op(false);
                    self.background.push_front(op);
                    self.pump(cx);
                }
            }
            POLL => {
                self.want_refresh(cx, false);
                if self.ready {
                    cx.set_timer(POLL, self.poll_every);
                }
            }
            HTTP_TIMEOUT => {
                self.fail_http(cx, CommandError::Timeout);
            }
            files::UPLOAD_WAIT => self.upload_timeout(cx),
            files::DELETE_WAIT => self.delete_timeout(cx),
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
        if self.http.is_some() {
            cx.tcp_close(HTTP);
        }
        if self.download.is_some() {
            cx.file_close(FILE);
        }
        cx.tcp_close(EVT);
        cx.tcp_close(CMD);
    }
}

impl SonyCamera {
    fn http_input(&mut self, cx: &mut Cx, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                if let Some(job) = self.http.as_ref() {
                    cx.tcp_send(HTTP, job.url.request());
                }
            }
            TcpInput::Data(data) => {
                let Some(job) = self.http.as_mut() else {
                    return;
                };
                match job.reader.feed(&data) {
                    Ok(events) => self.http_events(cx, events),
                    Err(e) => self.fail_http(cx, refused("http", e)),
                }
            }
            TcpInput::Closed { reason } => {
                let Some(job) = self.http.as_mut() else {
                    return;
                };
                match job.reader.closed() {
                    Ok(events) => {
                        self.http_events(cx, events);
                        if self.http.is_some() {
                            self.fail_http(
                                cx,
                                CommandError::Transport {
                                    message: format!("HTTP stream closed: {reason}"),
                                },
                            );
                        }
                    }
                    Err(e) => self.fail_http(
                        cx,
                        CommandError::Transport {
                            message: format!("{e}: {reason}"),
                        },
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use ds::build;
    use std::net::Ipv4Addr;

    fn settings(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    /// A camera of the given model; the session mode is remote control
    /// unless the settings say otherwise.
    fn camera_model(model: &str, s: Value) -> SonyCamera {
        let mut s = settings(s);
        s.entry("session_mode").or_insert(json!("remote"));
        SonyCamera::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)),
            port: None,
            model: model.into(),
            channels: None,
            settings: s,
        })
        .unwrap()
    }

    fn camera(s: Value) -> SonyCamera {
        camera_model("ilce-7sm3", s)
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

    /// What the simulated camera answers during the handshake.
    struct Sim {
        vendor: u32,
        version: u16,
        props: Vec<u8>,
        controls: Vec<u16>,
        operations: Vec<u16>,
    }

    impl Default for Sim {
        fn default() -> Sim {
            Sim {
                vendor: 300,
                version: 0x012C,
                props: props_dataset(),
                controls: vec![0xD2C1, 0xD2C2, 0xD2C8, 0xD2DD],
                operations: vec![0x1001, 0x1002],
            }
        }
    }

    /// Runs the whole connection sequence and returns the actions.
    fn connected(m: &mut SonyCamera, vendor: u32) -> Vec<Action> {
        connected_with(
            m,
            Sim {
                vendor,
                ..Sim::default()
            },
        )
    }

    fn connected_with(m: &mut SonyCamera, sim: Sim) -> Vec<Action> {
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
        for _ in 0..20 {
            let (code, t, _) = last_request(&all);
            let packets = match code {
                OP_GET_DEVICE_INFO => {
                    let mut v = data_in(
                        t,
                        build::device_info_with(
                            &sim.operations,
                            "Sony Corporation",
                            "ILCE-7SM3",
                            "3.00",
                            "5001",
                        ),
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
                            sim.version,
                            &[0x5005, 0x5007, 0xD21D],
                            &sim.controls,
                        ),
                    );
                    v.push(ok(
                        t,
                        if sim.version >= 300 {
                            vec![sim.vendor]
                        } else {
                            vec![]
                        },
                    ));
                    v
                }
                OP_GET_ALL_PROPERTIES => {
                    let mut v = data_in(t, sim.props.clone());
                    v.push(ok(t, vec![]));
                    all.extend(feed(m, CMD, &v));
                    return all;
                }
                _ => vec![ok(t, vec![])],
            };
            all.extend(feed(m, CMD, &packets));
        }
        panic!("the handshake does not end");
    }

    /// Answers the pending request with data and OK.
    fn answer(
        m: &mut SonyCamera,
        actions: &[Action],
        payload: Vec<u8>,
        params: Vec<u32>,
    ) -> Vec<Action> {
        let (_, t, _) = last_request(actions);
        let mut packets = data_in(t, payload);
        packets.push(ok(t, params));
        feed(m, CMD, &packets)
    }

    fn run(m: &mut SonyCamera, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(10);
        m.command(&mut cx, id, name, &settings(p));
        cx.take()
    }

    fn completion(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
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
            if let Err(CommandError::UnknownCommand { .. }) = m.plan(name, &Params::new(), 0) {
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

    // ----- Camera Control PTP 2 -----

    fn ops_sent(actions: &[Action]) -> Vec<(u16, Vec<u32>)> {
        sent(actions, CMD)
            .into_iter()
            .filter_map(|p| match p {
                Packet::OperationRequest { code, params, .. } => Some((code, params)),
                _ => None,
            })
            .collect()
    }

    fn ptp2_props() -> Vec<u8> {
        build::prop_array(&[
            // F-number reported only, with the values it steps through.
            build::enum_prop(0x5007, dt::UINT16, false, 1, 280, &[280, 400, 560]),
            build::enum_prop(0x5005, dt::UINT16, true, 1, 2, &[2, 4]),
            build::enum_prop(0xD221, dt::UINT8, false, 1, 1, &[]),
        ])
    }

    #[test]
    fn ptp2_bodies_announce_2_00_and_read_everything() {
        let mut m = camera_model("ilce-7m3", json!({}));
        let all = connected_with(
            &mut m,
            Sim {
                version: 0x00C8,
                props: ptp2_props(),
                controls: vec![0x5007, 0xD2C1, 0xD2C7],
                ..Sim::default()
            },
        );
        let ops = ops_sent(&all);
        assert!(ops.contains(&(OP_EXT_DEVICE_INFO, vec![0x00C8])));
        assert_eq!(ops.last().unwrap(), &(OP_GET_ALL_PROPERTIES, vec![]));
        assert!(ops.contains(&(OP_OPEN_SESSION, vec![1])));
        let s = state(&all);
        assert_eq!(s["session"]["protocol"], "ptp2");
        assert_eq!(s["exposure"]["iris"], 2.8);

        // A writable property: just the code as parameter.
        let a = run(&mut m, 1, "set_white_balance", json!({"value": "daylight"}));
        assert_eq!(last_request(&a).2, vec![0x5005]);
        let a = answer(&mut m, &a, vec![], vec![]);
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));
        // The read after it is a full one.
        assert_eq!(last_request(&a).2, Vec::<u32>::new());
        answer(&mut m, &a, ptp2_props(), vec![]);

        // A reported-only value: two steps up the camera's list.
        let a = run(&mut m, 2, "set_iris", json!({"value": 5.6}));
        let packets = sent(&a, CMD);
        assert!(
            matches!(&packets[0], Packet::OperationRequest { code: OP_CONTROL, params, .. } if params == &vec![0x5007])
        );
        assert!(matches!(&packets[2], Packet::EndData { payload, .. } if payload == &vec![2]));
        let a = answer(&mut m, &a, vec![], vec![]);
        answer(&mut m, &a, ptp2_props(), vec![]);
        // Already there: nothing to send.
        let a = run(&mut m, 3, "set_iris", json!({"value": 2.8}));
        assert_eq!(completion(&a, 3), Some(Ok(Outcome::Ack)));
        assert!(sent(&a, CMD).is_empty());
        // One-shot release where the PTP 3 button is missing.
        let a = run(&mut m, 4, "take_photo", json!({}));
        assert!(
            matches!(&sent(&a, CMD)[0], Packet::OperationRequest { params, .. } if params == &vec![0xD2C7])
        );
        // Transfer modes do not exist in PTP 2.
        let a = run(
            &mut m,
            5,
            "switch_session_mode",
            json!({"mode": "content_transfer"}),
        );
        assert!(matches!(
            completion(&a, 5),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn a_ptp2_answer_to_a_ptp3_request_switches_protocol() {
        let mut m = camera(json!({}));
        let all = connected_with(
            &mut m,
            Sim {
                version: 0x00C8,
                ..Sim::default()
            },
        );
        assert!(all.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(state(&all)["session"]["protocol"], "ptp2");
        assert_eq!(
            ops_sent(&all).last().unwrap(),
            &(OP_GET_ALL_PROPERTIES, vec![])
        );
    }

    // ----- live view -----

    #[test]
    fn live_view_frames_with_a_retry_when_asked_too_soon() {
        let mut m = camera(json!({}));
        connected(&mut m, 300);
        let a = run(&mut m, 7, "get_live_view_image", json!({}));
        assert_eq!(last_request(&a), (OP_GET_OBJECT, 7, vec![LIVE_VIEW_HANDLE]));
        // No image yet.
        let a = answer(&mut m, &a, vec![16, 0, 0, 0, 0, 0, 0, 0], vec![]);
        assert!(a.contains(&Action::SetTimer {
            key: PAUSE,
            after: LIVE_VIEW_RETRY
        }));
        let mut cx = Cx::new(60);
        m.timer(&mut cx, PAUSE);
        let a = cx.take();
        assert_eq!(last_request(&a).0, OP_GET_OBJECT);
        let jpeg = content::build::jpeg(1024, 576);
        let a = answer(&mut m, &a, content::build::live_view(&jpeg, true), vec![]);
        match completion(&a, 7) {
            Some(Ok(Outcome::Value { value })) => {
                assert_eq!(value["width"], 1024);
                assert_eq!(value["height"], 576);
                assert_eq!(value["image"], b64(&jpeg));
                assert_eq!(value["focal_frame_info"], b64(&[7; 4]));
            }
            other => panic!("expected a frame, got {other:?}"),
        }
    }

    #[test]
    fn pan_tilt_live_view_is_read_over_http_through_the_tunnel() {
        let mut m = camera(json!({"connection": "ssh", "ssh_username": "u", "ssh_password": "p"}));
        let mut props = vec![build::string_prop(
            0xD278,
            "http://localhost:8080/liveview/stream",
        )];
        props.push(build::enum_prop(0xD221, dt::UINT8, false, 1, 1, &[]));
        connected_with(
            &mut m,
            Sim {
                props: build::prop_array(&props),
                ..Sim::default()
            },
        );
        let a = run(&mut m, 8, "get_live_view_image", json!({}));
        let tunnel = a
            .iter()
            .find_map(|x| match x {
                Action::TcpOpenSsh { socket, tunnel } if *socket == HTTP => Some(tunnel.clone()),
                _ => None,
            })
            .expect("an HTTP stream through the tunnel");
        assert_eq!(
            (tunnel.target_host.as_str(), tunnel.target_port),
            ("localhost", 8080)
        );
        let mut cx = Cx::new(20);
        m.tcp(&mut cx, HTTP, TcpInput::Connected);
        let a = cx.take();
        let request = a
            .iter()
            .find_map(|x| match x {
                Action::TcpSend { socket, data } if *socket == HTTP => {
                    Some(String::from_utf8(data.clone()).unwrap())
                }
                _ => None,
            })
            .unwrap();
        assert!(request.starts_with("GET /liveview/stream HTTP/1.1\r\n"));
        let jpeg = content::build::jpeg(640, 360);
        let dataset = content::build::live_view(&jpeg, true);
        let mut response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
        response.extend_from_slice(format!("{:x}\r\n", dataset.len()).as_bytes());
        response.extend_from_slice(&dataset);
        response.extend_from_slice(b"\r\n");
        let mut cx = Cx::new(30);
        m.tcp(&mut cx, HTTP, TcpInput::Data(response));
        let a = cx.take();
        assert!(
            matches!(completion(&a, 8), Some(Ok(Outcome::Value { value })) if value["width"] == 640)
        );
        assert!(a.contains(&Action::TcpClose { socket: HTTP }));
    }

    // ----- content -----

    #[test]
    fn remote_with_transfer_lists_and_downloads_content() {
        let mut m = camera(json!({"session_mode": "auto"}));
        let all = connected(&mut m, 300);
        assert_eq!(ops_sent(&all)[0], (OP_SDIO_OPEN_SESSION, vec![1, 2]));
        assert_eq!(
            state(&all)["session"]["function_mode"],
            "remote_with_transfer"
        );

        let a = run(&mut m, 9, "list_content", json!({"slot": 1, "limit": 50}));
        assert_eq!(last_request(&a).2, vec![0, 0, 50, 1, 0]);
        let list = content::build::content_info_list(1, 42, "DCIM/100MSDCF/DSC00001.JPG", 10);
        let a = answer(&mut m, &a, list, vec![]);
        let Some(Ok(Outcome::Value { value })) = completion(&a, 9) else {
            panic!("no listing")
        };
        assert_eq!(value["items"][0]["files"][0]["id"], "c:1:42:1");

        let a = run(
            &mut m,
            10,
            "download_content",
            json!({"id": "c:1:42:1", "path": "/tmp/x.jpg"}),
        );
        assert!(a.contains(&Action::FileOpen {
            file: FILE,
            path: "/tmp/x.jpg".into()
        }));
        assert_eq!(
            last_request(&a),
            (OP_GET_CONTENT_DATA, 8, vec![42, (1 << 24) | 1, 0, 0, CHUNK])
        );
        let a = answer(&mut m, &a, b"0123456789".to_vec(), vec![]);
        assert!(a.contains(&Action::FileWrite {
            file: FILE,
            data: b"0123456789".to_vec()
        }));
        assert!(a.contains(&Action::FileClose { file: FILE }));
        let mut cx = Cx::new(40);
        m.file(&mut cx, FILE, FileInput::Closed { bytes: 10 });
        assert_eq!(
            completion(&cx.take(), 10),
            Some(Ok(Outcome::Value {
                value: json!({"path": "/tmp/x.jpg", "bytes": 10})
            }))
        );
    }

    #[test]
    fn content_transfer_mode_turns_transfer_on_and_refuses_control() {
        let mut m = camera(json!({"session_mode": "content_transfer"}));
        let all = connected(&mut m, 300);
        let ops = ops_sent(&all);
        assert_eq!(ops[0], (OP_SDIO_OPEN_SESSION, vec![1, 1]));
        assert!(ops.contains(&(OP_SET_CONTENTS_TRANSFER_MODE, vec![2, 1, 0])));
        let a = run(
            &mut m,
            11,
            "set_white_balance",
            json!({"value": "daylight"}),
        );
        assert!(
            matches!(completion(&a, 11), Some(Err(CommandError::DeviceError { code: Some(c), .. })) if c == "session_mode")
        );

        // Listing: the handles on the card, then each one's ObjectInfo.
        let a = run(&mut m, 12, "list_content", json!({"slot": 2}));
        assert_eq!(
            last_request(&a),
            (OP_GET_OBJECT_HANDLES, 8, vec![0x0002_0001, 0, 0])
        );
        let a = answer(&mut m, &a, vec![1, 0, 0, 0, 0x33, 0, 0, 0], vec![]);
        assert_eq!(last_request(&a).2, vec![0x33]);
        let mut info = Vec::new();
        info.extend_from_slice(&0x0002_0001u32.to_le_bytes());
        info.extend_from_slice(&0x3801u16.to_le_bytes());
        info.extend_from_slice(&0u16.to_le_bytes());
        info.extend_from_slice(&3u32.to_le_bytes());
        info.extend_from_slice(&0u16.to_le_bytes());
        for v in [0u32; 7] {
            info.extend_from_slice(&v.to_le_bytes());
        }
        info.extend_from_slice(&0u16.to_le_bytes());
        info.extend_from_slice(&[0; 8]);
        ds::write_string(&mut info, "DSC00051.JPG");
        let a = answer(&mut m, &a, info.clone(), vec![]);
        let Some(Ok(Outcome::Value { value })) = completion(&a, 12) else {
            panic!("no listing")
        };
        assert_eq!(value["items"][0]["id"], "o:51");
        assert_eq!(value["items"][0]["name"], "DSC00051.JPG");

        // Download: ObjectInfo for the size, then partial reads.
        let a = run(
            &mut m,
            13,
            "download_content",
            json!({"id": "o:51", "path": "out.jpg"}),
        );
        assert_eq!(last_request(&a).0, OP_GET_OBJECT_INFO);
        let a = answer(&mut m, &a, info, vec![]);
        assert!(a.contains(&Action::FileOpen {
            file: FILE,
            path: "out.jpg".into()
        }));
        assert_eq!(last_request(&a).2, vec![51, 0, 0, CHUNK]);
        let a = answer(&mut m, &a, b"abc".to_vec(), vec![3]);
        assert!(a.contains(&Action::FileClose { file: FILE }));
        let mut cx = Cx::new(50);
        m.file(&mut cx, FILE, FileInput::Closed { bytes: 3 });
        assert!(
            matches!(completion(&cx.take(), 13), Some(Ok(Outcome::Value { value })) if value["name"] == "DSC00051.JPG")
        );

        // Back to remote control: CloseSession, then a new connection.
        let a = run(&mut m, 14, "switch_session_mode", json!({"mode": "remote"}));
        assert_eq!(last_request(&a).0, OP_SET_CONTENTS_TRANSFER_MODE);
        let a = answer(&mut m, &a, vec![], vec![]);
        assert_eq!(last_request(&a).0, OP_CLOSE_SESSION);
        let a = answer(&mut m, &a, vec![], vec![]);
        assert_eq!(completion(&a, 14), Some(Ok(Outcome::Ack)));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::TcpOpen { socket: CMD, .. })));
        assert_eq!(m.function, Function::Remote);
    }

    #[test]
    fn a_failed_file_ends_the_download() {
        let mut m = camera(json!({}));
        connected(&mut m, 300);
        let a = run(
            &mut m,
            15,
            "download_captured_image",
            json!({"path": "shot.jpg"}),
        );
        assert_eq!(
            last_request(&a),
            (OP_GET_OBJECT_INFO, 7, vec![CAPTURED_HANDLE])
        );
        let mut info = vec![0u8; 52];
        info.push(0);
        info.extend_from_slice(&[0, 0, 0]);
        let a = answer(&mut m, &a, info, vec![]);
        assert_eq!(last_request(&a), (OP_GET_OBJECT, 8, vec![CAPTURED_HANDLE]));
        let mut cx = Cx::new(60);
        m.file(
            &mut cx,
            FILE,
            FileInput::Failed {
                message: "disk full".into(),
            },
        );
        assert!(matches!(
            completion(&cx.take(), 15),
            Some(Err(CommandError::DeviceError { .. }))
        ));
        // The camera's data still arrives and is dropped without a second completion.
        let a = answer(&mut m, &a, vec![1, 2, 3], vec![]);
        assert!(completion(&a, 15).is_none());
    }

    #[test]
    fn a_session_mode_the_camera_refuses_falls_back_to_remote() {
        let mut m = camera(json!({"session_mode": "remote_with_transfer"}));
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
        let a = feed(
            &mut m,
            CMD,
            &[Packet::OperationResponse {
                code: 0x2006,
                transaction: 0,
                params: vec![],
            }],
        );
        assert_eq!(last_request(&a), (OP_OPEN_SESSION, 0, vec![1]));
        assert_eq!(m.function, Function::Remote);
    }

    #[test]
    fn video_only_clips_come_from_the_media_profile() {
        let mut m = camera_model(
            "ilme-fx6",
            json!({"connection": "ssh", "ssh_username": "u", "ssh_password": "p"}),
        );
        connected_with(
            &mut m,
            Sim {
                props: build::prop_array(&[build::string_prop(
                    0xD031,
                    "http://localhost:8080/A/MEDIAPRO.XML",
                )]),
                ..Sim::default()
            },
        );
        run(&mut m, 16, "list_content", json!({"slot": 1}));
        let mut cx = Cx::new(20);
        m.tcp(&mut cx, HTTP, TcpInput::Connected);
        cx.take();
        let xml = r#"<MediaProfile><Contents><Material uri="./Clip/C0001.MXF" type="MXF" dur="10"/></Contents></MediaProfile>"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{xml}",
            xml.len()
        );
        let mut cx = Cx::new(30);
        m.tcp(&mut cx, HTTP, TcpInput::Data(response.into_bytes()));
        let a = cx.take();
        assert!(
            matches!(completion(&a, 16), Some(Ok(Outcome::Value { value })) if value["items"][0]["id"] == "m:1:./Clip/C0001.MXF")
        );

        // The clip itself, streamed to the file.
        let a = run(
            &mut m,
            17,
            "download_content",
            json!({"id": "m:1:./Clip/C0001.MXF", "path": "c.mxf"}),
        );
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::TcpOpenSsh { socket: HTTP, .. })));
        let mut cx = Cx::new(40);
        m.tcp(&mut cx, HTTP, TcpInput::Connected);
        let request = String::from_utf8(match &cx.take()[0] {
            Action::TcpSend { data, .. } => data.clone(),
            other => panic!("{other:?}"),
        })
        .unwrap();
        assert!(request.starts_with("GET /A/Clip/C0001.MXF HTTP/1.1"));
        let mut cx = Cx::new(50);
        m.tcp(
            &mut cx,
            HTTP,
            TcpInput::Data(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nclip".to_vec()),
        );
        let a = cx.take();
        assert!(a.contains(&Action::FileOpen {
            file: FILE,
            path: "c.mxf".into()
        }));
        assert!(a.contains(&Action::FileWrite {
            file: FILE,
            data: b"clip".to_vec()
        }));
        assert!(a.contains(&Action::FileClose { file: FILE }));
        let mut cx = Cx::new(60);
        m.file(&mut cx, FILE, FileInput::Closed { bytes: 4 });
        assert!(
            matches!(completion(&cx.take(), 17), Some(Ok(Outcome::Value { value })) if value["name"] == "C0001.MXF")
        );
    }

    // ----- FTP -----

    #[test]
    fn ftp_settings_round_trip_without_echoing_passwords() {
        let mut m = camera(json!({}));
        connected(&mut m, 300);
        let a = run(&mut m, 20, "get_ftp_settings", json!({}));
        assert_eq!(last_request(&a).0, OP_GET_FTP_SETTING_LIST);
        let a = answer(
            &mut m,
            &a,
            ftp::build::setting_list(101, &[(1, "News", "ftp.example")]),
            vec![],
        );
        let s = state(&a);
        assert_eq!(s["ftp"]["servers"]["1"]["host"], "ftp.example");
        assert_eq!(s["ftp"]["servers"]["1"]["password_set"], true);
        assert!(
            matches!(completion(&a, 20), Some(Ok(Outcome::Value { value })) if value["version"] == 101)
        );

        let a = run(
            &mut m,
            21,
            "set_ftp_server",
            json!({"server_id": 1, "host": "10.1.1.1", "username": "cam", "password": "hunter2", "secure": "ftps"}),
        );
        let packets = sent(&a, CMD);
        let Packet::EndData { payload, .. } = &packets[2] else {
            panic!("no data")
        };
        let (_, servers) = ftp::parse_setting_list(payload).unwrap();
        assert_eq!(
            (servers[0].host.as_str(), servers[0].secure),
            ("10.1.1.1", 2)
        );
        let a = answer(&mut m, &a, vec![], vec![]);
        assert_eq!(completion(&a, 21), Some(Ok(Outcome::Ack)));
        // The list is read back into the state; the password never appears.
        assert!(!format!("{a:?}").contains("hunter2\""));
    }

    #[test]
    fn ftp_jobs_follow_the_sync_id() {
        let mut m = camera_model("ilme-fx6", json!({}));
        let props = build::prop_array(&[build::range_prop(
            0xD02A,
            dt::UINT32,
            false,
            4,
            (0, 0xFFFF, 1),
        )]);
        let all = connected_with(
            &mut m,
            Sim {
                props,
                operations: vec![0x1001, 0x1002, OP_GET_FTP_JOB_LIST, OP_CONTROL_FTP_JOB_LIST],
                ..Sim::default()
            },
        );
        // A new sync id: the job list is read.
        assert_eq!(last_request(&all).0, OP_GET_FTP_JOB_LIST);
        let a = answer(
            &mut m,
            &all,
            ftp::build::job_list(101, 4, &[(5, 0x200, "C0001.MXF")]),
            vec![],
        );
        assert_eq!(state(&a)["ftp"]["jobs"]["5"]["status"], "transferring");

        let a = run(
            &mut m,
            22,
            "add_ftp_job",
            json!({"server_id": 1, "slot": 1, "clip_path": "/Clip/C0002.MXF"}),
        );
        assert_eq!(last_request(&a).2, vec![1, 0]);
        let a = answer(&mut m, &a, vec![], vec![]);
        assert_eq!(completion(&a, 22), Some(Ok(Outcome::Ack)));
        run(&mut m, 23, "ftp_job", json!({"action": "delete_finished"}));
        // Queued behind the job list read the add triggered.
        assert!(m
            .commands
            .iter()
            .any(|op| op.code == OP_CONTROL_FTP_JOB_LIST && op.params == vec![2, 3]));
    }

    // ----- uploads, setting files and deletion -----

    fn file_read_of(actions: &[Action]) -> Option<(String, u64)> {
        actions.iter().find_map(|a| match a {
            Action::FileRead {
                file,
                path,
                max_bytes,
            } if *file == files::READ => Some((path.clone(), *max_bytes)),
            _ => None,
        })
    }

    fn read_done(m: &mut SonyCamera, data: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(100);
        m.file(
            &mut cx,
            files::READ,
            FileInput::Read {
                data: data.to_vec(),
            },
        );
        cx.take()
    }

    fn event_in(m: &mut SonyCamera, code: u16, params: Vec<u32>) -> Vec<Action> {
        feed(
            m,
            EVT,
            &[Packet::Event {
                code,
                transaction: 0,
                params,
            }],
        )
    }

    fn lut_camera(import_enabled: u8) -> SonyCamera {
        let mut m = camera(json!({}));
        let mut props = vec![
            build::enum_prop(0xD08B, dt::UINT8, false, 1, import_enabled as i128, &[]),
            // BaseLookImport Command Version: display-only while importable.
            build::enum_prop(0xD059, dt::UINT32, false, 2, 100, &[]),
            build::enum_prop(0xD057, dt::UINT32, false, 1, 100, &[]),
        ];
        props.push(build::range_prop(0xD0C1, dt::UINT32, false, 0, (0, 64, 1)));
        connected_with(
            &mut m,
            Sim {
                props: build::prop_array(&props),
                ..Sim::default()
            },
        );
        m
    }

    #[test]
    fn a_lut_is_uploaded_applied_and_reported() {
        let mut m = lut_camera(1);
        let a = run(
            &mut m,
            30,
            "import_lut",
            json!({"path": "/luts/Show.cube", "user_base_look": 3}),
        );
        assert_eq!(file_read_of(&a).unwrap().0, "/luts/Show.cube");
        let cube = b"LUT_3D_SIZE 2\n0 0 0\n";
        let a = read_done(&mut m, cube);
        let packets = sent(&a, CMD);
        assert!(
            matches!(&packets[0], Packet::OperationRequest { code: 0x921A, params, .. } if params == &vec![0x0002_0001])
        );
        let Packet::EndData { payload, .. } = &packets[2] else {
            panic!()
        };
        assert_eq!(payload, &files::upload_dataset(100, "Show.cube", cube));
        let a = answer(&mut m, &a, vec![], vec![]);
        let packets = sent(&a, CMD);
        assert!(
            matches!(&packets[0], Packet::OperationRequest { code: 0x921B, params, .. } if params == &vec![0x0002_0000])
        );
        let Packet::EndData { payload, .. } = &packets[2] else {
            panic!()
        };
        assert_eq!(payload, &[100, 0, 0, 0, 3, 0]);
        // The result event comes before the answer to the last step.
        event_in(&mut m, 0xC214, vec![1, 0x0002_0000]);
        let a = answer(&mut m, &a, vec![], vec![]);
        assert!(
            matches!(completion(&a, 30), Some(Ok(Outcome::Value { value })) if value["result"] == "ok")
        );
    }

    #[test]
    fn uploads_are_refused_when_the_camera_says_so_or_the_file_is_wrong() {
        let mut m = lut_camera(0);
        let a = run(
            &mut m,
            31,
            "import_lut",
            json!({"path": "x.cube", "user_base_look": 1}),
        );
        assert!(
            matches!(completion(&a, 31), Some(Err(CommandError::DeviceError { code: Some(c), .. })) if c == "not_available")
        );
        assert!(file_read_of(&a).is_none(), "nothing is read");

        let mut m = lut_camera(1);
        run(
            &mut m,
            32,
            "import_lut",
            json!({"path": "x.txt", "user_base_look": 1}),
        );
        let a = read_done(&mut m, b"LUT_3D_SIZE 2");
        assert!(matches!(
            completion(&a, 32),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        run(
            &mut m,
            33,
            "import_lut",
            json!({"path": "x.cube", "user_base_look": 1}),
        );
        let a = read_done(&mut m, &[b'x'; 80]);
        assert!(matches!(
            completion(&a, 33),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        // A camera result other than OK fails the command.
        run(
            &mut m,
            34,
            "import_lut",
            json!({"path": "x.cube", "user_base_look": 1}),
        );
        let a = read_done(&mut m, b"LUT_3D_SIZE 2");
        let a = answer(&mut m, &a, vec![], vec![]);
        let a = answer(&mut m, &a, vec![], vec![]);
        assert!(completion(&a, 34).is_none());
        let a = event_in(&mut m, 0xC214, vec![3, 0x0002_0000]);
        assert!(
            matches!(completion(&a, 34), Some(Err(CommandError::DeviceError { message, .. })) if message.contains("invalid_file_name"))
        );
    }

    #[test]
    fn a_grid_line_file_goes_in_parts() {
        let mut m = camera(json!({}));
        let props = vec![
            build::enum_prop(0xE110, dt::UINT32, false, 1, 100, &[]),
            // The camera's largest operation: 12 header bytes and 8 of data.
            build::range_prop(0xD0C1, dt::UINT32, false, 0, (0, 20, 1)),
        ];
        connected_with(
            &mut m,
            Sim {
                props: build::prop_array(&props),
                ..Sim::default()
            },
        );
        run(
            &mut m,
            35,
            "import_grid_line_file",
            json!({"path": "grid.png", "custom": 2}),
        );
        let mut a = read_done(&mut m, b"0123456789abcdefghij");
        let mut parts = Vec::new();
        loop {
            let (code, _, params) = last_request(&a);
            if code != 0x9229 {
                assert_eq!((code, params), (0x921B, vec![0x0009_0000]));
                break;
            }
            parts.push(params);
            a = answer(&mut m, &a, vec![], vec![]);
        }
        assert_eq!(
            parts,
            [
                vec![0x0009_0001, 0, 0, 8, 0],
                vec![0x0009_0001, 8, 0, 8, 0],
                vec![0x0009_0001, 16, 0, 4, 1]
            ]
        );
        let a = answer(&mut m, &a, vec![], vec![]);
        let a2 = event_in(&mut m, 0xC214, vec![1, 0x0009_0000]);
        assert!(completion(&a, 35).is_none());
        assert!(matches!(completion(&a2, 35), Some(Ok(_))));
    }

    #[test]
    fn camera_settings_export_and_import() {
        let mut m = camera(json!({}));
        let props = vec![
            build::enum_prop(0xD271, dt::UINT8, false, 1, 1, &[]),
            build::enum_prop(0xD272, dt::UINT8, false, 1, 0, &[]),
        ];
        connected_with(
            &mut m,
            Sim {
                props: build::prop_array(&props),
                ..Sim::default()
            },
        );
        let a = run(
            &mut m,
            36,
            "export_camera_settings",
            json!({"path": "cam.dat"}),
        );
        assert_eq!(last_request(&a).2, vec![files::CAMERA_SETTING_HANDLE]);
        let mut info = vec![0u8; 52];
        info.extend_from_slice(&[0, 0]);
        let a = answer(&mut m, &a, info, vec![]);
        assert_eq!(
            last_request(&a),
            (OP_GET_OBJECT, 8, vec![files::CAMERA_SETTING_HANDLE])
        );
        let a = answer(&mut m, &a, b"SETTINGS".to_vec(), vec![]);
        assert!(a.contains(&Action::FileWrite {
            file: FILE,
            data: b"SETTINGS".to_vec()
        }));
        assert!(a.contains(&Action::FileClose { file: FILE }));
        // Reading a setting file into the camera is off right now.
        let a = run(
            &mut m,
            37,
            "import_camera_settings",
            json!({"path": "cam.dat"}),
        );
        assert!(matches!(
            completion(&a, 37),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn content_deletion_needs_an_id_and_waits_for_the_result() {
        let mut m = camera(json!({"session_mode": "auto"}));
        let props = vec![build::enum_prop(0xE0F3, dt::UINT8, false, 1, 1, &[])];
        connected_with(
            &mut m,
            Sim {
                props: build::prop_array(&props),
                ..Sim::default()
            },
        );
        for bad in [
            json!({}),
            json!({"id": ""}),
            json!({"id": "o:5"}),
            json!({"id": "c:1"}),
        ] {
            let a = run(&mut m, 38, "delete_content", bad);
            assert!(matches!(
                completion(&a, 38),
                Some(Err(CommandError::InvalidParams { .. }))
            ));
        }
        let a = run(&mut m, 39, "delete_content", json!({"id": "c:1:42:1"}));
        assert_eq!(last_request(&a).2, vec![42, 1]);
        let a = answer(&mut m, &a, vec![], vec![]);
        assert!(a.contains(&Action::SetTimer {
            key: files::DELETE_WAIT,
            after: 30_000
        }));
        // A result for another content is not this one's.
        assert!(completion(&event_in(&mut m, 0xC240, vec![1, 7, 1]), 39).is_none());
        let a = event_in(&mut m, 0xC240, vec![4, 42, 1]);
        assert!(
            matches!(completion(&a, 39), Some(Err(CommandError::DeviceError { message, .. })) if message.contains("protected"))
        );
        // The next deletion waits out the spacing the reference asks for.
        let mut cx = Cx::new(150);
        m.command(
            &mut cx,
            40,
            "delete_content",
            &settings(json!({"id": "c:1:43"})),
        );
        cx.take();
        // Queued behind the property read the result event triggered.
        assert!(matches!(
            m.commands.front().map(|o| &o.step),
            Some(Step::Pause(450))
        ));
    }

    #[test]
    fn a_general_setting_file_returns_the_camera_result_file() {
        let mut m = camera(json!({}));
        let props = vec![build::enum_prop(0xE081, dt::UINT8, false, 1, 1, &[])];
        connected_with(
            &mut m,
            Sim {
                props: build::prop_array(&props),
                ..Sim::default()
            },
        );
        run(
            &mut m,
            41,
            "apply_settings_file",
            json!({"path": "s.xml", "check_only": true}),
        );
        let a = read_done(&mut m, b"<?xml version=\"1.0\"?><CameraSetting/>");
        assert_eq!(last_request(&a).2, vec![1]);
        let a = answer(&mut m, &a, vec![], vec![]);
        let a = [a, event_in(&mut m, 0xC21A, vec![1, 1])].concat();
        assert_eq!(last_request(&a), (0x9222, 8, vec![1]));
        let mut result = 8u32.to_le_bytes().to_vec();
        result.extend_from_slice(&5u32.to_le_bytes());
        result.extend_from_slice(b"<ok/>");
        let a = answer(&mut m, &a, result, vec![]);
        assert!(
            matches!(completion(&a, 41), Some(Ok(Outcome::Value { value })) if value["result_file"] == "<ok/>")
        );
    }

    /// A camera that offers the information operations, with the given
    /// properties.
    fn info_camera(props: Vec<Vec<u8>>, controls: Vec<u16>) -> (SonyCamera, Vec<Action>) {
        let mut m = camera(json!({}));
        let all = connected_with(
            &mut m,
            Sim {
                props: build::prop_array(&props),
                controls,
                operations: vec![
                    0x1001, 0x1002, 0x9215, 0x922F, 0x924D, 0x9231, 0x9248, 0x9223, 0x9239, 0x9238,
                    0x9241, 0x9242, 0x922A, 0x9249,
                ],
                ..Sim::default()
            },
        );
        (m, all)
    }

    #[test]
    fn camera_information_is_read_into_the_state_once_connected() {
        let (mut m, all) = info_camera(
            vec![
                build::enum_prop(0xD03C, dt::UINT16, true, 1, 0x0101, &[1, 0x0101]),
                build::enum_prop(0xE086, dt::UINT8, false, 1, 0, &[]),
            ],
            vec![],
        );
        // Display lists first, all types.
        assert_eq!(last_request(&all).0, 0x9215);
        assert_eq!(last_request(&all).2, vec![0]);
        let lists = info::build::display_lists(&[(3, &[(0x0101, "My Look"), (1, "s709")])]);
        let a = answer(&mut m, &all, lists, vec![7]);
        let s = state(&a);
        assert_eq!(s["display_lists"]["base_look_name"]["257"], "My Look");
        assert_eq!(s["image"]["base_look_name"], "My Look");
        assert_eq!(s["session"]["display_list_version"], 7);
        // Then the rest the camera offers; lens information waits for its
        // enable status.
        let mut a = a;
        let mut asked = vec![];
        for _ in 0..5 {
            let (code, _, _) = last_request(&a);
            asked.push(code);
            a = feed(
                &mut m,
                CMD,
                &[Packet::OperationResponse {
                    code: 0x2005,
                    transaction: last_request(&a).1,
                    params: vec![],
                }],
            );
            if sent(&a, CMD).is_empty() {
                break;
            }
        }
        assert_eq!(asked, vec![0x922F, 0x924D, 0x9231, 0x9248]);
        // The lens becomes readable: its table is read.
        let mut props = build::prop_array(&[build::enum_prop(0xE086, dt::UINT8, false, 1, 1, &[])]);
        let mut cx = Cx::new(0);
        m.apply(&mut cx, parse_prop_info_array(&props).unwrap());
        m.pump(&mut cx);
        let a = cx.take();
        assert_eq!(last_request(&a).0, 0x9223);
        assert_eq!(last_request(&a).2, vec![2]);
        props.clear();
    }

    #[test]
    fn events_become_state_and_list_changes_are_read_again() {
        let (mut m, _) = info_camera(vec![], vec![]);
        m.background.clear();
        m.current = None;
        let a = event_in(&mut m, 0xC206, vec![]);
        assert_eq!(state(&a)["still"]["captures"], 1);
        let a = event_in(&mut m, 0x4004, vec![0x0001_0001]);
        assert_eq!(state(&a)["content"]["storages"]["00010001"], true);
        let a = event_in(&mut m, 0x4005, vec![0x0001_0001]);
        assert_eq!(state(&a)["content"]["storages"]["00010001"], false);
        let a = event_in(&mut m, 0xC202, vec![0xFFFF_C001]);
        assert_eq!(state(&a)["still"]["last_object_removed"], "FFFFC001");
        let a = event_in(&mut m, 0xC228, vec![]);
        assert_eq!(state(&a)["status"]["cautions"], 1);
        let a = event_in(&mut m, 0xC205, vec![1]);
        assert_eq!(state(&a)["clock"]["last_result"], "ok");
        // A display list change reads that list type again.
        m.background.clear();
        m.current = None;
        let a = event_in(&mut m, 0xC20F, vec![5]);
        assert_eq!(ops_sent(&a)[0], (0x9215, vec![5]));
    }

    #[test]
    fn camera_buttons_dials_and_their_capabilities() {
        let (mut m, all) = info_camera(
            vec![
                build::enum_prop(0xD208, dt::UINT16, false, 1, 0, &[0x01, 0x1A]),
                build::enum_prop(0xD20A, dt::UINT16, false, 1, 0, &[0x4002]),
                build::enum_prop(0xD20C, dt::UINT16, false, 1, 1, &[]),
            ],
            vec![0xD309, 0xD30A, 0xD30B],
        );
        let s = state(&all);
        assert_eq!(s["camera_controls"]["buttons"], json!(["up", "movie"]));
        assert_eq!(s["camera_controls"]["dials"], json!(["front_dial"]));
        assert_eq!(s["camera_controls"]["status"], "idle");
        let plan = m.plan("camera_button", &settings(json!({"button": "movie"})), 1);
        let Ok(Plan::Ops(ops)) = plan else { panic!() };
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].params[0], 0xD309);
        assert_eq!(
            ops[0].data,
            Some(((0x1Au32 << 16) | 2).to_le_bytes().to_vec())
        );
        assert_eq!(
            ops[2].data,
            Some(((0x1Au32 << 16) | 1).to_le_bytes().to_vec())
        );
        let Ok(Plan::Ops(ops)) = m.plan(
            "camera_dial",
            &settings(json!({"dial": "front_dial", "steps": -2})),
            2,
        ) else {
            panic!()
        };
        assert_eq!(
            ops[0].data,
            Some(((0x4002u32 << 16) | 0xFFFE).to_le_bytes().to_vec())
        );
        // A key held elsewhere blocks a plain press.
        m.props.get_mut(&0xD20C).unwrap().current = PtpValue::Int(2);
        assert!(m
            .plan("camera_button", &settings(json!({"button": "up"})), 3)
            .is_err());
        assert!(m
            .plan(
                "camera_button",
                &settings(json!({"button": "up", "simultaneous": true})),
                3
            )
            .is_ok());
    }

    #[test]
    fn user_base_look_numbers_follow_what_the_camera_lists() {
        let (m, _) = info_camera(
            vec![
                build::enum_prop(0xD0C7, dt::UINT16, true, 1, 0, &[0, 0xFFFF, 0x0101, 0x0102]),
                build::enum_prop(0xD0C8, dt::UINT16, true, 1, 1, &[1, 2]),
            ],
            vec![],
        );
        let data = |name: &str, p: Value| match m.plan(name, &settings(p), 1) {
            Ok(Plan::Ops(ops)) => ops[0].data.clone().unwrap(),
            Ok(_) => panic!("not ops"),
            Err(e) => panic!("{e}"),
        };
        assert_eq!(
            data("delete_user_base_look", json!({"user": 2})),
            vec![0x02, 0x01]
        );
        assert_eq!(
            data("delete_user_base_look", json!({"all": true})),
            vec![0xFF, 0xFF]
        );
        assert_eq!(
            data("select_user_base_look", json!({"user": 2})),
            vec![0x02, 0x00]
        );
        assert!(m
            .plan("delete_user_base_look", &settings(json!({"user": 17})), 1)
            .is_err());
    }

    #[test]
    fn eframing_needs_its_command_version() {
        let (m, _) = info_camera(vec![], vec![]);
        assert!(m
            .plan("execute_eframing", &settings(json!({"type": "auto"})), 1)
            .is_err());
        let (m, _) = info_camera(
            vec![build::enum_prop(0xD123, dt::UINT32, false, 1, 100, &[])],
            vec![],
        );
        let p =
            settings(json!({"type": "single", "x": 0.25, "y": 0.25, "width": 0.5, "height": 0.5}));
        let Ok(Plan::Ops(ops)) = m.plan("execute_eframing", &p, 1) else {
            panic!()
        };
        assert_eq!(ops[0].code, 0x922A);
        assert_eq!(
            ops[0].data,
            Some(info::encode_eframing(
                100,
                3,
                false,
                Some([0.25, 0.25, 0.5, 0.5])
            ))
        );
        // Single and PTZ need the rectangle.
        assert!(m
            .plan("execute_eframing", &settings(json!({"type": "ptz"})), 1)
            .is_err());
    }

    #[test]
    fn stream_settings_report_the_camera_check() {
        let (mut m, _) = info_camera(
            vec![build::enum_prop(0xD1CC, dt::UINT8, false, 1, 1, &[])],
            vec![],
        );
        m.background.clear();
        m.current = None;
        let a = run(
            &mut m,
            5,
            "set_stream_settings",
            json!({"stream": 1, "protocol": "rtmp", "url": "rtmp://x/live", "key": "k"}),
        );
        assert_eq!(ops_sent(&a)[0], (0x9242, vec![1]));
        let (_, t, _) = last_request(&a);
        let a = feed(&mut m, CMD, &[ok(t, vec![4])]);
        assert!(matches!(
            completion(&a, 5),
            Some(Err(CommandError::DeviceError { message, .. })) if message.contains("stream key")
        ));
        let a = run(
            &mut m,
            6,
            "set_stream_settings",
            json!({"stream": 1, "protocol": "srt", "url": "srt://x"}),
        );
        let (_, t, _) = last_request(&a);
        let a = feed(&mut m, CMD, &[ok(t, vec![0])]);
        assert_eq!(completion(&a, 6), Some(Ok(Outcome::Ack)));
        // The list is read again after a successful write.
        assert_eq!(last_request(&a).0, 0x9241);
    }

    #[test]
    fn the_osd_image_needs_its_mode_on() {
        let (mut m, _) = info_camera(
            vec![build::enum_prop(0xD207, dt::UINT8, true, 1, 0, &[0, 1])],
            vec![],
        );
        assert!(m.plan("get_osd_image", &Params::new(), 1).is_err());
        m.props.get_mut(&0xD207).unwrap().current = PtpValue::Int(1);
        m.background.clear();
        m.current = None;
        let a = run(&mut m, 7, "get_osd_image", json!({}));
        assert_eq!(last_request(&a).0, 0x9238);
        let mut png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&360u32.to_be_bytes());
        let a = answer(&mut m, &a, info::build::osd(&png), vec![]);
        let Some(Ok(Outcome::Value { value })) = completion(&a, 7) else {
            panic!()
        };
        assert_eq!(
            (value["format"].as_str(), value["width"].as_u64()),
            (Some("png"), Some(640))
        );
        assert_eq!(value["meta"]["yuv_layout"]["x_max"], 1920);
    }

    #[test]
    fn content_can_be_chosen_on_the_camera() {
        let mut m =
            camera(json!({"session_mode": "content_transfer", "content_selection": "camera"}));
        let all = connected(&mut m, 300);
        assert!(ops_sent(&all).contains(&(OP_SET_CONTENTS_TRANSFER_MODE, vec![1, 1, 0])));
    }

    #[test]
    fn a_download_resumes_from_the_part_on_the_host_by_umid() {
        let mut m = camera(json!({"session_mode": "remote_with_transfer"}));
        connected(&mut m, 300);
        let a = run(&mut m, 9, "list_content", json!({"slot": 1}));
        let list = content::build::content_info_list(1, 42, "DCIM/100MSDCF/DSC00001.JPG", 10);
        let a = answer(&mut m, &a, list, vec![]);
        let Some(Ok(Outcome::Value { value })) = completion(&a, 9) else {
            panic!()
        };
        let umid = value["items"][0]["files"][0]["umid"]
            .as_str()
            .unwrap()
            .to_string();
        let a = run(
            &mut m,
            10,
            "download_content",
            json!({"umid": umid, "path": "/tmp/x.jpg", "resume": true}),
        );
        assert!(a.contains(&Action::FileRead {
            file: RESUME,
            path: "/tmp/x.jpg".into(),
            max_bytes: 10
        }));
        let mut cx = Cx::new(20);
        m.file(
            &mut cx,
            RESUME,
            FileInput::Read {
                data: b"0123".to_vec(),
            },
        );
        let a = cx.take();
        assert!(a.contains(&Action::FileOpen {
            file: FILE,
            path: "/tmp/x.jpg".into()
        }));
        assert!(a.contains(&Action::FileWrite {
            file: FILE,
            data: b"0123".to_vec()
        }));
        assert_eq!(last_request(&a).2, vec![42, (1 << 24) | 1, 4, 0, CHUNK]);
        let a = answer(&mut m, &a, b"456789".to_vec(), vec![]);
        assert!(a.contains(&Action::FileClose { file: FILE }));
        // A list the camera has since regenerated is not trusted.
        let regenerated = build::prop_array(&[build::range_prop(
            0xD1D6,
            dt::UINT64,
            false,
            i64::MAX as i128,
            (0, i64::MAX as i128, 1),
        )]);
        let mut cx = Cx::new(0);
        m.download = None;
        m.apply(&mut cx, parse_prop_info_array(&regenerated).unwrap());
        assert!(matches!(
            m.plan("download_content", &settings(json!({"umid": umid, "path": "/tmp/x.jpg"})), 11),
            Err(CommandError::DeviceError { code: Some(c), .. }) if c == "list_changed"
        ));
    }
}
