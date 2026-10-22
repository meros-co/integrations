//! NEC (Sharp NEC) large-format displays: the "External Control" protocol
//! over TCP 7142 or an RS-232C port of the host. This is the displays'
//! protocol, not the projectors'.
//!
//! Written from NEC's "External Control" documents for the M series
//! (M751/M861/M981, Rev.1.1), the P404/V404 generation (Rev.1.0) and the
//! P403 and P402 generations (Rev.2.4 and Rev.3.8); see the spec's sources.
//!
//! - A message is SOH, a reserved '0', the destination (monitor ID n as the
//!   byte 40h + n), the source ('0' from the controller), the message type
//!   ('A' command, 'C' get, 'E' set; the display answers 'B', 'D' and 'F'),
//!   the length of the message from STX to ETX as two hex digits, STX, the
//!   message in hex digits, ETX, a check code (the XOR of every byte after
//!   SOH up to ETX) and CR (§4). A reply puts '0' as its destination and the
//!   display's own ID as its source.
//! - A get is the operation code page and code, answered by the result (00,
//!   or 01 unsupported), page, code, type, the maximum and the current value,
//!   four hex digits each; a set adds the value and is answered the same way
//!   with the value asked for (§5). An out-of-range value is ignored with
//!   result 00 on some models, so every set is followed by its get.
//! - A message the display cannot take now (busy switching power or input,
//!   a bad check code) is answered by the NULL message, BE.
//! - The controller waits for each reply, then 15 s after power on or off and
//!   10 s after an input change, auto setup or factory reset, before the next
//!   message (§3.3); the oldest generation also needs `packet_interval_ms`
//!   (600 ms) between messages. The display drops a LAN connection silent for
//!   15 minutes; the reads below keep it busy.
//! - Nothing is pushed: power, input, volume and mute are read every
//!   `poll_ms`, every other setting, the temperature sensors and the
//!   self-diagnosis once a minute, and identity on connecting.
//!
//! Opened for commands only (`monitor` false), nothing is read beyond what
//! commands ask and their read-back, except the power status every 10 s
//! while idle, which also keeps the LAN connection open. Every answered
//! request reports its round trip.

use std::collections::VecDeque;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const SOCKET: Key = "nec";
const REPLY_TIMER: Key = "reply";
const HOLD_TIMER: Key = "hold";
const FAST_TIMER: Key = "poll-fast";
const SLOW_TIMER: Key = "poll-slow";
const PROBE_TIMER: Key = "probe";
const RETRY_TIMER: Key = "retry";

pub(crate) const DEFAULT_PORT: u16 = 7142;

const SOH: u8 = 0x01;
const STX: u8 = 0x02;
const ETX: u8 = 0x03;
const CR: u8 = 0x0D;

/// NEC sets no reply timeout for the controller; the display itself gives up
/// on a message after 10 s.
const REPLY_TIMEOUT: Millis = 5_000;
const MISSES: u32 = 3;
const PROBE_EVERY: Millis = 10_000;
const SLOW_EVERY: Millis = 60_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// After power on or off (§3.3).
const POWER_HOLD: Millis = 15_000;
/// After an input change, auto setup or factory reset (§3.3).
const SWITCH_HOLD: Millis = 10_000;

fn hex2(b: u8) -> [u8; 2] {
    let d = |n: u8| if n < 10 { b'0' + n } else { b'A' + n - 10 };
    [d(b >> 4), d(b & 0xF)]
}

fn hex_text(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// Frame a message of `kind` ('A', 'C' or 'E') whose body is hex text.
pub(crate) fn frame(monitor: u8, kind: u8, body: &str) -> Vec<u8> {
    let mut out = vec![SOH, b'0', 0x40 + monitor, b'0', kind];
    out.extend(hex2((body.len() + 2) as u8));
    out.push(STX);
    out.extend(body.as_bytes());
    out.push(ETX);
    let bcc = out[1..].iter().fold(0, |a, b| a ^ b);
    out.push(bcc);
    out.push(CR);
    out
}

/// A reply read off the stream.
#[derive(Debug, Clone, PartialEq)]
struct Reply {
    /// The display's monitor ID byte (40h + n).
    source: u8,
    kind: u8,
    body: String,
}

#[derive(Debug, Default)]
struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Reply> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            let Some(start) = self.buf.iter().position(|&b| b == SOH) else {
                self.buf.clear();
                break;
            };
            self.buf.drain(..start);
            if self.buf.len() < 7 {
                break;
            }
            let len = std::str::from_utf8(&self.buf[5..7])
                .ok()
                .and_then(|s| usize::from_str_radix(s, 16).ok());
            let Some(len) = len.filter(|l| *l >= 2) else {
                self.buf.remove(0);
                continue;
            };
            let total = 7 + len + 2;
            if self.buf.len() < total {
                break;
            }
            let etx = 7 + len - 1;
            let bcc = self.buf[1..=etx].iter().fold(0, |a, b| a ^ b);
            if self.buf[7] != STX || self.buf[etx] != ETX || self.buf[etx + 1] != bcc {
                self.buf.remove(0);
                continue;
            }
            out.push(Reply {
                source: self.buf[3],
                kind: self.buf[4],
                body: String::from_utf8_lossy(&self.buf[8..etx]).into_owned(),
            });
            // The CR after the check code, when it is there.
            let end = if self.buf.get(etx + 2) == Some(&CR) {
                etx + 3
            } else {
                etx + 2
            };
            self.buf.drain(..end.min(self.buf.len()));
        }
        out
    }
}

