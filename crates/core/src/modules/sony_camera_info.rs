//! What the camera reports about itself beyond its properties, and the
//! commands that go with it: display string lists (the camera's own names for
//! base looks, scene files, buttons and more), lens focus tables, the zoom and
//! focus preset list, the OSD overlay image, restriction, licence and
//! operation-result lists, the clock and time zone, stream destination
//! settings, eframing, the camera's physical buttons, dials and levers,
//! pixel shift shooting, and user base look management.
//!
//! Layouts and sequences follow Sony's Camera Control PTP 3 Reference
//! (Operations: SDIO_GetDisplayStringList, SDIO_GetLensInformation,
//! SDIO_ExecuteEFraming, SDIO_OperationResultsSupported,
//! SDIO_GetPresetInfoList, SDIO_GetOSDImage, SDIO_GetRestrictionInfo,
//! SDIO_GetStreamSettingList, SDIO_SetStreamSettingList,
//! SDIO_GetAreaTimeZoneSetting, SDIO_SetAreaTimeZoneSetting,
//! SDIO_GetLicenseInfoList; Data Format: DisplayStringList, LensInformation,
//! OSDImageMetaInfo, StreamSettingList, EframingCommand, PresetInfoList,
//! AreaTimeZoneDataset, LicenseInfoListDataset; the Controls and Device
//! Properties named in each command) and, for pixel shift, the Camera Control
//! PTP 2 Reference.

use super::*;
use ds::Reader;

const OP_GET_DISPLAY_STRING_LIST: u16 = 0x9215;
const OP_GET_LENS_INFORMATION: u16 = 0x9223;
const OP_EXECUTE_EFRAMING: u16 = 0x922A;
const OP_OPERATION_RESULTS_SUPPORTED: u16 = 0x922F;
const OP_GET_PRESET_INFO_LIST: u16 = 0x9231;
const OP_GET_OSD_IMAGE: u16 = 0x9238;
const OP_GET_RESTRICTION_INFO: u16 = 0x9239;
const OP_GET_STREAM_SETTING_LIST: u16 = 0x9241;
const OP_SET_STREAM_SETTING_LIST: u16 = 0x9242;
const OP_GET_AREA_TIME_ZONE: u16 = 0x9248;
const OP_SET_AREA_TIME_ZONE: u16 = 0x9249;
const OP_GET_LICENSE_INFO_LIST: u16 = 0x924D;

/// What an information read becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Info {
    DisplayLists,
    Lens,
    Presets,
    Osd,
    Restrictions,
    Licenses,
    ResultsSupported,
    Clock,
    Streams,
    /// SDIO_SetStreamSettingList: its response parameter is the result.
    StreamSet,
}

/// Display string list types, by the names this module gives them.
const LIST_TYPES: &[(u32, &str)] = &[
    (0x01, "base_look_ae_level_offset"),
    (0x02, "base_look_input"),
    (0x03, "base_look_name"),
    (0x04, "base_look_output"),
    (0x05, "scene_file_name"),
    (0x06, "cinema_color_gamut"),
    (0x07, "target_display"),
    (0x08, "base_iso"),
    (0x09, "ei_gain"),
    (0x0A, "button_assign"),
    (0x0B, "button_assign_short"),
    (0x0C, "ftp_server_name"),
    (0x0D, "ftp_upload_directory"),
    (0x0E, "ftp_job_status"),
    (0x0F, "exposure_index_preset1"),
    (0x10, "movie_transfer_extension"),
    (0x11, "movie_transfer_video_codec"),
    (0x12, "movie_transfer_frame_rate"),
    (0x13, "creative_look_style"),
    (0x14, "iptc_metadata"),
    (0x15, "subject_recognition_af"),
    (0x16, "base_look_meta_record_support"),
    (0x17, "streaming_destination"),
    (0x18, "camera_button"),
    (0x19, "camera_lever"),
    (0x1A, "camera_dial"),
    (0x1B, "custom_grid_line_name"),
];

pub(super) fn list_type_name(t: u32) -> String {
    LIST_TYPES
        .iter()
        .find(|(c, _)| *c == t)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| format!("type_{t}"))
}

/// Properties whose value a display list names: the property, the list, and
/// the state path the name goes to.
pub(super) const LABELLED: &[(u16, u32, &str)] = &[
    (0xD03C, 0x03, "image.base_look_name"),
    (0xD0C8, 0x03, "base_look.editing_name"),
    (0xD0CC, 0x03, "base_look.pplut_name"),
    (0xD0CA, 0x01, "base_look.ae_level_offset_name"),
    (0xE01F, 0x05, "files.scene.name"),
    (0xD2BE, 0x17, "streaming.destination_name"),
    (0xD0FA, 0x13, "image.creative_look_name"),
];

/// Camera buttons the button-function controls can press, by name.
pub(super) const BUTTONS: &[(&str, u16)] = &[
    ("up", 0x01),
    ("down", 0x02),
    ("left", 0x03),
    ("right", 0x04),
    ("enter", 0x05),
    ("menu", 0x06),
    ("multi_up", 0x07),
    ("multi_down", 0x08),
    ("multi_left", 0x09),
    ("multi_right", 0x0A),
    ("multi_enter", 0x0B),
    ("multi_up_right", 0x0C),
    ("multi_down_right", 0x0D),
    ("multi_up_left", 0x0E),
    ("multi_down_left", 0x0F),
    ("fn", 0x10),
    ("playback", 0x11),
    ("delete", 0x12),
    ("mode", 0x13),
    ("c1", 0x14),
    ("c2", 0x15),
    ("c3", 0x16),
    ("c4", 0x17),
    ("c5", 0x18),
    ("c6", 0x19),
    ("movie", 0x1A),
    ("ael", 0x1B),
    ("af_on", 0x1C),
    ("home", 0x1D),
    ("clips", 0x1E),
    ("slot_select", 0x1F),
    ("display", 0x20),
    ("c7", 0x21),
    ("back", 0x22),
    ("thumbnail", 0x23),
];

pub(super) const DIALS: &[(&str, u16)] = &[
    ("control_wheel", 0x4001),
    ("front_dial", 0x4002),
    ("rear_dial_left", 0x4003),
    ("rear_dial_right", 0x4004),
];

pub(super) const LEVERS: &[(&str, u16)] = &[("tele_wide", 0x5001)];

