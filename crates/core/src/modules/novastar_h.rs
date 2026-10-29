//! NovaStar H series video wall splicers over the H Series OpenAPI.
//!
//! Protocol from NovaStar's "H Series OpenAPI" documentation (English pages
//! on openapi.novastar.tech, read 2026-10-03 and 2026-10-13; section numbers
//! are its "Instructions" page's).
//!
//! - Every request is an HTTP POST of JSON to
//!   `http://<splicer>:8000/open/api/<endpoint>` (§2, §5), with the body
//!   `{"body": {...}, "sign": ..., "pId": ..., "timeStamp": ...}` (§4).
//!   Unencrypted, `sign` is Base64 of the hexadecimal MD5 of the timestamp
//!   followed by the pId, and the timestamp is the current time in
//!   milliseconds. The splicer refuses a wrong time (error 11), so the module
//!   needs the wall clock: it takes the time once, when the device is opened,
//!   and adds the session's own clock to it. That is the one place the module
//!   reads a clock; tests give it a fixed one.
//! - Replies are `{"status", "msg", "sign", "body"}`; status 0 is success
//!   (one documented example spells the key `"status "`, which is read too,
//!   and a few endpoints answer in `data` instead of `body`).
//!   Other statuses fail the command with that code.
//! - Commands come from [`ENDPOINTS`]: each copies its parameters into the
//!   body under the documented field names, adds `deviceId`, and merges a
//!   `body` parameter's fields where the endpoint takes nested structures.
//!   `call` sends any endpoint with a body given whole.
//! - The splicer does not push in any documented form, so the module polls:
//!   the initialisation status (which is also the liveness check), the
//!   screen list, inputs, outputs, background images and signal source
//!   playlists, and for each listed screen its details (brightness, gamma,
//!   FTB, freeze, background, size, 3D, OSD), layers, presets, current
//!   preset, preset groups and the preset group playing. A command that
//!   succeeds has what it changed read back at once.
//! - Lists (screens, layers, presets, inputs, crops ...) are kept whole: an
//!   item a later read no longer lists leaves state.
//!
//! Opened for commands only (`monitor` false), the module asks only the
//! initialisation status at the poll interval, as its liveness check, and
//! reads nothing else, not even after a command. Each answered request
//! reports its round trip.

use std::collections::{BTreeSet, HashMap};

use base64::Engine;
use md5::{Digest, Md5};
use serde_json::{json, Map, Value};

use super::novastar_h_commands::{Endpoint, ENDPOINTS};
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, HttpRequest, HttpResponse, Key, Level, Millis, Module,
    OpenContext, Outcome, RequestId,
};

pub(crate) const DEFAULT_PORT: u16 = 8000;
const POLL: Key = "poll";
const TIMEOUT: Millis = 5_000;

/// A state read: the endpoint asked and what its answer fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Read {
    InitStatus,
    DeviceDetail,
    Screens,
    ScreenDetail(i64),
    Layers(i64),
    Presets(i64),
    CurrentPreset(i64),
    PresetGroupPlaying(i64),
    PresetGroups(i64),
    Inputs,
    Outputs,
    Bkgs,
    SourcePlaylists,
}

impl Read {
    fn path(self) -> &'static str {
        match self {
            Read::InitStatus => "main/initStatus",
            Read::DeviceDetail => "device/readDetail",
            Read::Screens => "screen/readList",
            Read::ScreenDetail(_) => "screen/readDetail",
            Read::Layers(_) => "layer/detailList",
            Read::Presets(_) => "preset/readList",
            Read::CurrentPreset(_) => "preset/readPlay",
            Read::PresetGroupPlaying(_) => "preset/readPresetPoll",
            Read::PresetGroups(_) => "preset/groupList",
            Read::Inputs => "input/readList",
            Read::Outputs => "output/readList",
            Read::Bkgs => "bkg/readAllList",
            Read::SourcePlaylists => "input/readSourceList",
        }
    }

    fn body(self) -> Value {
        match self {
            Read::InitStatus => json!({}),
            Read::ScreenDetail(s)
            | Read::Layers(s)
            | Read::Presets(s)
            | Read::CurrentPreset(s)
            | Read::PresetGroupPlaying(s)
            | Read::PresetGroups(s) => json!({"deviceId": 0, "screenId": s}),
            _ => json!({"deviceId": 0}),
        }
    }
}

/// Everything read for one screen.
fn screen_reads(sid: i64) -> [Read; 6] {
    [
        Read::ScreenDetail(sid),
        Read::Layers(sid),
        Read::Presets(sid),
        Read::CurrentPreset(sid),
        Read::PresetGroupPlaying(sid),
        Read::PresetGroups(sid),
    ]
}

/// What a successful command changed, read back after it.
fn read_back(name: &str, params: &Params) -> Vec<Read> {
    let screen = params.get("screen_id").and_then(Value::as_i64);
    let on_screen = |f: fn(i64) -> Read| screen.map(f).into_iter().collect::<Vec<_>>();
    match name {
        "set_screen_brightness"
        | "save_screen_brightness"
        | "set_screen_gamma"
        | "screen_ftb"
        | "set_screen_freeze"
        | "set_screen_bkg"
        | "set_screen_layout"
        | "set_screen_3d"
        | "set_text_osd"
        | "set_image_osd" => on_screen(Read::ScreenDetail),
        "rename_screen" | "create_screen" | "delete_screens" | "clear_screens"
        | "open_layer_layout" => vec![Read::Screens],
        "create_layer"
        | "delete_layer"
        | "clear_layers"
        | "set_layer_window"
        | "set_layer_general"
        | "set_layer_zorder"
        | "lock_layer"
        | "set_layer_source"
        | "freeze_layer"
        | "set_layer_stream_rule" => on_screen(Read::Layers),
        "load_preset" => screen
            .map(|s| {
                vec![
                    Read::CurrentPreset(s),
                    Read::Layers(s),
                    Read::ScreenDetail(s),
                ]
            })
            .unwrap_or_default(),
        "create_preset" | "overwrite_preset" | "rename_preset" | "delete_preset"
        | "clear_presets" => screen
            .map(|s| vec![Read::Presets(s), Read::CurrentPreset(s)])
            .unwrap_or_default(),
        "create_preset_group" | "update_preset_group" | "delete_preset_group" => {
            on_screen(Read::PresetGroups)
        }
        "play_preset_group" => screen
            .map(|s| vec![Read::PresetGroupPlaying(s), Read::CurrentPreset(s)])
            .unwrap_or_default(),
        "rename_input"
        | "set_input_resolution"
        | "set_input_timing"
        | "create_input_crop"
        | "update_input_crop"
        | "delete_input_crop"
        | "import_edid" => vec![Read::Inputs],
        "rename_output"
        | "set_output_gamma"
        | "set_output_image_quality"
        | "set_output_resolution"
        | "set_output_timing"
        | "set_4k_output_timing"
        | "set_all_in_one_payload" => vec![Read::Outputs],
        "create_bkg" | "delete_bkgs" => vec![Read::Bkgs],
        "create_source_playlist"
        | "update_source_playlist"
        | "enable_source_playlists"
        | "delete_source_playlists" => vec![Read::SourcePlaylists],
        _ => Vec::new(),
    }
}