/// One message of a job.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// Get an operation code (page, code).
    Get(u8, u8),
    /// Set an operation code to a value.
    Set(u8, u8, u16),
    /// A command message ('A'), its body as hex text; its reply begins
    /// with `reply`.
    Command { body: String, reply: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Hold {
    None,
    Power,
    Switch,
}

#[derive(Debug)]
struct Job {
    id: Option<CommandId>,
    steps: VecDeque<Step>,
    returns_value: bool,
    /// get_vcp and set_vcp: the value returned is the raw maximum and value.
    raw: bool,
    hold: Hold,
}

impl Job {
    fn internal(steps: Vec<Step>) -> Job {
        Job {
            id: None,
            steps: steps.into(),
            returns_value: false,
            raw: false,
            hold: Hold::None,
        }
    }
}

// ── Operation codes ────────────────────────────────────────────────────────

/// Input terminals, operation code 00-60 (and the picture inputs).
const INPUTS: &[(&str, u16)] = &[
    ("vga", 0x01),
    ("rgbhv", 0x02),
    ("dvi", 0x03),
    ("hdmi_set_only", 0x04),
    ("video1", 0x05),
    ("video2", 0x06),
    ("s_video", 0x07),
    ("tv", 0x0A),
    ("ypbpr", 0x0C),
    ("option", 0x0D),
    ("ypbpr2", 0x0E),
    ("displayport1", 0x0F),
    ("displayport2", 0x10),
    ("hdmi1", 0x11),
    ("hdmi2", 0x12),
    ("displayport3", 0x80),
    ("hdmi3", 0x82),
    ("media_player", 0x87),
    ("compute_module", 0x88),
    ("usb_c", 0x89),
];
const ASPECT: &[(&str, u16)] = &[
    ("normal", 1),
    ("full", 2),
    ("wide", 3),
    ("zoom", 4),
    ("trim", 5),
    ("dynamic", 6),
    ("dot_by_dot", 7),
    ("cinema", 10),
    ("auto", 11),
];
const FLIP: &[(&str, u16)] = &[
    ("none", 1),
    ("horizontal", 2),
    ("vertical", 3),
    ("rotate_180", 4),
];
const STEREO: &[(&str, u16)] = &[("mono", 1), ("stereo", 2)];
const LINE_OUT: &[(&str, u16)] = &[("fixed", 1), ("variable", 2)];
const MULTI_PICTURE: &[(&str, u16)] = &[("off", 1), ("pip2", 2), ("pbp2", 3), ("pbp4", 4)];
const FAN_SPEED: &[(&str, u16)] = &[("high", 1), ("low", 2)];
const INPUT_DETECT: &[(&str, u16)] = &[
    ("first", 0),
    ("last", 1),
    ("none", 2),
    ("video", 3),
    ("custom", 4),
];
const VIDEO_RANGE: &[(&str, u16)] = &[("expand", 1), ("raw", 2), ("auto", 3)];
const IR_LOCK: &[(&str, u16)] = &[("unlocked", 1), ("all", 2), ("custom", 3)];
const TILE: &[(&str, u16)] = &[("disabled", 1), ("enabled", 2), ("disabled_erase_frame", 3)];

#[derive(Clone, Copy)]
enum Kind {
    Int,
    Table(&'static [(&'static str, u16)]),
    /// A boolean: these values for true and false.
    Bool(u16, u16),
}

/// A named operation code: the command key (set_<key>, get_<key>), the
/// state path, page and code, its kind, and whether it can be set.
struct Op {
    key: &'static str,
    path: &'static str,
    page: u8,
    code: u8,
    kind: Kind,
    writable: bool,
    /// Read every `poll_ms` rather than every minute.
    fast: bool,
}

const fn op(key: &'static str, path: &'static str, page: u8, code: u8, kind: Kind) -> Op {
    Op {
        key,
        path,
        page,
        code,
        kind,
        writable: true,
        fast: false,
    }
}

const fn fast(o: Op) -> Op {
    Op { fast: true, ..o }
}

const fn read_only(o: Op) -> Op {
    Op {
        writable: false,
        ..o
    }
}