/// Eframing types the command takes, by name.
pub(super) const EFRAMING_TYPES: &[(&str, u8)] = &[
    ("none", 0x01),
    ("auto", 0x02),
    ("single", 0x03),
    ("ptz", 0x05),
    ("hold", 0x08),
    ("zoom_out", 0x09),
];

fn name_of(table: &[(&str, u16)], code: u16) -> String {
    table
        .iter()
        .find(|(_, c)| *c == code)
        .map(|(n, _)| n.to_string())
        .unwrap_or_else(|| format!("0x{code:04X}"))
}

/// A name from the table, or a hex code the camera listed.
fn code_of(table: &[(&str, u16)], text: &str) -> Option<u16> {
    table
        .iter()
        .find(|(n, _)| *n == text)
        .map(|(_, c)| *c)
        .or_else(|| u16::from_str_radix(text.trim_start_matches("0x"), 16).ok())
}

/// The offset-and-size wrapper several lists come in.
fn unwrap(data: &[u8]) -> Result<&[u8], String> {
    let mut r = Reader::new(data);
    let offset = r.u32()? as usize;
    let size = r.u32()? as usize;
    let end = offset.checked_add(size).ok_or("size overflows")?;
    data.get(offset..end)
        .ok_or_else(|| "the list lies outside the dataset".to_string())
}

fn wrap(list: &[u8]) -> Vec<u8> {
    let mut b = 8u32.to_le_bytes().to_vec();
    b.extend_from_slice(&(list.len() as u32).to_le_bytes());
    b.extend_from_slice(list);
    b
}

fn utf8(r: &mut Reader, size: usize) -> Result<String, String> {
    if size > r.remaining() {
        return Err("text does not fit the dataset".into());
    }
    let bytes: Vec<u8> = (0..size).map(|_| r.u8()).collect::<Result<_, _>>()?;
    let end = bytes.iter().position(|c| *c == 0).unwrap_or(bytes.len());
    Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
}

fn text_with_nul(text: &str) -> Vec<u8> {
    let mut b = text.as_bytes().to_vec();
    b.push(0);
    b
}

/// Display lists: list type, then each value (as decimal text, matching
/// `properties.*.value`) with its display name.
/// One display list: its type and its names by value.
pub(super) type DisplayList = (u32, Map<String, Value>);

pub(super) fn parse_display_lists(data: &[u8]) -> Result<Vec<DisplayList>, String> {
    let list = unwrap(data)?;
    let mut r = Reader::new(list);
    let count = r.u16()?;
    r.u16()?;
    let mut out = Vec::new();
    for _ in 0..count {
        let list_type = r.u32()?;
        let datatype = r.u16()?;
        let n = r.u16()?;
        let mut names = Map::new();
        for _ in 0..n {
            let value = r.value(datatype)?;
            let size = r.u16()? as usize;
            let name = utf8(&mut r, size)?;
            let key = match value.to_json() {
                Value::String(s) => s,
                other => other.to_string(),
            };
            names.insert(key, json!(name));
        }
        out.push((list_type, names));
    }
    Ok(out)
}

/// Lens focus conversion tables: normalised focus position to distance.
pub(super) fn parse_lens_information(data: &[u8]) -> Result<Value, String> {
    let info = unwrap(data)?;
    let mut r = Reader::new(info);
    let version = r.u16()?;
    let tables = r.u16()?;
    let mut out = Vec::new();
    for _ in 0..tables {
        let n = r.u16()?;
        r.u16()?;
        let mut rows = Vec::new();
        for _ in 0..n {
            let normalized = r.u32()?;
            let distance = r.u32()? as f64 / 100.0;
            rows.push(json!({"position": normalized, "distance": distance}));
        }
        out.push(Value::Array(rows));
    }
    Ok(json!({"version": version, "tables": out}))
}

/// The zoom and focus presets (Save/Load Zoom and Focus Position slots),
/// numbered from 0 as those commands number them.
pub(super) fn parse_preset_info(data: &[u8]) -> Result<Vec<Value>, String> {
    let list = unwrap(data)?;
    let mut r = Reader::new(list);
    r.u16()?;
    r.u16()?;
    let count = r.u16()?;
    let mut out = Vec::new();
    for slot in 0..count {
        let set = r.u8()? == 0x02;
        let lens = r.string()?;
        let zoom = r.u32()?;
        let focus = r.u32()?;
        let zoom_only_available = r.u8()? == 0x02;
        let zoom_only = r.u8()? == 0x02;
        out.push(json!({
            "slot": slot,
            "set": set,
            "lens": lens,
            "zoom_mm": zoom as f64 / 1000.0,
            "focus_m": if focus == u32::MAX { Value::Null } else { json!(focus as f64 / 1000.0) },
            "zoom_only_available": zoom_only_available,
            "zoom_only": zoom_only,
        }));
    }
    Ok(out)
}

/// The OSD image (PNG) and its layout information.
pub(super) fn parse_osd(data: &[u8]) -> Result<(Vec<u8>, Value), String> {
    let mut r = Reader::new(data);
    let image_at = r.u32()? as usize;
    let image_size = r.u32()? as usize;
    let meta_at = r.u32()? as usize;
    let meta_size = r.u32()? as usize;
    let image = data
        .get(image_at..image_at.checked_add(image_size).ok_or("size overflows")?)
        .ok_or("the OSD image lies outside the dataset")?
        .to_vec();
    let meta = data
        .get(meta_at..meta_at.saturating_add(meta_size))
        .filter(|m| m.len() >= 8)
        .map(|m| -> Result<Value, String> {
            let mut r = Reader::new(m);
            let version = r.u16()?;
            r.u16()?;
            let has_yuv = r.u32()? != 0;
            if !has_yuv {
                return Ok(json!({"version": version, "yuv_layout": null}));
            }
            let mut v = json!({
                "version": version,
                "yuv_layout": {
                    "x_max": r.u32()?,
                    "y_max": r.u32()?,
                    "x": r.u32()?,
                    "y": r.u32()?,
                    "width": r.u32()?,
                    "height": r.u32()?,
                },
            });
            if version >= 101 {
                v["yuv_layout"]["tilt_degrees"] = json!(r.u32()?);
            }
            Ok(v)
        })
        .transpose()?
        .unwrap_or(Value::Null);
    Ok((image, meta))
}

