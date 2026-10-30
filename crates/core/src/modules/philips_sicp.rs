//! Philips professional displays: SICP, the Serial / Ethernet Interface
//! Communication Protocol, over TCP 5000 or an RS-232 port of the host.
//!
//! Written from Philips's "SICP V2.03" command document (section numbers
//! below are that document's) and the Q Line user manual for the LAN and
//! power-save behaviour; see the spec's sources.
//!
//! - A message is MsgSize (the whole message's length, itself and the
//!   checksum included, 3 to 40), Control (the monitor ID, 1 to 255; 0
//!   broadcasts and is never answered), Group (0 to address by monitor ID),
//!   DATA[0] the command and its data, then a checksum, the XOR of every byte
//!   before it (§2.2.1). Firmware before SICP 1.86 has no Group byte; the
//!   `group_byte` setting leaves it out of requests and replies.
//! - Every message is answered before the next is sent (§2.2): a set by the
//!   generic report 00 with ACK 06, NACK 15 or NAV 18 (not available, or not
//!   now: most commands in standby), a get by a report whose DATA[0] is the
//!   get's own code. A reply may carry a group byte other than 0 (§2.4's own
//!   examples), so the group of a reply is not checked.
//! - Several settings travel together in one message (video parameters 32,
//!   color 36, volume 44, treble and bass 42, volume limits B8 and B9, VGA 38):
//!   a command changing one of them sends the others as last read, reading
//!   them first when they are not known yet. Every set is followed by its get,
//!   so state follows the change at once.
//! - Nothing is pushed: power, input, volume and mute are read every
//!   `poll_ms`, every other setting once a minute, and identity on connecting.
//!   A report the display refuses (NAV: not supported, or not in standby)
//!   leaves state as it was.
//!
//! Opened for commands only (`monitor` false), nothing is read beyond what
//! commands ask and their read-back, except the power state every 10 s while
//! idle, the only sign the display is there. Every answered request reports
//! its round trip.

use std::collections::{BTreeMap, VecDeque};

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const SOCKET: Key = "sicp";
const REPLY_TIMER: Key = "reply";
const FAST_TIMER: Key = "poll-fast";
const SLOW_TIMER: Key = "poll-slow";
const PROBE_TIMER: Key = "probe";
const RETRY_TIMER: Key = "retry";

pub(crate) const DEFAULT_PORT: u16 = 5000;

/// The display answers within 500 ms, when the controller may send again
/// (§2.2); Android models answer slower in standby.
const REPLY_TIMEOUT: Millis = 2_000;
const MISSES: u32 = 3;
const PROBE_EVERY: Millis = 10_000;
const SLOW_EVERY: Millis = 60_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

const ACK: u8 = 0x06;
const NACK: u8 = 0x15;
const NAV: u8 = 0x18;

/// The XOR of every byte (§2.2.1).
fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |a, b| a ^ b)
}

/// Frame DATA[0..] for a monitor, with or without the group byte.
pub(crate) fn frame(monitor: u8, group: Option<u8>, data: &[u8]) -> Vec<u8> {
    let size = data.len() + 3 + usize::from(group.is_some());
    let mut out = Vec::with_capacity(size);
    out.push(size as u8);
    out.push(monitor);
    if let Some(g) = group {
        out.push(g);
    }
    out.extend_from_slice(data);
    out.push(checksum(&out));
    out
}

/// A message read off the stream: who sent it, and DATA[0..].
#[derive(Debug, Clone, PartialEq)]
struct Message {
    monitor: u8,
    data: Vec<u8>,
}

#[derive(Debug)]
struct Deframer {
    group: bool,
    buf: Vec<u8>,
}

impl Deframer {
    fn new(group: bool) -> Deframer {
        Deframer {
            group,
            buf: Vec::new(),
        }
    }

    fn feed(&mut self, bytes: &[u8]) -> Vec<Message> {
        self.buf.extend_from_slice(bytes);
        let header = 2 + usize::from(self.group);
        let mut out = Vec::new();
        while let Some(&size) = self.buf.first() {
            let size = usize::from(size);
            if !(header + 2..=40).contains(&size) {
                self.buf.remove(0);
                continue;
            }
            if self.buf.len() < size {
                break;
            }
            if checksum(&self.buf[..size - 1]) != self.buf[size - 1] {
                self.buf.remove(0);
                continue;
            }
            out.push(Message {
                monitor: self.buf[1],
                data: self.buf[header..size - 1].to_vec(),
            });
            self.buf.drain(..size);
        }
        out
    }
}

/// One message of a job.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// A get: DATA[0] and its arguments; answered by a report of that code.
    Get(Vec<u8>),
    /// A set, answered by ACK.
    Set(Vec<u8>),
    /// A set of one or more bytes of a message whose other bytes are as last
    /// read by `get`: `set` is the set code, `edits` the data bytes changed
    /// (index from DATA[1]).
    Modify {
        get: u8,
        set: u8,
        edits: Vec<(usize, u8)>,
    },
}