const OPS: &[Op] = &[
    fast(op("input", "input", 0x00, 0x60, Kind::Table(INPUTS))),
    op("backlight", "picture.backlight", 0x00, 0x10, Kind::Int),
    op("contrast", "picture.contrast", 0x00, 0x12, Kind::Int),
    op("black_level", "picture.black_level", 0x00, 0x92, Kind::Int),
    op("sharpness", "picture.sharpness", 0x00, 0x8C, Kind::Int),
    op("tint", "picture.tint", 0x00, 0x90, Kind::Int),
    op("color", "picture.color", 0x02, 0x1F, Kind::Int),
    op(
        "color_temperature",
        "picture.color_temperature",
        0x00,
        0x54,
        Kind::Int,
    ),
    op(
        "color_preset",
        "picture.color_preset",
        0x00,
        0x14,
        Kind::Int,
    ),
    op("red_gain", "picture.red_gain", 0x00, 0x16, Kind::Int),
    op("green_gain", "picture.green_gain", 0x00, 0x18, Kind::Int),
    op("blue_gain", "picture.blue_gain", 0x00, 0x1A, Kind::Int),
    op("picture_mode", "picture.mode", 0x02, 0x1A, Kind::Int),
    op("gamma", "picture.gamma", 0x02, 0x68, Kind::Int),
    op(
        "adaptive_contrast",
        "picture.adaptive_contrast",
        0x02,
        0x8D,
        Kind::Int,
    ),
    op(
        "noise_reduction",
        "picture.noise_reduction",
        0x02,
        0x26,
        Kind::Int,
    ),
    op("aspect", "picture.aspect", 0x02, 0x70, Kind::Table(ASPECT)),
    op("overscan", "picture.overscan", 0x02, 0xE3, Kind::Int),
    op(
        "image_flip",
        "picture.image_flip",
        0x02,
        0xD7,
        Kind::Table(FLIP),
    ),
    op(
        "auto_dimming",
        "picture.auto_dimming",
        0x02,
        0x2D,
        Kind::Int,
    ),
    op(
        "ambient_light_sensing",
        "picture.ambient_light_sensing",
        0x10,
        0xC8,
        Kind::Int,
    ),
    op(
        "video_range",
        "picture.video_range",
        0x10,
        0x40,
        Kind::Table(VIDEO_RANGE),
    ),
    fast(op("volume", "audio.volume", 0x00, 0x62, Kind::Int)),
    fast(op("mute", "audio.muted", 0x00, 0x8D, Kind::Bool(1, 2))),
    op(
        "screen_mute",
        "picture.screen_muted",
        0x10,
        0xB6,
        Kind::Bool(1, 2),
    ),
    op("balance", "audio.balance", 0x00, 0x93, Kind::Int),
    op("treble", "audio.treble", 0x00, 0x8F, Kind::Int),
    op("bass", "audio.bass", 0x00, 0x91, Kind::Int),
    op("stereo", "audio.stereo", 0x00, 0x94, Kind::Table(STEREO)),
    op("audio_input", "audio.input", 0x02, 0x2E, Kind::Int),
    op(
        "line_out",
        "audio.line_out",
        0x10,
        0x81,
        Kind::Table(LINE_OUT),
    ),
    op("pip_mode", "pip.mode", 0x02, 0x72, Kind::Int),
    op("pip_input", "pip.input", 0x02, 0x73, Kind::Table(INPUTS)),
    op("pip_size", "pip.size", 0x10, 0xB9, Kind::Int),
    op("pip_x", "pip.x", 0x02, 0x74, Kind::Int),
    op("pip_y", "pip.y", 0x02, 0x75, Kind::Int),
    op(
        "multi_picture_mode",
        "pip.multi_picture_mode",
        0x11,
        0xEB,
        Kind::Table(MULTI_PICTURE),
    ),
    op(
        "picture1_input",
        "pip.pictures.1",
        0x11,
        0x0E,
        Kind::Table(INPUTS),
    ),
    op(
        "picture2_input",
        "pip.pictures.2",
        0x11,
        0x0F,
        Kind::Table(INPUTS),
    ),
    op(
        "picture3_input",
        "pip.pictures.3",
        0x11,
        0x10,
        Kind::Table(INPUTS),
    ),
    op(
        "picture4_input",
        "pip.pictures.4",
        0x11,
        0x11,
        Kind::Table(INPUTS),
    ),
    op("tile_columns", "tile.columns", 0x02, 0xD0, Kind::Int),
    op("tile_rows", "tile.rows", 0x02, 0xD1, Kind::Int),
    op("tile_position", "tile.position", 0x02, 0xD2, Kind::Int),
    op("tile_matrix", "tile.matrix", 0x02, 0xD3, Kind::Table(TILE)),
    op(
        "tile_compensation",
        "tile.compensation",
        0x02,
        0xD5,
        Kind::Bool(2, 1),
    ),
    op("power_on_delay", "power_on_delay", 0x02, 0xD8, Kind::Int),
    op("osd_language", "osd.language", 0x00, 0x68, Kind::Int),
    op("osd_time", "osd.time", 0x00, 0xFC, Kind::Int),
    op("information_osd", "osd.information", 0x02, 0x3D, Kind::Int),
    op("osd_rotation", "osd.rotation", 0x02, 0x41, Kind::Int),
    op("power_led", "power_led", 0x02, 0xBE, Kind::Bool(1, 2)),
    op("monitor_id", "ids.monitor", 0x02, 0x3E, Kind::Int),
    op("group_ids", "ids.groups", 0x10, 0x7F, Kind::Int),
    op("off_timer", "off_timer", 0x02, 0x2B, Kind::Int),
    op("quick_start", "quick_start", 0x11, 0xEA, Kind::Bool(2, 1)),
    op("fan_control", "fan.control", 0x02, 0x7D, Kind::Int),
    op("fan_speed", "fan.speed", 0x10, 0x3F, Kind::Table(FAN_SPEED)),
    op(
        "input_detect",
        "input_detect",
        0x02,
        0x40,
        Kind::Table(INPUT_DETECT),
    ),
    op(
        "screen_saver_motion",
        "screen_saver.motion",
        0x02,
        0xDD,
        Kind::Int,
    ),
    op(
        "side_border_color",
        "side_border_color",
        0x02,
        0xDF,
        Kind::Int,
    ),
    op("ir_lock", "locks.ir", 0x10, 0xD4, Kind::Table(IR_LOCK)),
    op("key_lock", "locks.keys", 0x11, 0x6A, Kind::Int),
    op("cec", "cec", 0x11, 0x76, Kind::Int),
    op("human_sensing", "human_sensing", 0x10, 0x75, Kind::Int),
    read_only(op(
        "illuminance",
        "status.illuminance",
        0x02,
        0xB4,
        Kind::Int,
    )),
];

/// Factory and partial resets, operation code 02-CB.
const RESETS: &[(&str, u16)] = &[
    ("factory", 1),
    ("picture", 2),
    ("adjust", 3),
    ("audio", 4),
    ("schedule", 5),
    ("pip", 6),
    ("osd", 7),
    ("multi_display", 8),
    ("protection", 9),
    ("advanced", 10),
    ("advanced2", 11),
    ("control", 12),
    ("remote_control", 13),
    ("slot", 15),
    ("network", 16),
    ("protect", 17),
    ("system", 18),
    ("input", 19),
];