/// Width and height from a PNG's header chunk.
fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24
        || png[..8] != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        || &png[12..16] != b"IHDR"
    {
        return None;
    }
    let be = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    Some((be(16), be(20)))
}

fn hex_list(codes: &[u16]) -> Value {
    Value::Array(codes.iter().map(|c| json!(format!("{c:04X}"))).collect())
}

pub(super) fn parse_restrictions(data: &[u8]) -> Result<Value, String> {
    let mut r = Reader::new(data);
    Ok(json!({
        "operations": hex_list(&r.u16_array()?),
        "properties": hex_list(&r.u16_array()?),
        "controls": hex_list(&r.u16_array()?),
    }))
}

pub(super) fn parse_licenses(data: &[u8]) -> Result<Value, String> {
    let mut r = Reader::new(data);
    r.u16()?;
    r.u16()?;
    let count = r.u8()?;
    for _ in 0..3 {
        r.u8()?;
    }
    let mut out = Vec::new();
    for _ in 0..count {
        let length = r.u32()? as usize;
        if length < 4 || length - 4 > r.remaining() {
            return Err("licence entry does not fit".into());
        }
        let body: Vec<u8> = (0..length - 4).map(|_| r.u8()).collect::<Result<_, _>>()?;
        let mut e = Reader::new(&body);
        let hours = e.u32()?;
        e.u32()?;
        let n = e.u8()? as usize;
        let id = utf8(&mut e, n)?;
        let n = e.u8()? as usize;
        let key = utf8(&mut e, n)?;
        out.push(json!({
            "license_id": id,
            "install_key_id": key,
            "remaining_hours": if hours == u32::MAX { Value::Null } else { json!(hours) },
        }));
    }
    Ok(Value::Array(out))
}

pub(super) fn parse_results_supported(data: &[u8]) -> Result<Value, String> {
    let codes = content::parse_u32_array(data)?;
    Ok(Value::Array(
        codes
            .iter()
            .map(|c| json!({"operation": format!("{:04X}", c >> 16), "target": format!("{:04X}", c & 0xFFFF)}))
            .collect(),
    ))
}

pub(super) fn parse_clock(data: &[u8]) -> Result<Value, String> {
    let mut r = Reader::new(data);
    r.u16()?;
    r.u16()?;
    let date_time = utf8(&mut r, 18)?;
    let offset = utf8(&mut r, 6)?;
    let dst = r.u8()? == 1;
    Ok(json!({"date_time": date_time, "utc_offset": offset, "daylight_saving": dst}))
}

fn fixed(text: &str, size: usize) -> Vec<u8> {
    let mut b = text.as_bytes().to_vec();
    b.resize(size, 0);
    b
}

pub(super) fn encode_clock(
    date_time: Option<&str>,
    offset: Option<&str>,
    dst: Option<bool>,
) -> Vec<u8> {
    let mut b = 100u16.to_le_bytes().to_vec();
    b.extend_from_slice(&[0, 0]);
    match date_time {
        Some(t) => {
            b.push(1);
            b.extend(fixed(t, 18));
        }
        None => b.push(0),
    }
    match offset {
        Some(o) => {
            b.push(1);
            b.extend(fixed(o, 6));
        }
        None => b.push(0),
    }
    match dst {
        Some(d) => {
            b.push(1);
            b.push(d as u8);
        }
        None => b.push(0),
    }
    b
}

fn protocols(bits: u8) -> Value {
    let mut v = Vec::new();
    if bits & 1 != 0 {
        v.push("rtmp");
    }
    if bits & 2 != 0 {
        v.push("srt");
    }
    json!(v)
}

/// Stream destinations, numbered from 1 as the Video Stream number is; keys
/// and cipher keys are reported only as set or not.
pub(super) fn parse_streams(data: &[u8]) -> Result<(u16, Vec<Value>), String> {
    let list = unwrap(data)?;
    let mut r = Reader::new(list);
    let version = r.u16()?;
    r.u16()?;
    let count = r.u32()?;
    let mut out = Vec::new();
    for i in 0..count {
        let protocol = r.u8()?;
        let allowed = r.u8()?;
        let n = r.u16()? as usize;
        let url = utf8(&mut r, n)?;
        let mut v = json!({
            "stream": i + 1,
            "protocol": protocols(protocol),
            "allowed_protocols": protocols(allowed),
            "url": url,
        });
        if r.u8()? != 0 {
            v["port"] = json!(r.u32()?);
        }
        let key_set = if r.u8()? != 0 {
            let n = r.u16()? as usize;
            !utf8(&mut r, n)?.is_empty()
        } else {
            false
        };
        v["key_set"] = json!(key_set);
        if r.u8()? != 0 {
            v["latency_ms"] = json!(r.u16()?);
        }
        if r.u8()? != 0 {
            v["ttl"] = json!(r.u8()?);
        }
        if r.u8()? != 0 {
            v["cipher_type"] = json!(r.u8()?);
            let n = r.u8()? as usize;
            v["cipher_key_set"] = json!(!utf8(&mut r, n)?.is_empty());
        }
        if r.u8()? != 0 {
            v["mode"] = json!(r.u8()?);
        }
        out.push(v);
    }
    Ok((version, out))
}

/// One stream destination to write.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct StreamWrite {
    /// Bit 0 RTMP, bit 1 SRT.
    pub protocol: u8,
    pub url: String,
    pub port: Option<u32>,
    pub key: Option<String>,
    pub latency_ms: Option<u16>,
    pub ttl: Option<u8>,
    pub cipher: Option<(u8, String)>,
    pub mode: Option<u8>,
}