impl Step {
    fn expects(&self) -> Option<u8> {
        match self {
            Step::Get(d) => d.first().copied(),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct Job {
    id: Option<CommandId>,
    steps: VecDeque<Step>,
    /// A get whose report is the command's value.
    returns_value: bool,
    /// Gets queued after the job's sets succeed: the read-back.
    then: Vec<Vec<u8>>,
}

impl Job {
    fn internal(get: Vec<u8>) -> Job {
        Job {
            id: None,
            steps: VecDeque::from([Step::Get(get)]),
            returns_value: false,
            then: Vec::new(),
        }
    }
}

// ── Value tables (§4-§14) ──────────────────────────────────────────────────

/// Input sources, 0xAC/0xAD and the PIP and schedule sources (§4.4).
const SOURCES: &[(&str, u8)] = &[
    ("video", 0x01),
    ("s_video", 0x02),
    ("component", 0x03),
    ("cvi2", 0x04),
    ("vga", 0x05),
    ("hdmi2", 0x06),
    ("displayport2", 0x07),
    ("usb2", 0x08),
    ("card_dvi_d", 0x09),
    ("displayport", 0x0A),
    ("card_ops", 0x0B),
    ("usb", 0x0C),
    ("hdmi", 0x0D),
    ("dvi_d", 0x0E),
    ("hdmi3", 0x0F),
    ("browser", 0x10),
    ("smartcms", 0x11),
    ("dms", 0x12),
    ("internal_storage", 0x13),
    ("media_player", 0x16),
    ("pdf_player", 0x17),
    ("custom", 0x18),
    ("hdmi4", 0x19),
    ("vga2", 0x1A),
    ("vga3", 0x1B),
    ("iwb", 0x1C),
];

/// The failover list's own numbering (§4.5).
const FAILOVER: &[(&str, u8)] = &[
    ("hdmi", 0x00),
    ("component", 0x01),
    ("composite", 0x02),
    ("displayport", 0x03),
    ("dvi_d", 0x04),
    ("vga", 0x05),
    ("ops", 0x06),
    ("usb", 0x07),
    ("browser", 0x08),
    ("smartcms", 0x09),
    ("internal_storage", 0x0A),
    ("dms", 0x0B),
    ("hdmi2", 0x0C),
    ("hdmi3", 0x0D),
    ("usb_playlist", 0x0E),
    ("usb_autoplay", 0x0F),
    ("media_player", 0x10),
    ("pdf_player", 0x11),
    ("custom", 0x12),
    ("hdmi4", 0x13),
    ("vga2", 0x14),
    ("vga3", 0x15),
    ("iwb", 0x16),
];

const POWER_AT_START: &[(&str, u8)] = &[("off", 0x00), ("on", 0x01), ("last", 0x02)];
const IR_LOCK: &[(&str, u8)] = &[
    ("unlocked", 0x01),
    ("all", 0x02),
    ("all_but_power", 0x03),
    ("all_but_volume", 0x04),
    ("primary", 0x05),
    ("secondary", 0x06),
    ("all_but_power_and_volume", 0x07),
];
const KEYPAD_LOCK: &[(&str, u8)] = &[
    ("unlocked", 0x01),
    ("all", 0x02),
    ("all_but_power", 0x03),
    ("all_but_volume", 0x04),
    ("all_but_power_and_volume", 0x07),
];
const AUTO_SIGNAL: &[(&str, u8)] = &[
    ("off", 0x00),
    ("all", 0x01),
    ("pc_sources", 0x03),
    ("video_sources", 0x04),
    ("failover", 0x05),
];
const GAMMA: &[(&str, u8)] = &[
    ("native", 0x01),
    ("s_gamma", 0x02),
    ("2.2", 0x03),
    ("2.4", 0x04),
    ("dicom", 0x05),
];
const COLOR_TEMPERATURE: &[(&str, u8)] = &[
    ("user1", 0x00),
    ("native", 0x01),
    ("11000k", 0x02),
    ("10000k", 0x03),
    ("9300k", 0x04),
    ("7500k", 0x05),
    ("6500k", 0x06),
    ("5770k", 0x07),
    ("5500k", 0x08),
    ("5000k", 0x09),
    ("4000k", 0x0A),
    ("3400k", 0x0B),
    ("3350k", 0x0C),
    ("3000k", 0x0D),
    ("2800k", 0x0E),
    ("2600k", 0x0F),
    ("1850k", 0x10),
    ("user2", 0x12),
];
const PICTURE_FORMAT: &[(&str, u8)] = &[
    ("normal", 0x00),
    ("custom", 0x01),
    ("real", 0x02),
    ("full", 0x03),
    ("21_9", 0x04),
    ("dynamic", 0x05),
    ("16_9", 0x06),
];
const PICTURE_STYLE: &[(&str, u8)] = &[
    ("highbright", 0x00),
    ("srgb", 0x01),
    ("vivid", 0x02),
    ("natural", 0x03),
    ("standard", 0x04),
    ("video", 0x05),
    ("static_signage", 0x06),
    ("text", 0x07),
    ("energy_saving", 0x08),
    ("soft", 0x09),
    ("user", 0x0A),
];
const LEVEL4: &[(&str, u8)] = &[
    ("off", 0x00),
    ("low", 0x01),
    ("medium", 0x02),
    ("high", 0x03),
];
const NOISE_REDUCTION: &[(&str, u8)] = &[
    ("off", 0x00),
    ("low", 0x01),
    ("middle", 0x02),
    ("high", 0x03),
    ("default", 0x04),
];
const SCAN_MODE: &[(&str, u8)] = &[("overscan", 0x00), ("underscan", 0x01), ("off", 0x02)];
const SCAN_CONVERSION: &[(&str, u8)] = &[("progressive", 0x00), ("interlace", 0x01)];
const PIP_MODE: &[(&str, u8)] = &[
    ("off", 0x00),
    ("pip", 0x01),
    ("pop", 0x02),
    ("quick_swap", 0x03),
    ("pbp_2win", 0x04),
    ("pbp_3win", 0x05),
    ("pbp_4win", 0x06),
    ("pbp_3win_1", 0x07),
    ("pbp_3win_2", 0x08),
    ("pbp_4win_1", 0x09),
    ("custom", 0x0A),
];
const PIP_POSITION: &[(&str, u8)] = &[
    ("bottom_left", 0x00),
    ("top_left", 0x01),
    ("top_right", 0x02),
    ("bottom_right", 0x03),
    ("center", 0x04),
];
const FAN_SPEED: &[(&str, u8)] = &[
    ("off", 0x00),
    ("auto", 0x01),
    ("low", 0x02),
    ("middle", 0x03),
    ("high", 0x04),
];
const APM: &[(&str, u8)] = &[
    ("off", 0x00),
    ("on", 0x01),
    ("mode1", 0x02),
    ("mode2", 0x03),
];
const POWER_SAVE: &[(&str, u8)] = &[
    ("rgb_off_video_off", 0x00),
    ("rgb_off_video_on", 0x01),
    ("rgb_on_video_off", 0x02),
    ("rgb_on_video_on", 0x03),
    ("mode1", 0x04),
    ("mode2", 0x05),
    ("mode3", 0x06),
    ("mode4", 0x07),
];
const ECO: &[(&str, u8)] = &[("low_power_standby", 0x00), ("normal", 0x01)];
const LOGO: &[(&str, u8)] = &[("off", 0x00), ("on", 0x01), ("user", 0x02)];
const ANYTILE_RESOLUTION: &[(&str, u8)] = &[("default", 0x00), ("fhd", 0x01), ("uhd", 0x02)];
const IMAGE_ROTATION: &[(&str, u8)] = &[
    ("off", 0x00),
    ("on", 0x01),
    ("clockwise", 0x02),
    ("counter_clockwise", 0x03),
];
const WINDOW_ROTATION: &[(&str, u8)] = &[
    ("none", 0x00),
    ("rot90", 0x01),
    ("rot270", 0x02),
    ("mirror_h", 0x03),
    ("mirror_v", 0x04),
    ("mirror_hv", 0x05),
];
const RESTART: &[(&str, u8)] = &[("android", 0x00), ("scalar", 0x01)];
const IDENTITY: &[(&str, u8, u8)] = &[
    ("sicp_version", 0xA2, 0x00),
    ("platform", 0xA2, 0x01),
    ("platform_version", 0xA2, 0x02),
    ("model", 0xA1, 0x00),
    ("firmware_version", 0xA1, 0x01),
    ("build_date", 0xA1, 0x02),
    ("android_firmware_version", 0xA1, 0x03),
];

fn code_of(table: &[(&str, u8)], name: &str) -> Option<u8> {
    table.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

fn name_of(table: &[(&str, u8)], code: u8) -> Value {
    table
        .iter()
        .find(|(_, c)| *c == code)
        .map_or(Value::Null, |(n, _)| json!(n))
}

// ── Commands ───────────────────────────────────────────────────────────────

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

fn byte(params: &Params, name: &str) -> Result<u8, CommandError> {
    u8::try_from(int(params, name)?).map_err(|_| bad(format!("'{name}' is out of range")))
}

fn boolean(params: &Params, name: &str) -> Result<bool, CommandError> {
    params
        .get(name)
        .and_then(Value::as_bool)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

fn choice(params: &Params, name: &str, table: &[(&str, u8)]) -> Result<u8, CommandError> {
    let value = params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(format!("'{name}' is required")))?;
    code_of(table, value).ok_or_else(|| bad(format!("'{value}' is not a value of '{name}'")))
}

/// A setting with its own get and set codes and one data byte.
struct Simple {
    key: &'static str,
    get: u8,
    set: u8,
    param: &'static str,
    kind: Kind,
}

#[derive(Clone, Copy)]
enum Kind {
    Table(&'static [(&'static str, u8)]),
    /// A boolean: these bytes for true and false.
    Bool(u8, u8),
    Byte,
}

const SIMPLE: &[Simple] = &[
    Simple {
        key: "power",
        get: 0x19,
        set: 0x18,
        param: "on",
        kind: Kind::Bool(0x02, 0x01),
    },
    Simple {
        key: "power_at_cold_start",
        get: 0xA4,
        set: 0xA3,
        param: "state",
        kind: Kind::Table(POWER_AT_START),
    },
    Simple {
        key: "backlight",
        get: 0x71,
        set: 0x72,
        param: "on",
        kind: Kind::Bool(0x00, 0x01),
    },
    Simple {
        key: "ir_lock",
        get: 0x1D,
        set: 0x1C,
        param: "lock",
        kind: Kind::Table(IR_LOCK),
    },
    Simple {
        key: "keypad_lock",
        get: 0x1B,
        set: 0x1A,
        param: "lock",
        kind: Kind::Table(KEYPAD_LOCK),
    },
    Simple {
        key: "auto_signal_detecting",
        get: 0xAF,
        set: 0xAE,
        param: "mode",
        kind: Kind::Table(AUTO_SIGNAL),
    },
    Simple {
        key: "color_temperature",
        get: 0x35,
        set: 0x34,
        param: "preset",
        kind: Kind::Table(COLOR_TEMPERATURE),
    },
    Simple {
        key: "color_temperature_kelvin",
        get: 0x12,
        set: 0x11,
        param: "hundreds",
        kind: Kind::Byte,
    },
    Simple {
        key: "picture_format",
        get: 0x3B,
        set: 0x3A,
        param: "format",
        kind: Kind::Table(PICTURE_FORMAT),
    },
    Simple {
        key: "picture_style",
        get: 0x65,
        set: 0x66,
        param: "style",
        kind: Kind::Table(PICTURE_STYLE),
    },
    Simple {
        key: "noise_reduction",
        get: 0x2B,
        set: 0x2A,
        param: "level",
        kind: Kind::Table(NOISE_REDUCTION),
    },
    Simple {
        key: "memc",
        get: 0x29,
        set: 0x28,
        param: "level",
        kind: Kind::Table(LEVEL4),
    },
    Simple {
        key: "scan_mode",
        get: 0x51,
        set: 0x50,
        param: "mode",
        kind: Kind::Table(SCAN_MODE),
    },
    Simple {
        key: "scan_conversion",
        get: 0x53,
        set: 0x52,
        param: "mode",
        kind: Kind::Table(SCAN_CONVERSION),
    },
    Simple {
        key: "mute",
        get: 0x46,
        set: 0x47,
        param: "muted",
        kind: Kind::Bool(0x01, 0x00),
    },
    Simple {
        key: "switch_on_delay",
        get: 0x55,
        set: 0x54,
        param: "delay",
        kind: Kind::Byte,
    },
    Simple {
        key: "frame_compensation_h",
        get: 0x5E,
        set: 0x5F,
        param: "value",
        kind: Kind::Byte,
    },
    Simple {
        key: "frame_compensation_v",
        get: 0x67,
        set: 0x68,
        param: "value",
        kind: Kind::Byte,
    },
    Simple {
        key: "anytile_resolution",
        get: 0x4E,
        set: 0x4F,
        param: "mode",
        kind: Kind::Table(ANYTILE_RESOLUTION),
    },
    Simple {
        key: "group_id",
        get: 0x5D,
        set: 0x5C,
        param: "group",
        kind: Kind::Byte,
    },
    Simple {
        key: "light_sensor",
        get: 0x25,
        set: 0x24,
        param: "on",
        kind: Kind::Bool(0x01, 0x00),
    },
    Simple {
        key: "human_sensor",
        get: 0xB3,
        set: 0xB4,
        param: "minutes",
        kind: Kind::Byte,
    },
    Simple {
        key: "fan_speed",
        get: 0x62,
        set: 0x61,
        param: "speed",
        kind: Kind::Table(FAN_SPEED),
    },
    Simple {
        key: "smart_power",
        get: 0xDE,
        set: 0xDD,
        param: "level",
        kind: Kind::Table(LEVEL4),
    },
    Simple {
        key: "apm",
        get: 0xD1,
        set: 0xD0,
        param: "mode",
        kind: Kind::Table(APM),
    },
    Simple {
        key: "power_save",
        get: 0xD3,
        set: 0xD2,
        param: "mode",
        kind: Kind::Table(POWER_SAVE),
    },
    Simple {
        key: "eco_mode",
        get: 0x63,
        set: 0x64,
        param: "mode",
        kind: Kind::Table(ECO),
    },
    Simple {
        key: "pixel_shift",
        get: 0xB1,
        set: 0xB2,
        param: "steps",
        kind: Kind::Byte,
    },
    Simple {
        key: "off_timer",
        get: 0x91,
        set: 0x92,
        param: "hours",
        kind: Kind::Byte,
    },
    Simple {
        key: "osd_rotation",
        get: 0x27,
        set: 0x26,
        param: "on",
        kind: Kind::Bool(0x01, 0x00),
    },
    Simple {
        key: "information_osd",
        get: 0x2D,
        set: 0x2C,
        param: "seconds",
        kind: Kind::Byte,
    },
    Simple {
        key: "touch",
        get: 0x1F,
        set: 0x1E,
        param: "on",
        kind: Kind::Bool(0x01, 0x00),
    },
    Simple {
        key: "power_on_logo",
        get: 0x3F,
        set: 0x3E,
        param: "logo",
        kind: Kind::Table(LOGO),
    },
    Simple {
        key: "external_storage_lock",
        get: 0xF2,
        set: 0xF1,
        param: "locked",
        kind: Kind::Bool(0x01, 0x00),
    },
];

/// Video parameters (0x32/0x33): the parameter for each byte.
const VIDEO: &[&str] = &[
    "brightness",
    "color",
    "contrast",
    "sharpness",
    "tint",
    "black_level",
    "gamma",
];
/// Color parameters (0x36/0x37).
const COLOR: &[&str] = &[
    "red_gain",
    "green_gain",
    "blue_gain",
    "red_offset",
    "green_offset",
    "blue_offset",
];

/// The gets read once a minute (and the identity on connecting), a model
/// lacking one answering NAV.
fn slow_reads() -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = SIMPLE
        .iter()
        .filter(|s| !matches!(s.key, "power" | "mute"))
        .map(|s| vec![s.get])
        .collect();
    out.extend([
        vec![0x15],
        vec![0x33],
        vec![0x37],
        vec![0x39],
        vec![0x43],
        vec![0xB6],
        vec![0xB7],
        vec![0x3D],
        vec![0x85],
        vec![0x23],
        vec![0x4A],
        vec![0xA6],
        vec![0x0F, 0x02],
        vec![0x2F],
        vec![0x59],
        vec![0x16],
        vec![0xF4],
    ]);
    for page in 1..=7 {
        out.push(vec![0x5B, page]);
    }
    for window in 0..4 {
        out.push(vec![0xFD, window]);
    }
    out
}

fn fast_reads() -> Vec<Vec<u8>> {
    vec![vec![0x19], vec![0xAD], vec![0x45], vec![0x46]]
}

fn identity_reads() -> Vec<Vec<u8>> {
    IDENTITY.iter().map(|(_, c, s)| vec![*c, *s]).collect()
}

/// What a command sends, and whether its get's report is its value.
fn plan(name: &str, params: &Params) -> Result<(Vec<Step>, bool), CommandError> {
    let set = |data: Vec<u8>| Ok((vec![Step::Set(data)], false));
    let get = |data: Vec<u8>| Ok((vec![Step::Get(data)], true));
    if let Some(key) = name.strip_prefix("get_") {
        if let Some(s) = SIMPLE.iter().find(|s| s.key == key) {
            return get(vec![s.get]);
        }
        if let Some((_, c, sub)) = IDENTITY.iter().find(|(k, _, _)| *k == key) {
            return get(vec![*c, *sub]);
        }
    }
    if let Some(key) = name.strip_prefix("set_") {
        if let Some(s) = SIMPLE.iter().find(|s| s.key == key) {
            let value = match s.kind {
                Kind::Table(t) => choice(params, s.param, t)?,
                Kind::Bool(on, off) => {
                    if boolean(params, s.param)? {
                        on
                    } else {
                        off
                    }
                }
                Kind::Byte => byte(params, s.param)?,
            };
            return set(vec![s.set, value]);
        }
        if let Some(i) = VIDEO.iter().position(|k| *k == key) {
            let value = if key == "gamma" {
                choice(params, "gamma", GAMMA)?
            } else {
                byte(params, "level")?
            };
            return modify(0x33, 0x32, vec![(i, value)]);
        }
        if let Some(i) = COLOR.iter().position(|k| *k == key) {
            return modify(0x37, 0x36, vec![(i, byte(params, "level")?)]);
        }
    }
    match name {
        "get_serial_number" => get(vec![0x15]),
        "get_video_parameters" => get(vec![0x33]),
        "get_color_parameters" => get(vec![0x37]),
        "get_vga_parameters" => get(vec![0x39]),
        "get_input" => get(vec![0xAD]),
        "set_input" => {
            let source = choice(params, "source", SOURCES)?;
            let playlist = byte(params, "playlist")?;
            let label = u8::from(boolean(params, "show_label")?);
            set(vec![0xAC, source, playlist, label, 0x00])
        }
        "get_failover_priority" => get(vec![0xA6]),
        "set_failover_priority" => {
            let list = params
                .get("sources")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("'sources' is required"))?;
            let mut data = vec![0xA5];
            for item in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                data.push(
                    code_of(FAILOVER, item)
                        .ok_or_else(|| bad(format!("'{item}' is not a failover source")))?,
                );
            }
            if data.len() < 2 || data.len() > 37 {
                return Err(bad("give 1 to 36 failover sources"));
            }
            set(data)
        }
        "restart" => set(vec![0x57, choice(params, "target", RESTART)?]),
        "set_vga_h_position" => modify(0x39, 0x38, vec![(2, byte(params, "position")?)]),
        "set_vga_v_position" => modify(0x39, 0x38, vec![(3, byte(params, "position")?)]),
        "auto_adjust" => set(vec![0x70, 0x40, 0x00]),
        "get_signal" => get(vec![0x59]),
        "get_volume" => get(vec![0x45]),
        "set_volume" => modify(0x45, 0x44, vec![(0, byte(params, "level")?)]),
        "set_audio_out_volume" => modify(0x45, 0x44, vec![(1, byte(params, "level")?)]),
        "volume_up" | "volume_down" => {
            let step = u8::from(name == "volume_up");
            let out = if boolean(params, "audio_out")? {
                step
            } else {
                2
            };
            set(vec![0x41, step, out])
        }
        "get_audio_parameters" => get(vec![0x43]),
        "set_treble" => modify(0x43, 0x42, vec![(0, byte(params, "level")?)]),
        "set_bass" => modify(0x43, 0x42, vec![(1, byte(params, "level")?)]),
        "get_speaker_volume_limits" => get(vec![0xB6]),
        "get_audio_out_volume_limits" => get(vec![0xB7]),
        "set_speaker_volume_limits" | "set_audio_out_volume_limits" => {
            let (min, max, start) = (
                byte(params, "minimum")?,
                byte(params, "maximum")?,
                byte(params, "switch_on")?,
            );
            if !(min <= start && start <= max) {
                return Err(bad("the minimum, switch-on and maximum volumes must rise"));
            }
            let code = if name == "set_speaker_volume_limits" {
                0xB8
            } else {
                0xB9
            };
            set(vec![code, min, max, start])
        }
        "get_pip" => get(vec![0x3D]),
        "set_pip" => set(vec![
            0x3C,
            choice(params, "mode", PIP_MODE)?,
            choice(params, "position", PIP_POSITION)?,
            0x00,
            0x00,
        ]),
        "get_pip_sources" => get(vec![0x85]),
        "set_pip_source" => set(vec![0x84, 0xFD, choice(params, "source", SOURCES)?]),
        "set_pip_sources" => set(vec![
            0x84,
            0xFD,
            choice(params, "q2", SOURCES)?,
            choice(params, "q3", SOURCES)?,
            choice(params, "q4", SOURCES)?,
        ]),
        "get_tiling" => get(vec![0x23]),
        "set_tiling" => {
            let columns = int(params, "columns")?;
            let rows = int(params, "rows")?;
            let position = int(params, "position")?;
            let width = if boolean(params, "zero_bezel")? {
                15
            } else {
                5
            };
            if columns > width || rows > if width == 15 { 10 } else { 5 } {
                return Err(bad(
                    "the wall is at most 5 by 5, or 15 by 10 with zero_bezel",
                ));
            }
            if position > columns * rows {
                return Err(bad("the position is past the last display of the wall"));
            }
            let hv = (rows - 1) * width + columns;
            set(vec![
                0x22,
                u8::from(boolean(params, "enabled")?),
                u8::from(boolean(params, "frame_compensation")?),
                position as u8,
                hv as u8,
            ])
        }
        "set_tiling_enabled" => set(vec![
            0x22,
            u8::from(boolean(params, "enabled")?),
            0x02,
            0,
            0,
        ]),
        "get_anytile" => get(vec![0x4A]),
        "set_anytile" => {
            let mut data = vec![0x4B, u8::from(boolean(params, "enabled")?)];
            for key in ["rotation", "h_start", "v_start", "h_size", "v_size"] {
                let v = u16::try_from(int(params, key)?)
                    .map_err(|_| bad(format!("'{key}' is out of range")))?;
                data.extend(v.to_le_bytes());
            }
            set(data)
        }
        "assign_ids" => set(vec![
            0xC0,
            byte(params, "monitor_id")?,
            byte(params, "group_id")?,
        ]),
        "show_monitor_id" => set(vec![0x4C, byte(params, "monitor_id")?]),
        "set_monitor_id" => set(vec![0x69, byte(params, "monitor_id")?]),
        "get_operating_hours" => get(vec![0x0F, 0x02]),
        "get_temperature" => get(vec![0x2F]),
        "set_pixel_shift_auto" => set(vec![0xB2, 0x5B]),
        "get_orientation" => get(vec![0x16]),
        "set_orientation" => {
            let mut data = vec![
                0x17,
                0x00,
                u8::from(boolean(params, "osd_portrait")?),
                choice(params, "image", IMAGE_ROTATION)?,
            ];
            for w in 1..=4 {
                data.push(u8::from(boolean(params, &format!("window{w}"))?));
            }
            set(data)
        }
        "factory_reset" => set(vec![0x56]),
        "screenshot" => set(vec![0x58]),
        "get_led_strip" => get(vec![0xF4]),
        "set_led_strip" => set(vec![
            0xF3,
            u8::from(boolean(params, "on")?),
            byte(params, "red")?,
            byte(params, "green")?,
            byte(params, "blue")?,
        ]),
        "get_schedule" => get(vec![0x5B, byte(params, "page")?]),
        "set_schedule" => {
            let page = byte(params, "page")?;
            let mut days = 0u8;
            for (bit, key) in [
                (0, "every_week"),
                (1, "monday"),
                (2, "tuesday"),
                (3, "wednesday"),
                (4, "thursday"),
                (5, "friday"),
                (6, "saturday"),
                (7, "sunday"),
            ] {
                if boolean(params, key)? {
                    days |= 1 << bit;
                }
            }
            set(vec![
                0x5A,
                (page << 4) | u8::from(boolean(params, "enabled")?),
                byte(params, "start_hour")?,
                byte(params, "start_minute")?,
                byte(params, "end_hour")?,
                byte(params, "end_minute")?,
                choice(params, "source", SOURCES)?,
                days,
                byte(params, "playlist")?,
            ])
        }
        "set_multi_window" => set(vec![
            0xFB,
            u8::from(boolean(params, "enabled")?),
            byte(params, "windows")?.saturating_sub(1),
        ]),
        "get_window" => get(vec![0xFD, byte(params, "window")?]),
        "set_window" => {
            let mut data = vec![
                0xFC,
                byte(params, "window")?,
                choice(params, "rotation", WINDOW_ROTATION)?,
            ];
            for key in ["x", "y", "width", "height"] {
                let v = u16::try_from(int(params, key)?)
                    .map_err(|_| bad(format!("'{key}' is out of range")))?;
                data.extend(v.to_be_bytes());
            }
            data.push(choice(params, "format", PICTURE_FORMAT)?);
            set(data)
        }
        other => Err(CommandError::UnknownCommand {
            command: other.into(),
        }),
    }
}