const DIAGNOSIS: &[(u8, &str)] = &[
    (0x00, "normal"),
    (0x70, "standby_power_3v3"),
    (0x71, "standby_power_5v"),
    (0x72, "panel_power_12v"),
    (0x78, "converter_power_24v"),
    (0x80, "fan1"),
    (0x81, "fan2"),
    (0x82, "fan3"),
    (0x91, "led_backlight"),
    (0xA0, "temperature_shutdown"),
    (0xA1, "temperature_half_brightness"),
    (0xA2, "temperature_user_limit"),
    (0xB0, "no_signal"),
    (0xD0, "proof_of_play_buffer"),
    (0xE0, "system_error"),
    (0xE5, "hdmi_switch_error"),
    (0xE7, "displayport_error"),
];

fn bad(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn int(params: &Params, name: &str) -> Result<i64, CommandError> {
    params
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

fn word(params: &Params, name: &str) -> Result<u16, CommandError> {
    u16::try_from(int(params, name)?).map_err(|_| bad(format!("'{name}' is out of range")))
}

fn byte(params: &Params, name: &str) -> Result<u8, CommandError> {
    u8::try_from(int(params, name)?).map_err(|_| bad(format!("'{name}' is out of range")))
}

fn value_of(o: &Op, params: &Params) -> Result<u16, CommandError> {
    match o.kind {
        Kind::Int => word(params, "value"),
        Kind::Table(t) => {
            let name = params
                .get("value")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("'value' is required"))?;
            t.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| *v)
                .ok_or_else(|| bad(format!("'{name}' is not a value of {}", o.key)))
        }
        Kind::Bool(on, off) => {
            let b = params
                .get("value")
                .and_then(Value::as_bool)
                .ok_or_else(|| bad("'value' is required"))?;
            Ok(if b { on } else { off })
        }
    }
}

fn shown(o: &Op, v: u16) -> Value {
    match o.kind {
        Kind::Int => json!(v),
        Kind::Table(t) => t
            .iter()
            .find(|(_, c)| *c == v)
            .map_or(Value::Null, |(n, _)| json!(n)),
        Kind::Bool(on, _) if v == on => json!(true),
        Kind::Bool(_, off) if v == off => json!(false),
        Kind::Bool(..) => Value::Null,
    }
}

/// A dotted state path holding one value, as a merge patch.
fn at_path(path: &str, value: Value) -> Value {
    path.rsplit('.').fold(value, |inner, part| {
        let mut m = Map::new();
        m.insert(part.to_string(), inner);
        Value::Object(m)
    })
}

/// Text sent as two hex digits per character (§11).
fn decode_hex_text(hex: &str) -> String {
    let bytes: Vec<u8> = (0..hex.len() / 2)
        .filter_map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok())
        .collect();
    String::from_utf8_lossy(&bytes)
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_string()
}

fn command(body: &str, reply: &str) -> Step {
    Step::Command {
        body: body.into(),
        reply: reply.into(),
    }
}

fn power_status() -> Step {
    command("01D6", "0200D6")
}

/// What a command sends, whether its last reply is its value, and the wait
/// after it.
fn plan(name: &str, params: &Params) -> Result<(Vec<Step>, bool, Hold), CommandError> {
    if let Some(key) = name.strip_prefix("get_") {
        if let Some(o) = OPS.iter().find(|o| o.key == key) {
            return Ok((vec![Step::Get(o.page, o.code)], true, Hold::None));
        }
    }
    if let Some(key) = name.strip_prefix("set_") {
        if let Some(o) = OPS.iter().find(|o| o.key == key && o.writable) {
            let hold = if key == "input" {
                Hold::Switch
            } else {
                Hold::None
            };
            return Ok((
                vec![
                    Step::Set(o.page, o.code, value_of(o, params)?),
                    Step::Get(o.page, o.code),
                ],
                false,
                hold,
            ));
        }
    }
    let one = |s: Step| Ok((vec![s], true, Hold::None));
    match name {
        "get_power" => one(power_status()),
        "set_power" => {
            let on = params
                .get("on")
                .and_then(Value::as_bool)
                .ok_or_else(|| bad("'on' is required"))?;
            let mode = if on { "0001" } else { "0004" };
            Ok((
                vec![command(&format!("C203D6{mode}"), "00C203D6")],
                false,
                Hold::Power,
            ))
        }
        "save_settings" => Ok((vec![command("0C", "000C")], false, Hold::None)),
        "get_timing" => one(command("07", "4E")),
        "self_diagnosis" => one(command("B1", "A1")),
        "get_serial_number" => one(command("C216", "C316")),
        "get_model" => one(command("C217", "C317")),
        "get_firmware_version" => one(command("CA0200", "CB02")),
        "get_date" => one(command("C211", "C311")),
        "set_date" => {
            let year = int(params, "year")? - 2000;
            let body = format!(
                "C212{}{}{}{}{}{}{}",
                hex_text(&[year as u8]),
                hex_text(&[byte(params, "month")?]),
                hex_text(&[byte(params, "day")?]),
                hex_text(&[byte(params, "weekday")?]),
                hex_text(&[byte(params, "hour")?]),
                hex_text(&[byte(params, "minute")?]),
                hex_text(&[u8::from(
                    params
                        .get("daylight_saving")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                )]),
            );
            Ok((
                vec![command(&body, "C312"), command("C211", "C311")],
                false,
                Hold::None,
            ))
        }
        "get_temperature" => {
            let sensor = word(params, "sensor")?;
            Ok((
                vec![Step::Set(0x02, 0x78, sensor), Step::Get(0x02, 0x79)],
                true,
                Hold::None,
            ))
        }
        "get_fan_status" => {
            let fan = word(params, "fan")?;
            Ok((
                vec![Step::Set(0x02, 0x7A, fan), Step::Get(0x02, 0x7B)],
                true,
                Hold::None,
            ))
        }
        "auto_setup" => Ok((vec![Step::Set(0x00, 0x1E, 1)], false, Hold::Switch)),
        "reset" => {
            let target = params
                .get("target")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("'target' is required"))?;
            let v = RESETS
                .iter()
                .find(|(n, _)| *n == target)
                .map(|(_, v)| *v)
                .ok_or_else(|| bad(format!("'{target}' is not a reset")))?;
            let hold = if v == 1 { Hold::Switch } else { Hold::None };
            Ok((vec![Step::Set(0x02, 0xCB, v)], false, hold))
        }
        "get_vcp" => one(Step::Get(byte(params, "page")?, byte(params, "code")?)),
        "set_vcp" => Ok((
            vec![Step::Set(
                byte(params, "page")?,
                byte(params, "code")?,
                word(params, "value")?,
            )],
            true,
            Hold::None,
        )),
        other => Err(CommandError::UnknownCommand {
            command: other.into(),
        }),
    }
}