pub(super) fn encode_stream(version: u16, s: &StreamWrite) -> Result<Vec<u8>, String> {
    let mut b = version.to_le_bytes().to_vec();
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(&1u32.to_le_bytes());
    b.push(s.protocol);
    let url = text_with_nul(&s.url);
    b.extend_from_slice(&(url.len() as u16).to_le_bytes());
    b.extend_from_slice(&url);
    match s.port {
        Some(p) => {
            b.push(1);
            b.extend_from_slice(&p.to_le_bytes());
        }
        None => b.push(0),
    }
    match &s.key {
        Some(k) => {
            b.push(1);
            let k = text_with_nul(k);
            b.extend_from_slice(&(k.len() as u16).to_le_bytes());
            b.extend_from_slice(&k);
        }
        None => b.push(0),
    }
    match s.latency_ms {
        Some(l) => {
            b.push(1);
            b.extend_from_slice(&l.to_le_bytes());
        }
        None => b.push(0),
    }
    match s.ttl {
        Some(t) => {
            b.push(1);
            b.push(t);
        }
        None => b.push(0),
    }
    match &s.cipher {
        Some((kind, key)) => {
            let key = text_with_nul(key);
            if key.len() > 80 {
                return Err("the cipher key is at most 79 characters".into());
            }
            b.push(1);
            b.push(*kind);
            b.push(key.len() as u8);
            b.extend_from_slice(&key);
        }
        None => b.push(0),
    }
    match s.mode {
        Some(m) => {
            b.push(1);
            b.push(m);
        }
        None => b.push(0),
    }
    Ok(wrap(&b))
}

/// An eframing command: the type, and for single or PTZ the crop rectangle
/// as fractions of the picture (0 to 1), normalised to a denominator of 1.
pub(super) fn encode_eframing(
    version: u32,
    kind: u8,
    relative: bool,
    area: Option<[f64; 4]>,
) -> Vec<u8> {
    let scaled = |v: f64| (v * 1024.0).round() as i32;
    let mut body = version.to_le_bytes().to_vec();
    body.extend_from_slice(&[0; 8]);
    body.extend_from_slice(&scaled(1.0).to_le_bytes());
    body.extend_from_slice(&scaled(1.0).to_le_bytes());
    body.push(kind);
    body.push(relative as u8);
    body.extend_from_slice(&[0, 0]);
    match area {
        Some([x, y, w, h]) => {
            body.extend_from_slice(&1u32.to_le_bytes());
            body.extend_from_slice(&[0; 4]);
            body.extend_from_slice(&1u16.to_le_bytes());
            body.extend_from_slice(&[0; 6]);
            body.push(1);
            body.extend_from_slice(&[0; 7]);
            for v in [x, y, w, h] {
                body.extend_from_slice(&scaled(v).to_le_bytes());
            }
            body.extend_from_slice(&[0; 24]);
        }
        None => {
            body.extend_from_slice(&0u32.to_le_bytes());
            body.extend_from_slice(&[0; 4]);
        }
    }
    let mut b = (body.len() as u32).to_le_bytes().to_vec();
    b.extend(body);
    b
}

fn clock_result_name(v: u32) -> &'static str {
    match v {
        1 => "ok",
        2 => "parameter_error",
        3 => "exclusion_error",
        4 => "system_error",
        _ => "invalid",
    }
}

fn stream_result(code: u32) -> Result<(), String> {
    match code {
        0 => Ok(()),
        2 => Err("the camera refused the URL".into()),
        3 => Err("the camera refused the port".into()),
        4 => Err("the camera refused the stream key".into()),
        other => Err(format!("the camera refused the settings (result {other})")),
    }
}

impl SonyCamera {
    fn info_op(&self, code: u16, params: Vec<u32>, info: Info, id: Option<CommandId>) -> Op {
        let mut op = Op::new(code, params, Step::Info(info));
        op.command = id;
        op.last = true;
        op
    }

    /// The camera's own user base look number for user 1 to 16: whichever of
    /// the plain number and the user-LUT form (0x0100 + number) the property
    /// lists.
    fn user_base_look(&self, code: u16, user: i128) -> PtpValue {
        let listed = |v: i128| match self.props.get(&code).map(|p| &p.form) {
            Some(Form::Enum { settable, values }) => {
                settable.contains(&PtpValue::Int(v)) || values.contains(&PtpValue::Int(v))
            }
            _ => false,
        };
        if !listed(user) && listed(0x0100 | user) {
            PtpValue::Int(0x0100 | user)
        } else {
            PtpValue::Int(user)
        }
    }