fn modify(get: u8, set: u8, edits: Vec<(usize, u8)>) -> Result<(Vec<Step>, bool), CommandError> {
    Ok((vec![Step::Modify { get, set, edits }], false))
}

/// The get that reads back what a set changed.
fn read_back(set: &[u8]) -> Option<Vec<u8>> {
    let code = *set.first()?;
    if let Some(s) = SIMPLE.iter().find(|s| s.set == code) {
        return Some(vec![s.get]);
    }
    Some(match code {
        0xAC => vec![0xAD],
        0xA5 => vec![0xA6],
        0x32 => vec![0x33],
        0x36 => vec![0x37],
        0x38 => vec![0x39],
        0x44 | 0x41 => vec![0x45],
        0x42 => vec![0x43],
        0xB8 => vec![0xB6],
        0xB9 => vec![0xB7],
        0x3C => vec![0x3D],
        0x84 => vec![0x85],
        0x22 => vec![0x23],
        0x4B => vec![0x4A],
        0xC0 | 0x5C => vec![0x5D],
        0x17 => vec![0x16],
        0xF3 => vec![0xF4],
        0x5A => vec![0x5B, set.get(1)? >> 4],
        0xFC => vec![0xFD, *set.get(1)?],
        _ => return None,
    })
}

fn ascii(data: &[u8]) -> String {
    String::from_utf8_lossy(data)
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_string()
}