/// How a read field becomes a state value.
#[derive(Clone, Copy)]
enum Kind {
    Int,
    Float,
    Bool,
    Text,
}

/// The fields at JSON pointers in `src`, as `(dotted state path, pointer, kind)`,
/// gathered into one nested object. A field that is absent or of another
/// type is left out.
fn pick(src: &Value, fields: &[(&str, &str, Kind)]) -> Map<String, Value> {
    let mut out = Map::new();
    for (dest, pointer, kind) in fields {
        let Some(v) = src.pointer(pointer) else {
            continue;
        };
        let v = match kind {
            Kind::Int => v.as_i64().map(Value::from),
            Kind::Float => v.as_f64().map(Value::from),
            Kind::Text => v.as_str().map(Value::from),
            Kind::Bool => match v {
                Value::Bool(b) => Some(Value::Bool(*b)),
                _ => match v.as_i64() {
                    Some(0) => Some(Value::Bool(false)),
                    Some(1) => Some(Value::Bool(true)),
                    _ => None,
                },
            },
        };
        let Some(v) = v else { continue };
        let mut at = &mut out;
        let mut segments = dest.split('.').peekable();
        while let Some(seg) = segments.next() {
            if segments.peek().is_none() {
                at.insert(seg.into(), v.clone());
                break;
            }
            let next = at
                .entry(seg.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !next.is_object() {
                *next = Value::Object(Map::new());
            }
            at = next.as_object_mut().unwrap();
        }
    }
    out
}

/// `value` placed at the dotted path `at`.
fn nest(at: &[String], value: Value) -> Value {
    at.iter()
        .rev()
        .fold(value, |v, seg| json!({ seg.as_str(): v }))
}

/// The elements of an array at `pointer` in `src`.
fn elements<'a>(src: &'a Value, pointer: &str) -> &'a [Value] {
    src.pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// device/readDetail: the device's health and identity (H OpenAPI, Devices,
/// Read Device Detailed Information).
const DEVICE_FIELDS: &[(&str, &str, Kind)] = &[
    ("name", "/name", Kind::Text),
    ("ready", "/status", Kind::Bool),
    ("model_id", "/modelId", Kind::Int),
    ("protocol_version", "/protoVersion", Kind::Text),
    ("main_card_state", "/mainState", Kind::Int),
    ("backup_card_state", "/deputyState", Kind::Int),
    ("temperature", "/backboardTemperature", Kind::Float),
    ("temperature_abnormal", "/temp", Kind::Bool),
    ("cpu_status", "/cpuStatus", Kind::Int),
    ("memory_status", "/memStatus", Kind::Int),
    ("ethernet_status", "/ethernetStatus", Kind::Int),
    ("usb_signal", "/usbStatus", Kind::Bool),
    ("mac", "/MAC", Kind::Text),
];

const GENLOCK_FIELDS: &[(&str, &str, Kind)] = &[
    ("enabled", "/genlock/enable", Kind::Bool),
    ("type", "/genlock/type", Kind::Int),
    ("input", "/genlock/inputId", Kind::Int),
    ("external_signal", "/genlock/extSignal", Kind::Bool),
];

const SLOT_FIELDS: &[(&str, &str, Kind)] = &[
    ("card_type", "/cardType", Kind::Int),
    ("model_id", "/modelId", Kind::Int),
    ("status", "/status", Kind::Int),
    ("work_status", "/workStatus", Kind::Int),
    ("serial", "/sn", Kind::Text),
];

const SCREEN_FIELDS: &[(&str, &str, Kind)] = &[
    ("name", "/general/name", Kind::Text),
    ("brightness", "/brightness", Kind::Int),
    ("gamma", "/gamma", Kind::Float),
    ("ftb", "/Ftb/enable", Kind::Bool),
    ("freeze", "/Freeze/enable", Kind::Bool),
    ("bkg.enable", "/Bkg/enable", Kind::Int),
    ("bkg.id", "/Bkg/bkgId", Kind::Int),
    ("size.x", "/outputMode/size/x", Kind::Int),
    ("size.y", "/outputMode/size/y", Kind::Int),
    ("size.width", "/outputMode/size/width", Kind::Int),
    ("size.height", "/outputMode/size/height", Kind::Int),
    ("mosaic.rows", "/outputMode/mosaic/row", Kind::Int),
    ("mosaic.columns", "/outputMode/mosaic/column", Kind::Int),
    ("stereo_3d.enabled", "/screen3D/enable", Kind::Bool),
    ("stereo_3d.type", "/screen3D/type", Kind::Int),
    ("osd.text", "/Osd/enable", Kind::Bool),
    ("osd.image", "/OsdImage/enable", Kind::Bool),
];

const LAYER_FIELDS: &[(&str, &str, Kind)] = &[
    ("name", "/general/name", Kind::Text),
    ("zorder", "/general/zorder", Kind::Int),
    ("lock", "/general/lock", Kind::Bool),
    ("freeze", "/general/isFreeze", Kind::Bool),
    ("background", "/general/isBackground", Kind::Bool),
    ("flip", "/general/flipType", Kind::Int),
    ("size_type", "/general/sizeType", Kind::Int),
    ("type", "/general/type", Kind::Int),
    ("source.type", "/source/sourceType", Kind::Int),
    ("source.input", "/source/inputId", Kind::Int),
    ("source.crop", "/source/cropId", Kind::Int),
    ("source.slot", "/source/slotId", Kind::Int),
    ("source.interface_type", "/source/interfaceType", Kind::Int),
    ("source.name", "/source/name", Kind::Text),
    ("source.channel", "/source/channelId", Kind::Int),
    ("source.stream_index", "/source/streamIndex", Kind::Int),
    ("window.x", "/window/x", Kind::Int),
    ("window.y", "/window/y", Kind::Int),
    ("window.width", "/window/width", Kind::Int),
    ("window.height", "/window/height", Kind::Int),
    ("window.lock_ratio", "/window/lockRatio", Kind::Bool),
];

const INPUT_FIELDS: &[(&str, &str, Kind)] = &[
    ("name", "/name", Kind::Text),
    ("online", "/online", Kind::Int),
    ("signal.width", "/resolution/width", Kind::Int),
    ("signal.height", "/resolution/height", Kind::Int),
    ("signal.refresh", "/resolution/refresh", Kind::Int),
    ("signal.interlaced", "/resolution/isInterlace", Kind::Bool),
    ("edid.width", "/timing/horizontal/addrTime", Kind::Int),
    ("edid.height", "/timing/vertical/addrTime", Kind::Int),
    ("edid.refresh_rate", "/timing/refreshRate", Kind::Int),
    // What layer/writeSource takes besides the input ID.
    ("slot", "/slotId", Kind::Int),
    ("interface_id", "/interfaceId", Kind::Int),
    ("interface_type", "/interfaceType", Kind::Int),
    ("source_type", "/sourceType", Kind::Int),
    ("default_name", "/defaultName", Kind::Text),
    ("audio.supported", "/isSupportAudio", Kind::Bool),
    ("audio.source", "/audioInterface/audioSrc", Kind::Int),
    ("audio.linked", "/audioInterface/linkStatus", Kind::Bool),
    ("format.color_depth", "/general/colorDepth", Kind::Int),
    ("format.color_space", "/general/colorSpace", Kind::Int),
    ("format.sample_rate", "/general/sampleRate", Kind::Int),
    ("format.capacity", "/general/connectCapacity", Kind::Int),
    ("format.hdcp_encrypted", "/general/HDCPEncryted", Kind::Bool),
];