    /// Commands of this part of the module; `None` for any other name.
    pub(super) fn plan_info(
        &self,
        name: &str,
        params: &Params,
        id: CommandId,
    ) -> Option<Result<Plan, CommandError>> {
        let one = |op: Op| Ok(Plan::Ops(vec![op]));
        let user = |name: &str| -> Result<i128, CommandError> {
            let v = param_int(params, name)?;
            if !(1..=16).contains(&v) {
                return Err(invalid(format!("'{name}' must be 1 to 16")));
            }
            Ok(v)
        };
        let result = (|| -> Result<Plan, CommandError> {
            match name {
                "get_display_lists" => {
                    let t = param_int_or(params, "list_type", 0)? as u32;
                    one(self.info_op(
                        OP_GET_DISPLAY_STRING_LIST,
                        vec![t],
                        Info::DisplayLists,
                        Some(id),
                    ))
                }
                "get_lens_information" => {
                    self.require_enabled(0xE086, "give lens information")?;
                    let unit = if param_str_or(params, "unit") == "feet" {
                        1
                    } else {
                        2
                    };
                    one(self.info_op(OP_GET_LENS_INFORMATION, vec![unit], Info::Lens, Some(id)))
                }
                "get_zoom_focus_presets" => {
                    one(self.info_op(OP_GET_PRESET_INFO_LIST, vec![], Info::Presets, Some(id)))
                }
                "set_preset_zoom_only" => {
                    let slot = param_int(params, "slot")?;
                    if !(0..=255).contains(&slot) {
                        return Err(invalid("'slot' must be 0 to 255"));
                    }
                    let on = param_bool(params, "zoom_only")
                        .ok_or_else(|| invalid("'zoom_only' is required"))?;
                    Ok(Plan::Ops(vec![
                        self.control(0xD2F2, (slot << 8) | if on { 2 } else { 1 })?
                    ]))
                }
                "get_osd_image" => {
                    if let Some(info) = self.props.get(&0xD207) {
                        if info.current != PtpValue::Int(1) {
                            return Err(refused(
                                "not_available",
                                "the OSD image is off: set_osd_image_mode true first",
                            ));
                        }
                    }
                    one(self.info_op(OP_GET_OSD_IMAGE, vec![], Info::Osd, Some(id)))
                }
                "get_restriction_info" => one(self.info_op(
                    OP_GET_RESTRICTION_INFO,
                    vec![self.flag()],
                    Info::Restrictions,
                    Some(id),
                )),
                "get_licenses" => {
                    one(self.info_op(OP_GET_LICENSE_INFO_LIST, vec![], Info::Licenses, Some(id)))
                }
                "get_operation_results_supported" => one(self.info_op(
                    OP_OPERATION_RESULTS_SUPPORTED,
                    vec![],
                    Info::ResultsSupported,
                    Some(id),
                )),
                "get_clock" => {
                    one(self.info_op(OP_GET_AREA_TIME_ZONE, vec![], Info::Clock, Some(id)))
                }
                "set_clock" => {
                    let date_time = params.get("date_time").and_then(Value::as_str);
                    let offset = params.get("utc_offset").and_then(Value::as_str);
                    let dst = param_bool(params, "daylight_saving");
                    if date_time.is_none() && offset.is_none() && dst.is_none() {
                        return Err(invalid("give date_time, utc_offset or daylight_saving"));
                    }
                    let mut op = Op::with_data(
                        OP_SET_AREA_TIME_ZONE,
                        vec![],
                        encode_clock(date_time, offset, dst),
                    );
                    op.last = true;
                    one(op)
                }
                "set_date_time" => {
                    let value = param_str(params, "value")?;
                    Ok(Plan::Ops(vec![
                        self.set_prop(0xD223, PtpValue::Str(value.into()))?
                    ]))
                }
                "get_stream_settings" => {
                    self.require_enabled(0xD1CC, "give its stream settings")?;
                    one(self.info_op(OP_GET_STREAM_SETTING_LIST, vec![], Info::Streams, Some(id)))
                }
                "set_stream_settings" => {
                    self.require_enabled(0xD1CC, "take stream settings")?;
                    let stream = param_u32(params, "stream")?;
                    let opt_int = |n: &str| params.get(n).and_then(Value::as_i64);
                    let write = StreamWrite {
                        protocol: match param_str(params, "protocol")? {
                            "srt" => 2,
                            _ => 1,
                        },
                        url: param_str(params, "url")?.into(),
                        port: opt_int("port").map(|v| v as u32),
                        key: params.get("key").and_then(Value::as_str).map(String::from),
                        latency_ms: opt_int("latency_ms").map(|v| v as u16),
                        ttl: opt_int("ttl").map(|v| v as u8),
                        cipher: match (
                            opt_int("cipher_type"),
                            params.get("cipher_key").and_then(Value::as_str),
                        ) {
                            (Some(t), key) => Some((t as u8, key.unwrap_or("").to_string())),
                            (None, Some(_)) => {
                                return Err(invalid("give cipher_type with cipher_key"))
                            }
                            (None, None) => None,
                        },
                        mode: opt_int("mode").map(|v| v as u8),
                    };
                    let data = encode_stream(self.stream_version, &write).map_err(invalid)?;
                    let mut op = Op::with_data(OP_SET_STREAM_SETTING_LIST, vec![stream], data);
                    op.step = Step::Info(Info::StreamSet);
                    op.last = true;
                    one(op)
                }
                "execute_eframing" => {
                    let info = self.props.get(&0xD123).ok_or_else(|| {
                        refused(
                            "not_reported",
                            "the camera does not report its eframing command version",
                        )
                    })?;
                    if info.enabled == Enabled::No {
                        return Err(refused(
                            "not_available",
                            "the camera cannot take an eframing command now",
                        ));
                    }
                    let version = info.current.as_int().unwrap_or(100) as u32;
                    let kind_name = param_str(params, "type")?;
                    let kind = EFRAMING_TYPES
                        .iter()
                        .find(|(n, _)| *n == kind_name)
                        .map(|(_, k)| *k)
                        .ok_or_else(|| invalid(format!("unknown eframing type '{kind_name}'")))?;
                    let area = if matches!(kind, 0x03 | 0x05) {
                        let f = |n: &str| {
                            params.get(n).and_then(Value::as_f64).ok_or_else(|| {
                                invalid(format!("'{n}' is required for {kind_name}"))
                            })
                        };
                        Some([f("x")?, f("y")?, f("width")?, f("height")?])
                    } else {
                        None
                    };
                    let relative = param_bool(params, "relative").unwrap_or(false);
                    let mut op = Op::with_data(
                        OP_EXECUTE_EFRAMING,
                        vec![],
                        encode_eframing(version, kind, relative, area),
                    );
                    op.last = true;
                    one(op)
                }
                "camera_button" => {
                    let text = param_str(params, "button")?;
                    let button = code_of(BUTTONS, text)
                        .ok_or_else(|| invalid(format!("unknown button '{text}'")))?;
                    let simultaneous = param_bool(params, "simultaneous").unwrap_or(false);
                    let code = if simultaneous { 0xD30A } else { 0xD309 };
                    if !simultaneous {
                        if let Some(info) = self.props.get(&0xD20C) {
                            if info.current != PtpValue::Int(1) {
                                return Err(refused(
                                    "busy",
                                    "a camera key is held: wait, or press with simultaneous",
                                ));
                            }
                        }
                    }
                    let value = |on: bool| ((button as i128) << 16) | if on { 2 } else { 1 };
                    Ok(Plan::Ops(match param_bool(params, "pressed") {
                        Some(on) => vec![self.control(code, value(on))?],
                        None => vec![
                            self.control(code, value(true))?,
                            Op::pause(PRESS),
                            self.control(code, value(false))?,
                        ],
                    }))
                }
                "camera_dial" => {
                    let text = param_str(params, "dial")?;
                    let dial = code_of(DIALS, text)
                        .ok_or_else(|| invalid(format!("unknown dial '{text}'")))?;
                    let steps = param_int(params, "steps")?;
                    if !(-32767..=32767).contains(&steps) {
                        return Err(invalid("'steps' must be -32767 to 32767"));
                    }
                    let value = ((dial as i128) << 16) | ((steps as i16 as u16) as i128);
                    Ok(Plan::Ops(vec![self.control(0xD30B, value)?]))
                }
                "camera_lever" => {
                    let text = params
                        .get("lever")
                        .and_then(Value::as_str)
                        .unwrap_or("tele_wide");
                    let lever = code_of(LEVERS, text)
                        .ok_or_else(|| invalid(format!("unknown lever '{text}'")))?;
                    let v = param_int(params, "value")?;
                    if let Some(PropInfo {
                        form: Form::Range { min, max, .. },
                        ..
                    }) = self.props.get(&0xD2BD)
                    {
                        if let (Some(lo), Some(hi)) = (min.as_int(), max.as_int()) {
                            if v < lo || v > hi {
                                return Err(invalid(format!("the lever takes {lo} to {hi}")));
                            }
                        }
                    }
                    let value = ((lever as i128) << 16) | ((v as i16 as u16) as i128);
                    Ok(Plan::Ops(vec![self.control(0xD30C, value)?]))
                }
                "pixel_shift_mode" => {
                    let on = param_bool(params, "enabled")
                        .ok_or_else(|| invalid("'enabled' is required"))?;
                    Ok(Plan::Ops(vec![
                        self.control(0xD2D4, if on { DOWN } else { UP })?
                    ]))
                }
                "pixel_shift_cancel" => Ok(Plan::Ops(vec![self.control(0xD2D3, UP)?])),
                "delete_user_base_look" => {
                    let value = if param_bool(params, "all").unwrap_or(false) {
                        PtpValue::Int(0xFFFF)
                    } else {
                        self.user_base_look(0xD0C7, user("user")?)
                    };
                    Ok(Plan::Ops(vec![self.set_prop(0xD0C7, value)?]))
                }
                "select_user_base_look" => {
                    let value = self.user_base_look(0xD0C8, user("user")?);
                    Ok(Plan::Ops(vec![self.set_prop(0xD0C8, value)?]))
                }
                "set_pplut_base_look" => {
                    let value = self.user_base_look(0xD0CC, user("user")?);
                    Ok(Plan::Ops(vec![self.set_prop(0xD0CC, value)?]))
                }
                _ => Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            }
        })();
        match result {
            Err(CommandError::UnknownCommand { .. }) => None,
            other => Some(other),
        }
    }