/// A report as state, and the value a get returns: `req` is the get sent,
/// `data` the report's DATA[1..].
fn decode(req: &[u8], data: &[u8]) -> Option<(Value, Value)> {
    let code = *req.first()?;
    let at = |i: usize| data.get(i).copied();
    if let Some(s) = SIMPLE.iter().find(|s| s.get == code) {
        let b = at(0)?;
        let value = match s.kind {
            Kind::Table(t) => name_of(t, b),
            Kind::Bool(on, _) if b == on => json!(true),
            Kind::Bool(_, off) if b == off => json!(false),
            Kind::Bool(..) => {
                // 0xFF: the sensor is not fitted (§7.8).
                return Some((json!({}), Value::Null));
            }
            Kind::Byte => json!(b),
        };
        if value.is_null() {
            return None;
        }
        let mut patch = Map::new();
        patch.insert(s.key.into(), value.clone());
        return Some((Value::Object(patch), value));
    }
    if let Some((key, _, _)) = IDENTITY
        .iter()
        .find(|(_, c, sub)| *c == code && req.get(1) == Some(sub))
    {
        let text = ascii(data);
        return Some((json!({"info": {*key: text}}), json!(text)));
    }
    let value = match code {
        0x15 => {
            let text = ascii(data);
            return Some((json!({"info": {"serial_number": text}}), json!(text)));
        }
        0xAD => {
            // Some firmware puts a source type (FD) first, as the PIP report does.
            let data = if data.first() == Some(&0xFD) {
                &data[1..]
            } else {
                data
            };
            let mut input = Map::new();
            input.insert("source".into(), name_of(SOURCES, *data.first()?));
            if let Some(p) = data.get(1) {
                input.insert("playlist".into(), json!(p));
            }
            json!({ "input": input })
        }
        0xA6 => {
            let list: Vec<String> = data
                .iter()
                .filter_map(|b| name_of(FAILOVER, *b).as_str().map(str::to_string))
                .collect();
            json!({"failover_priority": list.join(",")})
        }
        0x33 => {
            let mut picture = Map::new();
            for (i, key) in VIDEO.iter().enumerate() {
                if let Some(b) = at(i) {
                    let v = if *key == "gamma" {
                        name_of(GAMMA, b)
                    } else {
                        json!(b)
                    };
                    picture.insert((*key).into(), v);
                }
            }
            json!({ "picture": picture })
        }
        0x37 => {
            let mut picture = Map::new();
            for (i, key) in COLOR.iter().enumerate() {
                if let Some(b) = at(i) {
                    picture.insert((*key).into(), json!(b));
                }
            }
            json!({ "picture": picture })
        }
        0x39 => {
            json!({"vga": {"clock": at(0)?, "phase": at(1)?, "h_position": at(2)?, "v_position": at(3)?}})
        }
        0x59 => json!({"signal": at(0)? == 0x01}),
        0x45 => match at(1) {
            Some(out) => json!({"audio": {"volume": at(0)?, "audio_out_volume": out}}),
            None => json!({"audio": {"volume": at(0)?}}),
        },
        0x43 => json!({"audio": {"treble": at(0)?, "bass": at(1)?}}),
        0xB6 | 0xB7 => {
            let which = if code == 0xB6 { "speaker" } else { "audio_out" };
            json!({"audio": {"limits": {which: {"minimum": at(0)?, "maximum": at(1)?, "switch_on": at(2)?}}}})
        }
        0x3D => {
            json!({"pip": {"mode": name_of(PIP_MODE, at(0)?), "position": name_of(PIP_POSITION, at(1)? & 0x07)}})
        }
        0x85 => {
            let mut pip = Map::new();
            for (i, key) in ["q2", "q3", "q4"].iter().enumerate() {
                if let Some(b) = at(i + 1) {
                    pip.insert((*key).into(), name_of(SOURCES, b));
                }
            }
            json!({"pip": {"sources": pip}})
        }
        0x23 => {
            let hv = i64::from(at(3)?);
            // Zero-bezel walls count 15 to a row; others 5 (§7.6). A report
            // does not say which, so both readings are given.
            let split = |width: i64| {
                let h = if hv % width == 0 { width } else { hv % width };
                (h, (hv - h) / width + 1)
            };
            let (c5, r5) = split(5);
            let (c15, r15) = split(15);
            json!({"tiling": {"enabled": at(0)? == 1, "frame_compensation": at(1)? == 1,
                              "position": at(2)?, "hv_code": hv,
                              "columns": c5, "rows": r5, "zero_bezel_columns": c15, "zero_bezel_rows": r15}})
        }
        0x4A => {
            let word = |i: usize| -> Option<u16> { Some(u16::from_le_bytes([at(i)?, at(i + 1)?])) };
            json!({"anytile": {"enabled": at(0)? == 1, "rotation": word(1)?, "h_start": word(3)?,
                               "v_start": word(5)?, "h_size": word(7)?, "v_size": word(9)?}})
        }
        0x0F => json!({"status": {"operating_hours": u16::from_be_bytes([at(0)?, at(1)?])}}),
        0x2F => match at(1) {
            Some(second) => json!({"status": {"temperature1": at(0)?, "temperature2": second}}),
            None => json!({"status": {"temperature1": at(0)?}}),
        },
        0x16 => {
            let mut windows = Map::new();
            for (i, b) in data.iter().skip(3).take(4).enumerate() {
                windows.insert((i + 1).to_string(), json!(*b == 1));
            }
            json!({"orientation": {"auto_rotate": at(0)? == 1, "osd_portrait": at(1)? == 1,
                                   "image": name_of(IMAGE_ROTATION, at(2)?), "windows": windows}})
        }
        0xF4 => {
            json!({"led_strip": {"on": at(0)? == 1, "red": at(1)?, "green": at(2)?, "blue": at(3)?}})
        }
        0x5B => {
            let page = req.get(1)?.to_string();
            let days = at(6)?;
            let mut entry = json!({
                "enabled": at(0)? & 0x0F == 1,
                "start_hour": at(1)?, "start_minute": at(2)?, "end_hour": at(3)?, "end_minute": at(4)?,
                "source": name_of(SOURCES, at(5)?), "days": days,
            });
            if let Some(p) = at(7) {
                entry["playlist"] = json!(p);
            }
            json!({"schedules": {page: entry}})
        }
        0xFD => {
            let window = (u16::from(*req.get(1)?) + 1).to_string();
            let word = |i: usize| -> Option<u16> { Some(u16::from_be_bytes([at(i)?, at(i + 1)?])) };
            // The report starts with the window number when the display
            // echoes it: 11 data bytes then, 10 otherwise.
            let o = usize::from(data.len() >= 11);
            json!({"windows": {window: {"rotation": name_of(WINDOW_ROTATION, at(o)?), "x": word(o + 1)?,
                                        "y": word(o + 3)?, "width": word(o + 5)?, "height": word(o + 7)?,
                                        "format": name_of(PICTURE_FORMAT, at(o + 9)?)}}})
        }
        _ => return None,
    };
    let returned = first_leaf(&value);
    Some((value, returned))
}