const CROP_FIELDS: &[(&str, &str, Kind)] = &[
    ("name", "/name", Kind::Text),
    ("x", "/x", Kind::Int),
    ("y", "/y", Kind::Int),
    ("width", "/width", Kind::Int),
    ("height", "/height", Kind::Int),
];

const OUTPUT_FIELDS: &[(&str, &str, Kind)] = &[
    ("name", "/name", Kind::Text),
    ("gamma", "/gamma/gamma", Kind::Int),
    ("width", "/timing/horizontal/addrTime", Kind::Int),
    ("height", "/timing/vertical/addrTime", Kind::Int),
    ("refresh_rate", "/timing/refreshRate", Kind::Int),
    ("interlaced", "/timing/isInterlace", Kind::Bool),
    (
        "image_quality.brightness.all",
        "/imageQuality/brightness/all",
        Kind::Int,
    ),
    (
        "image_quality.brightness.r",
        "/imageQuality/brightness/R",
        Kind::Int,
    ),
    (
        "image_quality.brightness.g",
        "/imageQuality/brightness/G",
        Kind::Int,
    ),
    (
        "image_quality.brightness.b",
        "/imageQuality/brightness/B",
        Kind::Int,
    ),
    (
        "image_quality.contrast.all",
        "/imageQuality/contrast/all",
        Kind::Int,
    ),
    (
        "image_quality.contrast.r",
        "/imageQuality/contrast/R",
        Kind::Int,
    ),
    (
        "image_quality.contrast.g",
        "/imageQuality/contrast/G",
        Kind::Int,
    ),
    (
        "image_quality.contrast.b",
        "/imageQuality/contrast/B",
        Kind::Int,
    ),
    ("image_quality.hue", "/imageQuality/hue", Kind::Int),
    (
        "image_quality.saturation",
        "/imageQuality/saturation",
        Kind::Int,
    ),
    (
        "image_quality.colour_temperature",
        "/imageQuality/colorTemperature",
        Kind::Int,
    ),
];

const BKG_FIELDS: &[(&str, &str, Kind)] = &[
    ("name", "/general/name", Kind::Text),
    ("width", "/data/image/width", Kind::Int),
    ("height", "/data/image/height", Kind::Int),
];

const PLAYLIST_FIELDS: &[(&str, &str, Kind)] = &[
    ("name", "/sourceName", Kind::Text),
    ("enabled", "/ctrl/pullEnable", Kind::Bool),
    ("in_use", "/isUsed", Kind::Int),
];

/// What a request is for.
#[derive(Debug, Clone)]
enum Purpose {
    /// A command, and what to read back once it succeeds.
    Command(CommandId, Vec<Read>),
    Read(Read),
}

#[derive(Debug, Clone)]
struct Pending {
    purpose: Purpose,
    sent_at: Millis,
}

pub(crate) struct NovastarH {
    base: String,
    p_id: String,
    /// Unix time in milliseconds when the session's clock read 0.
    epoch: u64,
    poll_every: Millis,
    monitor: bool,
    next_id: RequestId,
    pending: HashMap<RequestId, Pending>,
    connected: Option<bool>,
    /// The keys each list last held, by its dotted state path.
    lists: HashMap<String, BTreeSet<String>>,
}

/// Base64 of the hexadecimal MD5 of `time` followed by `p_id` (§4, unencrypted).
pub(crate) fn sign(time: &str, p_id: &str) -> String {
    let digest = Md5::digest(format!("{time}{p_id}").as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    base64::engine::general_purpose::STANDARD.encode(hex)
}

/// The request envelope for a business body at `time` (Unix milliseconds).
pub(crate) fn envelope(body: Value, time: u64, p_id: &str) -> Value {
    let time = time.to_string();
    json!({
        "body": body,
        "sign": sign(&time, p_id),
        "pId": p_id,
        "timeStamp": time,
    })
}

/// The business body of a table command.
pub(crate) fn business_body(endpoint: &Endpoint, params: &Params) -> Result<Value, CommandError> {
    let mut body = Map::new();
    if endpoint.device {
        body.insert(
            "deviceId".into(),
            params.get("device_id").cloned().unwrap_or(json!(0)),
        );
    }
    for (param, field) in endpoint.fields {
        if let Some(v) = params.get(*param) {
            body.insert((*field).into(), v.clone());
        }
    }
    if endpoint.body {
        match params.get("body") {
            None | Some(Value::Null) => {}
            Some(Value::Object(extra)) => {
                for (k, v) in extra {
                    body.insert(k.clone(), v.clone());
                }
            }
            Some(_) => {
                return Err(CommandError::InvalidParams {
                    message: "'body' must be a JSON object".into(),
                })
            }
        }
    }
    Ok(Value::Object(body))
}

/// A reply's status and payload (`body`, or `data` where an endpoint answers
/// in that); `Err` when it is not the documented envelope.
fn read_reply(bytes: &[u8]) -> Result<(i64, String, Value), String> {
    let doc: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("reply is not JSON: {e}"))?;
    let status = doc
        .get("status")
        .or_else(|| doc.get("status "))
        .and_then(Value::as_i64)
        .ok_or("reply has no status")?;
    let msg = doc
        .get("msg")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let payload = match doc.get("body") {
        Some(b) if !b.is_null() => b.clone(),
        _ => doc.get("data").cloned().unwrap_or(Value::Null),
    };
    Ok((status, msg, payload))
}

impl NovastarH {
    pub(crate) fn new(ctx: OpenContext) -> NovastarH {
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        NovastarH::with(ctx, epoch)
    }