/// The reads of one poll round.
fn reads(fast_ones: bool) -> Vec<Vec<Step>> {
    let mut out: Vec<Vec<Step>> = Vec::new();
    if fast_ones {
        out.push(vec![power_status()]);
    }
    for o in OPS.iter().filter(|o| o.fast == fast_ones) {
        out.push(vec![Step::Get(o.page, o.code)]);
    }
    if !fast_ones {
        for sensor in 1..=3 {
            out.push(vec![Step::Set(0x02, 0x78, sensor), Step::Get(0x02, 0x79)]);
        }
        for fan in 1..=3 {
            out.push(vec![Step::Set(0x02, 0x7A, fan), Step::Get(0x02, 0x7B)]);
        }
        out.push(vec![command("B1", "A1")]);
        out.push(vec![command("07", "4E")]);
        out.push(vec![command("C211", "C311")]);
    }
    out
}

fn identity() -> Vec<Vec<Step>> {
    vec![
        vec![command("C217", "C317")],
        vec![command("C216", "C316")],
        vec![command("CA0200", "CB02")],
    ]
}

pub(crate) struct NecDisplay {
    link: Link,
    monitor: bool,
    monitor_id: u8,
    poll_ms: Millis,
    gap: Millis,
    open: bool,
    closed: bool,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    deframer: Deframer,
    queue: VecDeque<Job>,
    in_flight: Option<(Job, Step, Millis)>,
    /// Nothing is sent before this time (the waits of §3.3).
    hold_until: Millis,
    /// The temperature sensor and fan last selected.
    sensor: u16,
    fan: u16,
}

impl NecDisplay {
    pub(crate) fn new(ctx: OpenContext) -> Result<NecDisplay, String> {
        let s = &ctx.settings;
        let link = Link::from_settings(&ctx, DEFAULT_PORT)?;
        let num = |key: &str, default: u64| s.get(key).and_then(Value::as_u64).unwrap_or(default);
        Ok(NecDisplay {
            link,
            monitor: ctx.monitor,
            monitor_id: num("monitor_id", 1).clamp(1, 100) as u8,
            poll_ms: num("poll_ms", 5_000).max(1_000),
            gap: num("packet_interval_ms", 0).min(10_000),
            open: false,
            closed: false,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            deframer: Deframer::default(),
            queue: VecDeque::new(),
            in_flight: None,
            hold_until: 0,
            sensor: 0,
            fan: 0,
        })
    }

    fn queue_reads(&mut self, reads: Vec<Vec<Step>>) {
        for steps in reads {
            let first = steps.first().cloned();
            let queued = self
                .queue
                .iter()
                .any(|j| j.id.is_none() && j.steps.front() == first.as_ref())
                || self
                    .in_flight
                    .as_ref()
                    .is_some_and(|(j, s, _)| j.id.is_none() && Some(s) == first.as_ref());
            if !queued {
                self.queue.push_back(Job::internal(steps));
            }
        }
    }

    fn enqueue(&mut self, cx: &mut Cx, job: Job) {
        let at = self.queue.iter().position(|j| j.id.is_none());
        match at {
            Some(at) => self.queue.insert(at, job),
            None => self.queue.push_back(job),
        }
        self.pump(cx);
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || !self.open {
            return;
        }
        if cx.now() < self.hold_until {
            cx.set_timer(HOLD_TIMER, self.hold_until - cx.now());
            return;
        }
        let Some(mut job) = self.queue.pop_front() else {
            return;
        };
        let Some(step) = job.steps.pop_front() else {
            self.pump(cx);
            return;
        };
        let bytes = match &step {
            Step::Get(page, code) => frame(self.monitor_id, b'C', &hex_text(&[*page, *code])),
            Step::Set(page, code, value) => frame(
                self.monitor_id,
                b'E',
                &format!("{}{value:04X}", hex_text(&[*page, *code])),
            ),
            Step::Command { body, .. } => frame(self.monitor_id, b'A', body),
        };
        if let Step::Set(0x02, 0x78, v) = step {
            self.sensor = v;
        }
        if let Step::Set(0x02, 0x7A, v) = step {
            self.fan = v;
        }
        cx.tcp_send(SOCKET, bytes);
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        cx.set_timer(PROBE_TIMER, PROBE_EVERY);
        self.in_flight = Some((job, step, cx.now()));
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.misses = 0;
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
    }

