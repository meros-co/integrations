//! VISCA PTZ cameras: Sony's VISCA over IP (UDP 52381, 8-byte header), and
//! VISCA without a header over TCP or UDP where vendors document that.
//!
//! Written from Sony's own command lists (see the spec's sources): the
//! "Color Video Camera VISCA Command List" for BRC-X400/SRG-X400/SRG-X120
//! (software version 2.00, E-042-100-12) and the "Command List" for
//! BRC-X1000/H800/H780 (C-456-100-14). Page numbers below refer to the first
//! unless marked "H800". Vendors' differences come from their own documents.
//!
//! - A VISCA message is `8x QQ RR ... FF`, address x locked to 1 over IP. The
//!   camera answers a command with ACK `90 4z FF` (z = the socket that holds
//!   it) and later Completion `90 5z FF`, or an error `90 6z ee FF`; it
//!   answers an inquiry with `90 50 ... FF` and no ACK (p.5, p.7-8).
//! - Over IP every message gets an 8-byte header: payload type (0x0100
//!   command, 0x0110 inquiry, 0x0111 reply, 0x0120 device setting command,
//!   0x0200 control command, 0x0201 control reply), payload length and a
//!   32-bit sequence number that the camera copies into its reply (p.10-11).
//! - Control command RESET (payload 01) sets the camera's sequence number to
//!   0 and is acknowledged with 01; control reply 0F 01 means a sequence
//!   number abnormality, 0F 02 a message type abnormality (p.11).
//! - Delivery is the controller's job: on a timeout the message is sent again
//!   with the same sequence number; ERROR 0F 01 in reply means the first copy
//!   was performed (p.12).
//! - The controller sends the next message only after the previous one's
//!   first reply (ACK or error for a command, the reply for an inquiry); the
//!   camera has two command sockets and answers "buffer full" when both are
//!   busy (p.5, p.12).
//! - Nothing is pushed: zoom, focus and pan/tilt positions are polled often,
//!   settings rarely, and their replies become state.