    /// Reads the camera's descriptive lists into the state once connected.
    pub(super) fn queue_info_reads(&mut self) {
        let ops = self.operations.clone();
        let mut queue = |code: u16, params: Vec<u32>, info: Info| {
            if ops.contains(&code) {
                let op = self.info_op(code, params, info, None);
                self.background.push_back(op);
            }
        };
        queue(OP_GET_DISPLAY_STRING_LIST, vec![0], Info::DisplayLists);
        queue(
            OP_OPERATION_RESULTS_SUPPORTED,
            vec![],
            Info::ResultsSupported,
        );
        queue(OP_GET_LICENSE_INFO_LIST, vec![], Info::Licenses);
        queue(OP_GET_PRESET_INFO_LIST, vec![], Info::Presets);
        queue(OP_GET_AREA_TIME_ZONE, vec![], Info::Clock);
        if self
            .props
            .get(&0xE086)
            .is_some_and(|p| p.current == PtpValue::Int(1))
        {
            self.queue_info(OP_GET_LENS_INFORMATION, vec![2], Info::Lens);
        }
        if self
            .props
            .get(&0xD264)
            .is_some_and(|p| p.current == PtpValue::Int(1))
        {
            let flag = self.flag();
            self.queue_info(OP_GET_RESTRICTION_INFO, vec![flag], Info::Restrictions);
        }
    }

    fn queue_info(&mut self, code: u16, params: Vec<u32>, info: Info) {
        let queued = self
            .background
            .iter()
            .any(|op| op.step == Step::Info(info) && op.command.is_none());
        if !queued && self.operations.contains(&code) {
            let op = self.info_op(code, params, info, None);
            self.background.push_back(op);
        }
    }

    /// An information read was answered.
    pub(super) fn info_done(
        &mut self,
        cx: &mut Cx,
        info: Info,
        op: &Op,
        code: u16,
        params: &[u32],
        data: &[u8],
    ) {
        if info == Info::StreamSet {
            let Some(id) = op.command else { return };
            let result = if code != RC_OK {
                Err(rejected(code))
            } else {
                stream_result(params.first().copied().unwrap_or(0))
                    .map(|()| Outcome::Ack)
                    .map_err(|e| refused("stream_settings", e))
            };
            if result.is_ok() {
                self.queue_info(OP_GET_STREAM_SETTING_LIST, vec![], Info::Streams);
            }
            cx.complete(id, result);
            return;
        }
        if code != RC_OK {
            if info == Info::Restrictions {
                // Not restricted: the camera answers with an error.
                cx.state(json!({"restrictions": null}));
            }
            if let Some(id) = op.command {
                cx.complete(id, Err(rejected(code)));
            }
            return;
        }
        let value = self.info_value(cx, info, params, data);
        if let Some(id) = op.command {
            cx.complete(
                id,
                value
                    .map(|value| Outcome::Value { value })
                    .map_err(|e| refused("unreadable", e)),
            );
        } else if let Err(e) = value {
            cx.log(
                Level::Warning,
                format!("unreadable camera information: {e}"),
            );
        }
    }

    fn info_value(
        &mut self,
        cx: &mut Cx,
        info: Info,
        params: &[u32],
        data: &[u8],
    ) -> Result<Value, String> {
        Ok(match info {
            Info::DisplayLists => {
                let lists = parse_display_lists(data)?;
                let mut patch = json!({});
                let mut value = Map::new();
                for (t, names) in lists {
                    let name = list_type_name(t);
                    self.display_lists.insert(t, names.clone());
                    put(
                        &mut patch,
                        &format!("display_lists.{name}"),
                        Value::Object(names.clone()),
                    );
                    value.insert(name, Value::Object(names));
                }
                if let Some(version) = params.first() {
                    put(&mut patch, "session.display_list_version", json!(version));
                }
                cx.state(patch);
                self.refresh_labels(cx);
                Value::Object(value)
            }
            Info::Lens => {
                let v = parse_lens_information(data)?;
                cx.state(json!({"lens": {"focus_tables": v["tables"]}}));
                v
            }
            Info::Presets => {
                let presets = parse_preset_info(data)?;
                let mut patch = json!({});
                for p in &presets {
                    put(
                        &mut patch,
                        &format!("zoom_focus_presets.{}", p["slot"]),
                        p.clone(),
                    );
                }
                cx.state(patch);
                Value::Array(presets)
            }
            Info::Osd => {
                let (png, meta) = parse_osd(data)?;
                let mut v =
                    json!({"format": "png", "bytes": png.len(), "image": b64(&png), "meta": meta});
                if let Some((w, h)) = png_size(&png) {
                    v["width"] = json!(w);
                    v["height"] = json!(h);
                }
                v
            }
            Info::Restrictions => {
                let v = parse_restrictions(data)?;
                cx.state(json!({"restrictions": v.clone()}));
                v
            }
            Info::Licenses => {
                let v = parse_licenses(data)?;
                cx.state(json!({"licenses": v.clone()}));
                v
            }
            Info::ResultsSupported => {
                let v = parse_results_supported(data)?;
                cx.state(json!({"operation": {"results_supported": v.clone()}}));
                v
            }
            Info::Clock => {
                let v = parse_clock(data)?;
                cx.state(json!({"clock": v.clone()}));
                v
            }
            Info::Streams => {
                let (version, streams) = parse_streams(data)?;
                self.stream_version = version;
                let mut patch = json!({});
                for s in &streams {
                    put(
                        &mut patch,
                        &format!("streaming.settings.{}", s["stream"]),
                        s.clone(),
                    );
                }
                cx.state(patch);
                json!({"version": version, "streams": streams})
            }
            Info::StreamSet => Value::Null,
        })
    }