    /// Whether `reply` answers `step`, and if so its result.
    fn answer(
        &self,
        step: &Step,
        reply: &Reply,
    ) -> Option<Result<(Value, Value, Option<Value>), CommandError>> {
        if reply.body == "BE" {
            return Some(Err(CommandError::DeviceError {
                code: Some("BE".into()),
                message: "the display is busy or could not read the message (NULL message)".into(),
            }));
        }
        match step {
            Step::Get(page, code) | Step::Set(page, code, _) => {
                let want = if matches!(step, Step::Get(..)) {
                    b'D'
                } else {
                    b'F'
                };
                let b = &reply.body;
                if reply.kind != want || b.len() < 16 || b[2..6] != hex_text(&[*page, *code]) {
                    return None;
                }
                if &b[0..2] != "00" {
                    return Some(Err(CommandError::DeviceError {
                        code: Some(b[0..2].to_string()),
                        message: "the display does not support this operation now (result 01)"
                            .into(),
                    }));
                }
                let max = u16::from_str_radix(&b[8..12], 16).ok()?;
                let value = u16::from_str_radix(&b[12..16], 16).ok()?;
                let (patch, shown) = self.vcp(*page, *code, value);
                Some(Ok((
                    patch,
                    shown,
                    Some(json!({"max": max, "value": value})),
                )))
            }
            Step::Command { reply: prefix, .. } => {
                if reply.kind != b'B' || !reply.body.starts_with(prefix.as_str()) {
                    return None;
                }
                Some(decode_command(prefix, &reply.body).map(|(p, v)| (p, v, None)))
            }
        }
    }

    /// An operation code's value as state, and as a get's result.
    fn vcp(&self, page: u8, code: u8, value: u16) -> (Value, Value) {
        let raw = json!(value);
        if let Some(o) = OPS.iter().find(|o| o.page == page && o.code == code) {
            let v = shown(o, value);
            if v.is_null() {
                return (json!({}), raw);
            }
            return (at_path(o.path, v.clone()), v);
        }
        match (page, code) {
            (0x02, 0x79) => {
                // Two's complement in 0.5 °C (§6.2).
                let celsius = f64::from(value as i16) / 2.0;
                (
                    json!({"status": {"temperatures": {self.sensor.to_string(): celsius}}}),
                    json!(celsius),
                )
            }
            (0x02, 0x7B) => {
                let status = match value {
                    0 => json!("off"),
                    1 => json!("on"),
                    2 => json!("error"),
                    _ => Value::Null,
                };
                (
                    json!({"status": {"fans": {self.fan.to_string(): status.clone()}}}),
                    status,
                )
            }
            _ => (json!({}), raw),
        }
    }