/// The value a get returns: its report's state, as one object (or its one
/// value).
fn first_leaf(patch: &Value) -> Value {
    match patch {
        Value::Object(m) if m.len() == 1 => first_leaf(m.values().next().unwrap()),
        other => other.clone(),
    }
}

pub(crate) struct PhilipsSicp {
    link: Link,
    monitor: bool,
    monitor_id: u8,
    group: Option<u8>,
    poll_ms: Millis,
    open: bool,
    closed: bool,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    deframer: Deframer,
    queue: VecDeque<Job>,
    in_flight: Option<(Job, Step, Vec<u8>, Millis)>,
    /// The last report of each get, for the messages that carry several
    /// settings at once.
    cache: BTreeMap<u8, Vec<u8>>,
}

impl PhilipsSicp {
    pub(crate) fn new(ctx: OpenContext) -> Result<PhilipsSicp, String> {
        let s = &ctx.settings;
        let link = Link::from_settings(&ctx, DEFAULT_PORT)?;
        let num = |key: &str, default: u64| s.get(key).and_then(Value::as_u64).unwrap_or(default);
        let group_byte = s.get("group_byte").and_then(Value::as_bool).unwrap_or(true);
        Ok(PhilipsSicp {
            link,
            monitor: ctx.monitor,
            monitor_id: num("monitor_id", 1).clamp(1, 255) as u8,
            group: group_byte.then_some(0),
            poll_ms: num("poll_ms", 5_000).max(500),
            open: false,
            closed: false,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            deframer: Deframer::new(group_byte),
            queue: VecDeque::new(),
            in_flight: None,
            cache: BTreeMap::new(),
        })
    }