    fn with(ctx: OpenContext, epoch: u64) -> NovastarH {
        let s = &ctx.settings;
        let port = ctx.port.unwrap_or(DEFAULT_PORT);
        NovastarH {
            base: format!(
                "http://{}/open/api/",
                std::net::SocketAddr::new(ctx.host, port)
            ),
            p_id: s
                .get("p_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            epoch,
            poll_every: s.get("poll_ms").and_then(Value::as_u64).unwrap_or(5_000),
            monitor: ctx.monitor,
            next_id: 1,
            pending: HashMap::new(),
            connected: None,
            lists: HashMap::new(),
        }
    }

    fn post(&mut self, cx: &mut Cx, path: &str, body: Value, purpose: Purpose) {
        let id = self.next_id;
        self.next_id += 1;
        let request = envelope(body, self.epoch + cx.now(), &self.p_id);
        cx.http(
            id,
            HttpRequest {
                method: "POST",
                url: format!("{}{path}", self.base),
                headers: vec![("Content-Type".into(), "application/json".into())],
                body: Some(request.to_string().into_bytes()),
                body_stream: None,
                timeout: Some(TIMEOUT),
                accept_invalid_certs: false,
                digest: None,
            },
        );
        self.pending.insert(
            id,
            Pending {
                purpose,
                sent_at: cx.now(),
            },
        );
    }

    fn read(&mut self, cx: &mut Cx, read: Read) {
        self.post(cx, read.path(), read.body(), Purpose::Read(read));
    }

    fn poll(&mut self, cx: &mut Cx) {
        self.read(cx, Read::InitStatus);
        if self.monitor {
            for read in [
                Read::DeviceDetail,
                Read::Screens,
                Read::Inputs,
                Read::Outputs,
                Read::Bkgs,
                Read::SourcePlaylists,
            ] {
                self.read(cx, read);
            }
        }
    }

    fn set_connected(&mut self, cx: &mut Cx, up: bool, reason: &str) {
        if self.connected == Some(up) {
            return;
        }
        self.connected = Some(up);
        cx.connection(if up {
            Connection::Connected
        } else {
            Connection::Disconnected {
                reason: reason.into(),
            }
        });
    }

    /// `items` as the whole list at the dotted path `at`: keys it held last
    /// time and no longer does are set to null (removed), along with what
    /// was kept under them.
    fn list(&mut self, at: &str, mut items: Map<String, Value>) -> Map<String, Value> {
        let keys: BTreeSet<String> = items.keys().cloned().collect();
        let before = self
            .lists
            .insert(at.to_string(), keys.clone())
            .unwrap_or_default();
        for gone in before.difference(&keys) {
            let prefix = format!("{at}.{gone}.");
            self.lists.retain(|k, _| !k.starts_with(&prefix));
            items.insert(gone.clone(), Value::Null);
        }
        items
    }

    /// Elements keyed by the integer at `id`, each picked by `fields`.
    fn keyed(
        elements: &[Value],
        id: &str,
        fields: &[(&str, &str, Kind)],
    ) -> Vec<(String, Map<String, Value>)> {
        elements
            .iter()
            .filter_map(|e| {
                let key = e.pointer(id)?.as_i64()?;
                Some((key.to_string(), pick(e, fields)))
            })
            .collect()
    }

    fn apply(&mut self, cx: &mut Cx, read: Read, body: &Value) {
        let screen = |s: i64, rest: &[&str]| {
            let mut at = vec!["screens".to_string(), s.to_string()];
            at.extend(rest.iter().map(|r| r.to_string()));
            at
        };
        match read {
            Read::InitStatus => {
                let device = pick(
                    body,
                    &[
                        ("initialized", "/initStatus", Kind::Bool),
                        ("serial", "/sn", Kind::Text),
                        ("software_version", "/softwareVersion", Kind::Text),
                        ("main_model_id", "/mainModelId", Kind::Int),
                    ],
                );
                if !device.is_empty() {
                    cx.state(json!({ "device": device }));
                }
            }
            Read::DeviceDetail => {
                let device = pick(body, DEVICE_FIELDS);
                if !device.is_empty() {
                    cx.state(json!({ "device": device }));
                }
                let genlock = pick(body, GENLOCK_FIELDS);
                if !genlock.is_empty() {
                    cx.state(json!({ "genlock": genlock }));
                }
                // Lists of power supplies, fans and card slots, each keyed by
                // its id and replaced whole.
                for (at, pointer, id, fields) in [
                    (
                        "power_supplies",
                        "/powerList",
                        "/powerId",
                        &[("connected", "/iSignal", Kind::Bool)][..],
                    ),
                    (
                        "fans",
                        "/fanList",
                        "/fanId",
                        &[("abnormal", "/status", Kind::Bool)][..],
                    ),
                    ("slots", "/slotList", "/slotId", SLOT_FIELDS),
                ] {
                    if body.pointer(pointer).and_then(Value::as_array).is_none() {
                        continue;
                    }
                    let items = Self::keyed(elements(body, pointer), id, fields)
                        .into_iter()
                        .map(|(k, v)| (k, Value::Object(v)))
                        .collect();
                    let items = self.list(at, items);
                    cx.state(json!({ at: items }));
                }
            }
            Read::Screens => {
                let Some(screens) = body.get("screens").and_then(Value::as_array) else {
                    return;
                };
                let items = Self::keyed(screens, "/screenId", &[("name", "/name", Kind::Text)]);
                let ids: Vec<i64> = items.iter().filter_map(|(k, _)| k.parse().ok()).collect();
                let items = items
                    .into_iter()
                    .map(|(k, v)| (k, Value::Object(v)))
                    .collect();
                let items = self.list("screens", items);
                if !items.is_empty() {
                    cx.state(json!({ "screens": items }));
                }
                if self.monitor {
                    for sid in ids {
                        for read in screen_reads(sid) {
                            self.read(cx, read);
                        }
                    }
                }
            }
            Read::ScreenDetail(s) => {
                let detail = pick(body, SCREEN_FIELDS);
                if !detail.is_empty() {
                    cx.state(nest(&screen(s, &[]), Value::Object(detail)));
                }
            }
            Read::Layers(s) => {
                let Some(layers) = body.get("screenLayers").and_then(Value::as_array) else {
                    return;
                };
                let items = Self::keyed(layers, "/layerId", LAYER_FIELDS)
                    .into_iter()
                    .map(|(k, v)| (k, Value::Object(v)))
                    .collect();
                let items = self.list(&format!("screens.{s}.layers"), items);
                cx.state(nest(&screen(s, &["layers"]), Value::Object(items)));
            }
            Read::Presets(s) => {
                let Some(presets) = body.get("presets").and_then(Value::as_array) else {
                    return;
                };
                let items = Self::keyed(presets, "/presetId", &[("name", "/name", Kind::Text)])
                    .into_iter()
                    .map(|(k, v)| (k, Value::Object(v)))
                    .collect();
                let items = self.list(&format!("screens.{s}.presets"), items);
                cx.state(nest(&screen(s, &["presets"]), Value::Object(items)));
            }
            Read::CurrentPreset(s) => {
                if let Some(p) = body.get("presetId").and_then(Value::as_i64) {
                    cx.state(nest(&screen(s, &["preset"]), json!(p)));
                }
            }
            Read::PresetGroupPlaying(s) => {
                let playing = pick(
                    body,
                    &[
                        ("playing", "/enable", Kind::Bool),
                        ("id", "/presetGroupId", Kind::Int),
                    ],
                );
                if !playing.is_empty() {
                    cx.state(nest(&screen(s, &["preset_group"]), Value::Object(playing)));
                }
            }
            Read::PresetGroups(s) => {
                let Some(groups) = body.get("presetGroups").and_then(Value::as_array) else {
                    return;
                };
                let items = Self::keyed(groups, "/presetGroupId", &[("name", "/name", Kind::Text)])
                    .into_iter()
                    .map(|(k, v)| (k, Value::Object(v)))
                    .collect();
                let items = self.list(&format!("screens.{s}.preset_groups"), items);
                cx.state(nest(&screen(s, &["preset_groups"]), Value::Object(items)));
            }
            Read::Inputs => {
                let Some(inputs) = body.get("inputs").and_then(Value::as_array) else {
                    return;
                };
                let mut items = Map::new();
                for input in inputs {
                    let Some(iid) = input.get("inputId").and_then(Value::as_i64) else {
                        continue;
                    };
                    let mut fields = pick(input, INPUT_FIELDS);
                    let crops = Self::keyed(elements(input, "/crops"), "/cropId", CROP_FIELDS)
                        .into_iter()
                        .map(|(k, v)| (k, Value::Object(v)))
                        .collect();
                    let crops = self.list(&format!("inputs.{iid}.crops"), crops);
                    if !crops.is_empty() {
                        fields.insert("crops".into(), Value::Object(crops));
                    }
                    items.insert(iid.to_string(), Value::Object(fields));
                }
                let items = self.list("inputs", items);
                cx.state(json!({ "inputs": items }));
            }
            Read::Outputs => {
                let Some(outputs) = body.get("outputs").and_then(Value::as_array) else {
                    return;
                };
                let items = Self::keyed(outputs, "/outputId", OUTPUT_FIELDS)
                    .into_iter()
                    .map(|(k, v)| (k, Value::Object(v)))
                    .collect();
                let items = self.list("outputs", items);
                cx.state(json!({ "outputs": items }));
            }
            Read::Bkgs => {
                // Comprehensive Query answers the list in `data` itself.
                let Some(bkgs) = body.as_array() else {
                    return;
                };
                let items = Self::keyed(bkgs, "/bkgId", BKG_FIELDS)
                    .into_iter()
                    .map(|(k, v)| (k, Value::Object(v)))
                    .collect();
                let items = self.list("bkgs", items);
                cx.state(json!({ "bkgs": items }));
            }
            Read::SourcePlaylists => {
                let Some(sources) = body.get("sources").and_then(Value::as_array) else {
                    return;
                };
                let items = Self::keyed(sources, "/sourceId", PLAYLIST_FIELDS)
                    .into_iter()
                    .map(|(k, v)| (k, Value::Object(v)))
                    .collect();
                let items = self.list("source_playlists", items);
                cx.state(json!({ "source_playlists": items }));
            }
        }
    }
}

impl Module for NovastarH {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.poll(cx);
        if self.poll_every > 0 {
            cx.set_timer(POLL, self.poll_every);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if name == "call" {
            let path = params.get("path").and_then(Value::as_str).unwrap_or("");
            let path = path.trim_start_matches('/').to_string();
            let body = params.get("body").cloned().unwrap_or(json!({}));
            self.post(cx, &path, body, Purpose::Command(id, Vec::new()));
            return;
        }
        let Some(endpoint) = ENDPOINTS.iter().find(|e| e.name == name) else {
            cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            );
            return;
        };
        let reads = if self.monitor {
            read_back(name, params)
        } else {
            Vec::new()
        };
        match business_body(endpoint, params) {
            Ok(body) => self.post(cx, endpoint.path, body, Purpose::Command(id, reads)),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let Some(pending) = self.pending.remove(&id) else {
            return;
        };
        let response = match result {
            Ok(r) => r,
            Err(reason) => {
                self.set_connected(cx, false, &reason);
                if let Purpose::Command(cid, _) = pending.purpose {
                    cx.complete(cid, Err(CommandError::Transport { message: reason }));
                }
                return;
            }
        };
        cx.alive();
        cx.round_trip(cx.now().saturating_sub(pending.sent_at));
        self.set_connected(cx, true, "");
        let parsed = if response.status == 200 {
            read_reply(&response.body)
        } else {
            Err(format!("HTTP {}", response.status))
        };
        match pending.purpose {
            Purpose::Command(cid, reads) => match parsed {
                Ok((0, _, body)) => {
                    cx.complete(cid, Ok(Outcome::Value { value: body }));
                    for read in reads {
                        self.read(cx, read);
                    }
                }
                Ok((status, msg, _)) => cx.complete(
                    cid,
                    Err(CommandError::DeviceError {
                        code: Some(status.to_string()),
                        message: if msg.is_empty() {
                            format!("status {status}")
                        } else {
                            msg
                        },
                    }),
                ),
                Err(message) => cx.complete(
                    cid,
                    Err(CommandError::DeviceError {
                        code: None,
                        message,
                    }),
                ),
            },
            Purpose::Read(read) => {
                if let Ok((0, _, body)) = parsed {
                    self.apply(cx, read, &body);
                }
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == POLL {
            self.poll(cx);
            if self.poll_every > 0 {
                cx.set_timer(POLL, self.poll_every);
            }
        } else {
            cx.log(Level::Debug, format!("NovaStar H: unknown timer {key}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    fn context(monitor: bool) -> OpenContext {
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 168, 10, 228)),
            host_name: None,
            port: None,
            model: "h5".into(),
            channels: None,
            settings: json!({"p_id": "YmRj", "poll_ms": 5000})
                .as_object()
                .unwrap()
                .clone(),
            monitor,
        }
    }

    fn requests(actions: &[Action]) -> Vec<(RequestId, String, Value)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Http { id, request } => Some((
                    *id,
                    request.url.clone(),
                    serde_json::from_slice(request.body.as_ref().unwrap()).unwrap(),
                )),
                _ => None,
            })
            .collect()
    }