    fn received(&mut self, cx: &mut Cx, reply: Reply) {
        if reply.source != 0x40 + self.monitor_id {
            return;
        }
        let Some((_, step, _)) = &self.in_flight else {
            return;
        };
        let Some(result) = self.answer(step, &reply) else {
            return;
        };
        let (job, _, sent_at) = self.in_flight.take().unwrap();
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.heard(cx);
        self.hold_until = cx.now() + self.gap;
        match result {
            Err(e) => {
                if let Some(id) = job.id {
                    cx.complete(id, Err(e));
                }
            }
            Ok((patch, value, raw)) => {
                if patch.as_object().is_some_and(|m| !m.is_empty()) {
                    cx.state(patch);
                }
                if job.steps.is_empty() {
                    if let Some(id) = job.id {
                        let outcome = if job.raw {
                            Outcome::Value {
                                value: raw.unwrap_or(value),
                            }
                        } else if job.returns_value {
                            Outcome::Value { value }
                        } else {
                            Outcome::Ack
                        };
                        cx.complete(id, Ok(outcome));
                    }
                    let wait = match job.hold {
                        Hold::Power => POWER_HOLD,
                        Hold::Switch => SWITCH_HOLD,
                        Hold::None => 0,
                    };
                    self.hold_until = self.hold_until.max(cx.now() + wait);
                    if job.hold == Hold::Power {
                        // Read the new power state once the display settles.
                        self.queue.push_front(Job::internal(vec![power_status()]));
                    }
                } else {
                    self.queue.push_front(job);
                }
            }
        }
        self.pump(cx);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.open = false;
        self.closed = true;
        self.reachable = false;
        self.deframer = Deframer::default();
        for job in self
            .in_flight
            .take()
            .map(|(j, _, _)| j)
            .into_iter()
            .chain(self.queue.drain(..))
        {
            if let Some(id) = job.id {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        for key in [REPLY_TIMER, HOLD_TIMER, FAST_TIMER, SLOW_TIMER, PROBE_TIMER] {
            cx.cancel_timer(key);
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn open_link(&mut self, cx: &mut Cx) {
        self.closed = false;
        cx.connection(Connection::Connecting);
        self.link.open(cx, SOCKET);
    }
}

/// A command message's reply (§3, "Command" chapters) as state and result.
fn decode_command(prefix: &str, body: &str) -> Result<(Value, Value), CommandError> {
    let rest = &body[prefix.len()..];
    let field = |from: usize, len: usize| rest.get(from..from + len);
    let num =
        |from: usize, len: usize| field(from, len).and_then(|s| u16::from_str_radix(s, 16).ok());
    let malformed = || CommandError::DeviceError {
        code: None,
        message: format!("unexpected reply '{body}'"),
    };
    match prefix {
        "0200D6" => {
            // Result already read as 00; then type 00, max, the mode.
            let mode = num(6, 4).ok_or_else(malformed)?;
            let state = match mode {
                1 => "on",
                2 => "standby",
                3 => "suspend",
                4 => "off",
                _ => return Err(malformed()),
            };
            Ok((json!({"power": state}), json!(state)))
        }
        "00C203D6" | "000C" | "C312" => Ok((json!({}), Value::Null)),
        "4E" => {
            let status = num(0, 2).ok_or_else(malformed)?;
            let h = num(2, 4).ok_or_else(malformed)?;
            let v = num(6, 4).ok_or_else(malformed)?;
            let timing = json!({"no_signal": status & 0x80 != 0, "unstable": status & 0x40 != 0,
                                "h_frequency_khz": f64::from(h) / 100.0, "v_frequency_hz": f64::from(v) / 100.0});
            Ok((json!({ "timing": timing }), timing))
        }
        "A1" => {
            let codes: Vec<&str> = (0..rest.len() / 2)
                .filter_map(|i| u8::from_str_radix(&rest[2 * i..2 * i + 2], 16).ok())
                .map(|c| {
                    DIAGNOSIS
                        .iter()
                        .find(|(k, _)| *k == c)
                        .map_or("unknown", |(_, n)| *n)
                })
                .collect();
            let joined = codes.join(",");
            Ok((json!({"status": {"diagnosis": joined}}), json!(joined)))
        }
        "C316" | "C317" => {
            let text = decode_hex_text(rest);
            let key = if prefix == "C316" {
                "serial_number"
            } else {
                "model"
            };
            Ok((json!({"info": {key: text}}), json!(text)))
        }
        "CB02" => {
            // Result, the type 00, then the version: raw characters on the M
            // series, two hex digits per character before it.
            let version = rest.get(4..).unwrap_or("");
            let text = if version.starts_with('R') {
                version.trim().to_string()
            } else {
                decode_hex_text(version)
            };
            Ok((json!({"info": {"firmware_version": text}}), json!(text)))
        }
        "C311" => {
            let year = num(0, 2).ok_or_else(malformed)?;
            let date = json!({
                "year": 2000 + year, "month": num(2, 2).ok_or_else(malformed)?, "day": num(4, 2).ok_or_else(malformed)?,
                "weekday": num(6, 2).ok_or_else(malformed)?, "hour": num(8, 2).ok_or_else(malformed)?,
                "minute": num(10, 2).ok_or_else(malformed)?,
            });
            Ok((json!({ "clock": date }), date))
        }
        _ => Err(malformed()),
    }
}

impl Module for NecDisplay {
    fn start(&mut self, cx: &mut Cx) {
        self.open_link(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match plan(name, params) {
            Ok((steps, returns_value, hold)) => self.enqueue(
                cx,
                Job {
                    id: Some(id),
                    steps: steps.into(),
                    returns_value,
                    raw: matches!(name, "get_vcp" | "set_vcp"),
                    hold,
                },
            ),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.open = true;
                self.retry_after = RETRY_MIN;
                self.deframer = Deframer::default();
                self.hold_until = 0;
                if self.monitor {
                    self.queue_reads(reads(true));
                    self.queue_reads(identity());
                    self.queue_reads(reads(false));
                    cx.set_timer(FAST_TIMER, self.poll_ms);
                    cx.set_timer(SLOW_TIMER, SLOW_EVERY);
                } else {
                    // Nothing to read: the power status shows the display is
                    // there, and the idle check repeats it.
                    self.queue_reads(vec![vec![power_status()]]);
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
                self.pump(cx);
            }
            TcpInput::Data(bytes) => {
                for reply in self.deframer.feed(&bytes) {
                    self.received(cx, reply);
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => self.open_link(cx),
            HOLD_TIMER => self.pump(cx),
            REPLY_TIMER => {
                if let Some((job, _, _)) = self.in_flight.take() {
                    if let Some(id) = job.id {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES && self.reachable {
                        self.reachable = false;
                        cx.log(
                            Level::Warning,
                            "the display answered none of three requests",
                        );
                        cx.connection(Connection::Disconnected {
                            reason: "no reply from the display".into(),
                        });
                    }
                }
                self.pump(cx);
            }
            FAST_TIMER if self.open && self.monitor => {
                self.queue_reads(reads(true));
                self.pump(cx);
                cx.set_timer(FAST_TIMER, self.poll_ms);
            }
            SLOW_TIMER if self.open && self.monitor => {
                self.queue_reads(reads(false));
                self.pump(cx);
                cx.set_timer(SLOW_TIMER, SLOW_EVERY);
            }
            PROBE_TIMER if self.open => {
                if self.in_flight.is_none() && self.queue.is_empty() {
                    self.queue_reads(vec![vec![power_status()]]);
                    self.pump(cx);
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn module(monitor: bool, settings: Value) -> NecDisplay {
        let spec = Catalog::source_tree().devices["nec-display"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        NecDisplay::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 9)),
            host_name: None,
            port: None,
            model: "m-series".into(),
            channels: None,
            settings,
            monitor,
        })
        .unwrap()
    }

    /// A reply from display 1 ('A').
    fn reply(kind: u8, body: &str) -> Vec<u8> {
        let mut out = vec![SOH, b'0', b'0', b'A', kind];
        out.extend(hex2((body.len() + 2) as u8));
        out.push(STX);
        out.extend(body.as_bytes());
        out.push(ETX);
        let bcc = out[1..].iter().fold(0, |a, b| a ^ b);
        out.push(bcc);
        out.push(CR);
        out
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(data.clone()),
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

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn opened(m: &mut NecDisplay) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    fn feed(m: &mut NecDisplay, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut NecDisplay, now: Millis, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    #[test]
    fn frames_follow_the_documents_example() {
        // §4.3: set backlight (00-10) to 100 on monitor 1, check code 77h.
        assert_eq!(
            frame(1, b'E', "00100064"),
            [
                0x01, 0x30, 0x41, 0x30, 0x45, 0x30, 0x41, 0x02, 0x30, 0x30, 0x31, 0x30, 0x30, 0x30,
                0x36, 0x34, 0x03, 0x77, 0x0D
            ]
        );
        // Monitor 100 is the byte A4h.
        assert_eq!(frame(100, b'C', "0010")[2], 0xA4);
    }

    #[test]
    fn the_reader_resynchronises() {
        let mut d = Deframer::default();
        let good = reply(b'D', "000010000064003C");
        let mut bad = good.clone();
        bad[good.len() - 2] ^= 1;
        let mut bytes = vec![0x55];
        bytes.extend(&bad);
        bytes.extend(&good[..5]);
        assert!(d.feed(&bytes).is_empty());
        assert_eq!(
            d.feed(&good[5..]),
            [Reply {
                source: b'A',
                kind: b'D',
                body: "000010000064003C".into()
            }]
        );
    }

    #[test]
    fn monitored_it_reads_and_keeps_state() {
        let mut m = module(true, json!({}));
        let a = opened(&mut m);
        assert_eq!(sent(&a), [frame(1, b'A', "01D6")]);
        let a = feed(&mut m, 3, &reply(b'B', "0200D60000040001"));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(3)));
        assert_eq!(state(&a), json!({"power": "on"}));
        assert_eq!(sent(&a), [frame(1, b'C', "0060")]);
        let a = feed(&mut m, 4, &reply(b'D', "00006000008C0011"));
        assert_eq!(state(&a), json!({"input": "hdmi1"}));
    }

    #[test]
    fn a_set_is_read_back_and_an_unsupported_one_fails() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        feed(&mut m, 1, &reply(b'B', "0200D60000040001"));
        let a = run(&mut m, 1, 1, "set_volume", json!({"value": 30}));
        assert_eq!(sent(&a), [frame(1, b'E', "0062001E")]);
        let a = feed(&mut m, 2, &reply(b'F', "000062000064001E"));
        assert_eq!(sent(&a), [frame(1, b'C', "0062")]);
        let a = feed(&mut m, 3, &reply(b'D', "000062000064001E"));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(state(&a), json!({"audio": {"volume": 30}}));
        run(&mut m, 4, 2, "set_mute", json!({"value": true}));
        let a = feed(&mut m, 5, &reply(b'F', "01008D0000020001"));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn power_holds_the_next_message_and_reads_the_power_again() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        feed(&mut m, 1, &reply(b'B', "0200D60000040001"));
        let a = run(&mut m, 10, 1, "set_power", json!({"on": false}));
        assert_eq!(sent(&a), [frame(1, b'A', "C203D60004")]);
        let a = feed(&mut m, 20, &reply(b'B', "00C203D60004"));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        // Nothing for 15 s.
        assert!(sent(&a).is_empty());
        assert!(a.contains(&Action::SetTimer {
            key: HOLD_TIMER,
            after: POWER_HOLD
        }));
        let mut cx = Cx::new(20 + POWER_HOLD);
        m.timer(&mut cx, HOLD_TIMER);
        assert_eq!(sent(&cx.take()), [frame(1, b'A', "01D6")]);
    }

    #[test]
    fn temperatures_select_a_sensor_then_read_it() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        feed(&mut m, 1, &reply(b'B', "0200D60000040001"));
        let a = run(&mut m, 2, 1, "get_temperature", json!({"sensor": 2}));
        assert_eq!(sent(&a), [frame(1, b'E', "02780002")]);
        let a = feed(&mut m, 3, &reply(b'F', "0002780000030002"));
        assert_eq!(sent(&a), [frame(1, b'C', "0279")]);
        // §6.2: 0032h is +25.0 °C; FFFFh -0.5 °C.
        let a = feed(&mut m, 4, &reply(b'D', "00027900FFFF0032"));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(25.0) }))
        );
        assert_eq!(state(&a), json!({"status": {"temperatures": {"2": 25.0}}}));
    }

    #[test]
    fn identity_and_busy_replies() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        feed(&mut m, 1, &reply(b'B', "0200D60000040001"));
        run(&mut m, 2, 1, "get_model", json!({}));
        let a = feed(
            &mut m,
            3,
            &reply(b'B', &format!("C317{}", hex_text(b"M751"))),
        );
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!("M751")
            }))
        );
        assert_eq!(state(&a), json!({"info": {"model": "M751"}}));
        run(&mut m, 4, 2, "get_volume", json!({}));
        let a = feed(&mut m, 5, &reply(b'B', "BE"));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn every_command_of_the_spec_is_sent() {
        let spec = Catalog::source_tree().devices["nec-display"].clone();
        for (name, command) in &spec.commands {
            let mut params = Params::new();
            for (p, decl) in &command.params {
                let value = match (&decl.values, decl.kind) {
                    (Some(values), _) => json!(values[0]),
                    (None, crate::catalog::ParamType::Bool) => json!(false),
                    _ => json!(decl.min.unwrap_or(0.0).max(1.0) as i64),
                };
                params.insert(p.clone(), value);
            }
            let planned = plan(name, &params);
            assert!(planned.is_ok(), "{name}: {:?}", planned.err());
        }
    }

    #[test]
    fn the_serial_link_and_a_packet_interval() {
        let mut m = module(
            true,
            json!({"link": "serial", "serial_port": "COM5", "packet_interval_ms": 600}),
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::SerialOpen { port, .. } if port.baud_rate == 9600)));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        let a = feed(&mut m, 100, &reply(b'B', "0200D60000040001"));
        assert!(sent(&a).is_empty());
        let mut cx = Cx::new(700);
        m.timer(&mut cx, HOLD_TIMER);
        assert_eq!(sent(&cx.take()), [frame(1, b'C', "0060")]);
    }
}