    /// Names for property values from the camera's display lists.
    pub(super) fn refresh_labels(&mut self, cx: &mut Cx) {
        let mut patch = json!({});
        let mut changed = false;
        for &(code, list, path) in LABELLED {
            let Some(names) = self.display_lists.get(&list) else {
                continue;
            };
            let Some(info) = self.props.get(&code) else {
                continue;
            };
            let key = match info.current.to_json() {
                Value::String(s) => s,
                other => other.to_string(),
            };
            let name = names.get(&key).cloned().unwrap_or(Value::Null);
            if self.reported_named.get(path) != Some(&name) {
                put(&mut patch, path, name.clone());
                self.reported_named.insert(path, name);
                changed = true;
            }
        }
        if changed {
            cx.state(patch);
        }
    }

    /// The physical controls the camera lets the button, dial and lever
    /// commands use, from its capability properties.
    pub(super) fn capabilities(&mut self, cx: &mut Cx, info: &PropInfo) {
        let (path, table): (&'static str, &[(&str, u16)]) = match info.code {
            0xD208 => ("camera_controls.buttons", BUTTONS),
            0xD209 => ("camera_controls.simultaneous_buttons", BUTTONS),
            0xD20A => ("camera_controls.dials", DIALS),
            0xD20B => ("camera_controls.levers", LEVERS),
            0xD2BD => {
                if let Form::Range { min, max, .. } = &info.form {
                    let v = json!({"min": min.to_json(), "max": max.to_json()});
                    if self.reported_named.get("camera_controls.lever_range") != Some(&v) {
                        cx.state(json!({"camera_controls": {"lever_range": v.clone()}}));
                        self.reported_named.insert("camera_controls.lever_range", v);
                    }
                }
                return;
            }
            _ => return,
        };
        let values = match &info.form {
            Form::Enum { values, settable } => {
                if values.is_empty() {
                    settable
                } else {
                    values
                }
            }
            _ => return,
        };
        let names: Vec<Value> = values
            .iter()
            .filter_map(PtpValue::as_int)
            .map(|v| json!(name_of(table, v as u16)))
            .collect();
        let v = Value::Array(names);
        if self.reported_named.get(path) != Some(&v) {
            let mut patch = json!({});
            put(&mut patch, path, v.clone());
            cx.state(patch);
            self.reported_named.insert(path, v);
        }
    }

    /// A property changed that decides what else is read.
    pub(super) fn info_property(&mut self, cx: &mut Cx, code: u16, value: &PtpValue) {
        match code {
            // Remote restriction began or ended.
            0xD264 if *value == PtpValue::Int(1) => {
                let flag = self.flag();
                self.queue_info(OP_GET_RESTRICTION_INFO, vec![flag], Info::Restrictions);
            }
            0xD264 => cx.state(json!({"restrictions": null})),
            // Lens information became available.
            0xE086 if *value == PtpValue::Int(1) => {
                self.queue_info(OP_GET_LENS_INFORMATION, vec![2], Info::Lens)
            }
            _ => {}
        }
    }