    fn reply(body: Value) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            headers: Vec::new(),
            status: 200,
            body: body.to_string().into_bytes(),
        })
    }

    /// Both worked examples of the documentation's pages.
    #[test]
    fn signatures_match_the_documented_examples() {
        assert_eq!(
            sign("1689586062335", "YmRj"),
            "YjlhMWRmZTVlNzJhYTg4MTgzMDFhNTdlOWE0NjMyNDc="
        );
        assert_eq!(
            sign("1689586062335", "ZTlj"),
            "Y2YxNjE4NjRmZTFlOThhODIxMDNhOWY2YmU4MzU5ODk="
        );
    }

    #[test]
    fn a_command_is_enveloped_signed_and_timed() {
        let mut m = NovastarH::with(context(true), 1_689_586_062_000);
        let mut cx = Cx::new(335);
        m.command(
            &mut cx,
            7,
            "screen_ftb",
            json!({"device_id": 0, "screen_id": 1, "type": 0, "seconds": 2})
                .as_object()
                .unwrap(),
        );
        let sent = requests(&cx.take());
        assert_eq!(sent.len(), 1);
        let (_, url, body) = &sent[0];
        assert_eq!(url, "http://192.168.10.228:8000/open/api/screen/ftb");
        assert_eq!(
            body,
            &json!({
                "body": {"deviceId": 0, "screenId": 1, "type": 0, "time": 2},
                "sign": "YjlhMWRmZTVlNzJhYTg4MTgzMDFhNTdlOWE0NjMyNDc=",
                "pId": "YmRj",
                "timeStamp": "1689586062335"
            })
        );
    }

    #[test]
    fn nested_bodies_are_merged_and_call_sends_any_path() {
        let ep = ENDPOINTS
            .iter()
            .find(|e| e.name == "set_layer_source")
            .unwrap();
        let body = business_body(
            ep,
            json!({"device_id": 0, "screen_id": 0, "layer_id": 3, "body": {"source": {"sourceType": 1, "inputId": 4}}})
                .as_object()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            body,
            json!({"deviceId": 0, "screenId": 0, "layerId": 3, "source": {"sourceType": 1, "inputId": 4}})
        );
        let mut m = NovastarH::with(context(true), 0);
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            1,
            "call",
            json!({"path": "/ipc/readList", "body": {"deviceId": 0}})
                .as_object()
                .unwrap(),
        );
        let sent = requests(&cx.take());
        assert_eq!(
            sent[0].1,
            "http://192.168.10.228:8000/open/api/ipc/readList"
        );
        assert_eq!(sent[0].2["body"], json!({"deviceId": 0}));
    }

    #[test]
    fn replies_complete_commands_and_report_round_trips() {
        let mut m = NovastarH::with(context(true), 0);
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            1,
            "load_preset",
            json!({"device_id": 0, "screen_id": 0, "preset_id": 1})
                .as_object()
                .unwrap(),
        );
        let (id, _, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(25);
        m.http_response(
            &mut cx,
            id,
            reply(json!({"status": 0, "msg": "Success", "sign": "", "body": ""})),
        );
        let a = cx.take();
        assert!(a.contains(&Action::RoundTrip(25)));
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value { value: json!("") })
        }));

        let mut cx = Cx::new(30);
        m.command(
            &mut cx,
            2,
            "get_screen",
            json!({"device_id": 0, "screen_id": 9}).as_object().unwrap(),
        );
        let (id, _, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(40);
        m.http_response(
            &mut cx,
            id,
            reply(json!({"status": 404, "msg": "The screen does not exist.", "body": ""})),
        );
        assert!(cx.take().iter().any(|a| matches!(a,
            Action::Complete { id: 2, result: Err(CommandError::DeviceError { code: Some(c), .. }) } if c == "404")));
    }

    #[test]
    fn the_poll_reads_screens_and_their_presets() {
        let mut m = NovastarH::with(context(true), 0);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let sent = requests(&cx.take());
        let paths: Vec<&str> = sent
            .iter()
            .map(|s| s.1.rsplit("/api/").next().unwrap())
            .collect();
        assert_eq!(
            paths,
            [
                "main/initStatus",
                "device/readDetail",
                "screen/readList",
                "input/readList",
                "output/readList",
                "bkg/readAllList",
                "input/readSourceList"
            ]
        );
        let screens = sent
            .iter()
            .find(|s| s.1.ends_with("screen/readList"))
            .unwrap()
            .0;
        let init = sent
            .iter()
            .find(|s| s.1.ends_with("main/initStatus"))
            .unwrap()
            .0;
        let mut cx = Cx::new(10);
        // The documented initStatus example spells its status key "status ".
        m.http_response(&mut cx, init, reply(json!({"body": {"initStatus": 1, "mainModelId": 29962, "sn": "FFFFFFFFFFFFFFFF", "softwareVersion": "1.9.7.0.S1.T1"}, "msg": "Success", "sign": "", "status ": 0})));
        let a = cx.take();
        assert!(a.contains(&Action::State(json!({"device": {"initialized": true, "serial": "FFFFFFFFFFFFFFFF", "software_version": "1.9.7.0.S1.T1", "main_model_id": 29962}}))));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let mut cx = Cx::new(20);
        m.http_response(&mut cx, screens, reply(json!({"body": {"deviceId": 0, "screens": [{"createTime": "1757509392.7567859", "name": "Screen 1", "screenId": 0}]}, "msg": "", "sign": "", "status": 0})));
        let a = cx.take();
        assert!(a.contains(&Action::State(
            json!({"screens": {"0": {"name": "Screen 1"}}})
        )));
        let reads = requests(&a);
        let paths: Vec<&str> = reads
            .iter()
            .map(|s| s.1.rsplit("/api/").next().unwrap())
            .collect();
        assert_eq!(
            paths,
            [
                "screen/readDetail",
                "layer/detailList",
                "preset/readList",
                "preset/readPlay",
                "preset/readPresetPoll",
                "preset/groupList"
            ]
        );
        assert!(reads
            .iter()
            .all(|r| r.2["body"] == json!({"deviceId": 0, "screenId": 0})));
        let preset = &reads[3];
        let mut cx = Cx::new(30);
        m.http_response(&mut cx, preset.0, reply(json!({"body": {"deviceId": 0, "presetId": 1, "screenId": 0}, "msg": "Success", "sign": "", "status": 0})));
        assert!(cx
            .take()
            .contains(&Action::State(json!({"screens": {"0": {"preset": 1}}}))));
    }

    #[test]
    fn the_poll_reads_the_devices_health() {
        let mut m = NovastarH::with(context(true), 0);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let sent = requests(&cx.take());
        let detail = sent
            .iter()
            .find(|s| s.1.ends_with("device/readDetail"))
            .unwrap();
        assert_eq!(detail.2["body"], json!({"deviceId": 0}));
        let mut cx = Cx::new(10);
        // The documented example, cut down, with the table's fields it leaves out.
        m.http_response(&mut cx, detail.0, reply(json!({"body": {
            "MAC": "be-27-73-a0-68-b6", "cpuStatus": 0, "deviceId": 0, "ethernetStatus": 1, "memStatus": 0,
            "modelId": 29965, "name": "", "protoVersion": "1.0.0.0", "status": 1, "mainState": 1, "deputyState": 0,
            "backboardTemperature": 41, "temp": 0, "usbStatus": 0,
            "fanList": [{"fanId": 0, "status": 0}, {"fanId": 1, "status": 1}],
            "genlock": {"deviceId": 0, "enable": 0, "extSignal": 0, "inputId": 0, "type": 0},
            "powerList": [{"iSignal": 1, "powerId": 0}],
            "slotList": [{"cardType": 1, "modelId": 29953, "slotId": 0, "sn": "0000000000000000", "status": 1, "workStatus": 0}]
        }, "msg": "Success", "sign": "", "status": 0})));
        let a = cx.take();
        assert!(a.contains(&Action::State(json!({"device": {
            "name": "", "ready": true, "model_id": 29965, "protocol_version": "1.0.0.0", "main_card_state": 1,
            "backup_card_state": 0, "temperature": 41.0, "temperature_abnormal": false, "cpu_status": 0,
            "memory_status": 0, "ethernet_status": 1, "usb_signal": false, "mac": "be-27-73-a0-68-b6"}}))));
        assert!(a.contains(&Action::State(
            json!({"genlock": {"enabled": false, "type": 0, "input": 0, "external_signal": false}})
        )));
        assert!(a.contains(&Action::State(
            json!({"power_supplies": {"0": {"connected": true}}})
        )));
        assert!(a.contains(&Action::State(
            json!({"fans": {"0": {"abnormal": false}, "1": {"abnormal": true}}})
        )));
        assert!(a.contains(&Action::State(json!({"slots": {"0": {
            "card_type": 1, "model_id": 29953, "status": 1, "work_status": 0, "serial": "0000000000000000"}}}))));
    }

    #[test]
    fn opened_for_commands_only_it_asks_only_the_init_status() {
        let mut m = NovastarH::with(context(false), 0);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let sent = requests(&cx.take());
        assert_eq!(sent.len(), 1);
        assert!(sent[0].1.ends_with("main/initStatus"));
        // Commands still work.
        let mut cx = Cx::new(1);
        m.command(
            &mut cx,
            1,
            "list_screens",
            json!({"device_id": 0}).as_object().unwrap(),
        );
        assert_eq!(requests(&cx.take()).len(), 1);
    }

    /// Answers `read` with `payload` and returns the actions.
    fn answer(m: &mut NovastarH, read: Read, payload: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.read(&mut cx, read);
        let (id, url, _) = requests(&cx.take())[0].clone();
        assert!(url.ends_with(read.path()));
        let mut cx = Cx::new(5);
        m.http_response(&mut cx, id, reply(payload));
        cx.take()
    }

    fn states(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::State(v) => Some(v.clone()),
                _ => None,
            })
            .collect()
    }

    /// The fields of Read Screen Detailed Information's response schema.
    #[test]
    fn screen_details_are_kept() {
        let mut m = NovastarH::with(context(true), 0);
        let a = answer(
            &mut m,
            Read::ScreenDetail(2),
            json!({"status": 0, "msg": "", "sign": "", "body": {
                "screenId": 2, "brightness": 30, "gamma": 2, "general": {"name": "Wall"},
                "Ftb": {"enable": 1}, "Freeze": {"enable": 0}, "Bkg": {"bkgId": 1, "enable": 0, "imgUrl": ""},
                "outputMode": {"size": {"x": 0, "y": 0, "width": 3840, "height": 1080}, "mosaic": {"row": 1, "column": 2}},
                "screen3D": {"enable": 0, "type": 1}, "Osd": {"enable": 1}, "OsdImage": {"enable": 0}}}),
        );
        assert_eq!(
            states(&a),
            [json!({"screens": {"2": {
                "name": "Wall", "brightness": 30, "gamma": 2.0, "ftb": true, "freeze": false,
                "bkg": {"enable": 0, "id": 1},
                "size": {"x": 0, "y": 0, "width": 3840, "height": 1080},
                "mosaic": {"rows": 1, "columns": 2},
                "stereo_3d": {"enabled": false, "type": 1},
                "osd": {"text": true, "image": false}}}})]
        );
    }

    /// Read Layer List's documented example, then a list without layer 0.
    #[test]
    fn layers_are_kept_whole() {
        let mut m = NovastarH::with(context(true), 0);
        let layer = |id: i64, name: &str, z: i64| {
            json!({"audioStatus": {"isAvailable": 0, "isOpen": 0}, "deviceId": 0,
                "general": {"flipType": 0, "isBackground": false, "isFreeze": false, "layerId": id, "lock": 0, "name": name, "sizeType": 0, "type": 1, "zorder": z},
                "isExceedingLimit": false, "layerId": id, "reverseControl": {"ip": "", "port": 18986}, "screenId": 0,
                "source": {"channelId": 65535, "cropId": 255, "inputId": 10, "interfaceType": 27, "name": "input 3-4-1", "slotId": 2, "sourceType": 1, "streamIndex": 0},
                "window": {"height": 1080, "lockRatio": 0, "width": 960, "x": 3245, "y": 1115}})
        };
        let a = answer(
            &mut m,
            Read::Layers(0),
            json!({"status": 0, "msg": "Success", "sign": "", "body": {"deviceId": 0, "screenId": 0,
                "screenLayers": [layer(2, "Layer 3", 1), layer(0, "Layer 1", 0)]}}),
        );
        let s = states(&a);
        assert_eq!(s.len(), 1);
        assert_eq!(
            s[0]["screens"]["0"]["layers"]["2"],
            json!({"name": "Layer 3", "zorder": 1, "lock": false, "freeze": false, "background": false,
                "flip": 0, "size_type": 0, "type": 1,
                "source": {"type": 1, "input": 10, "crop": 255, "slot": 2, "interface_type": 27, "name": "input 3-4-1", "channel": 65535, "stream_index": 0},
                "window": {"x": 3245, "y": 1115, "width": 960, "height": 1080, "lock_ratio": false}})
        );
        let a = answer(
            &mut m,
            Read::Layers(0),
            json!({"status": 0, "msg": "", "sign": "", "body": {"deviceId": 0, "screenId": 0,
                "screenLayers": [layer(2, "Layer 3", 0)]}}),
        );
        let s = states(&a);
        assert_eq!(s[0]["screens"]["0"]["layers"]["0"], Value::Null);
        assert!(s[0]["screens"]["0"]["layers"]
            .as_object()
            .unwrap()
            .contains_key("0"));
        assert_eq!(s[0]["screens"]["0"]["layers"]["2"]["zorder"], json!(0));
    }

    /// Read All Presets, Read Preset Playlist and Get Preset Group List examples.
    #[test]
    fn presets_and_preset_groups_are_kept() {
        let mut m = NovastarH::with(context(true), 0);
        let a = answer(
            &mut m,
            Read::Presets(0),
            json!({"body": {"deviceId": 0, "presets": [{"name": "Preset 1", "presetId": 0}, {"name": "Preset 2", "presetId": 1}], "screenId": 0}, "msg": "Success", "sign": "", "status": 0}),
        );
        assert_eq!(
            states(&a),
            [
                json!({"screens": {"0": {"presets": {"0": {"name": "Preset 1"}, "1": {"name": "Preset 2"}}}}})
            ]
        );
        let a = answer(
            &mut m,
            Read::PresetGroupPlaying(0),
            json!({"body": {"devicedId": 0, "enable": 1, "presetGroupId": 2, "screenId": 0}, "msg": "", "sign": "", "status": 0}),
        );
        assert_eq!(
            states(&a),
            [json!({"screens": {"0": {"preset_group": {"playing": true, "id": 2}}}})]
        );
        let a = answer(
            &mut m,
            Read::PresetGroups(0),
            json!({"body": {"deviceId": 0, "presetGroups": [{"deviceId": 0, "name": "Preset playlist 1", "presetGroupId": 0, "presets": []}]}, "msg": "", "sign": "", "status": 0}),
        );
        assert_eq!(
            states(&a),
            [json!({"screens": {"0": {"preset_groups": {"0": {"name": "Preset playlist 1"}}}}})]
        );
    }

    /// Read Input List's and Read Output List's documented examples.
    #[test]
    fn inputs_and_outputs_are_kept() {
        let mut m = NovastarH::with(context(true), 0);
        let a = answer(
            &mut m,
            Read::Inputs,
            json!({"status": 0, "msg": "", "sign": "", "body": {"inputs": [{
                "crops": [{"cropId": 0, "name": "Left", "x": 0, "y": 0, "width": 1920, "height": 2160}],
                "general": {"name": "input 3-3-1", "colorDepth": 0, "colorSpace": 0, "sampleRate": 0,
                    "connectCapacity": 0, "HDCPEncryted": 0, "defaultName": "input 3-3-1"},
                "audioInterface": {"audioSrc": 1, "linkStatus": 0, "withAudioInterface": 0},
                "defaultName": "input 3-3-1", "isSupportAudio": 0,
                "inputId": 8, "interfaceId": 0, "interfaceType": 27, "slotId": 2, "sourceType": 1,
                "name": "input 3-3-1", "online": 1,
                "resolution": {"height": 0, "isInterlace": 0, "refresh": 0, "width": 0},
                "timing": {"horizontal": {"addrTime": 3840}, "refreshRate": 60, "vertical": {"addrTime": 2160}}}]}}),
        );
        assert_eq!(
            states(&a),
            [json!({"inputs": {"8": {"name": "input 3-3-1", "online": 1,
                "signal": {"width": 0, "height": 0, "refresh": 0, "interlaced": false},
                "edid": {"width": 3840, "height": 2160, "refresh_rate": 60},
                "slot": 2, "interface_id": 0, "interface_type": 27, "source_type": 1,
                "default_name": "input 3-3-1",
                "audio": {"supported": false, "source": 1, "linked": false},
                "format": {"color_depth": 0, "color_space": 0, "sample_rate": 0, "capacity": 0,
                    "hdcp_encrypted": false},
                "crops": {"0": {"name": "Left", "x": 0, "y": 0, "width": 1920, "height": 2160}}}}})]
        );
        // The crop is deleted.
        let a = answer(
            &mut m,
            Read::Inputs,
            json!({"status": 0, "msg": "", "sign": "", "body": {"inputs": [{"crops": [], "inputId": 8, "name": "input 3-3-1", "online": 0}]}}),
        );
        assert_eq!(
            states(&a),
            [json!({"inputs": {"8": {"name": "input 3-3-1", "online": 0, "crops": {"0": null}}}})]
        );
        let a = answer(
            &mut m,
            Read::Outputs,
            json!({"body": {"deviceId": 0, "outputs": [{"cardMode": 2, "gamma": {"gamma": 1}, "general": {"name": "output 12-1"},
                "imageQuality": {"brightness": {"B": 50, "G": 50, "R": 50, "all": 50}, "colorTemperature": 6500, "contrast": {"B": 50, "G": 50, "R": 50, "all": 50}, "hue": 0, "saturation": 50},
                "name": "output 12-1", "outputId": 4,
                "timing": {"horizontal": {"addrTime": 1920}, "isInterlace": 0, "refreshRate": 60, "vertical": {"addrTime": 2160}}}]}, "msg": "", "sign": "", "status": 0}),
        );
        assert_eq!(
            states(&a),
            [
                json!({"outputs": {"4": {"name": "output 12-1", "gamma": 1, "width": 1920, "height": 2160,
                "refresh_rate": 60, "interlaced": false,
                "image_quality": {"brightness": {"all": 50, "r": 50, "g": 50, "b": 50},
                    "contrast": {"all": 50, "r": 50, "g": 50, "b": 50},
                    "hue": 0, "saturation": 50, "colour_temperature": 6500}}}})
            ]
        );
    }

    /// Comprehensive Query and Read Signal Source Playlists answer in `data`.
    #[test]
    fn backgrounds_and_source_playlists_are_kept() {
        let mut m = NovastarH::with(context(true), 0);
        let a = answer(
            &mut m,
            Read::Bkgs,
            json!({"data": [{"bkgId": 0, "data": {"image": {"height": 1080, "width": 1920}}, "general": {"name": "BKG-1"}, "name": "BKG-1"}], "msg": "", "status": 0}),
        );
        assert_eq!(
            states(&a),
            [json!({"bkgs": {"0": {"name": "BKG-1", "width": 1920, "height": 1080}}})]
        );
        let a = answer(
            &mut m,
            Read::SourcePlaylists,
            json!({"data": {"deviceId": 0, "sources": [{"ctrl": {"pullEnable": 1, "zorder": 1}, "deviceId": 0, "isUsed": 1, "sourceId": 0, "sourceName": "TestName1", "sourceType": 6, "subList": []}]}, "msg": "", "status": 0}),
        );
        assert_eq!(
            states(&a),
            [
                json!({"source_playlists": {"0": {"name": "TestName1", "enabled": true, "in_use": 1}}})
            ]
        );
    }

    #[test]
    fn a_write_is_read_back_once_it_succeeds() {
        let mut m = NovastarH::with(context(true), 0);
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            1,
            "set_screen_brightness",
            json!({"device_id": 0, "screen_id": 2, "brightness": 30})
                .as_object()
                .unwrap(),
        );
        let (id, _, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(10);
        m.http_response(
            &mut cx,
            id,
            reply(json!({"status": 0, "msg": "", "sign": "", "body": ""})),
        );
        let reads = requests(&cx.take());
        assert_eq!(reads.len(), 1);
        assert!(reads[0].1.ends_with("screen/readDetail"));
        assert_eq!(reads[0].2["body"], json!({"deviceId": 0, "screenId": 2}));

        // A refused write is not read back.
        let mut cx = Cx::new(20);
        m.command(
            &mut cx,
            2,
            "rename_output",
            json!({"device_id": 0, "output_id": 4, "name": "Left"})
                .as_object()
                .unwrap(),
        );
        let (id, _, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(30);
        m.http_response(
            &mut cx,
            id,
            reply(json!({"status": 7, "msg": "", "sign": "", "body": ""})),
        );
        assert!(requests(&cx.take()).is_empty());

        // Nothing is read back when opened for commands only.
        let mut m = NovastarH::with(context(false), 0);
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            3,
            "load_preset",
            json!({"device_id": 0, "screen_id": 0, "preset_id": 1})
                .as_object()
                .unwrap(),
        );
        let (id, _, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(10);
        m.http_response(
            &mut cx,
            id,
            reply(json!({"status": 0, "msg": "", "sign": "", "body": ""})),
        );
        assert!(requests(&cx.take()).is_empty());
    }

    #[test]
    fn a_screen_no_longer_listed_leaves_state() {
        let mut m = NovastarH::with(context(true), 0);
        let list = |ids: &[i64]| {
            json!({"status": 0, "msg": "", "sign": "", "body": {"deviceId": 0,
                "screens": ids.iter().map(|i| json!({"screenId": i, "name": format!("Screen {i}")})).collect::<Vec<_>>()}})
        };
        answer(&mut m, Read::Screens, list(&[0, 1]));
        answer(
            &mut m,
            Read::Presets(1),
            json!({"status": 0, "body": {"presets": [{"name": "A", "presetId": 0}]}}),
        );
        let a = answer(&mut m, Read::Screens, list(&[0]));
        assert_eq!(
            states(&a),
            [json!({"screens": {"0": {"name": "Screen 0"}, "1": null}})]
        );
        assert!(!m.lists.contains_key("screens.1.presets"));
    }

    #[test]
    fn every_spec_command_has_an_endpoint() {
        let spec: Value =
            crate::yaml::from_str(include_str!("../../../../specs/novastar-h.yaml")).unwrap();
        for name in spec["commands"].as_object().unwrap().keys() {
            assert!(
                name == "call" || ENDPOINTS.iter().any(|e| e.name == name),
                "{name}"
            );
        }
        assert_eq!(
            ENDPOINTS.len() + 1,
            spec["commands"].as_object().unwrap().len()
        );
    }
}