    fn queue_reads(&mut self, reads: Vec<Vec<u8>>) {
        for get in reads {
            let queued = self
                .queue
                .iter()
                .any(|j| j.id.is_none() && j.steps.front() == Some(&Step::Get(get.clone())))
                || self
                    .in_flight
                    .as_ref()
                    .is_some_and(|(j, s, _, _)| j.id.is_none() && *s == Step::Get(get.clone()));
            if !queued {
                self.queue.push_back(Job::internal(get));
            }
        }
    }

    fn enqueue(&mut self, cx: &mut Cx, job: Job) {
        // Commands go ahead of background reads.
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
        let Some(mut job) = self.queue.pop_front() else {
            return;
        };
        let Some(step) = job.steps.pop_front() else {
            self.pump(cx);
            return;
        };
        let (step, data) = match step {
            Step::Modify { get, set, edits } => match self.cache.get(&get) {
                Some(current) if edits.iter().all(|(i, _)| *i < current.len()) => {
                    let mut data = vec![set];
                    data.extend(current);
                    for (i, v) in &edits {
                        data[i + 1] = *v;
                    }
                    (Step::Set(data.clone()), data)
                }
                _ => {
                    // Not read yet: read it, then make the change.
                    job.steps.push_front(Step::Modify { get, set, edits });
                    (Step::Get(vec![get]), vec![get])
                }
            },
            Step::Get(d) => (Step::Get(d.clone()), d),
            Step::Set(d) => (Step::Set(d.clone()), d),
        };
        cx.tcp_send(SOCKET, frame(self.monitor_id, self.group, &data));
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        cx.set_timer(PROBE_TIMER, PROBE_EVERY);
        self.in_flight = Some((job, step, data, cx.now()));
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.misses = 0;
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
    }