    /// The events this part reports.
    pub(super) fn info_event(&mut self, cx: &mut Cx, code: u16, p: &dyn Fn(usize) -> u32) {
        let now = cx.now();
        match code {
            0xC20F => {
                let t = p(0);
                self.queue_info(OP_GET_DISPLAY_STRING_LIST, vec![t], Info::DisplayLists);
            }
            0xC21B => {
                cx.state(json!({"lens": {"information_changed_at": now}}));
                if self
                    .props
                    .get(&0xE086)
                    .is_some_and(|i| i.current == PtpValue::Int(1))
                {
                    self.queue_info(OP_GET_LENS_INFORMATION, vec![2], Info::Lens);
                }
            }
            0xC226 => self.queue_info(OP_GET_PRESET_INFO_LIST, vec![], Info::Presets),
            0xC205 => {
                cx.state(json!({"clock": {"last_result": clock_result_name(p(0))}}));
                self.queue_info(OP_GET_AREA_TIME_ZONE, vec![], Info::Clock);
            }
            0xC206 => {
                self.captures += 1;
                cx.state(json!({"still": {"captures": self.captures, "last_capture_at": now}}));
            }
            0xC201 => cx.state(json!({"still": {"last_object_added": format!("{:08X}", p(0))}})),
            0xC202 => cx.state(json!({"still": {"last_object_removed": format!("{:08X}", p(0))}})),
            0xC228 => {
                self.cautions += 1;
                cx.state(json!({"status": {"cautions": self.cautions, "last_caution_at": now}}));
                cx.log(Level::Info, "the camera is showing a caution");
            }
            0x4004 | 0x4005 => {
                let mut patch = json!({});
                put(
                    &mut patch,
                    &format!("content.storages.{:08X}", p(0)),
                    json!(code == 0x4004),
                );
                cx.state(patch);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
pub(super) mod build {
    //! Test datasets built from the layouts.
    use super::*;

    pub(crate) fn display_lists(lists: &[(u32, &[(u16, &str)])]) -> Vec<u8> {
        let mut b = (lists.len() as u16).to_le_bytes().to_vec();
        b.extend_from_slice(&[0, 0]);
        for (t, entries) in lists {
            b.extend_from_slice(&t.to_le_bytes());
            b.extend_from_slice(&0x0004u16.to_le_bytes());
            b.extend_from_slice(&(entries.len() as u16).to_le_bytes());
            for (v, name) in *entries {
                b.extend_from_slice(&v.to_le_bytes());
                let n = text_with_nul(name);
                b.extend_from_slice(&(n.len() as u16).to_le_bytes());
                b.extend_from_slice(&n);
            }
        }
        wrap(&b)
    }

    pub(crate) fn osd(png: &[u8]) -> Vec<u8> {
        let mut meta = 100u16.to_le_bytes().to_vec();
        meta.extend_from_slice(&[0, 0]);
        meta.extend_from_slice(&1u32.to_le_bytes());
        for v in [1920u32, 1080, 960, 540, 640, 360] {
            meta.extend_from_slice(&v.to_le_bytes());
        }
        let image_at = 16u32;
        let meta_at = image_at + png.len() as u32;
        let mut b = image_at.to_le_bytes().to_vec();
        b.extend_from_slice(&(png.len() as u32).to_le_bytes());
        b.extend_from_slice(&meta_at.to_le_bytes());
        b.extend_from_slice(&(meta.len() as u32).to_le_bytes());
        b.extend_from_slice(png);
        b.extend(meta);
        b
    }

    pub(crate) fn presets() -> Vec<u8> {
        let mut b = 100u16.to_le_bytes().to_vec();
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&1u16.to_le_bytes());
        b.push(2);
        ds::write_string(&mut b, "FE 24-70mm");
        b.extend_from_slice(&35_000u32.to_le_bytes());
        b.extend_from_slice(&u32::MAX.to_le_bytes());
        b.push(2);
        b.push(1);
        wrap(&b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_lists_name_values() {
        let lists = parse_display_lists(&build::display_lists(&[(
            3,
            &[(0x0101, "User 1"), (1, "s709")],
        )]))
        .unwrap();
        assert_eq!(lists[0].0, 3);
        assert_eq!(lists[0].1["257"], "User 1");
        assert_eq!(list_type_name(3), "base_look_name");
        assert_eq!(list_type_name(0x99), "type_153");
    }

    #[test]
    fn presets_osd_and_clock() {
        let p = parse_preset_info(&build::presets()).unwrap();
        assert_eq!(p[0]["slot"], 0);
        assert_eq!(p[0]["zoom_mm"], 35.0);
        assert_eq!(p[0]["focus_m"], Value::Null);
        assert_eq!(p[0]["zoom_only_available"], true);
        assert_eq!(p[0]["zoom_only"], false);
        let (png, meta) = parse_osd(&build::osd(b"\x89PNG")).unwrap();
        assert_eq!(png, b"\x89PNG");
        assert_eq!(meta["yuv_layout"]["width"], 640);
        let clock = encode_clock(Some("20260101T120000.0"), Some("+0100"), Some(true));
        assert_eq!(clock.len(), 4 + 1 + 18 + 1 + 6 + 1 + 1);
        let mut get = clock[..4].to_vec();
        get.extend_from_slice(&clock[5..23]);
        get.extend_from_slice(&clock[24..30]);
        get.push(1);
        assert_eq!(
            parse_clock(&get).unwrap(),
            json!({"date_time": "20260101T120000.0", "utc_offset": "+0100", "daylight_saving": true})
        );
    }

    #[test]
    fn stream_settings_round_trip_without_keys() {
        let write = StreamWrite {
            protocol: 2,
            url: "srt://10.0.0.5".into(),
            port: Some(9000),
            key: Some("secret".into()),
            latency_ms: Some(120),
            ttl: Some(64),
            cipher: Some((1, "k".into())),
            mode: Some(1),
        };
        let bytes = encode_stream(100, &write).unwrap();
        // A set list has no permission byte; a get list does. Build the get
        // form from the set form to read it back.
        let list = unwrap(&bytes).unwrap();
        let mut get = list[..9].to_vec();
        get.push(3);
        get.extend_from_slice(&list[9..]);
        let (_, streams) = parse_streams(&wrap(&get)).unwrap();
        let s = &streams[0];
        assert_eq!(s["protocol"], json!(["srt"]));
        assert_eq!(s["allowed_protocols"], json!(["rtmp", "srt"]));
        assert_eq!(s["url"], "srt://10.0.0.5");
        assert_eq!(s["port"], 9000);
        assert_eq!(s["key_set"], true);
        assert_eq!(s["latency_ms"], 120);
        assert_eq!(s["cipher_key_set"], true);
        assert!(!s.to_string().contains("secret"));
    }

    #[test]
    fn eframing_commands() {
        let none = encode_eframing(100, 1, false, None);
        assert_eq!(
            u32::from_le_bytes(none[..4].try_into().unwrap()) as usize,
            none.len() - 4
        );
        assert_eq!(&none[4..8], &100u32.to_le_bytes());
        assert_eq!(none[24], 1);
        let single = encode_eframing(100, 3, false, Some([0.25, 0.25, 0.5, 0.5]));
        assert_eq!(single.len(), none.len() + 56);
        // The rectangle, in 1024ths.
        assert_eq!(&single[52..56], &256i32.to_le_bytes());
        assert_eq!(&single[60..64], &512i32.to_le_bytes());
    }

    #[test]
    fn licences_and_restrictions() {
        let mut b = 100u16.to_le_bytes().to_vec();
        b.extend_from_slice(&[0, 0, 1, 0, 0, 0]);
        let mut e = u32::MAX.to_le_bytes().to_vec();
        e.extend_from_slice(&0u32.to_le_bytes());
        e.push(5);
        e.extend_from_slice(b"2566\0");
        e.push(3);
        e.extend_from_slice(b"AB\0");
        b.extend_from_slice(&((e.len() + 4) as u32).to_le_bytes());
        b.extend(e);
        let l = parse_licenses(&b).unwrap();
        assert_eq!(
            l[0],
            json!({"license_id": "2566", "install_key_id": "AB", "remaining_hours": null})
        );
        let mut r = Vec::new();
        for list in [&[0x9207u16][..], &[0x5005], &[]] {
            r.extend_from_slice(&(list.len() as u32).to_le_bytes());
            for c in list {
                r.extend_from_slice(&c.to_le_bytes());
            }
        }
        assert_eq!(
            parse_restrictions(&r).unwrap(),
            json!({"operations": ["9207"], "properties": ["5005"], "controls": []})
        );
    }
}