use std::collections::{BTreeSet, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const SONY_PORT: u16 = 52381;
/// PTZOptics' defaults (PTZOptics HTTP API p.165, p.167).
const PTZOPTICS_TCP_PORT: u16 = 5678;
const PTZOPTICS_UDP_PORT: u16 = 1259;

const SOCKET: Key = "visca";

const T_COMMAND: u16 = 0x0100;
const T_INQUIRY: u16 = 0x0110;
const T_REPLY: u16 = 0x0111;
const T_DEVICE_SETTING: u16 = 0x0120;
const T_CONTROL: u16 = 0x0200;
const T_CONTROL_REPLY: u16 = 0x0201;

/// First reply (ACK, error or inquiry reply) within this. Sony gives 4V,
/// at most 167 ms at 23.98p (p.5); the margin covers the network and a busy
/// camera.
const REPLY_TIMEOUT: Millis = 1_000;
/// A command's Completion follows its motion, which at the slowest pan speed
/// takes minutes across the full range; this only frees the slot eventually.
const COMPLETION_TIMEOUT: Millis = 120_000;
/// Wait after "command buffer full" before sending the command again.
const BUSY_RETRY: Millis = 100;
const BUSY_ATTEMPTS: u8 = 20;
/// Consecutive unanswered messages that mean the camera is gone.
const MISSES_LOST: u32 = 3;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 10_000;
/// Sony turns the tally lamp off 15 s after the last ON from any controller
/// (p.13); ON is repeated within that.
const TALLY_REFRESH: Millis = 10_000;
const DEFAULT_FAST_MS: i64 = 500;
const DEFAULT_SLOW_MS: i64 = 5_000;

const REPLY: Key = "reply";
const COMPLETION: Key = "completion";
const FAST: Key = "poll-fast";
const SLOW: Key = "poll-slow";
const PROBE: Key = "probe";
const TALLY: Key = "tally";
const BUSY: Key = "busy";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transport {
    /// Sony VISCA over IP: UDP with the 8-byte header.
    SonyIp,
    /// VISCA messages with no header on a TCP stream.
    RawTcp,
    /// VISCA messages with no header, one or more per UDP datagram.
    RawUdp,
}

impl Transport {
    fn parse(s: &str) -> Option<Transport> {
        match s {
            "sony-ip" => Some(Transport::SonyIp),
            "raw-tcp" => Some(Transport::RawTcp),
            "raw-udp" => Some(Transport::RawUdp),
            _ => None,
        }
    }

    fn default_port(self) -> u16 {
        match self {
            Transport::SonyIp => SONY_PORT,
            Transport::RawTcp => PTZOPTICS_TCP_PORT,
            Transport::RawUdp => PTZOPTICS_UDP_PORT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Menu {
    /// ENTER is `8x 01 7E 01 02 00 01 FF` (p.21).
    Sony,
    /// Navigation is pan/tilt drive at speed 0E, ENTER `06 06 05`, back
    /// `06 06 04` (PTZOptics VISCA API p.23).
    PtzOptics,
}

/// What differs between camera families, from each vendor's own document.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Profile {
    transport: Transport,
    /// Nibbles in a pan position: 4 on SRG/BRC-X400 (p.20), 5 on
    /// BRC-X1000/H800 (H800 p.17) and Datavideo (Rev 2.7 p.13).
    pan_nibbles: usize,
    /// Absolute and relative moves carry a tilt speed (`VV WW`) rather than
    /// Sony's `vv 00` (PTZOptics p.18, Lumens p.16, Marshall p.12, BirdDog).
    abs_tilt_speed: bool,
    max_pan_speed: u8,
    max_tilt_speed: u8,
    /// Presets as numbered for the operator, from 1.
    max_preset: u16,
    /// Presets above 128 use `3F 1x` (Lumens p.15, Marshall CV730 p.14).
    preset_banks: bool,
    max_preset_speed: u8,
    /// The speed bytes sent with a drive stop.
    stop_speed: u8,
    menu: Menu,
    /// Camera replies go to the controller's UDP 52381 (Canon's default,
    /// "Response Port Number", CR-N500/CR-N300 Settings Guide p.64, p.103).
    fixed_reply_port: bool,
}

impl Profile {
    pub(crate) fn of(model: &str) -> Profile {
        let sony = Profile {
            transport: Transport::SonyIp,
            pan_nibbles: 4,
            abs_tilt_speed: false,
            max_pan_speed: 0x18,
            max_tilt_speed: 0x17,
            max_preset: 100,
            preset_banks: false,
            max_preset_speed: 0x19,
            stop_speed: 0x01,
            menu: Menu::Sony,
            fixed_reply_port: false,
        };
        match model {
            "sony-brc-x1000" => Profile {
                pan_nibbles: 5,
                max_tilt_speed: 0x18,
                max_preset_speed: 0x18,
                ..sony
            },
            "ptzoptics" => Profile {
                transport: Transport::RawTcp,
                abs_tilt_speed: true,
                max_tilt_speed: 0x14,
                max_preset: 128,
                menu: Menu::PtzOptics,
                ..sony
            },
            "birddog" => Profile {
                abs_tilt_speed: true,
                max_tilt_speed: 0x14,
                max_preset: 64,
                ..sony
            },
            "lumens" | "marshall" => Profile {
                abs_tilt_speed: true,
                max_tilt_speed: 0x18,
                max_preset: 256,
                preset_banks: true,
                stop_speed: 0x00,
                ..sony
            },
            "datavideo" => Profile {
                pan_nibbles: 5,
                max_pan_speed: 0x12,
                max_tilt_speed: 0x12,
                max_preset: 128,
                max_preset_speed: 0x12,
                ..sony
            },
            "aver" => Profile {
                max_tilt_speed: 0x18,
                max_preset: 128,
                ..sony
            },
            "canon" => Profile {
                max_tilt_speed: 0x18,
                max_preset: 128,
                fixed_reply_port: true,
                ..sony
            },
            _ => sony,
        }
    }
}

// ---------------------------------------------------------------------------
// Encoding

/// `value` as `n` nibbles, one per byte, most significant first: the
/// `0p 0p 0p 0p` form of the command lists.
pub(crate) fn nibbles(value: u32, n: usize) -> Vec<u8> {
    (0..n)
        .rev()
        .map(|i| ((value >> (4 * i)) & 0x0F) as u8)
        .collect()
}

/// A signed position as `n` nibbles, two's complement in 4n bits (pan
/// DE00 = -170 degrees, p.37). None when it does not fit.
pub(crate) fn signed_nibbles(value: i64, n: usize) -> Option<Vec<u8>> {
    let bits = 4 * n as u32;
    let half = 1i64 << (bits - 1);
    if value < -half || value >= half {
        return None;
    }
    let mask = (1u64 << bits) - 1;
    Some(nibbles(((value as u64) & mask) as u32, n))
}

pub(crate) fn from_nibbles(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0u32, |acc, b| (acc << 4) | u32::from(b & 0x0F))
}

pub(crate) fn from_signed_nibbles(bytes: &[u8]) -> i64 {
    let bits = 4 * bytes.len() as u32;
    let raw = i64::from(from_nibbles(bytes));
    if bits > 0 && raw >= 1i64 << (bits - 1) {
        raw - (1i64 << bits)
    } else {
        raw
    }
}

/// The 8-byte VISCA over IP header and its payload (p.10-11).
pub(crate) fn ip_frame(payload_type: u16, seq: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&payload_type.to_be_bytes());
    out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    out.extend_from_slice(&seq.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// A VISCA over IP message: type, sequence number, payload.
pub(crate) fn parse_ip(data: &[u8]) -> Option<(u16, u32, &[u8])> {
    if data.len() < 8 {
        return None;
    }
    let kind = u16::from_be_bytes([data[0], data[1]]);
    let len = u16::from_be_bytes([data[2], data[3]]) as usize;
    let seq = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let payload = data.get(8..8 + len)?;
    Some((kind, seq, payload))
}

/// VISCA messages from a byte stream, each ending in FF, keeping a partial
/// tail.
#[derive(Default)]
struct Framer {
    buffer: Vec<u8>,
}

impl Framer {
    fn feed(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(data);
        let mut out = Vec::new();
        while let Some(end) = self.buffer.iter().position(|&b| b == 0xFF) {
            let message: Vec<u8> = self.buffer.drain(..=end).collect();
            // A reply starts with its sender's address byte, 9y to Fy.
            if let Some(start) = message.iter().position(|&b| b >= 0x90 && b != 0xFF) {
                out.push(message[start..].to_vec());
            }
        }
        if self.buffer.len() > 256 {
            self.buffer.clear();
        }
        out
    }
}

fn error_meaning(code: u8) -> &'static str {
    // p.5 and p.8.
    match code {
        0x01 => "message length error",
        0x02 => "syntax error: command not supported, or a parameter is invalid",
        0x03 => "command buffer full: two commands are already executing",
        0x04 => "command cancelled",
        0x05 => "no socket: nothing to cancel in that socket",
        0x41 => "command not executable in the camera's current mode",
        _ => "unknown VISCA error",
    }
}

// ---------------------------------------------------------------------------
// Inquiries

#[derive(Debug, Clone, Copy, PartialEq)]
enum Decode {
    /// All bytes' low nibbles: `00 00 0p 0p`, `0p 0p 0p 0p`.
    Number,
    /// The first byte whole: `pp`.
    Byte,
    /// The first byte against an on and an off value.
    Flag(u8, u8),
    Choice(&'static [(u8, &'static str)]),
    /// Pan nibbles then four tilt nibbles, signed.
    PanTilt,
    /// The pan/tilt status code `pp pp` (p.39).
    Status,
    /// CAM_VersionInq: vendor, model, ROM revision, sockets (p.6).
    Version,
    /// The last preset recalled, wire 0-based, 7F for none (p.26).
    LastPreset,
    /// Two bytes, pan and tilt.
    Pair,
    /// The whole reply as hex.
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Poll {
    No,
    Once,
    Fast,
    Slow,
}

#[derive(Debug)]
struct InqDef {
    name: &'static str,
    /// The inquiry after the address byte, without the FF.
    bytes: &'static [u8],
    decode: Decode,
    /// Where the value goes in the state; empty for none.
    path: &'static str,
    poll: Poll,
}

const ON_OFF: Decode = Decode::Flag(0x02, 0x03);

macro_rules! inq {
    ($name:expr, [$($b:expr),*], $decode:expr, $path:expr, $poll:expr) => {
        InqDef { name: $name, bytes: &[$($b),*], decode: $decode, path: $path, poll: $poll }
    };
}

static INQUIRIES: &[InqDef] = &[
    inq!(
        "get_version",
        [0x09, 0x00, 0x02],
        Decode::Version,
        "device",
        Poll::Once
    ),
    inq!(
        "get_power",
        [0x09, 0x04, 0x00],
        Decode::Choice(&[(0x02, "on"), (0x03, "standby"), (0x04, "power_error")]),
        "power",
        Poll::Slow
    ),
    inq!(
        "get_zoom_position",
        [0x09, 0x04, 0x47],
        Decode::Number,
        "zoom.position",
        Poll::Fast
    ),
    inq!(
        "get_zoom_mode",
        [0x09, 0x04, 0x06],
        Decode::Choice(&[(0x02, "digital"), (0x03, "optical"), (0x04, "clear_image")]),
        "zoom.mode",
        Poll::No
    ),
    inq!(
        "get_focus_position",
        [0x09, 0x04, 0x48],
        Decode::Number,
        "focus.position",
        Poll::Fast
    ),
    inq!(
        "get_focus_mode",
        [0x09, 0x04, 0x38],
        Decode::Choice(&[(0x02, "auto"), (0x03, "manual")]),
        "focus.mode",
        Poll::Slow
    ),
    inq!(
        "get_af_sensitivity",
        [0x09, 0x04, 0x58],
        Decode::Choice(&[(0x01, "high"), (0x02, "normal"), (0x03, "low")]),
        "focus.af_sensitivity",
        Poll::No
    ),
    inq!(
        "get_af_mode",
        [0x09, 0x04, 0x57],
        Decode::Choice(&[(0x00, "normal"), (0x01, "interval"), (0x02, "zoom_trigger")]),
        "focus.af_mode",
        Poll::No
    ),
    inq!(
        "get_focus_near_limit",
        [0x09, 0x04, 0x28],
        Decode::Number,
        "focus.near_limit",
        Poll::No
    ),
    inq!(
        "get_pan_tilt_position",
        [0x09, 0x06, 0x12],
        Decode::PanTilt,
        "pan_tilt",
        Poll::Fast
    ),
    inq!(
        "get_pan_tilt_status",
        [0x09, 0x06, 0x10],
        Decode::Status,
        "pan_tilt.status",
        Poll::Fast
    ),
    inq!(
        "get_pan_tilt_max_speed",
        [0x09, 0x06, 0x11],
        Decode::Pair,
        "pan_tilt.max_speed",
        Poll::No
    ),
    inq!(
        "get_pan_tilt_slow",
        [0x09, 0x06, 0x44],
        ON_OFF,
        "pan_tilt.slow",
        Poll::No
    ),
    inq!(
        "get_pan_reverse",
        [0x09, 0x7E, 0x01, 0x06],
        Decode::Flag(0x01, 0x00),
        "pan_tilt.pan_reverse",
        Poll::No
    ),
    inq!(
        "get_tilt_reverse",
        [0x09, 0x7E, 0x01, 0x09],
        Decode::Flag(0x01, 0x00),
        "pan_tilt.tilt_reverse",
        Poll::No
    ),
    inq!(
        "get_white_balance_mode",
        [0x09, 0x04, 0x35],
        Decode::Choice(&[
            (0x00, "auto"),
            (0x01, "indoor"),
            (0x02, "outdoor"),
            (0x03, "one_push"),
            (0x04, "auto2"),
            (0x05, "manual"),
            (0x20, "color_temperature")
        ]),
        "white_balance.mode",
        Poll::Slow
    ),
    inq!(
        "get_red_gain",
        [0x09, 0x04, 0x43],
        Decode::Number,
        "white_balance.red_gain",
        Poll::Slow
    ),
    inq!(
        "get_blue_gain",
        [0x09, 0x04, 0x44],
        Decode::Number,
        "white_balance.blue_gain",
        Poll::Slow
    ),
    inq!(
        "get_white_balance_speed",
        [0x09, 0x04, 0x56],
        Decode::Byte,
        "white_balance.speed",
        Poll::No
    ),
    inq!(
        "get_color_temperature",
        [0x09, 0x04, 0x20],
        Decode::Byte,
        "white_balance.color_temperature",
        Poll::No
    ),
    inq!(
        "get_exposure_mode",
        [0x09, 0x04, 0x39],
        Decode::Choice(&[
            (0x00, "full_auto"),
            (0x03, "manual"),
            (0x0A, "shutter_priority"),
            (0x0B, "iris_priority"),
            (0x0D, "bright"),
            (0x0E, "gain_priority")
        ]),
        "exposure.mode",
        Poll::Slow
    ),
    inq!(
        "get_iris",
        [0x09, 0x04, 0x4B],
        Decode::Number,
        "exposure.iris",
        Poll::Slow
    ),
    inq!(
        "get_shutter",
        [0x09, 0x04, 0x4A],
        Decode::Number,
        "exposure.shutter",
        Poll::Slow
    ),
    inq!(
        "get_gain",
        [0x09, 0x04, 0x4C],
        Decode::Number,
        "exposure.gain",
        Poll::Slow
    ),
    inq!(
        "get_bright",
        [0x09, 0x04, 0x4D],
        Decode::Number,
        "exposure.bright",
        Poll::No
    ),
    inq!(
        "get_gain_limit",
        [0x09, 0x04, 0x2C],
        Decode::Number,
        "exposure.gain_limit",
        Poll::No
    ),
    inq!(
        "get_max_shutter",
        [0x09, 0x05, 0x2A, 0x00],
        Decode::Number,
        "exposure.max_shutter",
        Poll::No
    ),
    inq!(
        "get_min_shutter",
        [0x09, 0x05, 0x2A, 0x01],
        Decode::Number,
        "exposure.min_shutter",
        Poll::No
    ),
    inq!(
        "get_ae_speed",
        [0x09, 0x04, 0x5D],
        Decode::Byte,
        "exposure.ae_speed",
        Poll::No
    ),
    inq!(
        "get_auto_slow_shutter",
        [0x09, 0x04, 0x5A],
        ON_OFF,
        "exposure.auto_slow_shutter",
        Poll::No
    ),
    inq!(
        "get_exposure_compensation",
        [0x09, 0x04, 0x3E],
        ON_OFF,
        "exposure.compensation.enabled",
        Poll::Slow
    ),
    inq!(
        "get_exposure_compensation_level",
        [0x09, 0x04, 0x4E],
        Decode::Number,
        "exposure.compensation.level",
        Poll::Slow
    ),
    inq!(
        "get_backlight",
        [0x09, 0x04, 0x33],
        ON_OFF,
        "exposure.backlight",
        Poll::Slow
    ),
    inq!(
        "get_spotlight",
        [0x09, 0x04, 0x3A],
        ON_OFF,
        "exposure.spotlight",
        Poll::No
    ),
    inq!(
        "get_high_sensitivity",
        [0x09, 0x04, 0x5E],
        ON_OFF,
        "exposure.high_sensitivity",
        Poll::No
    ),
    inq!(
        "get_visibility_enhancer",
        [0x09, 0x04, 0x3D],
        Decode::Flag(0x06, 0x03),
        "exposure.visibility_enhancer",
        Poll::No
    ),
    inq!(
        "get_nd_filter",
        [0x09, 0x7E, 0x01, 0x53],
        Decode::Choice(&[(0x00, "off"), (0x01, "nd4"), (0x02, "nd16"), (0x03, "nd64")]),
        "exposure.nd_filter",
        Poll::No
    ),
    inq!(
        "get_detail_level",
        [0x09, 0x04, 0x42],
        Decode::Number,
        "image.detail_level",
        Poll::No
    ),
    inq!(
        "get_aperture_mode",
        [0x09, 0x04, 0x05],
        Decode::Choice(&[(0x02, "auto"), (0x03, "manual")]),
        "image.aperture_mode",
        Poll::No
    ),
    inq!(
        "get_noise_reduction",
        [0x09, 0x04, 0x53],
        Decode::Byte,
        "image.noise_reduction",
        Poll::No
    ),
    inq!(
        "get_picture_effect",
        [0x09, 0x04, 0x63],
        Decode::Choice(&[(0x00, "off"), (0x02, "negative"), (0x04, "black_white")]),
        "image.picture_effect",
        Poll::No
    ),
    inq!(
        "get_flip",
        [0x09, 0x04, 0x66],
        ON_OFF,
        "image.flip",
        Poll::Slow
    ),
    inq!(
        "get_mirror",
        [0x09, 0x04, 0x61],
        ON_OFF,
        "image.mirror",
        Poll::No
    ),
    inq!(
        "get_image_stabilizer",
        [0x09, 0x04, 0x34],
        ON_OFF,
        "image.stabilizer",
        Poll::No
    ),
    inq!(
        "get_freeze",
        [0x09, 0x04, 0x62],
        ON_OFF,
        "image.freeze",
        Poll::No
    ),
    inq!(
        "get_last_preset",
        [0x09, 0x04, 0x3F],
        Decode::LastPreset,
        "presets.last_recalled",
        Poll::Slow
    ),
    inq!(
        "get_preset_speed_mode",
        [0x09, 0x7E, 0x04, 0x1B],
        Decode::Choice(&[(0x00, "compatible"), (0x01, "separate"), (0x02, "common")]),
        "presets.speed_mode",
        Poll::No
    ),
    inq!(
        "get_preset_common_speed",
        [0x09, 0x7E, 0x04, 0x1C],
        Decode::Number,
        "presets.common_speed",
        Poll::No
    ),
    inq!(
        "get_tally",
        [0x09, 0x7E, 0x01, 0x0A],
        ON_OFF,
        "tally.on",
        Poll::Slow
    ),
    inq!(
        "get_ir_receive",
        [0x09, 0x06, 0x08],
        ON_OFF,
        "ir.receive",
        Poll::No
    ),
    inq!(
        "get_ir_cut_filter",
        [0x09, 0x04, 0x01],
        ON_OFF,
        "ir.cut_filter_night",
        Poll::No
    ),
    inq!(
        "get_menu",
        [0x09, 0x06, 0x06],
        ON_OFF,
        "menu.open",
        Poll::Slow
    ),
    inq!(
        "get_camera_id",
        [0x09, 0x04, 0x22],
        Decode::Number,
        "device.camera_id",
        Poll::No
    ),
];

static PRESET_SPEED_INQ: InqDef = inq!("get_preset_speed", [], Decode::Byte, "", Poll::No);
static PAN_TILT_LIMIT_INQ: InqDef = inq!("get_pan_tilt_limit", [], Decode::PanTilt, "", Poll::No);
static RAW_INQ: InqDef = inq!("raw_inquiry", [], Decode::Raw, "", Poll::No);

fn inquiry(name: &str) -> Option<&'static InqDef> {
    INQUIRIES.iter().find(|d| d.name == name)
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// An inquiry reply's data (after `90 50`, before FF) as a value.
fn decode(decode: Decode, reply: &[u8]) -> Option<Value> {
    let data = reply.get(2..reply.len().saturating_sub(1)).unwrap_or(&[]);
    let first = data.first().copied();
    Some(match decode {
        Decode::Number => {
            if data.is_empty() {
                return None;
            }
            json!(from_nibbles(data))
        }
        Decode::Byte => json!(first?),
        Decode::Flag(on, off) => match first? {
            b if b == on => json!(true),
            b if b == off => json!(false),
            _ => return None,
        },
        Decode::Choice(table) => {
            let b = first?;
            json!(table.iter().find(|(v, _)| *v == b)?.1)
        }
        Decode::PanTilt => {
            if data.len() < 5 {
                return None;
            }
            let split = data.len() - 4;
            json!({
                "pan": from_signed_nibbles(&data[..split]),
                "tilt": from_signed_nibbles(&data[split..]),
            })
        }
        Decode::Status => {
            let code = u16::from_be_bytes([*data.first()?, *data.get(1)?]);
            let motion = ["idle", "moving", "complete", "failed"][usize::from((code >> 10) & 3)];
            let init = ["not_initialized", "initializing", "initialized", "failed"]
                [usize::from((code >> 12) & 3)];
            json!({"code": code, "motion": motion, "initialization": init})
        }
        Decode::Version => {
            if data.len() < 7 {
                return None;
            }
            json!({
                "vendor_id": u16::from_be_bytes([data[0], data[1]]),
                "model_id": u16::from_be_bytes([data[2], data[3]]),
                "rom_version": u16::from_be_bytes([data[4], data[5]]),
                "sockets": data[6],
            })
        }
        Decode::LastPreset => match first? {
            0x7F => Value::Null,
            b => json!(u32::from(b) + 1),
        },
        Decode::Pair => json!({"pan": first?, "tilt": *data.get(1)?}),
        Decode::Raw => json!(hex(reply)),
    })
}

/// `a.b.c` and a value as a nested merge patch.
fn at_path(path: &str, value: Value) -> Value {
    path.rsplit('.').fold(value, |inner, key| {
        let mut m = Map::new();
        m.insert(key.to_string(), inner);
        Value::Object(m)
    })
}

// ---------------------------------------------------------------------------
// Commands

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Command,
    Inquiry,
    /// IF_Clear: answered with `90 50 FF`, no socket (p.6, p.10).
    DeviceSetting,
    /// Cancel the command in this socket (p.6-7).
    Cancel(u8),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Origin {
    User(CommandId),
    Poll,
    /// Repeated tally ON.
    Tally,
}

#[derive(Debug, Clone)]
struct Msg {
    /// The whole VISCA message, `81 ... FF`.
    payload: Vec<u8>,
    kind: Kind,
    origin: Origin,
    inquiry: Option<&'static InqDef>,
    /// Inquiry to send after the command completes, to refresh its state.
    follow: Option<&'static str>,
    busy_attempts: u8,
    seq_resets: u8,
}

#[derive(Debug)]
struct Flight {
    msg: Msg,
    seq: u32,
    retransmitted: bool,
}

#[derive(Debug)]
struct Executing {
    msg: Msg,
    socket: u8,
    seq: u32,
    deadline: Millis,
}

enum Built {
    Ready(Vec<u8>, Kind, Option<&'static InqDef>, Option<&'static str>),
    Invalid(String),
    Unknown,
}

fn on_off(on: bool) -> u8 {
    if on {
        0x02
    } else {
        0x03
    }
}

fn step(action: &str) -> u8 {
    match action {
        "up" => 0x02,
        "down" => 0x03,
        _ => 0x00, // reset
    }
}

pub(crate) struct Visca {
    device: SocketAddr,
    profile: Profile,
    transport: Transport,
    bind: Bind,
    fast_ms: Millis,
    slow_ms: Millis,
    seq: u32,
    /// The socket (UDP) or connection (TCP) is open.
    link: bool,
    /// Sony IP: RESET acknowledged, messages may flow.
    ready: bool,
    connected: bool,
    framer: Framer,
    commands: VecDeque<Msg>,
    polls: VecDeque<Msg>,
    in_flight: Option<Flight>,
    executing: Vec<Executing>,
    busy_until: Millis,
    unsupported: BTreeSet<&'static str>,
    misses: u32,
    retry_after: Millis,
    tally_on: bool,
}

impl Visca {
    pub(crate) fn new(ctx: OpenContext) -> Visca {
        let mut profile = Profile::of(&ctx.model);
        if let Some(t) = ctx
            .settings
            .get("transport")
            .and_then(Value::as_str)
            .and_then(Transport::parse)
        {
            profile.transport = t;
        }
        if let Some(fixed) = ctx
            .settings
            .get("fixed_reply_port")
            .and_then(Value::as_bool)
        {
            profile.fixed_reply_port = fixed;
        }
        let ms = |key: &str, default: i64| {
            ctx.settings
                .get(key)
                .and_then(Value::as_i64)
                .unwrap_or(default)
                .max(0) as Millis
        };
        let port = ctx.port.unwrap_or(profile.transport.default_port());
        let mut m = Visca::for_device(SocketAddr::new(ctx.host, port), profile);
        m.fast_ms = ms("poll_position_ms", DEFAULT_FAST_MS);
        m.slow_ms = ms("poll_settings_ms", DEFAULT_SLOW_MS);
        m
    }

    pub(crate) fn for_device(device: SocketAddr, profile: Profile) -> Visca {
        Visca {
            device,
            profile,
            transport: profile.transport,
            bind: if profile.fixed_reply_port && profile.transport != Transport::RawTcp {
                Bind::Shared(SONY_PORT)
            } else {
                Bind::Ephemeral
            },
            fast_ms: DEFAULT_FAST_MS as Millis,
            slow_ms: DEFAULT_SLOW_MS as Millis,
            seq: 0,
            link: false,
            ready: false,
            connected: false,
            framer: Framer::default(),
            commands: VecDeque::new(),
            polls: VecDeque::new(),
            in_flight: None,
            executing: Vec::new(),
            busy_until: 0,
            unsupported: BTreeSet::new(),
            misses: 0,
            retry_after: RETRY_MIN,
            tally_on: false,
        }
    }

    // -- building -----------------------------------------------------------

    fn build(&self, name: &str, params: &Params) -> Built {
        use Built::{Invalid, Ready};
        let p = &self.profile;
        let int = |k: &str| params.get(k).and_then(Value::as_i64);
        let flag = |k: &str| params.get(k).and_then(Value::as_bool).unwrap_or(true);
        let text = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or("");
        let cmd = |body: &[u8]| -> Vec<u8> {
            let mut v = vec![0x81, 0x01];
            v.extend_from_slice(body);
            v.push(0xFF);
            v
        };
        let command = |body: &[u8], follow: Option<&'static str>| {
            Ready(cmd(body), Kind::Command, None, follow)
        };
        let direct2 = |prefix: &[u8], v: i64| {
            let mut b = prefix.to_vec();
            b.extend_from_slice(&[0x00, 0x00]);
            b.extend(nibbles(v as u32, 2));
            b
        };

        if let Some(def) = inquiry(name) {
            let mut v = vec![0x81];
            v.extend_from_slice(def.bytes);
            v.push(0xFF);
            return Ready(v, Kind::Inquiry, Some(def), None);
        }

        match name {
            "power" => command(&[0x04, 0x00, on_off(flag("on"))], Some("get_power")),
            "if_clear" => Ready(
                vec![0x81, 0x01, 0x00, 0x01, 0xFF],
                Kind::DeviceSetting,
                None,
                None,
            ),
            "cancel" => {
                let s = int("socket").unwrap_or(1) as u8;
                Ready(vec![0x81, 0x20 | s, 0xFF], Kind::Cancel(s), None, None)
            }

            // Zoom (p.18).
            "zoom" => {
                let action = text("action");
                let speed = int("speed");
                let b = match (action, speed) {
                    ("stop", _) => 0x00,
                    ("tele", None) => 0x02,
                    ("wide", None) => 0x03,
                    ("tele", Some(s)) => 0x20 | s as u8,
                    (_, Some(s)) => 0x30 | s as u8,
                    _ => 0x03,
                };
                command(&[0x04, 0x07, b], None)
            }
            "zoom_to" => {
                let mut b = vec![0x04, 0x47];
                b.extend(nibbles(int("position").unwrap_or(0) as u32, 4));
                command(&b, None)
            }
            "zoom_mode" => {
                let b = match text("mode") {
                    "digital" => 0x02,
                    "clear_image" => 0x04,
                    _ => 0x03,
                };
                command(&[0x04, 0x06, b], Some("get_zoom_mode"))
            }

            // Focus (p.19).
            "focus" => {
                let b = match (text("action"), int("speed")) {
                    ("stop", _) => 0x00,
                    ("far", None) => 0x02,
                    ("near", None) => 0x03,
                    ("far", Some(s)) => 0x20 | s as u8,
                    (_, Some(s)) => 0x30 | s as u8,
                    _ => 0x03,
                };
                command(&[0x04, 0x08, b], None)
            }
            "focus_to" => {
                let mut b = vec![0x04, 0x48];
                b.extend(nibbles(int("position").unwrap_or(0) as u32, 4));
                command(&b, None)
            }
            "focus_mode" => {
                let b = match text("mode") {
                    "auto" => 0x02,
                    "manual" => 0x03,
                    _ => 0x10,
                };
                command(&[0x04, 0x38, b], Some("get_focus_mode"))
            }
            "focus_one_push" => command(&[0x04, 0x18, 0x01], None),
            "focus_infinity" => command(&[0x04, 0x18, 0x02], None),
            "focus_near_limit" => {
                let mut b = vec![0x04, 0x28];
                b.extend(nibbles(int("position").unwrap_or(0) as u32, 4));
                command(&b, Some("get_focus_near_limit"))
            }
            "af_mode" => {
                let b = match text("mode") {
                    "interval" => 1,
                    "zoom_trigger" => 2,
                    _ => 0,
                };
                command(&[0x04, 0x57, b], Some("get_af_mode"))
            }
            "af_interval" => {
                let mut b = vec![0x04, 0x27];
                b.extend(nibbles(int("operating_s").unwrap_or(0) as u32, 2));
                b.extend(nibbles(int("staying_s").unwrap_or(0) as u32, 2));
                command(&b, None)
            }
            "af_sensitivity" => {
                let b = match text("level") {
                    "high" => 1,
                    "low" => 3,
                    _ => 2,
                };
                command(&[0x04, 0x58, b], Some("get_af_sensitivity"))
            }
            "ir_correction" => {
                let b = u8::from(text("mode") == "ir_light");
                command(&[0x04, 0x11, b], None)
            }

            // Pan/tilt (p.20).
            "pan_tilt" => {
                let (ps, ts) = (
                    int("pan_speed").unwrap_or(1),
                    int("tilt_speed").unwrap_or(1),
                );
                if ps > i64::from(p.max_pan_speed) || ts > i64::from(p.max_tilt_speed) {
                    return Invalid(format!(
                        "this model's speeds are pan 1-{} and tilt 1-{}",
                        p.max_pan_speed, p.max_tilt_speed
                    ));
                }
                let pan = match text("pan") {
                    "left" => 0x01,
                    "right" => 0x02,
                    _ => 0x03,
                };
                let tilt = match text("tilt") {
                    "up" => 0x01,
                    "down" => 0x02,
                    _ => 0x03,
                };
                command(&[0x06, 0x01, ps as u8, ts as u8, pan, tilt], None)
            }
            "pan_tilt_stop" => command(&[0x06, 0x01, p.stop_speed, p.stop_speed, 0x03, 0x03], None),
            "pan_tilt_absolute" | "pan_tilt_relative" => {
                let speed = int("speed").unwrap_or(1);
                let tilt_speed = int("tilt_speed").unwrap_or(speed);
                if speed > i64::from(p.max_pan_speed)
                    || (p.abs_tilt_speed && tilt_speed > i64::from(p.max_tilt_speed))
                {
                    return Invalid(format!(
                        "this model's speeds are pan 1-{} and tilt 1-{}",
                        p.max_pan_speed, p.max_tilt_speed
                    ));
                }
                let Some(pan) = signed_nibbles(int("pan").unwrap_or(0), p.pan_nibbles) else {
                    return Invalid(format!(
                        "pan does not fit this model's {} position digits",
                        p.pan_nibbles
                    ));
                };
                let Some(tilt) = signed_nibbles(int("tilt").unwrap_or(0), 4) else {
                    return Invalid("tilt does not fit four position digits".into());
                };
                let op = if name == "pan_tilt_absolute" {
                    0x02
                } else {
                    0x03
                };
                let second = if p.abs_tilt_speed {
                    tilt_speed as u8
                } else {
                    0x00
                };
                let mut b = vec![0x06, op, speed as u8, second];
                b.extend(pan);
                b.extend(tilt);
                command(&b, None)
            }
            "pan_tilt_home" => command(&[0x06, 0x04], None),
            "pan_tilt_reset" => command(&[0x06, 0x05], None),
            "pan_tilt_limit_set" => {
                let corner = u8::from(text("corner") == "up_right");
                let Some(pan) = signed_nibbles(int("pan").unwrap_or(0), p.pan_nibbles) else {
                    return Invalid(format!(
                        "pan does not fit this model's {} position digits",
                        p.pan_nibbles
                    ));
                };
                let Some(tilt) = signed_nibbles(int("tilt").unwrap_or(0), 4) else {
                    return Invalid("tilt does not fit four position digits".into());
                };
                let mut b = vec![0x06, 0x07, 0x00, corner];
                b.extend(pan);
                b.extend(tilt);
                command(&b, None)
            }
            "pan_tilt_limit_clear" => {
                // 07 0F .. for pan, 07 0F 0F 0F for tilt (p.20; H800 p.18).
                let corner = u8::from(text("corner") == "up_right");
                let mut b = vec![0x06, 0x07, 0x01, corner, 0x07];
                b.extend(std::iter::repeat_n(0x0F, p.pan_nibbles - 1));
                b.extend_from_slice(&[0x07, 0x0F, 0x0F, 0x0F]);
                command(&b, None)
            }
            "pan_tilt_ramp_curve" => {
                let b = match text("curve") {
                    "standard" => 2,
                    "gentle" => 3,
                    _ => 1,
                };
                command(&[0x06, 0x31, b], None)
            }
            "pan_tilt_slow" => command(
                &[0x06, 0x44, on_off(flag("enabled"))],
                Some("get_pan_tilt_slow"),
            ),
            "pan_reverse" => command(
                &[0x7E, 0x01, 0x06, 0x00, u8::from(flag("enabled"))],
                Some("get_pan_reverse"),
            ),
            "tilt_reverse" => command(
                &[0x7E, 0x01, 0x09, 0x00, u8::from(flag("enabled"))],
                Some("get_tilt_reverse"),
            ),
            "get_pan_tilt_limit" => {
                let corner = u8::from(text("corner") == "up_right");
                Ready(
                    vec![0x81, 0x09, 0x06, 0x07, corner, 0xFF],
                    Kind::Inquiry,
                    Some(&PAN_TILT_LIMIT_INQ),
                    None,
                )
            }

            // Presets (p.21).
            "preset_recall" | "preset_set" | "preset_reset" => {
                let preset = int("preset").unwrap_or(1);
                if preset > i64::from(p.max_preset) {
                    return Invalid(format!("this model has presets 1-{}", p.max_preset));
                }
                let op = match name {
                    "preset_reset" => 0x00,
                    "preset_set" => 0x01,
                    _ => 0x02,
                };
                let wire = preset - 1;
                let (op, pp) = if p.preset_banks && wire >= 128 {
                    (op | 0x10, (wire - 128) as u8)
                } else {
                    (op, wire as u8)
                };
                let follow = (name == "preset_recall").then_some("get_last_preset");
                command(&[0x04, 0x3F, op, pp], follow)
            }
            "preset_speed" => {
                let preset = int("preset").unwrap_or(1);
                let speed = int("speed").unwrap_or(1);
                if preset > i64::from(p.max_preset.min(128))
                    || speed > i64::from(p.max_preset_speed)
                {
                    return Invalid(format!(
                        "this model takes presets 1-{} and speeds 1-{}",
                        p.max_preset.min(128),
                        p.max_preset_speed
                    ));
                }
                command(&[0x7E, 0x01, 0x0B, (preset - 1) as u8, speed as u8], None)
            }
            "get_preset_speed" => {
                let preset = int("preset").unwrap_or(1);
                if preset > i64::from(p.max_preset.min(128)) {
                    return Invalid(format!(
                        "this model has presets 1-{}",
                        p.max_preset.min(128)
                    ));
                }
                Ready(
                    vec![0x81, 0x09, 0x7E, 0x01, 0x0B, (preset - 1) as u8, 0xFF],
                    Kind::Inquiry,
                    Some(&PRESET_SPEED_INQ),
                    None,
                )
            }
            "preset_speed_mode" => {
                let b = match text("mode") {
                    "separate" => 1,
                    "common" => 2,
                    _ => 0,
                };
                command(&[0x7E, 0x04, 0x1B, b], Some("get_preset_speed_mode"))
            }
            "preset_common_speed" => {
                let speed = int("speed").unwrap_or(1);
                if speed > i64::from(p.max_preset_speed) {
                    return Invalid(format!(
                        "this model's preset speeds are 1-{}",
                        p.max_preset_speed
                    ));
                }
                let mut b = vec![0x7E, 0x04, 0x1C];
                b.extend(nibbles(speed as u32, 2));
                command(&b, Some("get_preset_common_speed"))
            }
            "preset_call_mode" => {
                let b = if text("mode") == "freeze" { 0x02 } else { 0x03 };
                command(&[0x7E, 0x04, 0x3B, b], None)
            }
            "preset_mode" => {
                let b = match text("mode") {
                    "mode2" => 0x01,
                    "trace" => 0x10,
                    _ => 0x00,
                };
                command(&[0x7E, 0x04, 0x3D, b], None)
            }

            // Exposure (p.14-15; H800 p.14).
            "exposure_mode" => {
                let b = match text("mode") {
                    "manual" => 0x03,
                    "shutter_priority" => 0x0A,
                    "iris_priority" => 0x0B,
                    "bright" => 0x0D,
                    "gain_priority" => 0x0E,
                    _ => 0x00,
                };
                command(&[0x04, 0x39, b], Some("get_exposure_mode"))
            }
            "iris" => command(&[0x04, 0x0B, step(text("action"))], Some("get_iris")),
            "iris_direct" => command(
                &direct2(&[0x04, 0x4B], int("position").unwrap_or(0)),
                Some("get_iris"),
            ),
            "gain" => command(&[0x04, 0x0C, step(text("action"))], Some("get_gain")),
            "gain_direct" => command(
                &direct2(&[0x04, 0x4C], int("position").unwrap_or(0)),
                Some("get_gain"),
            ),
            "gain_limit" => command(
                &[0x04, 0x2C, int("limit").unwrap_or(0) as u8],
                Some("get_gain_limit"),
            ),
            "shutter" => command(&[0x04, 0x0A, step(text("action"))], Some("get_shutter")),
            "shutter_direct" => command(
                &direct2(&[0x04, 0x4A], int("position").unwrap_or(0)),
                Some("get_shutter"),
            ),
            "max_shutter" | "min_shutter" => {
                let which = u8::from(name == "min_shutter");
                let mut b = vec![0x05, 0x2A, which];
                b.extend(nibbles(int("position").unwrap_or(0) as u32, 2));
                command(&b, None)
            }
            "auto_slow_shutter" => command(
                &[0x04, 0x5A, on_off(flag("enabled"))],
                Some("get_auto_slow_shutter"),
            ),
            "ae_speed" => command(
                &[0x04, 0x5D, int("speed").unwrap_or(1) as u8],
                Some("get_ae_speed"),
            ),
            "bright" => command(&[0x04, 0x0D, step(text("action"))], Some("get_bright")),
            "bright_direct" => command(
                &direct2(&[0x04, 0x4D], int("position").unwrap_or(0)),
                Some("get_bright"),
            ),
            "exposure_compensation" => command(
                &[0x04, 0x3E, on_off(flag("enabled"))],
                Some("get_exposure_compensation"),
            ),
            "exposure_compensation_level" => command(
                &direct2(&[0x04, 0x4E], int("level").unwrap_or(7)),
                Some("get_exposure_compensation_level"),
            ),
            "exposure_compensation_step" => command(
                &[0x04, 0x0E, step(text("action"))],
                Some("get_exposure_compensation_level"),
            ),
            "backlight" => command(
                &[0x04, 0x33, on_off(flag("enabled"))],
                Some("get_backlight"),
            ),
            "spotlight" => command(
                &[0x04, 0x3A, on_off(flag("enabled"))],
                Some("get_spotlight"),
            ),
            "high_sensitivity" => command(
                &[0x04, 0x5E, on_off(flag("enabled"))],
                Some("get_high_sensitivity"),
            ),
            "visibility_enhancer" => command(
                &[0x04, 0x3D, if flag("enabled") { 0x06 } else { 0x03 }],
                Some("get_visibility_enhancer"),
            ),
            "nd_filter" => {
                let b = match text("filter") {
                    "nd4" => 1,
                    "nd16" => 2,
                    "nd64" => 3,
                    _ => 0,
                };
                command(&[0x7E, 0x01, 0x53, b], Some("get_nd_filter"))
            }

            // White balance (p.16).
            "white_balance_mode" => {
                let b = match text("mode") {
                    "indoor" => 0x01,
                    "outdoor" => 0x02,
                    "one_push" => 0x03,
                    "auto2" => 0x04,
                    "manual" => 0x05,
                    "color_temperature" => 0x20,
                    _ => 0x00,
                };
                command(&[0x04, 0x35, b], Some("get_white_balance_mode"))
            }
            "white_balance_one_push" => command(&[0x04, 0x10, 0x05], None),
            "red_gain" => command(
                &direct2(&[0x04, 0x43], int("gain").unwrap_or(0x80)),
                Some("get_red_gain"),
            ),
            "blue_gain" => command(
                &direct2(&[0x04, 0x44], int("gain").unwrap_or(0x80)),
                Some("get_blue_gain"),
            ),
            "red_gain_step" => command(&[0x04, 0x03, step(text("action"))], Some("get_red_gain")),
            "blue_gain_step" => command(&[0x04, 0x04, step(text("action"))], Some("get_blue_gain")),
            "white_balance_speed" => command(
                &[0x04, 0x56, int("speed").unwrap_or(3) as u8],
                Some("get_white_balance_speed"),
            ),
            "color_temperature" => {
                let mut b = vec![0x04, 0x20];
                b.extend(nibbles(int("position").unwrap_or(0) as u32, 2));
                command(&b, Some("get_color_temperature"))
            }

            // Image (p.17-18).
            "detail_level" => command(
                &direct2(&[0x04, 0x42], int("level").unwrap_or(7)),
                Some("get_detail_level"),
            ),
            "detail_step" => command(
                &[0x04, 0x02, step(text("action"))],
                Some("get_detail_level"),
            ),
            "aperture_mode" => command(
                &[
                    0x04,
                    0x05,
                    if text("mode") == "manual" { 0x03 } else { 0x02 },
                ],
                Some("get_aperture_mode"),
            ),
            "noise_reduction" => command(
                &[0x04, 0x53, int("level").unwrap_or(0) as u8],
                Some("get_noise_reduction"),
            ),
            "picture_effect" => {
                let b = match text("effect") {
                    "negative" => 0x02,
                    "black_white" => 0x04,
                    _ => 0x00,
                };
                command(&[0x04, 0x63, b], Some("get_picture_effect"))
            }
            "flip" => command(&[0x04, 0x66, on_off(flag("enabled"))], Some("get_flip")),
            "mirror" => command(&[0x04, 0x61, on_off(flag("enabled"))], Some("get_mirror")),
            "image_stabilizer" => command(
                &[0x04, 0x34, on_off(flag("enabled"))],
                Some("get_image_stabilizer"),
            ),
            "freeze" => command(&[0x04, 0x62, on_off(flag("enabled"))], Some("get_freeze")),

            // System (p.21).
            "tally" => command(
                &[0x7E, 0x01, 0x0A, 0x00, on_off(flag("on"))],
                Some("get_tally"),
            ),
            "tally_level" => {
                let b = match text("level") {
                    "low" => 0x04,
                    "high" => 0x05,
                    _ => 0x00,
                };
                command(&[0x7E, 0x01, 0x0A, 0x01, b], None)
            }
            "ir_receive" => {
                let b = match text("mode") {
                    "on" => 0x02,
                    "off" => 0x03,
                    _ => 0x10,
                };
                command(&[0x06, 0x08, b], Some("get_ir_receive"))
            }
            "ir_cut_filter" => command(
                &[0x04, 0x01, on_off(flag("night"))],
                Some("get_ir_cut_filter"),
            ),
            "auto_icr" => command(&[0x04, 0x51, on_off(flag("enabled"))], None),
            "camera_id" => {
                let mut b = vec![0x04, 0x22];
                b.extend(nibbles(int("id").unwrap_or(0) as u32, 4));
                command(&b, Some("get_camera_id"))
            }
            "menu" => {
                let b = match text("mode") {
                    "on" => 0x02,
                    "off" => 0x03,
                    _ => 0x10,
                };
                command(&[0x06, 0x06, b], Some("get_menu"))
            }
            "menu_enter" => match p.menu {
                Menu::Sony => command(&[0x7E, 0x01, 0x02, 0x00, 0x01], None),
                Menu::PtzOptics => command(&[0x06, 0x06, 0x05], None),
            },
            "menu_back" => command(&[0x06, 0x06, 0x04], None),
            "menu_navigate" => {
                let (pan, tilt) = match text("direction") {
                    "up" => (0x03, 0x01),
                    "down" => (0x03, 0x02),
                    "left" => (0x01, 0x03),
                    _ => (0x02, 0x03),
                };
                command(&[0x06, 0x01, 0x0E, 0x0E, pan, tilt], None)
            }

            "raw_command" | "raw_inquiry" => {
                let Some(bytes) = parse_hex(text("hex")) else {
                    return Invalid("hex must be pairs of hex digits".into());
                };
                let inquiry = name == "raw_inquiry";
                let ok = bytes.len() >= 3
                    && bytes.len() <= 16
                    && bytes[0] == 0x81
                    && bytes[1] == if inquiry { 0x09 } else { 0x01 }
                    && bytes[bytes.len() - 1] == 0xFF
                    && !bytes[..bytes.len() - 1].contains(&0xFF);
                if !ok {
                    return Invalid(format!(
                        "a VISCA {} is 81 {} ... FF, 3 to 16 bytes, with FF only at the end",
                        if inquiry { "inquiry" } else { "command" },
                        if inquiry { "09" } else { "01" }
                    ));
                }
                if inquiry {
                    Ready(bytes, Kind::Inquiry, Some(&RAW_INQ), None)
                } else {
                    Ready(bytes, Kind::Command, None, None)
                }
            }
            _ => Built::Unknown,
        }
    }

    // -- transport ----------------------------------------------------------

    fn open(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        match self.transport {
            Transport::RawTcp => cx.tcp_open(SOCKET, self.device),
            _ => {
                cx.udp_open(SOCKET, self.bind);
                self.link = true;
                self.probe(cx);
            }
        }
    }

    /// Sony IP: RESET the sequence number. Headerless: ask the version.
    fn probe(&mut self, cx: &mut Cx) {
        match self.transport {
            Transport::SonyIp => {
                self.ready = false;
                let frame = ip_frame(T_CONTROL, self.seq, &[0x01]);
                cx.udp_send(SOCKET, self.device, frame);
            }
            _ => {
                self.ready = true;
                if !self
                    .polls
                    .iter()
                    .any(|m| m.inquiry.is_some_and(|d| d.name == "get_version"))
                {
                    self.queue_poll(inquiry("get_version").unwrap());
                }
                self.pump(cx);
            }
        }
        cx.set_timer(PROBE, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn transmit(&mut self, cx: &mut Cx, seq: u32, msg: &Msg) {
        match self.transport {
            Transport::SonyIp => {
                let t = match msg.kind {
                    Kind::Inquiry => T_INQUIRY,
                    Kind::DeviceSetting => T_DEVICE_SETTING,
                    _ => T_COMMAND,
                };
                cx.udp_send(SOCKET, self.device, ip_frame(t, seq, &msg.payload));
            }
            Transport::RawUdp => cx.udp_send(SOCKET, self.device, msg.payload.clone()),
            Transport::RawTcp => cx.tcp_send(SOCKET, msg.payload.clone()),
        }
    }

    fn queue_poll(&mut self, def: &'static InqDef) {
        let mut payload = vec![0x81];
        payload.extend_from_slice(def.bytes);
        payload.push(0xFF);
        self.polls.push_back(Msg {
            payload,
            kind: Kind::Inquiry,
            origin: Origin::Poll,
            inquiry: Some(def),
            follow: None,
            busy_attempts: 0,
            seq_resets: 0,
        });
    }

    fn schedule_polls(&mut self, which: Poll) {
        // A round still waiting is not doubled.
        let pending: BTreeSet<&str> = self
            .polls
            .iter()
            .filter_map(|m| m.inquiry.map(|d| d.name))
            .collect();
        for def in INQUIRIES.iter().filter(|d| d.poll == which) {
            if !self.unsupported.contains(def.name) && !pending.contains(def.name) {
                self.queue_poll(def);
            }
        }
    }

    /// Send the next message if nothing awaits its first reply. Commands go
    /// ahead of polls; a command waits while both sockets are busy or after
    /// "buffer full", but inquiries still go (p.5).
    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || !self.link || !self.ready {
            return;
        }
        let blocked = |m: &Msg, s: &Visca| {
            matches!(m.kind, Kind::Command) && (s.executing.len() >= 2 || cx.now() < s.busy_until)
        };
        let command_ready = self.commands.front().is_some_and(|m| !blocked(m, self));
        let msg = if command_ready {
            self.commands.pop_front()
        } else {
            self.polls.pop_front()
        };
        let Some(msg) = msg else {
            return;
        };
        self.seq = self.seq.wrapping_add(1);
        let seq = self.seq;
        self.transmit(cx, seq, &msg);
        self.in_flight = Some(Flight {
            msg,
            seq,
            retransmitted: false,
        });
        cx.set_timer(REPLY, REPLY_TIMEOUT);
    }

    fn arm_completion(&self, cx: &mut Cx) {
        match self.executing.iter().map(|e| e.deadline).min() {
            Some(at) => cx.set_timer(COMPLETION, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(COMPLETION),
        }
    }

    fn finish(&mut self, cx: &mut Cx, msg: &Msg, result: Result<Outcome, CommandError>) {
        let ok = result.is_ok();
        match msg.origin {
            Origin::User(id) => cx.complete(id, result),
            Origin::Poll | Origin::Tally => {}
        }
        if ok {
            if let Some(follow) = msg.follow.and_then(inquiry) {
                if !self.unsupported.contains(follow.name) {
                    self.queue_poll(follow);
                }
            }
            if msg.follow == Some("get_tally") && self.transport == Transport::SonyIp {
                self.tally_on = msg.payload.get(6) == Some(&0x02);
                if self.tally_on {
                    cx.set_timer(TALLY, TALLY_REFRESH);
                } else {
                    cx.cancel_timer(TALLY);
                }
            }
        }
    }

    fn mark_alive(&mut self, cx: &mut Cx) {
        cx.alive();
        self.misses = 0;
        if !self.connected {
            self.connected = true;
            self.retry_after = RETRY_MIN;
            cx.cancel_timer(PROBE);
            cx.connection(Connection::Connected);
            // Headerless transports asked for it as their probe.
            if self.transport == Transport::SonyIp {
                self.queue_poll(inquiry("get_version").unwrap());
            }
            self.schedule_polls(Poll::Fast);
            self.schedule_polls(Poll::Slow);
            if self.fast_ms > 0 {
                cx.set_timer(FAST, self.fast_ms);
            }
            if self.slow_ms > 0 {
                cx.set_timer(SLOW, self.slow_ms);
            }
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        let err = || CommandError::Transport {
            message: reason.clone(),
        };
        if let Some(f) = self.in_flight.take() {
            if let Origin::User(id) = f.msg.origin {
                cx.complete(id, Err(err()));
            }
        }
        for e in std::mem::take(&mut self.executing) {
            if let Origin::User(id) = e.msg.origin {
                cx.complete(id, Err(err()));
            }
        }
        for m in std::mem::take(&mut self.commands) {
            if let Origin::User(id) = m.origin {
                cx.complete(id, Err(err()));
            }
        }
        self.polls.clear();
        for key in [REPLY, COMPLETION, FAST, SLOW, TALLY, BUSY] {
            cx.cancel_timer(key);
        }
        self.connected = false;
        self.ready = false;
        self.misses = 0;
        self.framer = Framer::default();
        cx.connection(Connection::Disconnected { reason });
        if self.transport == Transport::RawTcp {
            if self.link {
                cx.tcp_close(SOCKET);
            }
            self.link = false;
        }
        cx.set_timer(PROBE, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    // -- replies ------------------------------------------------------------

    fn datagram_in(&mut self, cx: &mut Cx, data: &[u8]) {
        match self.transport {
            Transport::SonyIp => {
                let Some((kind, seq, payload)) = parse_ip(data) else {
                    return;
                };
                match kind {
                    T_REPLY => self.reply(cx, payload, Some(seq)),
                    T_CONTROL_REPLY => self.control_reply(cx, payload, seq),
                    _ => {}
                }
            }
            _ => {
                for m in Framer::default().feed(data) {
                    self.reply(cx, &m, None);
                }
            }
        }
    }

    fn control_reply(&mut self, cx: &mut Cx, payload: &[u8], seq: u32) {
        match payload {
            [0x01] => {
                // RESET acknowledged: the camera's sequence number is 0.
                self.seq = 0;
                self.ready = true;
                self.mark_alive(cx);
                self.pump(cx);
            }
            [0x0F, 0x01] => {
                cx.alive();
                let Some(f) = self.in_flight.take_if(|f| f.seq == seq) else {
                    return;
                };
                cx.cancel_timer(REPLY);
                if f.retransmitted && f.msg.kind == Kind::Command {
                    // The first copy was performed; only its reply was lost
                    // (p.12).
                    let msg = f.msg;
                    self.finish(cx, &msg, Ok(Outcome::Ack));
                    self.pump(cx);
                } else if f.msg.seq_resets < 1 {
                    // Out of step with the camera: RESET, then send again
                    // with a new number.
                    let mut msg = f.msg;
                    msg.seq_resets += 1;
                    self.commands_or_polls(msg);
                    self.probe(cx);
                } else {
                    let msg = f.msg;
                    self.finish(
                        cx,
                        &msg,
                        Err(CommandError::DeviceError {
                            code: Some("0F01".into()),
                            message: "sequence number abnormality".into(),
                        }),
                    );
                    self.pump(cx);
                }
            }
            [0x0F, 0x02] => {
                cx.alive();
                if let Some(f) = self.in_flight.take_if(|f| f.seq == seq) {
                    cx.cancel_timer(REPLY);
                    let msg = f.msg;
                    self.finish(
                        cx,
                        &msg,
                        Err(CommandError::DeviceError {
                            code: Some("0F02".into()),
                            message: "message type abnormality".into(),
                        }),
                    );
                    self.pump(cx);
                }
            }
            _ => {}
        }
    }

    fn commands_or_polls(&mut self, msg: Msg) {
        match msg.origin {
            Origin::Poll => self.polls.push_front(msg),
            _ => self.commands.push_front(msg),
        }
    }

    /// The in-flight message, if this reply is its own: same sequence number
    /// over Sony IP, otherwise the only one awaiting a first reply.
    fn take_in_flight(&mut self, seq: Option<u32>) -> Option<Flight> {
        let f = self.in_flight.take_if(|f| seq.is_none_or(|s| s == f.seq))?;
        Some(f)
    }

    fn reply(&mut self, cx: &mut Cx, m: &[u8], seq: Option<u32>) {
        if m.len() < 3 || m[0] < 0x90 || m[m.len() - 1] != 0xFF {
            return;
        }
        self.mark_alive(cx);
        let kind = m[1] & 0xF0;
        let socket = m[1] & 0x0F;
        match kind {
            0x40 => {
                // ACK: the command now executes in `socket`.
                if let Some(f) = self.take_in_flight(seq) {
                    cx.cancel_timer(REPLY);
                    if matches!(f.msg.kind, Kind::Command) {
                        self.executing.push(Executing {
                            msg: f.msg,
                            socket,
                            seq: f.seq,
                            deadline: cx.now() + COMPLETION_TIMEOUT,
                        });
                        self.arm_completion(cx);
                    } else {
                        self.in_flight = Some(f);
                        cx.set_timer(REPLY, REPLY_TIMEOUT);
                    }
                }
            }
            0x50 if socket != 0 && m.len() == 3 => {
                // Completion of the command in `socket`.
                if let Some(i) = self
                    .executing
                    .iter()
                    .position(|e| e.socket == socket && seq.is_none_or(|s| s == e.seq))
                {
                    let e = self.executing.remove(i);
                    self.arm_completion(cx);
                    self.finish(cx, &e.msg, Ok(Outcome::Ack));
                } else if let Some(f) = self.take_in_flight(seq) {
                    // The ACK was lost.
                    cx.cancel_timer(REPLY);
                    if matches!(f.msg.kind, Kind::Command) {
                        self.finish(cx, &f.msg, Ok(Outcome::Ack));
                    } else {
                        self.in_flight = Some(f);
                        cx.set_timer(REPLY, REPLY_TIMEOUT);
                    }
                }
            }
            0x50 => {
                let Some(f) = self.take_in_flight(seq) else {
                    return;
                };
                cx.cancel_timer(REPLY);
                let msg = f.msg;
                match (msg.kind, msg.inquiry) {
                    (Kind::Inquiry, Some(def)) => match decode(def.decode, m) {
                        Some(value) => {
                            if !def.path.is_empty() {
                                cx.state(at_path(def.path, value.clone()));
                            }
                            self.finish(cx, &msg, Ok(Outcome::Value { value }));
                        }
                        None => self.finish(
                            cx,
                            &msg,
                            Err(CommandError::DeviceError {
                                code: None,
                                message: format!("unexpected inquiry reply {}", hex(m)),
                            }),
                        ),
                    },
                    _ => self.finish(cx, &msg, Ok(Outcome::Ack)),
                }
            }
            0x60 => {
                let code = m.get(2).copied().unwrap_or(0);
                self.error(cx, socket, code, seq);
            }
            _ => {}
        }
        self.pump(cx);
    }

    fn error(&mut self, cx: &mut Cx, socket: u8, code: u8, seq: Option<u32>) {
        let device_error = |code: u8| CommandError::DeviceError {
            code: Some(format!("{code:02X}")),
            message: error_meaning(code).into(),
        };
        // An error for a command already executing (cancelled, or found not
        // executable after its ACK).
        // Without sequence numbers an error belongs to the message awaiting
        // its first reply, if there is one.
        let executing = match seq {
            Some(s) => self.executing.iter().position(|e| e.seq == s),
            None if self.in_flight.is_some() => None,
            None => self
                .executing
                .iter()
                .position(|e| socket != 0 && e.socket == socket),
        };
        if let Some(i) = executing {
            let e = self.executing.remove(i);
            self.arm_completion(cx);
            self.finish(cx, &e.msg, Err(device_error(code)));
            // The cancel that caused it is answered by the same message.
            if code == 0x04 {
                if let Some(f) = self
                    .in_flight
                    .take_if(|f| f.msg.kind == Kind::Cancel(e.socket))
                {
                    cx.cancel_timer(REPLY);
                    self.finish(cx, &f.msg, Ok(Outcome::Ack));
                }
            }
            return;
        }
        let Some(f) = self.take_in_flight(seq) else {
            return;
        };
        cx.cancel_timer(REPLY);
        let mut msg = f.msg;
        match (msg.kind, code) {
            (Kind::Cancel(s), 0x04) => {
                // y0 6p 04 answers the cancel; the cancelled command gets no
                // Completion (p.7).
                if let Some(i) = self.executing.iter().position(|e| e.socket == s) {
                    let e = self.executing.remove(i);
                    self.arm_completion(cx);
                    self.finish(cx, &e.msg, Err(device_error(0x04)));
                }
                self.finish(cx, &msg, Ok(Outcome::Ack));
            }
            (Kind::Command, 0x03) if msg.busy_attempts < BUSY_ATTEMPTS => {
                // Both sockets busy, perhaps with another controller's
                // commands: try again shortly.
                msg.busy_attempts += 1;
                self.commands.push_front(msg);
                self.busy_until = cx.now() + BUSY_RETRY;
                cx.set_timer(BUSY, BUSY_RETRY);
            }
            _ => {
                if code == 0x02 && msg.origin == Origin::Poll {
                    if let Some(def) = msg.inquiry {
                        // Not supported by this camera: stop asking.
                        self.unsupported.insert(def.name);
                        cx.log(
                            Level::Debug,
                            format!("{} is not supported by the camera; not polled", def.name),
                        );
                    }
                }
                self.finish(cx, &msg, Err(device_error(code)));
            }
        }
    }
}

fn parse_hex(s: &str) -> Option<Vec<u8>> {
    let digits: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if digits.is_empty() || !digits.len().is_multiple_of(2) {
        return None;
    }
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).ok())
        .collect()
}

impl Module for Visca {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.build(name, params) {
            Built::Ready(payload, kind, inquiry, follow) => {
                self.commands.push_back(Msg {
                    payload,
                    kind,
                    origin: Origin::User(id),
                    inquiry,
                    follow,
                    busy_attempts: 0,
                    seq_resets: 0,
                });
                self.pump(cx);
            }
            Built::Invalid(message) => {
                cx.complete(id, Err(CommandError::InvalidParams { message }))
            }
            Built::Unknown => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            ),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        if from.ip() != self.device.ip() {
            return;
        }
        self.datagram_in(cx, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        self.link = false;
        self.lost(cx, format!("socket error: {message}"));
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.link = true;
                self.framer = Framer::default();
                self.probe(cx);
            }
            TcpInput::Data(data) => {
                for m in self.framer.feed(&data) {
                    self.reply(cx, &m, None);
                }
            }
            TcpInput::Closed { reason } => {
                self.link = false;
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            PROBE => {
                if self.connected {
                    return;
                }
                match self.transport {
                    Transport::RawTcp if !self.link => {
                        cx.connection(Connection::Connecting);
                        cx.tcp_open(SOCKET, self.device);
                        cx.set_timer(PROBE, self.retry_after.max(5_000));
                    }
                    Transport::RawTcp => {
                        // Connected but silent: ask again.
                        self.in_flight = None;
                        self.probe(cx);
                    }
                    _ => {
                        if !self.link {
                            cx.udp_open(SOCKET, self.bind);
                            self.link = true;
                        }
                        self.in_flight = None;
                        self.probe(cx);
                    }
                }
            }
            REPLY => {
                let Some(mut f) = self.in_flight.take() else {
                    return;
                };
                // Sony IP: send again with the same number (p.12).
                // Headerless UDP: only an inquiry is safe to repeat.
                let resend = !f.retransmitted
                    && match self.transport {
                        Transport::SonyIp => true,
                        Transport::RawUdp => f.msg.kind == Kind::Inquiry,
                        Transport::RawTcp => false,
                    };
                if resend {
                    f.retransmitted = true;
                    self.transmit(cx, f.seq, &f.msg);
                    self.in_flight = Some(f);
                    cx.set_timer(REPLY, REPLY_TIMEOUT);
                    return;
                }
                self.finish(cx, &f.msg, Err(CommandError::Timeout));
                self.misses += 1;
                if self.misses >= MISSES_LOST {
                    self.lost(cx, "the camera stopped answering".into());
                } else {
                    self.pump(cx);
                }
            }
            COMPLETION => {
                let now = cx.now();
                while let Some(i) = self.executing.iter().position(|e| e.deadline <= now) {
                    let e = self.executing.remove(i);
                    self.finish(cx, &e.msg, Err(CommandError::Timeout));
                }
                self.arm_completion(cx);
                self.pump(cx);
            }
            FAST | SLOW => {
                if self.connected {
                    let (which, every) = if key == FAST {
                        (Poll::Fast, self.fast_ms)
                    } else {
                        (Poll::Slow, self.slow_ms)
                    };
                    self.schedule_polls(which);
                    cx.set_timer(key, every);
                    self.pump(cx);
                }
            }
            TALLY => {
                if self.tally_on && self.connected {
                    self.commands.push_back(Msg {
                        payload: vec![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00, 0x02, 0xFF],
                        kind: Kind::Command,
                        origin: Origin::Tally,
                        inquiry: None,
                        follow: None,
                        busy_attempts: 0,
                        seq_resets: 0,
                    });
                    cx.set_timer(TALLY, TALLY_REFRESH);
                    self.pump(cx);
                }
            }
            BUSY => self.pump(cx),
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.transport == Transport::RawTcp && self.link {
            cx.tcp_close(SOCKET);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    const CAMERA: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 40));

    fn visca(model: &str) -> Visca {
        let profile = Profile::of(model);
        Visca::for_device(
            SocketAddr::new(CAMERA, profile.transport.default_port()),
            profile,
        )
    }

    fn from() -> SocketAddr {
        SocketAddr::new(CAMERA, SONY_PORT)
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } | Action::TcpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    fn drain(cx: &mut Cx) -> Vec<Action> {
        let now = cx.now();
        std::mem::replace(cx, Cx::new(now)).take()
    }

    fn h(s: &str) -> Vec<u8> {
        parse_hex(s).unwrap()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
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

    /// A Sony camera after RESET and with the connect-time polls drained.
    fn ready(model: &str) -> Visca {
        let mut m = visca(model);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.datagram(&mut cx, SOCKET, from(), &h("02 01 00 01 00 00 00 00 01"));
        m.polls.clear();
        m.in_flight = None;
        m.seq = 0;
        m
    }

    fn built(m: &Visca, name: &str, p: Value) -> Vec<u8> {
        match m.build(name, &params(p)) {
            Built::Ready(b, ..) => b,
            Built::Invalid(e) => panic!("{name}: {e}"),
            Built::Unknown => panic!("{name}: unknown"),
        }
    }

    #[test]
    fn nibble_encoding() {
        assert_eq!(nibbles(0x4000, 4), [0x04, 0x00, 0x00, 0x00]);
        // Pan DE00 is -170 degrees, 2200 +170 (p.37).
        assert_eq!(
            signed_nibbles(-0x2200, 4).unwrap(),
            [0x0D, 0x0E, 0x00, 0x00]
        );
        assert_eq!(signed_nibbles(0x2200, 4).unwrap(), [0x02, 0x02, 0x00, 0x00]);
        assert_eq!(from_signed_nibbles(&[0x0D, 0x0E, 0x00, 0x00]), -0x2200);
        // H800: 170 degrees right is F6359 in five digits (H800 p.31).
        assert_eq!(
            from_signed_nibbles(&[0x0F, 0x06, 0x03, 0x05, 0x09]),
            -0x09CA7
        );
        assert_eq!(
            signed_nibbles(0x09CA7, 5).unwrap(),
            [0x00, 0x09, 0x0C, 0x0A, 0x07]
        );
        assert!(signed_nibbles(40_000, 4).is_none());
    }

    #[test]
    fn ip_header_matches_the_documents() {
        // AVer's guide gives these whole messages (AVer QSG p.2).
        assert_eq!(
            ip_frame(T_COMMAND, 1, &h("81 01 06 01 08 08 03 01 FF")),
            h("01 00 00 09 00 00 00 01 81 01 06 01 08 08 03 01 FF")
        );
        assert_eq!(
            ip_frame(T_COMMAND, 0, &h("81 01 04 00 03 FF")),
            h("01 00 00 06 00 00 00 00 81 01 04 00 03 FF")
        );
        // BirdDog's inquiry example.
        assert_eq!(
            ip_frame(T_INQUIRY, 0, &h("81 09 06 12 FF")),
            h("01 10 00 05 00 00 00 00 81 09 06 12 FF")
        );
        // A 16-byte payload's length is 00 10 (p.11).
        assert_eq!(&ip_frame(T_COMMAND, 0, &[0; 16])[2..4], &[0x00, 0x10]);
        let reply = h("01 11 00 03 00 00 00 2A 90 41 FF");
        let (t, seq, p) = parse_ip(&reply).unwrap();
        assert_eq!((t, seq, p), (T_REPLY, 42, &h("90 41 FF")[..]));
        assert!(parse_ip(&h("01 11 00 09 00 00 00 2A 90 41 FF")).is_none());
    }

    #[test]
    fn commands_as_the_command_list_gives_them() {
        let m = visca("sony-visca-ip");
        let cases: &[(&str, Value, &str)] = &[
            ("power", json!({"on": true}), "81 01 04 00 02 FF"),
            ("power", json!({"on": false}), "81 01 04 00 03 FF"),
            ("zoom", json!({"action": "tele"}), "81 01 04 07 02 FF"),
            (
                "zoom",
                json!({"action": "wide", "speed": 5}),
                "81 01 04 07 35 FF",
            ),
            ("zoom", json!({"action": "stop"}), "81 01 04 07 00 FF"),
            (
                "zoom_to",
                json!({"position": 0x4000}),
                "81 01 04 47 04 00 00 00 FF",
            ),
            (
                "focus",
                json!({"action": "far", "speed": 7}),
                "81 01 04 08 27 FF",
            ),
            (
                "focus_to",
                json!({"position": 0xF000}),
                "81 01 04 48 0F 00 00 00 FF",
            ),
            // The document's own example (p.7).
            ("focus_mode", json!({"mode": "manual"}), "81 01 04 38 03 FF"),
            ("focus_mode", json!({"mode": "auto"}), "81 01 04 38 02 FF"),
            ("focus_one_push", json!({}), "81 01 04 18 01 FF"),
            (
                "pan_tilt",
                json!({"pan": "left", "tilt": "up", "pan_speed": 0x18, "tilt_speed": 0x17}),
                "81 01 06 01 18 17 01 01 FF",
            ),
            ("pan_tilt_stop", json!({}), "81 01 06 01 01 01 03 03 FF"),
            (
                "pan_tilt_absolute",
                json!({"pan": -0x2200, "tilt": 0x1200, "speed": 0x18}),
                "81 01 06 02 18 00 0D 0E 00 00 01 02 00 00 FF",
            ),
            (
                "pan_tilt_relative",
                json!({"pan": 1, "tilt": -1, "speed": 1}),
                "81 01 06 03 01 00 00 00 00 01 0F 0F 0F 0F FF",
            ),
            ("pan_tilt_home", json!({}), "81 01 06 04 FF"),
            ("pan_tilt_reset", json!({}), "81 01 06 05 FF"),
            (
                "pan_tilt_limit_clear",
                json!({"corner": "up_right"}),
                "81 01 06 07 01 01 07 0F 0F 0F 07 0F 0F 0F FF",
            ),
            (
                "preset_recall",
                json!({"preset": 1}),
                "81 01 04 3F 02 00 FF",
            ),
            ("preset_set", json!({"preset": 100}), "81 01 04 3F 01 63 FF"),
            ("preset_reset", json!({"preset": 3}), "81 01 04 3F 00 02 FF"),
            (
                "preset_speed",
                json!({"preset": 2, "speed": 0x19}),
                "81 01 7E 01 0B 01 19 FF",
            ),
            (
                "preset_common_speed",
                json!({"speed": 0x19}),
                "81 01 7E 04 1C 01 09 FF",
            ),
            (
                "exposure_mode",
                json!({"mode": "iris_priority"}),
                "81 01 04 39 0B FF",
            ),
            (
                "iris_direct",
                json!({"position": 0x19}),
                "81 01 04 4B 00 00 01 09 FF",
            ),
            (
                "gain_direct",
                json!({"position": 0x11}),
                "81 01 04 4C 00 00 01 01 FF",
            ),
            (
                "shutter_direct",
                json!({"position": 0x21}),
                "81 01 04 4A 00 00 02 01 FF",
            ),
            (
                "max_shutter",
                json!({"position": 0x10}),
                "81 01 05 2A 00 01 00 FF",
            ),
            (
                "exposure_compensation_level",
                json!({"level": 0x0E}),
                "81 01 04 4E 00 00 00 0E FF",
            ),
            ("backlight", json!({"enabled": true}), "81 01 04 33 02 FF"),
            (
                "visibility_enhancer",
                json!({"enabled": true}),
                "81 01 04 3D 06 FF",
            ),
            (
                "white_balance_mode",
                json!({"mode": "manual"}),
                "81 01 04 35 05 FF",
            ),
            ("white_balance_one_push", json!({}), "81 01 04 10 05 FF"),
            (
                "red_gain",
                json!({"gain": 0x80}),
                "81 01 04 43 00 00 08 00 FF",
            ),
            (
                "blue_gain",
                json!({"gain": 0xFF}),
                "81 01 04 44 00 00 0F 0F FF",
            ),
            (
                "detail_level",
                json!({"level": 0x0F}),
                "81 01 04 42 00 00 00 0F FF",
            ),
            (
                "picture_effect",
                json!({"effect": "black_white"}),
                "81 01 04 63 04 FF",
            ),
            ("flip", json!({"enabled": true}), "81 01 04 66 02 FF"),
            (
                "image_stabilizer",
                json!({"enabled": false}),
                "81 01 04 34 03 FF",
            ),
            ("tally", json!({"on": true}), "81 01 7E 01 0A 00 02 FF"),
            (
                "tally_level",
                json!({"level": "high"}),
                "81 01 7E 01 0A 01 05 FF",
            ),
            ("ir_receive", json!({"mode": "toggle"}), "81 01 06 08 10 FF"),
            ("menu", json!({"mode": "off"}), "81 01 06 06 03 FF"),
            ("menu_enter", json!({}), "81 01 7E 01 02 00 01 FF"),
            (
                "camera_id",
                json!({"id": 0xABCD}),
                "81 01 04 22 0A 0B 0C 0D FF",
            ),
            ("if_clear", json!({}), "81 01 00 01 FF"),
            ("cancel", json!({"socket": 2}), "81 22 FF"),
            ("get_pan_tilt_position", json!({}), "81 09 06 12 FF"),
            ("get_version", json!({}), "81 09 00 02 FF"),
            ("get_max_shutter", json!({}), "81 09 05 2A 00 FF"),
            (
                "get_preset_speed",
                json!({"preset": 64}),
                "81 09 7E 01 0B 3F FF",
            ),
            (
                "get_pan_tilt_limit",
                json!({"corner": "down_left"}),
                "81 09 06 07 00 FF",
            ),
        ];
        for (name, p, want) in cases {
            assert_eq!(built(&m, name, p.clone()), h(want), "{name} {p}");
        }
    }

    #[test]
    fn vendor_differences() {
        // BRC-X1000/H800: five pan digits (H800 p.17-18).
        let m = visca("sony-brc-x1000");
        assert_eq!(
            built(
                &m,
                "pan_tilt_absolute",
                json!({"pan": 0x09CA7, "tilt": 0, "speed": 0x18})
            ),
            h("81 01 06 02 18 00 00 09 0C 0A 07 00 00 00 00 FF")
        );
        assert_eq!(
            built(&m, "pan_tilt_limit_clear", json!({"corner": "down_left"})),
            h("81 01 06 07 01 00 07 0F 0F 0F 0F 07 0F 0F 0F FF")
        );
        // PTZOptics: abs move carries a tilt speed; menu by pan/tilt drive.
        let m = visca("ptzoptics");
        assert_eq!(
            built(
                &m,
                "pan_tilt_absolute",
                json!({"pan": 0, "tilt": 0, "speed": 0x18, "tilt_speed": 0x14})
            ),
            h("81 01 06 02 18 14 00 00 00 00 00 00 00 00 FF")
        );
        assert_eq!(
            built(&m, "menu_navigate", json!({"direction": "down"})),
            h("81 01 06 01 0E 0E 03 02 FF")
        );
        assert_eq!(built(&m, "menu_enter", json!({})), h("81 01 06 06 05 FF"));
        assert!(matches!(
            m.build(
                "pan_tilt",
                &params(json!({"pan": "left", "tilt": "stop", "pan_speed": 1, "tilt_speed": 0x18}))
            ),
            Built::Invalid(_)
        ));
        // Lumens/Marshall: presets 129-256 in the second bank, stop at 00 00.
        let m = visca("lumens");
        assert_eq!(
            built(&m, "preset_recall", json!({"preset": 129})),
            h("81 01 04 3F 12 00 FF")
        );
        assert_eq!(
            built(&m, "preset_set", json!({"preset": 256})),
            h("81 01 04 3F 11 7F FF")
        );
        assert_eq!(
            built(&m, "pan_tilt_stop", json!({})),
            h("81 01 06 01 00 00 03 03 FF")
        );
        // Sony's 100 presets.
        assert!(matches!(
            visca("sony-visca-ip").build("preset_recall", &params(json!({"preset": 101}))),
            Built::Invalid(_)
        ));
        // Raw passthrough is checked for shape.
        let m = visca("birddog");
        assert_eq!(
            built(&m, "raw_command", json!({"hex": "81 01 04 75 02 FF"})),
            h("81 01 04 75 02 FF")
        );
        assert!(matches!(
            m.build("raw_command", &params(json!({"hex": "81 09 04 00 FF"}))),
            Built::Invalid(_)
        ));
        assert!(matches!(
            m.build("raw_inquiry", &params(json!({"hex": "81 09 FF 00 FF"}))),
            Built::Invalid(_)
        ));
    }

    #[test]
    fn inquiry_replies_decode_into_state() {
        let cases: &[(&str, &str, Value, Value)] = &[
            (
                "get_power",
                "90 50 02 FF",
                json!("on"),
                json!({"power": "on"}),
            ),
            (
                "get_zoom_position",
                "90 50 04 00 00 00 FF",
                json!(0x4000),
                json!({"zoom": {"position": 0x4000}}),
            ),
            (
                "get_focus_mode",
                "90 50 03 FF",
                json!("manual"),
                json!({"focus": {"mode": "manual"}}),
            ),
            (
                "get_pan_tilt_position",
                "90 50 0D 0E 00 00 01 02 00 00 FF",
                json!({"pan": -0x2200, "tilt": 0x1200}),
                json!({"pan_tilt": {"pan": -0x2200, "tilt": 0x1200}}),
            ),
            (
                "get_iris",
                "90 50 00 00 01 09 FF",
                json!(0x19),
                json!({"exposure": {"iris": 0x19}}),
            ),
            (
                "get_exposure_mode",
                "90 50 0A FF",
                json!("shutter_priority"),
                json!({"exposure": {"mode": "shutter_priority"}}),
            ),
            (
                "get_last_preset",
                "90 50 7F FF",
                Value::Null,
                json!({"presets": {"last_recalled": null}}),
            ),
            (
                "get_last_preset",
                "90 50 05 FF",
                json!(6),
                json!({"presets": {"last_recalled": 6}}),
            ),
            (
                "get_tally",
                "90 50 02 FF",
                json!(true),
                json!({"tally": {"on": true}}),
            ),
            (
                "get_visibility_enhancer",
                "90 50 06 FF",
                json!(true),
                json!({"exposure": {"visibility_enhancer": true}}),
            ),
            (
                "get_version",
                "90 50 00 01 06 17 01 00 02 FF",
                json!({"vendor_id": 1, "model_id": 0x0617, "rom_version": 0x0100, "sockets": 2}),
                json!({"device": {"vendor_id": 1, "model_id": 0x0617, "rom_version": 0x0100, "sockets": 2}}),
            ),
            (
                "get_pan_tilt_status",
                "90 50 24 00 FF",
                json!({"code": 0x2400, "motion": "moving", "initialization": "initialized"}),
                json!({"pan_tilt": {"status": {"code": 0x2400, "motion": "moving", "initialization": "initialized"}}}),
            ),
        ];
        for (name, reply, value, st) in cases {
            let def = inquiry(name).unwrap();
            let got = decode(def.decode, &h(reply)).unwrap();
            assert_eq!(&got, value, "{name}");
            assert_eq!(&at_path(def.path, got), st, "{name}");
        }
        // Five pan digits are recognised from the reply's length.
        assert_eq!(
            decode(Decode::PanTilt, &h("90 50 0F 06 03 05 09 00 00 00 00 FF")).unwrap(),
            json!({"pan": -0x09CA7, "tilt": 0})
        );
    }

    #[test]
    fn reset_then_command_ack_completion() {
        let mut m = visca("sony-visca-ip");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = drain(&mut cx);
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Ephemeral
        }));
        // RESET: control command, payload 01 (p.11).
        assert_eq!(sent(&a), [h("02 00 00 01 00 00 00 00 01")]);
        let mut cx = Cx::new(0);
        m.command(&mut cx, 9, "power", &params(json!({"on": true})));
        assert!(drain(&mut cx).contains(&Action::Complete {
            id: 9,
            result: Err(CommandError::NotConnected)
        }));

        let mut cx = Cx::new(10);
        m.datagram(&mut cx, SOCKET, from(), &h("02 01 00 01 00 00 00 00 01"));
        let a = drain(&mut cx);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // The version inquiry goes first, numbered 1 after the RESET.
        assert_eq!(sent(&a), [h("01 10 00 05 00 00 00 01 81 09 00 02 FF")]);
        m.polls.clear();
        let mut cx = Cx::new(20);
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 0A 00 00 00 01 90 50 00 01 06 17 01 00 02 FF"),
        );
        assert_eq!(state(&drain(&mut cx))["device"]["model_id"], 0x0617);

        let mut cx = Cx::new(30);
        m.command(&mut cx, 1, "zoom_to", &params(json!({"position": 0x4000})));
        assert_eq!(
            sent(&drain(&mut cx)),
            [h("01 00 00 09 00 00 00 02 81 01 04 47 04 00 00 00 FF")]
        );
        let mut cx = Cx::new(40);
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 03 00 00 00 02 90 41 FF"),
        );
        assert!(!drain(&mut cx)
            .iter()
            .any(|a| matches!(a, Action::Complete { .. })));
        assert_eq!(m.executing.len(), 1);
        let mut cx = Cx::new(900);
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 03 00 00 00 02 90 51 FF"),
        );
        assert!(drain(&mut cx).contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));
        assert!(m.executing.is_empty());
    }

    #[test]
    fn errors_map_to_device_errors() {
        let mut m = ready("sony-visca-ip");
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "focus_to", &params(json!({"position": 0x1000})));
        // Not executable in auto focus (p.8).
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 04 00 00 00 01 90 61 41 FF"),
        );
        assert!(drain(&mut cx).contains(&Action::Complete {
            id: 1,
            result: Err(CommandError::DeviceError {
                code: Some("41".into()),
                message: error_meaning(0x41).into()
            })
        }));
        // Syntax error to a poll: that inquiry is not polled again.
        m.queue_poll(inquiry("get_nd_filter").unwrap());
        let mut cx = Cx::new(10);
        m.pump(&mut cx);
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 04 00 00 00 02 90 60 02 FF"),
        );
        assert!(m.unsupported.contains("get_nd_filter"));
    }

    #[test]
    fn buffer_full_waits_and_sends_again() {
        let mut m = ready("sony-visca-ip");
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "pan_tilt_home", &params(json!({})));
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 04 00 00 00 01 90 60 03 FF"),
        );
        let a = drain(&mut cx);
        assert!(!a.iter().any(|x| matches!(x, Action::Complete { .. })));
        assert!(a.contains(&Action::SetTimer {
            key: BUSY,
            after: BUSY_RETRY
        }));
        let mut cx = Cx::new(BUSY_RETRY);
        m.timer(&mut cx, BUSY);
        assert_eq!(
            sent(&drain(&mut cx)),
            [h("01 00 00 05 00 00 00 02 81 01 06 04 FF")]
        );
    }

    #[test]
    fn two_sockets_busy_hold_commands_but_not_inquiries() {
        let mut m = ready("sony-visca-ip");
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "preset_recall", &params(json!({"preset": 1})));
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 03 00 00 00 01 90 41 FF"),
        );
        m.polls.clear();
        m.command(&mut cx, 2, "zoom_to", &params(json!({"position": 1})));
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 03 00 00 00 02 90 42 FF"),
        );
        drain(&mut cx);
        m.command(&mut cx, 3, "focus_to", &params(json!({"position": 1})));
        assert!(sent(&drain(&mut cx)).is_empty(), "both sockets busy");
        m.command(&mut cx, 4, "get_power", &params(json!({})));
        // The inquiry waits behind the held command in the user queue, but
        // polls still flow.
        m.queue_poll(inquiry("get_zoom_position").unwrap());
        m.pump(&mut cx);
        assert_eq!(
            sent(&drain(&mut cx)),
            [h("01 10 00 05 00 00 00 03 81 09 04 47 FF")]
        );
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 07 00 00 00 03 90 50 00 00 00 01 FF"),
        );
        // Completion of socket 1 frees a slot: the held command goes.
        drain(&mut cx);
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 03 00 00 00 01 90 51 FF"),
        );
        let a = drain(&mut cx);
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));
        assert_eq!(
            sent(&a),
            [h("01 00 00 09 00 00 00 04 81 01 04 48 00 00 00 01 FF")]
        );
    }

    #[test]
    fn lost_replies_are_retransmitted_with_the_same_number() {
        let mut m = ready("sony-visca-ip");
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "pan_tilt_home", &params(json!({})));
        let first = sent(&drain(&mut cx));
        let mut cx = Cx::new(REPLY_TIMEOUT);
        m.timer(&mut cx, REPLY);
        assert_eq!(sent(&drain(&mut cx)), first);
        // The first copy was performed and only its ACK lost: ERROR 0F 01
        // (p.12). The command counts as done.
        let mut cx = Cx::new(REPLY_TIMEOUT + 5);
        m.datagram(&mut cx, SOCKET, from(), &h("02 01 00 02 00 00 00 01 0F 01"));
        assert!(drain(&mut cx).contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));

        // A sequence error to a fresh message: RESET, then send it again.
        let mut cx = Cx::new(2_000);
        m.command(&mut cx, 2, "pan_tilt_reset", &params(json!({})));
        drain(&mut cx);
        m.datagram(&mut cx, SOCKET, from(), &h("02 01 00 02 00 00 00 02 0F 01"));
        let a = sent(&drain(&mut cx));
        assert_eq!(a[0][..2], [0x02, 0x00]);
        m.datagram(&mut cx, SOCKET, from(), &h("02 01 00 01 00 00 00 02 01"));
        assert_eq!(
            sent(&drain(&mut cx)),
            [h("01 00 00 05 00 00 00 01 81 01 06 05 FF")]
        );
    }

    #[test]
    fn unanswered_messages_time_out_and_lose_the_camera() {
        let mut m = ready("sony-visca-ip");
        for i in 0..MISSES_LOST {
            let mut cx = Cx::new(u64::from(i) * 10_000);
            m.command(&mut cx, u64::from(i), "pan_tilt_home", &params(json!({})));
            m.timer(&mut cx, REPLY);
            m.timer(&mut cx, REPLY);
            let a = drain(&mut cx);
            assert!(a.contains(&Action::Complete {
                id: u64::from(i),
                result: Err(CommandError::Timeout)
            }));
            if i + 1 == MISSES_LOST {
                assert!(a
                    .iter()
                    .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
            }
        }
        assert!(!m.connected);
    }

    #[test]
    fn cancel_answers_for_both_commands() {
        let mut m = ready("sony-visca-ip");
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            1,
            "pan_tilt_absolute",
            &params(json!({"pan": 0, "tilt": 0, "speed": 1})),
        );
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 03 00 00 00 01 90 41 FF"),
        );
        m.polls.clear();
        m.command(&mut cx, 2, "cancel", &params(json!({"socket": 1})));
        drain(&mut cx);
        // y0 6p 04: the command in socket 1 was cancelled (p.7).
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 04 00 00 00 02 90 61 04 FF"),
        );
        let a = drain(&mut cx);
        assert!(a.contains(&Action::Complete {
            id: 2,
            result: Ok(Outcome::Ack)
        }));
        assert!(a.iter().any(|x| matches!(x, Action::Complete { id: 1, result: Err(CommandError::DeviceError { code: Some(c), .. }) } if c == "04")));
    }

    #[test]
    fn headerless_tcp_matches_replies_by_order_and_socket() {
        let mut m = visca("ptzoptics");
        assert_eq!(m.device.port(), PTZOPTICS_TCP_PORT);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(drain(&mut cx)
            .iter()
            .any(|a| matches!(a, Action::TcpOpen { .. })));
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&drain(&mut cx)), [h("81 09 00 02 FF")]);
        // Split across reads.
        m.tcp(&mut cx, SOCKET, TcpInput::Data(h("90 50 00 01 05")));
        m.tcp(&mut cx, SOCKET, TcpInput::Data(h("1C 01 00 02 FF")));
        let a = drain(&mut cx);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(state(&a)["device"]["model_id"], 0x051C);
        m.polls.clear();
        m.in_flight = None;
        m.command(&mut cx, 1, "preset_recall", &params(json!({"preset": 5})));
        assert_eq!(sent(&drain(&mut cx)), [h("81 01 04 3F 02 04 FF")]);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(h("90 42 FF 90 52 FF")));
        let a = drain(&mut cx);
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));
    }

    #[test]
    fn tally_on_is_repeated_within_fifteen_seconds() {
        let mut m = ready("sony-visca-ip");
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "tally", &params(json!({"on": true})));
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 03 00 00 00 01 90 41 FF"),
        );
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &h("01 11 00 03 00 00 00 01 90 51 FF"),
        );
        assert!(drain(&mut cx).contains(&Action::SetTimer {
            key: TALLY,
            after: TALLY_REFRESH
        }));
        m.polls.clear();
        m.in_flight = None;
        let mut cx = Cx::new(TALLY_REFRESH);
        m.timer(&mut cx, TALLY);
        assert_eq!(
            sent(&drain(&mut cx)),
            [h("01 00 00 08 00 00 00 03 81 01 7E 01 0A 00 02 FF")]
        );
    }

    #[test]
    fn canon_replies_to_a_fixed_port() {
        let m = visca("canon");
        assert_eq!(m.bind, Bind::Shared(SONY_PORT));
    }

    #[test]
    fn datagrams_from_other_hosts_are_ignored() {
        let mut m = visca("sony-visca-ip");
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        drain(&mut cx);
        let other = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 41)), SONY_PORT);
        m.datagram(&mut cx, SOCKET, other, &h("02 01 00 01 00 00 00 00 01"));
        assert!(!m.connected);
    }
}