    fn received(&mut self, cx: &mut Cx, message: Message) {
        if message.monitor != self.monitor_id {
            return;
        }
        let Some(&code) = message.data.first() else {
            return;
        };
        let Some((_, step, sent, _)) = &self.in_flight else {
            return;
        };
        let answers = code == 0x00 || step.expects() == Some(code);
        if !answers {
            return;
        }
        let sent = sent.clone();
        let (mut job, step, _, sent_at) = self.in_flight.take().unwrap();
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.heard(cx);
        let body = &message.data[1..];
        let result: Result<Option<Value>, CommandError> = if code == 0x00 {
            match body.first() {
                Some(&ACK) => Ok(None),
                Some(&NACK) => Err(CommandError::DeviceError {
                    code: Some("NACK".into()),
                    message: "the display could not read the command (NACK)".into(),
                }),
                Some(&NAV) => Err(CommandError::DeviceError {
                    code: Some("NAV".into()),
                    message: "the command is not available on this display or not now (NAV)".into(),
                }),
                _ => Err(CommandError::DeviceError {
                    code: None,
                    message: format!("unexpected report {body:02x?}"),
                }),
            }
        } else {
            self.cache.insert(code, body.to_vec());
            match decode(&sent, body) {
                Some((patch, value)) => {
                    if patch.as_object().is_some_and(|m| !m.is_empty()) {
                        cx.state(patch);
                    }
                    Ok(Some(value))
                }
                None => Ok(None),
            }
        };
        match result {
            Err(e) => {
                if let Some(id) = job.id {
                    cx.complete(id, Err(e));
                }
            }
            Ok(value) => {
                if let Step::Set(data) = &step {
                    if let Some(get) = read_back(data) {
                        job.then.push(get);
                    }
                }
                if job.steps.is_empty() {
                    if let Some(id) = job.id {
                        let outcome = match (job.returns_value, value) {
                            (true, Some(v)) => Outcome::Value { value: v },
                            (true, None) => Outcome::Value { value: Value::Null },
                            _ => Outcome::Ack,
                        };
                        cx.complete(id, Ok(outcome));
                    }
                    let reads = std::mem::take(&mut job.then);
                    for get in reads.into_iter().rev() {
                        self.queue.push_front(Job::internal(get));
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
        self.deframer = Deframer::new(self.group.is_some());
        for job in self
            .in_flight
            .take()
            .map(|(j, _, _, _)| j)
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
        for key in [REPLY_TIMER, FAST_TIMER, SLOW_TIMER, PROBE_TIMER] {
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

impl Module for PhilipsSicp {
    fn start(&mut self, cx: &mut Cx) {
        self.open_link(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match plan(name, params) {
            Ok((steps, returns_value)) => self.enqueue(
                cx,
                Job {
                    id: Some(id),
                    steps: steps.into(),
                    returns_value,
                    then: Vec::new(),
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
                self.deframer = Deframer::new(self.group.is_some());
                if self.monitor {
                    self.queue_reads(fast_reads());
                    self.queue_reads(identity_reads());
                    self.queue_reads(slow_reads());
                    cx.set_timer(FAST_TIMER, self.poll_ms);
                    cx.set_timer(SLOW_TIMER, SLOW_EVERY);
                } else {
                    // Nothing to read: the power state shows the display is
                    // there, and the idle check repeats it.
                    self.queue_reads(vec![vec![0x19]]);
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
                self.pump(cx);
            }
            TcpInput::Data(bytes) => {
                for message in self.deframer.feed(&bytes) {
                    self.received(cx, message);
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => self.open_link(cx),
            REPLY_TIMER => {
                if let Some((job, _, _, _)) = self.in_flight.take() {
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
                self.queue_reads(fast_reads());
                self.pump(cx);
                cx.set_timer(FAST_TIMER, self.poll_ms);
            }
            SLOW_TIMER if self.open && self.monitor => {
                self.queue_reads(slow_reads());
                self.pump(cx);
                cx.set_timer(SLOW_TIMER, SLOW_EVERY);
            }
            PROBE_TIMER if self.open => {
                if self.in_flight.is_none() && self.queue.is_empty() {
                    self.queue_reads(vec![vec![0x19]]);
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

    fn module(monitor: bool, settings: Value) -> PhilipsSicp {
        let spec = Catalog::source_tree().devices["philips-sicp"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        PhilipsSicp::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7)),
            host_name: None,
            port: None,
            model: "android".into(),
            channels: None,
            settings,
            monitor,
        })
        .unwrap()
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

    fn opened(m: &mut PhilipsSicp) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    fn feed(m: &mut PhilipsSicp, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut PhilipsSicp, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn report(data: &[u8]) -> Vec<u8> {
        frame(1, Some(0), data)
    }

    #[test]
    fn frames_follow_the_documents_examples() {
        // §4.1: get power, and power off, for display 1.
        assert_eq!(frame(1, Some(0), &[0x19]), [0x05, 0x01, 0x00, 0x19, 0x1D]);
        assert_eq!(
            frame(1, Some(0), &[0x18, 0x01]),
            [0x06, 0x01, 0x00, 0x18, 0x01, 0x1E]
        );
        // Without the group byte (firmware before 1.86, §2.2.1).
        assert_eq!(frame(1, None, &[0x19]), [0x04, 0x01, 0x19, 0x1C]);
        // §4.4: HDMI 1 with its playlist and OSD bytes.
        assert_eq!(
            frame(1, Some(0), &[0xAC, 0x0D, 0x09, 0x01, 0x00]),
            [0x09, 0x01, 0x00, 0xAC, 0x0D, 0x09, 0x01, 0x00, 0xA1]
        );
    }

    #[test]
    fn the_reader_resynchronises_and_takes_any_group() {
        let mut d = Deframer::new(true);
        // §4.4's acknowledgement carries group 01.
        let ack = [0x06, 0x01, 0x01, 0x00, 0x06, 0x00];
        let mut bytes = vec![0x77, 0x02];
        bytes.extend(&ack[..3]);
        assert!(d.feed(&bytes).is_empty());
        assert_eq!(
            d.feed(&ack[3..]),
            [Message {
                monitor: 1,
                data: vec![0x00, 0x06]
            }]
        );
        let mut bad = report(&[0x19, 0x02]);
        bad[4] ^= 0x40;
        assert!(d.feed(&bad).is_empty());
    }

    #[test]
    fn monitored_it_reads_and_keeps_state() {
        let mut m = module(true, json!({}));
        let a = opened(&mut m);
        assert_eq!(sent(&a), [frame(1, Some(0), &[0x19])]);
        let a = feed(&mut m, 4, &report(&[0x19, 0x02]));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(4)));
        assert_eq!(state(&a), json!({"power": true}));
        assert_eq!(sent(&a), [frame(1, Some(0), &[0xAD])]);
        let a = feed(&mut m, 5, &report(&[0xAD, 0x0D, 0x00, 0x01, 0x00]));
        assert_eq!(
            state(&a),
            json!({"input": {"source": "hdmi", "playlist": 0}})
        );
        let a = feed(&mut m, 6, &report(&[0x45, 0x16, 0x0A]));
        assert_eq!(
            state(&a),
            json!({"audio": {"volume": 22, "audio_out_volume": 10}})
        );
    }

    #[test]
    fn a_set_answered_ack_is_read_back() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        feed(&mut m, 1, &report(&[0x19, 0x02]));
        let a = run(&mut m, 1, "set_power", json!({"on": false}));
        assert_eq!(sent(&a), [frame(1, Some(0), &[0x18, 0x01])]);
        let a = feed(&mut m, 2, &report(&[0x00, ACK]));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a), [frame(1, Some(0), &[0x19])]);
        let a = feed(&mut m, 3, &report(&[0x19, 0x01]));
        assert_eq!(state(&a), json!({"power": false}));
        // NAV fails the command.
        run(&mut m, 2, "set_memc", json!({"level": "high"}));
        let a = feed(&mut m, 4, &report(&[0x00, NAV]));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn one_video_parameter_is_changed_with_the_others_as_read() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        feed(&mut m, 1, &report(&[0x19, 0x02]));
        // Not read yet: the parameters are read first.
        let a = run(&mut m, 1, "set_contrast", json!({"level": 70}));
        assert_eq!(sent(&a), [frame(1, Some(0), &[0x33])]);
        let a = feed(&mut m, 2, &report(&[0x33, 50, 50, 50, 50, 50, 50, 0x03]));
        assert_eq!(state(&a)["picture"]["gamma"], json!("2.2"));
        assert_eq!(
            sent(&a),
            [frame(1, Some(0), &[0x32, 50, 50, 70, 50, 50, 50, 0x03])]
        );
        let a = feed(&mut m, 3, &report(&[0x00, ACK]));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a), [frame(1, Some(0), &[0x33])]);
    }

    #[test]
    fn gets_return_their_value() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        feed(&mut m, 1, &report(&[0x19, 0x02]));
        run(&mut m, 1, "get_model", json!({}));
        let mut data = vec![0xA1];
        data.extend(b"55BDL4051D/00");
        let a = feed(&mut m, 2, &report(&data));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!("55BDL4051D/00")
            }))
        );
        assert_eq!(state(&a), json!({"info": {"model": "55BDL4051D/00"}}));
        run(&mut m, 2, "get_operating_hours", json!({}));
        let a = feed(&mut m, 3, &report(&[0x0F, 0x12, 0x34]));
        assert_eq!(state(&a), json!({"status": {"operating_hours": 0x1234}}));
    }

    #[test]
    fn tiling_is_encoded_as_the_document_says() {
        let p = json!({"enabled": true, "frame_compensation": false, "position": 2,
                       "columns": 3, "rows": 2, "zero_bezel": true});
        // §7.6's BDL4675XU example: 09 01 00 22 01 00 02 12 3B.
        let (steps, _) = plan("set_tiling", p.as_object().unwrap()).unwrap();
        assert_eq!(steps, [Step::Set(vec![0x22, 0x01, 0x00, 0x02, 0x12])]);
        assert_eq!(
            frame(1, Some(0), &[0x22, 0x01, 0x00, 0x02, 0x12]),
            [0x09, 0x01, 0x00, 0x22, 0x01, 0x00, 0x02, 0x12, 0x3B]
        );
        let (patch, _) = decode(&[0x23], &[0x01, 0x00, 0x02, 0x12]).unwrap();
        assert_eq!(patch["tiling"]["zero_bezel_columns"], json!(3));
        assert_eq!(patch["tiling"]["zero_bezel_rows"], json!(2));
    }

    #[test]
    fn the_serial_link_and_the_monitor_id_are_used() {
        let mut m = module(
            true,
            json!({"link": "serial", "serial_port": "COM4", "monitor_id": 3, "group_byte": false}),
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::SerialOpen { port, .. } if port.baud_rate == 9600)));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&cx.take()), [vec![0x04, 0x03, 0x19, 0x1E]]);
        // Another display's report on the chain is not the answer.
        let a = feed(&mut m, 1, &frame(2, None, &[0x19, 0x02]));
        assert!(state(&a).as_object().unwrap().is_empty());
        let a = feed(&mut m, 1, &frame(3, None, &[0x19, 0x02]));
        assert_eq!(state(&a), json!({"power": true}));
    }

    #[test]
    fn every_command_of_the_spec_is_sent() {
        let spec = Catalog::source_tree().devices["philips-sicp"].clone();
        for (name, command) in &spec.commands {
            let mut params = Params::new();
            for (p, decl) in &command.params {
                let value = match (&decl.values, decl.kind) {
                    (Some(values), _) => json!(values[0]),
                    (None, crate::catalog::ParamType::Bool) => json!(false),
                    (None, crate::catalog::ParamType::String) => json!("hdmi, ops"),
                    _ => json!(decl.min.unwrap_or(0.0).max(1.0) as i64),
                };
                params.insert(p.clone(), value);
            }
            let planned = plan(name, &params);
            assert!(planned.is_ok(), "{name}: {planned:?}");
        }
    }

    #[test]
    fn unanswered_requests_report_the_display_gone() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        feed(&mut m, 1, &report(&[0x19, 0x02]));
        let mut last = Vec::new();
        for id in 1..=3 {
            run(&mut m, id, "get_power", json!({}));
            let mut cx = Cx::new(0);
            m.timer(&mut cx, REPLY_TIMER);
            last = cx.take();
            assert_eq!(completed(&last, id), Some(Err(CommandError::Timeout)));
        }
        assert!(last
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
    }
}
