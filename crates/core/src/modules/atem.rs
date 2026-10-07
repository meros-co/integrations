//! Blackmagic ATEM switchers over their UDP protocol on port 9910.
//!
//! Written from protocol documentation and MIT-licensed references, never from
//! GPL code: see the spec's sources. The shape of the protocol:
//!
//! - Every datagram is one packet with a 12-byte header: 5 flag bits and an
//!   11-bit length, then the session id, the id being acknowledged, the id the
//!   peer wants retransmitted from, two unused bytes, and the sender's own
//!   15-bit packet id. Big-endian throughout.
//! - The client opens with a fixed hello; the switcher answers with the new
//!   session flag; the client acknowledges, and from then on takes the session
//!   id from every packet it receives.
//! - The switcher then sends its whole state, from `_ver` to `InCm`, and keeps
//!   sending updates. Every packet it marks reliable must be acknowledged, and
//!   packets are accepted strictly in order: a gap is left for the switcher to
//!   resend.
//! - Commands go the other way in reliable packets, resent until acknowledged.
//!   A packet's payload is a run of commands, each an 8-byte header (length
//!   including the header, two unused bytes, a four-letter name) and a body.
//!
//! Command and state layouts cite their source next to each one. "Sofie" is
//! sofie-atem-connection at commit 6b91b9e, `src/commands/`; "OpenSwitcher" is
//! the docs.openswitcher.org field and command tables (prose only); "Companion"
//! is companion-module-bmd-atem at commit 4cd3d9a, `src/`, used for value
//! ranges and scaling; "camera-control" is @atem-connection/camera-control
//! 0.4.0 (MIT). Writable commands carry a bit mask of the fields they set, and
//! the switcher ignores fields whose bit is clear (Sofie `CommandBase.ts:58-91`),
//! so a setting command here sets exactly the parameters given.
//!
//! Feature areas beyond switching live in sibling files, each decoding its
//! own state and building its own commands: `atem_keyers.rs` (keyer DVE,
//! chroma, masks and flying keys).
//!
//! Opened for commands only (`monitor` false), the module neither polls the
//! streaming and recording durations nor renews a Fairlight level
//! subscription after reconnecting. The switcher still sends its whole state
//! on every connection, as the handshake makes it, and that is applied.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};
use crate::session::merge_patch;

#[path = "atem_keyers.rs"]
mod keyers;

const PORT: u16 = 9910;
const SOCKET: Key = "atem";

const FLAG_RELIABLE: u8 = 0x01;
const FLAG_SYN: u8 = 0x02;
const FLAG_RETRANSMIT_REQUEST: u8 = 0x08;
const FLAG_ACK: u8 = 0x10;

const HEADER: usize = 12;
/// Packet ids are 15 bits: the switcher wraps from 32767 to 0.
const ID_MODULO: u16 = 1 << 15;
/// The largest datagram the client builds, as ATEM Software Control does.
const MAX_PACKET: usize = 1416;

/// The client's hello: new-session flag, length 20, a client-chosen session
/// id, and the fixed payload 01 00 00 00 00 00 00 00.
const HELLO_PACKET: [u8; 20] = [
    0x10, 0x14, 0x53, 0xab, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3a, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

const HELLO_EVERY: Millis = 1_000;
const HELLO_EVERY_SLOW: Millis = 5_000;
const HELLO_ATTEMPTS_FAST: u32 = 5;
/// The switcher sends traffic continuously; this much silence is a lost
/// connection.
const SILENCE_TIMEOUT: Millis = 5_000;
const RESEND_AFTER: Millis = 60;
const RESEND_ATTEMPTS: u32 = 10;
/// Streaming and recording durations are sent only on request; Companion asks
/// every 500 ms (`main.ts:675-693`).
const DURATION_EVERY: Millis = 500;

const HELLO: Key = "hello";
const SILENCE: Key = "silence";
const RESEND: Key = "resend";
const DURATION: Key = "duration";

/// Protocol 2.28 (firmware 8.0): SuperSource commands gain a SuperSource
/// index (Sofie `SuperSource/*.ts`, `minimumVersion = V8_0`).
const V2_28: u32 = 0x0002_001C;
/// Protocol 2.29 (firmware 8.0.1): DskS and DDsA change layout.
const V2_29: u32 = 0x0002_001D;
/// Protocol 2.30 (firmware 8.1.1): `_top` gains a multiviewer byte; streaming
/// and recording commands exist.
const V2_30: u32 = 0x0002_001E;

/// Wipe and pattern-key styles, by wire value (Sofie `enums/index.ts:188-207`;
/// OpenSwitcher CTWp "Pattern style [0-17]").
const PATTERNS: [&str; 18] = [
    "left_to_right_bar",
    "top_to_bottom_bar",
    "horizontal_barn_door",
    "vertical_barn_door",
    "corners_in_four_box",
    "rectangle_iris",
    "diamond_iris",
    "circle_iris",
    "top_left_box",
    "top_right_box",
    "bottom_right_box",
    "bottom_left_box",
    "top_centre_box",
    "right_centre_box",
    "bottom_centre_box",
    "left_centre_box",
    "top_left_diagonal",
    "top_right_diagonal",
];

/// DVE transition styles, by wire value (Sofie `enums/index.ts:64-104`).
const DVE_STYLES: [&str; 35] = [
    "swoosh_top_left",
    "swoosh_top",
    "swoosh_top_right",
    "swoosh_left",
    "swoosh_right",
    "swoosh_bottom_left",
    "swoosh_bottom",
    "swoosh_bottom_right",
    "spin_cw_top_left",
    "spin_cw_top_right",
    "spin_cw_bottom_left",
    "spin_cw_bottom_right",
    "spin_ccw_top_left",
    "spin_ccw_top_right",
    "spin_ccw_bottom_left",
    "spin_ccw_bottom_right",
    "squeeze_top_left",
    "squeeze_top",
    "squeeze_top_right",
    "squeeze_left",
    "squeeze_right",
    "squeeze_bottom_left",
    "squeeze_bottom",
    "squeeze_bottom_right",
    "push_top_left",
    "push_top",
    "push_top_right",
    "push_left",
    "push_right",
    "push_bottom_left",
    "push_bottom",
    "push_bottom_right",
    "graphic_cw_spin",
    "graphic_ccw_spin",
    "graphic_logo_wipe",
];

/// Upstream keyer types (Sofie `enums/index.ts:209-214`).
const KEY_TYPES: [&str; 4] = ["luma", "chroma", "pattern", "dve"];

/// Transition selection bits: background, then keys 1 to 4 (Sofie
/// `enums/index.ts:56-62`; OpenSwitcher CTTp "Next transition").
const LAYERS: [&str; 5] = ["background", "key1", "key2", "key3", "key4"];

/// Camera control data types (camera-control and Sofie
/// `CameraControlCommand.ts:6-14`).
const CC_SINT8: u8 = 0x01;
const CC_SINT16: u8 = 0x02;
const CC_SINT32: u8 = 0x03;
const CC_FLOAT: u8 = 0x80;

#[derive(Debug, PartialEq)]
enum Phase {
    /// Hello sent, waiting for the switcher's answer.
    Hello,
    /// Session open, initial state arriving.
    Loading,
    /// Initial state complete (`InCm`).
    Ready,
}

struct Sent {
    id: u16,
    packet: Vec<u8>,
    sent_at: Millis,
    attempts: u32,
    /// Sent more than once: its acknowledgement does not time one send.
    resent: bool,
    /// The caller's command, or none for the module's own requests.
    command: Option<CommandId>,
}

/// What commands are checked against: the switcher's own description of
/// itself, from the initial state.
#[derive(Default)]
struct Topology {
    mes: u8,
    auxes: u8,
    dsks: u8,
    usks: BTreeMap<u8, u8>,
    macros: u8,
    macros_used: BTreeMap<u16, bool>,
    /// Source id to its internal port type (InPr byte 32).
    sources: BTreeMap<u16, u8>,
    encoder: bool,
    media_players: u8,
    stills: u8,
    clips: u8,
    dves: u8,
    stingers: u8,
    supersources: u8,
    /// SuperSource index to its box count (`_SSC`).
    supersource_boxes: BTreeMap<u8, u8>,
    camera_control: bool,
    color_generators: BTreeSet<u8>,
    classic_audio: bool,
    classic_inputs: BTreeSet<u16>,
    fairlight: bool,
    /// Fairlight (input, source) to its supported mix options bit set.
    fairlight_sources: BTreeMap<(u16, i64), u8>,
    /// Multiviewer windows seen, with their last source, for de-duplication.
    multiviewer_windows: BTreeMap<(u8, u8), u16>,
    /// (M/E, keyer) to whether the keyer can fly (KeBP).
    usk_can_fly: BTreeMap<(u8, u8), bool>,
    /// M/E to its next-transition selection (TrSS byte 4).
    next_selection: BTreeMap<u8, u8>,
}

pub(crate) struct Atem {
    device: SocketAddr,
    phase: Phase,
    connected: bool,
    session: u16,
    /// Last reliable packet id accepted from the switcher.
    last_received: u16,
    next_id: u16,
    in_flight: VecDeque<Sent>,
    hello_attempts: u32,
    version: u32,
    topology: Topology,
    streaming_active: bool,
    recording_active: bool,
    duration_armed: bool,
    /// The caller asked for Fairlight levels; asked again after reconnecting.
    levels_wanted: bool,
    /// False: commands only, no duration polling and no level renewal.
    monitor: bool,
}

/// A command to send: its name and body.
type Out = Vec<([u8; 4], Vec<u8>)>;

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

fn i16_at(b: &[u8], at: usize) -> i16 {
    i16::from_be_bytes([b[at], b[at + 1]])
}

fn i32_at(b: &[u8], at: usize) -> i32 {
    i32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

fn i64_at(b: &[u8], at: usize) -> i64 {
    i64::from_be_bytes(b[at..at + 8].try_into().unwrap())
}

fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

fn put_i16(b: &mut [u8], at: usize, v: i16) {
    b[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

fn put_i32(b: &mut [u8], at: usize, v: i32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

/// A NUL-terminated or NUL-padded string field. Bytes after the NUL are
/// arbitrary on the wire.
fn text(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// `a` is at or before `b` in 15-bit sequence space.
fn at_or_before(a: u16, b: u16) -> bool {
    (b.wrapping_sub(a) % ID_MODULO) < ID_MODULO / 2
}

fn header(flags: u8, length: usize, session: u16, ack: u16, id: u16) -> Vec<u8> {
    let first = ((flags as u16) << 11) | (length as u16 & 0x07FF);
    let mut h = Vec::with_capacity(length);
    h.extend_from_slice(&first.to_be_bytes());
    h.extend_from_slice(&session.to_be_bytes());
    h.extend_from_slice(&ack.to_be_bytes());
    h.extend_from_slice(&[0, 0, 0, 0]);
    h.extend_from_slice(&id.to_be_bytes());
    h
}

/// One command: length (header included), two zero bytes, name, body.
fn command(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut c = Vec::with_capacity(8 + body.len());
    c.extend_from_slice(&((8 + body.len()) as u16).to_be_bytes());
    c.extend_from_slice(&[0, 0]);
    c.extend_from_slice(name);
    c.extend_from_slice(body);
    c
}

fn source_kind(port: u8) -> &'static str {
    match port {
        0 => "external",
        1 => "black",
        2 => "color_bars",
        3 => "color",
        4 => "media_player_fill",
        5 => "media_player_key",
        6 => "supersource",
        7 => "external_direct",
        128 => "me_output",
        129 => "aux",
        130 => "mask",
        131 => "multiviewer",
        132 => "audio_monitor",
        _ => "unknown",
    }
}

fn transition_style(style: u8) -> &'static str {
    match style {
        0 => "mix",
        1 => "dip",
        2 => "wipe",
        3 => "dve",
        4 => "sting",
        _ => "unknown",
    }
}

fn named(list: &[&'static str], value: u8) -> &'static str {
    list.get(value as usize).copied().unwrap_or("unknown")
}

fn layers(mask: u8) -> Vec<&'static str> {
    LAYERS
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, l)| *l)
        .collect()
}

/// Classic audio gain: 32768 is 0 dB, 0 is silence (Sofie
/// `lib/atemUtil.ts:15-22`).
fn classic_db(raw: u16) -> Value {
    if raw == 0 {
        Value::Null
    } else {
        json!(((raw as f64 / 32768.0).log10() * 2_000.0).round() / 100.0)
    }
}

fn classic_raw(db: f64) -> u16 {
    (10f64.powf(db / 20.0) * 32768.0).floor().min(65535.0) as u16
}

/// Classic audio balance: -10000 to 10000 for -50 to 50 (Sofie
/// `lib/atemUtil.ts:24-30`).
fn classic_balance(raw: i16) -> f64 {
    (raw as f64 / 2.0).round() / 100.0
}

/// Classic mix options (Sofie `enums/index.ts:291-295`) and Fairlight ones,
/// which are bits (`enums/index.ts:341-345`).
fn classic_mix(v: u8) -> &'static str {
    match v {
        0 => "off",
        1 => "on",
        2 => "afv",
        _ => "unknown",
    }
}

fn fairlight_mix(v: u8) -> &'static str {
    match v {
        1 => "off",
        2 => "on",
        4 => "afv",
        _ => "unknown",
    }
}

fn fairlight_mixes(mask: u8) -> Vec<&'static str> {
    [1u8, 2, 4]
        .into_iter()
        .filter(|b| mask & b != 0)
        .map(fairlight_mix)
        .collect()
}

/// A timecode body: hours, minutes, seconds, frames, drop-frame flag (Sofie
/// `Streaming/StreamingDurationCommand.ts:27-34`).
fn timecode(b: &[u8]) -> String {
    let sep = if b[4] != 0 { ';' } else { ':' };
    format!("{:02}:{:02}:{:02}{sep}{:02}", b[0], b[1], b[2], b[3])
}

/// Hundredths, as a float with two decimals.
fn hundredths(v: i64) -> f64 {
    v as f64 / 100.0
}

fn tenths(v: u16) -> f64 {
    v as f64 / 10.0
}

fn int(params: &Params, name: &str) -> i64 {
    params.get(name).and_then(Value::as_i64).unwrap_or(0)
}

fn flag(params: &Params, name: &str) -> bool {
    params.get(name).and_then(Value::as_bool).unwrap_or(true)
}

fn opt_int(params: &Params, name: &str) -> Option<i64> {
    params.get(name).and_then(Value::as_i64)
}

fn opt_num(params: &Params, name: &str) -> Option<f64> {
    params.get(name).and_then(Value::as_f64)
}

fn opt_bool(params: &Params, name: &str) -> Option<bool> {
    params.get(name).and_then(Value::as_bool)
}

fn opt_str<'a>(params: &'a Params, name: &str) -> Option<&'a str> {
    params.get(name).and_then(Value::as_str)
}

/// A number scaled to the wire's fixed point.
fn scaled(params: &Params, name: &str, by: f64) -> Option<i64> {
    opt_num(params, name).map(|v| (v * by).round() as i64)
}

/// One command to send, from a feature area's builder.
fn one(name: &[u8; 4], body: Vec<u8>) -> Result<Option<Out>, CommandError> {
    Ok(Some(vec![(*name, body)]))
}

fn invalid(message: String) -> CommandError {
    CommandError::InvalidParams { message }
}

fn nothing_to_set() -> CommandError {
    invalid("give at least one setting to change".into())
}

fn unsupported(command: &str, why: &str) -> CommandError {
    CommandError::UnsupportedForModel {
        command: command.into(),
        model: why.into(),
    }
}

fn enum_param(params: &Params, name: &str, list: &[&str]) -> Result<Option<u8>, CommandError> {
    match opt_str(params, name) {
        None => Ok(None),
        Some(v) => list
            .iter()
            .position(|x| *x == v)
            .map(|i| Some(i as u8))
            .ok_or_else(|| invalid(format!("unknown {name} {v:?}"))),
    }
}

/// A string written into a fixed field, which must fit in `max` bytes.
fn fits(params: &Params, name: &str, max: usize) -> Result<Option<String>, CommandError> {
    match opt_str(params, name) {
        Some(s) if s.len() > max => Err(invalid(format!(
            "{name} is {} bytes; the switcher holds {max}",
            s.len()
        ))),
        other => Ok(other.map(str::to_owned)),
    }
}

fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

/// A field's width on the wire.
#[derive(Clone, Copy)]
enum W {
    U8,
    U16,
    I16,
    U32,
    I32,
}

fn put(b: &mut [u8], at: usize, w: W, v: i64) {
    match w {
        W::U8 => b[at] = v as u8,
        W::U16 => put16(b, at, v as u16),
        W::I16 => put_i16(b, at, v as i16),
        W::U32 => put32(b, at, v as u32),
        W::I32 => put_i32(b, at, v as i32),
    }
}

fn get(b: &[u8], at: usize, w: W) -> i64 {
    match w {
        W::U8 => b[at] as i64,
        W::U16 => u16_at(b, at) as i64,
        W::I16 => i16_at(b, at) as i64,
        W::U32 => u32_at(b, at) as i64,
        W::I32 => i32_at(b, at) as i64,
    }
}

/// A setting command's body: the fields given, and the mask of their bits.
/// Fields not given stay zero and their bit clear, so the switcher keeps them.
struct Body<'a> {
    b: Vec<u8>,
    mask: u32,
    params: &'a Params,
}

impl<'a> Body<'a> {
    fn new(params: &'a Params, len: usize) -> Body<'a> {
        Body {
            b: vec![0u8; len],
            mask: 0,
            params,
        }
    }

    /// A fixed byte, such as an index.
    fn at(&mut self, at: usize, v: u8) -> &mut Self {
        self.b[at] = v;
        self
    }

    /// A number scaled to the wire's fixed point.
    fn num(&mut self, name: &str, bit: u32, at: usize, by: f64, w: W) -> &mut Self {
        if let Some(v) = scaled(self.params, name, by) {
            self.mask |= 1 << bit;
            put(&mut self.b, at, w, v);
        }
        self
    }

    fn flag(&mut self, name: &str, bit: u32, at: usize) -> &mut Self {
        if let Some(v) = opt_bool(self.params, name) {
            self.mask |= 1 << bit;
            self.b[at] = v as u8;
        }
        self
    }

    /// An enum parameter written as its position in `list`.
    fn choice(
        &mut self,
        name: &str,
        bit: u32,
        at: usize,
        list: &[&str],
    ) -> Result<&mut Self, CommandError> {
        if let Some(v) = enum_param(self.params, name, list)? {
            self.mask |= 1 << bit;
            self.b[at] = v;
        }
        Ok(self)
    }

    /// The body with its mask at `at`, or an error when nothing was given.
    fn done(&mut self, at: usize, w: W) -> Result<Vec<u8>, CommandError> {
        if self.mask == 0 {
            return Err(nothing_to_set());
        }
        put(&mut self.b, at, w, self.mask as i64);
        Ok(std::mem::take(&mut self.b))
    }
}

/// Fields read from a body into a JSON object: (name, offset, width, divisor).
fn fields(b: &[u8], list: &[(&str, usize, W, f64)]) -> Map<String, Value> {
    let mut m = Map::new();
    for &(name, at, w, by) in list {
        let v = get(b, at, w);
        let value = if by == 1.0 {
            json!(v)
        } else {
            json!(v as f64 / by)
        };
        m.insert(name.into(), value);
    }
    m
}

/// The 16-byte camera control header and the values padded to 8 bytes
/// (Sofie `CameraControlCommand.ts:54-157`): camera, category, parameter,
/// relative, data type, then the count of 8-, 16-, 32- and 64-bit values at
/// offsets 6, 8, 10 and 12. FLOAT values are 5.11 fixed point in 16 bits.
fn camera_body(
    camera: u8,
    category: u8,
    parameter: u8,
    relative: bool,
    kind: u8,
    values: &[i64],
) -> Vec<u8> {
    let (width, count_at) = match kind {
        CC_SINT8 => (1, 6),
        CC_SINT16 | CC_FLOAT => (2, 8),
        _ => (4, 10),
    };
    let data = values.len() * width;
    let mut b = vec![0u8; 16 + data.div_ceil(8) * 8];
    b[0] = camera;
    b[1] = category;
    b[2] = parameter;
    b[3] = relative as u8;
    b[4] = kind;
    put16(&mut b, count_at, values.len() as u16);
    for (i, v) in values.iter().enumerate() {
        let at = 16 + i * width;
        match width {
            1 => b[at] = *v as i8 as u8,
            2 => put_i16(&mut b, at, *v as i16),
            _ => put_i32(&mut b, at, *v as i32),
        }
    }
    b
}

impl Atem {
    pub(crate) fn new(ctx: OpenContext) -> Atem {
        let mut m = Atem::for_device(SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)));
        m.monitor = ctx.monitor;
        m
    }

    fn for_device(device: SocketAddr) -> Atem {
        Atem {
            device,
            phase: Phase::Hello,
            connected: false,
            session: 0,
            last_received: 0,
            next_id: 1,
            in_flight: VecDeque::new(),
            hello_attempts: 0,
            version: 0,
            topology: Topology::default(),
            streaming_active: false,
            recording_active: false,
            duration_armed: false,
            levels_wanted: false,
            monitor: true,
        }
    }

    /// A fresh socket (the switcher may hold a slot for the old port) and a
    /// fresh session.
    fn connect(&mut self, cx: &mut Cx) {
        self.phase = Phase::Hello;
        self.session = 0;
        self.last_received = 0;
        self.next_id = 1;
        self.version = 0;
        self.topology = Topology::default();
        self.streaming_active = false;
        self.recording_active = false;
        self.duration_armed = false;
        cx.udp_open(SOCKET, Bind::Ephemeral);
        self.hello(cx);
    }

    fn hello(&mut self, cx: &mut Cx) {
        cx.udp_send(SOCKET, self.device, HELLO_PACKET.to_vec());
        self.hello_attempts += 1;
        let every = if self.hello_attempts < HELLO_ATTEMPTS_FAST {
            HELLO_EVERY
        } else {
            HELLO_EVERY_SLOW
        };
        cx.set_timer(HELLO, every);
    }

    fn ack(&self, cx: &mut Cx, id: u16) {
        cx.udp_send(
            SOCKET,
            self.device,
            header(FLAG_ACK, HEADER, self.session, id, 0),
        );
    }

    fn lost(&mut self, cx: &mut Cx, reason: &str) {
        for sent in self.in_flight.drain(..) {
            if let Some(command) = sent.command {
                cx.complete(
                    command,
                    Err(CommandError::Transport {
                        message: reason.into(),
                    }),
                );
            }
        }
        cx.cancel_timer(RESEND);
        cx.cancel_timer(SILENCE);
        cx.cancel_timer(DURATION);
        if self.connected || self.phase != Phase::Hello {
            cx.connection(Connection::Disconnected {
                reason: reason.into(),
            });
        }
        self.connected = false;
        self.hello_attempts = 0;
        self.connect(cx);
    }

    /// Commands in one reliable packet, resent until acknowledged.
    fn send_commands(
        &mut self,
        cx: &mut Cx,
        id: Option<CommandId>,
        commands: &[([u8; 4], Vec<u8>)],
    ) {
        let payload: Vec<u8> = commands
            .iter()
            .flat_map(|(name, body)| command(name, body))
            .collect();
        let packet_id = self.next_id;
        self.next_id = (self.next_id + 1) % ID_MODULO;
        let mut packet = header(
            FLAG_RELIABLE,
            HEADER + payload.len(),
            self.session,
            0,
            packet_id,
        );
        packet.extend_from_slice(&payload);
        debug_assert!(packet.len() <= MAX_PACKET);
        cx.udp_send(SOCKET, self.device, packet.clone());
        self.in_flight.push_back(Sent {
            id: packet_id,
            packet,
            sent_at: cx.now(),
            attempts: 1,
            resent: false,
            command: id,
        });
        cx.set_timer(RESEND, RESEND_AFTER);
    }

    fn acknowledged(&mut self, cx: &mut Cx, ack: u16) {
        while let Some(sent) = self.in_flight.front() {
            if !at_or_before(sent.id, ack) {
                break;
            }
            let sent = self.in_flight.pop_front().unwrap();
            if !sent.resent {
                cx.round_trip(cx.now().saturating_sub(sent.sent_at));
            }
            if let Some(command) = sent.command {
                cx.complete(command, Ok(Outcome::Ack));
            }
        }
        if self.in_flight.is_empty() {
            cx.cancel_timer(RESEND);
        }
    }

    fn retransmit_from(&mut self, cx: &mut Cx, from: u16) {
        let from = from % ID_MODULO;
        let Some(start) = self.in_flight.iter().position(|s| s.id == from) else {
            self.lost(cx, "the switcher asked for a packet no longer held");
            return;
        };
        let now = cx.now();
        for sent in self.in_flight.iter_mut().skip(start) {
            cx.udp_send(SOCKET, self.device, sent.packet.clone());
            sent.sent_at = now;
            sent.resent = true;
        }
    }

    fn receive(&mut self, cx: &mut Cx, data: &[u8]) {
        if data.len() < HEADER {
            return;
        }
        let flags = data[0] >> 3;
        let length = (u16_at(data, 0) & 0x07FF) as usize;
        if length != data.len() {
            return;
        }
        let session = u16_at(data, 2);
        let remote_id = u16_at(data, 10);

        if flags & FLAG_SYN != 0 {
            if self.phase != Phase::Hello {
                return;
            }
            // Byte 12 is the connection status: 2 accepted, 4 asked to start
            // again (OpenSwitcher). Sofie proceeds regardless; so does this.
            if data.get(HEADER) == Some(&0x04) {
                cx.log(
                    Level::Warning,
                    "the switcher asked to restart the connection",
                );
            }
            cx.cancel_timer(HELLO);
            self.session = session;
            self.last_received = remote_id;
            self.phase = Phase::Loading;
            self.ack(cx, remote_id);
            cx.set_timer(SILENCE, SILENCE_TIMEOUT);
            return;
        }
        if self.phase == Phase::Hello {
            return;
        }
        // The switcher assigns a new session id after the handshake; follow
        // whatever it uses.
        self.session = session;
        cx.set_timer(SILENCE, SILENCE_TIMEOUT);
        cx.alive();

        // The packet's state is applied before the commands it acknowledges
        // complete, so a caller reading the snapshot after its command
        // completes sees what the packet carried.
        if flags & FLAG_RELIABLE != 0 {
            let expected = (self.last_received + 1) % ID_MODULO;
            if remote_id == expected {
                self.last_received = remote_id;
                self.ack(cx, remote_id);
                self.commands(cx, &data[HEADER..]);
            } else if at_or_before(remote_id, self.last_received) {
                // Already have it: our acknowledgement was lost.
                self.ack(cx, self.last_received);
            }
            // Ahead of a gap: dropped, and the switcher resends in order.
        }
        if flags & FLAG_ACK != 0 {
            self.acknowledged(cx, u16_at(data, 4));
        }
        if flags & FLAG_RETRANSMIT_REQUEST != 0 {
            self.retransmit_from(cx, u16_at(data, 6));
        }
    }

    fn commands(&mut self, cx: &mut Cx, mut payload: &[u8]) {
        let mut patch = json!({});
        while payload.len() >= 8 {
            let length = u16_at(payload, 0) as usize;
            if length < 8 || length > payload.len() {
                break;
            }
            let name: [u8; 4] = payload[4..8].try_into().unwrap();
            let body = &payload[8..length];
            if let Some(p) = self.decode(cx, &name, body) {
                merge_patch(&mut patch, &p);
            }
            payload = &payload[length..];
        }
        if patch.as_object().is_some_and(|m| !m.is_empty()) {
            cx.state(patch);
        }
        self.arm_duration(cx);
    }

    /// Poll the durations while streaming or recording, once ready.
    fn arm_duration(&mut self, cx: &mut Cx) {
        if self.monitor
            && self.phase == Phase::Ready
            && !self.duration_armed
            && (self.streaming_active || self.recording_active)
        {
            self.duration_armed = true;
            cx.set_timer(DURATION, DURATION_EVERY);
        }
    }

    /// One command's effect on the state, if it has one. Bodies shorter than
    /// the layout are ignored.
    fn decode(&mut self, cx: &mut Cx, name: &[u8; 4], b: &[u8]) -> Option<Value> {
        let need = |n: usize| (b.len() >= n).then_some(());
        let one = |i: u8| (i as u32 + 1).to_string();
        match name {
            b"_ver" => {
                need(4)?;
                self.version = ((u16_at(b, 0) as u32) << 16) | u16_at(b, 2) as u32;
                // A new dump: forget the previous switcher's description.
                self.topology = Topology::default();
                Some(json!({"device": {
                    "protocol_version": format!("{}.{}", u16_at(b, 0), u16_at(b, 2)),
                }}))
            }
            b"_pin" => {
                need(41)?;
                Some(json!({"device": {"product": text(&b[..40]), "model_id": b[40]}}))
            }
            b"_top" => {
                // Before 2.30 there is no multiviewer byte at offset 6 (Sofie
                // `DeviceProfile/topologyCommand.ts:11-27`).
                let shift = if self.version >= V2_30 { 1 } else { 0 };
                need(12 + shift)?;
                let t = &mut self.topology;
                t.mes = b[0];
                t.dsks = b[2];
                t.auxes = b[3];
                t.media_players = b[5];
                t.dves = b[8 + shift];
                t.stingers = b[9 + shift];
                t.supersources = b[10 + shift];
                t.camera_control = b.get(17 + shift) == Some(&1);
                Some(json!({"topology": {
                    "mes": b[0],
                    "sources": b[1],
                    "dsks": b[2],
                    "auxes": b[3],
                    "media_players": b[5],
                    "dves": b[8 + shift],
                    "stingers": b[9 + shift],
                    "supersources": b[10 + shift],
                    "camera_control": t.camera_control,
                }}))
            }
            b"_MeC" => {
                need(2)?;
                self.topology.usks.insert(b[0], b[1]);
                Some(json!({"mes": {one(b[0]): {"keyers": b[1]}}}))
            }
            b"_MAC" => {
                need(1)?;
                self.topology.macros = b[0];
                Some(json!({"topology": {"macros": b[0]}}))
            }
            // Media pool: u8 stills, u8 clips (Sofie
            // `DeviceProfile/mediaPoolConfigCommand.ts:12-16`).
            b"_mpl" => {
                need(2)?;
                self.topology.stills = b[0];
                self.topology.clips = b[1];
                Some(json!({"topology": {"stills": b[0], "clips": b[1]}}))
            }
            // SuperSource boxes: from 2.28 u8 index, rsv, u8 boxes; before,
            // u8 boxes for the only SuperSource (Sofie
            // `DeviceProfile/superSourceConfigCommand.ts:17-23`).
            b"_SSC" => {
                if self.version >= V2_28 {
                    need(3)?;
                    self.topology.supersource_boxes.insert(b[0], b[2]);
                } else {
                    need(1)?;
                    self.topology.supersource_boxes.insert(0, b[0]);
                }
                None
            }
            // Classic audio and Fairlight mixer configurations (Sofie
            // `DeviceProfile/audioMixerConfigCommand.ts`,
            // `fairlightAudioMixerConfigCommand.ts`).
            b"_AMC" => {
                self.topology.classic_audio = true;
                Some(json!({"topology": {"audio": "classic"}}))
            }
            b"_FAC" => {
                self.topology.fairlight = true;
                Some(json!({"topology": {"audio": "fairlight"}}))
            }
            b"InPr" => {
                need(33)?;
                let id = u16_at(b, 0);
                self.topology.sources.insert(id, b[32]);
                Some(json!({"sources": {id.to_string(): {
                    "long_name": text(&b[2..22]),
                    "short_name": text(&b[22..26]),
                    "kind": source_kind(b[32]),
                }}}))
            }
            b"PrgI" => {
                need(4)?;
                Some(json!({"mes": {one(b[0]): {"program": u16_at(b, 2)}}}))
            }
            b"PrvI" => {
                need(4)?;
                Some(json!({"mes": {one(b[0]): {"preview": u16_at(b, 2)}}}))
            }
            // M/E, style, selection, next style, next selection (Sofie
            // `MixEffects/Transition/TransitionPropertiesCommand.ts:46-53`).
            b"TrSS" => {
                need(2)?;
                let mut t = json!({"style": transition_style(b[1])});
                if b.len() >= 5 {
                    self.topology.next_selection.insert(b[0], b[4]);
                    t["selection"] = json!(layers(b[2]));
                    t["next_style"] = json!(transition_style(b[3]));
                    t["next_selection"] = json!(layers(b[4]));
                }
                Some(json!({"mes": {one(b[0]): {"transition": t}}}))
            }
            b"TrPs" => {
                need(6)?;
                Some(json!({"mes": {one(b[0]): {"transition": {
                    "in_progress": b[1] != 0,
                    "position": u16_at(b, 4) as f64 / 10_000.0,
                }}}}))
            }
            // M/E, preview on (Sofie `TransitionPreviewCommand.ts:38-45`).
            b"TrPr" => {
                need(2)?;
                Some(json!({"mes": {one(b[0]): {"transition": {"preview": b[1] != 0}}}}))
            }
            // M/E, rate (Sofie `TransitionMixCommand.ts:35-42`).
            b"TMxP" => {
                need(2)?;
                Some(json!({"mes": {one(b[0]): {"transition": {"mix": {"rate": b[1]}}}}}))
            }
            // M/E, rate, u16 source (Sofie `TransitionDipCommand.ts:41-48`).
            b"TDpP" => {
                need(4)?;
                Some(json!({"mes": {one(b[0]): {"transition": {"dip": {
                    "rate": b[1], "source": u16_at(b, 2),
                }}}}}))
            }
            // Sofie `TransitionWipeCommand.ts:62-77`; OpenSwitcher TWpP.
            b"TWpP" => {
                need(18)?;
                Some(json!({"mes": {one(b[0]): {"transition": {"wipe": {
                    "rate": b[1],
                    "pattern": named(&PATTERNS, b[2]),
                    "border_width": hundredths(u16_at(b, 4) as i64),
                    "border_source": u16_at(b, 6),
                    "symmetry": hundredths(u16_at(b, 8) as i64),
                    "softness": hundredths(u16_at(b, 10) as i64),
                    "x": u16_at(b, 12) as f64 / 10_000.0,
                    "y": u16_at(b, 14) as f64 / 10_000.0,
                    "reverse": b[16] == 1,
                    "flip_flop": b[17] == 1,
                }}}}}))
            }
            // Sofie `TransitionDVECommand.ts:66-85`; OpenSwitcher TDvP.
            b"TDvP" => {
                need(17)?;
                Some(json!({"mes": {one(b[0]): {"transition": {"dve": {
                    "rate": b[1],
                    "style": named(&DVE_STYLES, b[3]),
                    "fill_source": u16_at(b, 4),
                    "key_source": u16_at(b, 6),
                    "enable_key": b[8] == 1,
                    "pre_multiplied": b[9] == 1,
                    "clip": tenths(u16_at(b, 10)),
                    "gain": tenths(u16_at(b, 12)),
                    "invert_key": b[14] == 1,
                    "reverse": b[15] == 1,
                    "flip_flop": b[16] == 1,
                }}}}}))
            }
            // Sofie `TransitionStingerCommand.ts:60-74`.
            b"TStP" => {
                need(18)?;
                Some(json!({"mes": {one(b[0]): {"transition": {"sting": {
                    "media_player": b[1],
                    "pre_multiplied": b[2] == 1,
                    "clip": tenths(u16_at(b, 4)),
                    "gain": tenths(u16_at(b, 6)),
                    "invert": b[8] == 1,
                    "preroll": u16_at(b, 10),
                    "clip_duration": u16_at(b, 12),
                    "trigger_point": u16_at(b, 14),
                    "mix_rate": u16_at(b, 16),
                }}}}}))
            }
            b"FtbS" => {
                need(3)?;
                Some(json!({"mes": {one(b[0]): {"fade_to_black": {
                    "black": b[1] != 0,
                    "in_progress": b[2] != 0,
                }}}}))
            }
            // M/E, rate (Sofie `FadeToBlackRateCommand.ts:35-40`).
            b"FtbP" => {
                need(2)?;
                Some(json!({"mes": {one(b[0]): {"fade_to_black": {"rate": b[1]}}}}))
            }
            b"KeOn" => {
                need(3)?;
                Some(json!({"mes": {one(b[0]): {
                    "usk": {one(b[1]): {"on_air": b[2] != 0}},
                }}}))
            }
            // M/E, keyer, type, rsv, can fly, fly enabled, u16 fill, u16 key,
            // mask enabled, rsv, i16 mask top, bottom, left, right (Sofie
            // `Key/MixEffectKeyPropertiesGetCommand.ts:18-35`).
            b"KeBP" => {
                need(20)?;
                self.topology.usk_can_fly.insert((b[0], b[1]), b[4] == 1);
                Some(json!({"mes": {one(b[0]): {"usk": {one(b[1]): {
                    "type": named(&KEY_TYPES, b[2]),
                    "can_fly": b[4] == 1,
                    "fly_enabled": b[5] == 1,
                    "fill_source": u16_at(b, 6),
                    "key_source": u16_at(b, 8),
                    "mask": keyers::mask(b, 10, true),
                }}}}}))
            }
            // Sofie `Key/MixEffectKeyLumaCommand.ts:52-60`.
            b"KeLm" => {
                need(9)?;
                Some(json!({"mes": {one(b[0]): {"usk": {one(b[1]): {"luma": {
                    "pre_multiplied": b[2] == 1,
                    "clip": tenths(u16_at(b, 4)),
                    "gain": tenths(u16_at(b, 6)),
                    "invert": b[8] == 1,
                }}}}}}))
            }
            // Sofie `Key/MixEffectKeyPatternCommand.ts:59-70`.
            b"KePt" => {
                need(15)?;
                Some(json!({"mes": {one(b[0]): {"usk": {one(b[1]): {"pattern": {
                    "style": named(&PATTERNS, b[2]),
                    "size": hundredths(u16_at(b, 4) as i64),
                    "symmetry": hundredths(u16_at(b, 6) as i64),
                    "softness": hundredths(u16_at(b, 8) as i64),
                    "x": u16_at(b, 10) as f64 / 10_000.0,
                    "y": u16_at(b, 12) as f64 / 10_000.0,
                    "invert": b[14] == 1,
                }}}}}}))
            }
            b"AuxS" => {
                need(4)?;
                Some(json!({"auxes": {one(b[0]): {"source": u16_at(b, 2)}}}))
            }
            b"DskS" => {
                need(3)?;
                Some(json!({"dsks": {one(b[0]): {
                    "on_air": b[1] != 0,
                    "in_transition": b[2] != 0,
                }}}))
            }
            // DSK, u16 fill, u16 key (Sofie `DownstreamKeySourcesCommand.ts:16-21`;
            // OpenSwitcher DskB).
            b"DskB" => {
                need(6)?;
                Some(json!({"dsks": {one(b[0]): {
                    "fill_source": u16_at(b, 2),
                    "key_source": u16_at(b, 4),
                }}}))
            }
            // Sofie `DownstreamKeyPropertiesCommand.ts:16-34`.
            b"DskP" => {
                need(2)?;
                let mut d = json!({"tie": b[1] != 0});
                if b.len() >= 9 {
                    d["rate"] = json!(b[2]);
                    d["pre_multiplied"] = json!(b[3] == 1);
                    d["clip"] = json!(tenths(u16_at(b, 4)));
                    d["gain"] = json!(tenths(u16_at(b, 6)));
                    d["invert"] = json!(b[8] == 1);
                }
                // Then mask enabled and the i16 edges from 10.
                if b.len() >= 18 {
                    d["mask"] = keyers::mask(b, 9, false);
                }
                Some(json!({"dsks": {one(b[0]): d}}))
            }
            // Index, rsv, hue 0-3599, saturation 0-1000, luma 0-1000
            // (OpenSwitcher ColorGeneratorField; Sofie `ColorGeneratorCommand.ts:43-52`).
            b"ColV" => {
                need(8)?;
                self.topology.color_generators.insert(b[0]);
                Some(json!({"colors": {one(b[0]): {
                    "hue": tenths(u16_at(b, 2)),
                    "saturation": tenths(u16_at(b, 4)),
                    "luma": tenths(u16_at(b, 6)),
                }}}))
            }
            // Player, source type (1 still, 2 clip), still, clip (Sofie
            // `Media/MediaPlayerSourceCommand.ts:44-51`).
            b"MPCE" => {
                need(4)?;
                let kind = match b[1] {
                    1 => "still",
                    2 => "clip",
                    _ => "unknown",
                };
                Some(json!({"media_players": {one(b[0]): {
                    "source_type": kind,
                    "still": b[2] as u32 + 1,
                    "clip": b[3] as u32 + 1,
                }}}))
            }
            // Player, playing, loop, at beginning, u16 frame (Sofie
            // `Media/MediaPlayerStatusCommand.ts:45-53`).
            b"RCPS" => {
                need(6)?;
                Some(json!({"media_players": {one(b[0]): {
                    "playing": b[1] == 1,
                    "loop": b[2] == 1,
                    "at_beginning": b[3] == 1,
                    "clip_frame": u16_at(b, 4),
                }}}))
            }
            // Clip, used, name[64], u16 frames (Sofie
            // `Media/MediaPoolClipDescription.ts:17-24`).
            b"MPCS" => {
                need(68)?;
                let clip = if b[1] == 1 {
                    json!({"name": text(&b[2..66]), "frames": u16_at(b, 66)})
                } else {
                    Value::Null
                };
                Some(json!({"media_pool": {"clips": {one(b[0]): clip}}}))
            }
            // Pool (0 stills), rsv, u16 index, used, hash[16], name length,
            // name (Sofie `Media/MediaPoolFrameDescription.ts:19-27`). Pools
            // 1 and up are clip frames, not kept.
            b"MPfe" => {
                need(24)?;
                if b[0] != 0 {
                    return None;
                }
                let slot = (u16_at(b, 2) as u32 + 1).to_string();
                let still = if b[4] == 1 {
                    let end = (24 + b[23] as usize).min(b.len());
                    json!({"name": text(&b[24..end])})
                } else {
                    Value::Null
                };
                Some(json!({"media_pool": {"stills": {slot: still}}}))
            }
            // From 2.28: u8 SuperSource, u8 box, enabled, rsv, then the box at
            // 4; before, u8 box, enabled, then the box at 2 (Sofie
            // `SuperSource/SuperSourceBoxParametersCommand.ts:71-91`).
            b"SSBP" => {
                let (ssrc, bx, enabled, i) = if self.version >= V2_28 {
                    need(22)?;
                    (b[0], b[1], b[2], 2)
                } else {
                    need(20)?;
                    (0, b[0], b[1], 0)
                };
                Some(json!({"supersources": {one(ssrc): {"boxes": {one(bx): {
                    "enabled": enabled == 1,
                    "source": u16_at(b, i + 2),
                    "x": hundredths(i16_at(b, i + 4) as i64),
                    "y": hundredths(i16_at(b, i + 6) as i64),
                    "size": u16_at(b, i + 8) as f64 / 1000.0,
                    "cropped": b[i + 10] == 1,
                    "crop_top": u16_at(b, i + 12) as f64 / 1000.0,
                    "crop_bottom": u16_at(b, i + 14) as f64 / 1000.0,
                    "crop_left": u16_at(b, i + 16) as f64 / 1000.0,
                    "crop_right": u16_at(b, i + 18) as f64 / 1000.0,
                }}}}}))
            }
            // From 2.28: u8 SuperSource, rsv, art at 2; before, art at 0 for
            // the only SuperSource (Sofie
            // `SuperSource/SuperSourcePropertiesCommand.ts:164-174, 220-230`).
            b"SSrc" => {
                let (ssrc, i) = if self.version >= V2_28 {
                    need(13)?;
                    (b[0], 2)
                } else {
                    need(11)?;
                    (0, 0)
                };
                Some(json!({"supersources": {one(ssrc): {"art": {
                    "fill_source": u16_at(b, i),
                    "key_source": u16_at(b, i + 2),
                    "placement": if b[i + 4] == 1 { "foreground" } else { "background" },
                    "pre_multiplied": b[i + 5] == 1,
                    "clip": tenths(u16_at(b, i + 6)),
                    "gain": tenths(u16_at(b, i + 8)),
                    "invert": b[i + 10] == 1,
                }}}}))
            }
            // u16 input, source type, rsv, u16 port, mix option, rsv, u16 gain,
            // i16 balance (Sofie `Audio/AudioMixerInputCommand.ts:50-58`).
            b"AMIP" => {
                need(14)?;
                let input = u16_at(b, 0);
                self.topology.classic_inputs.insert(input);
                let kind = match b[2] {
                    0 => "video",
                    1 => "media_player",
                    2 => "external",
                    _ => "unknown",
                };
                Some(json!({"audio": {"inputs": {input.to_string(): {
                    "source_type": kind,
                    "mix_option": classic_mix(b[8]),
                    "gain": classic_db(u16_at(b, 10)),
                    "balance": classic_balance(i16_at(b, 12)),
                }}}}))
            }
            // u16 gain, i16 balance (Sofie `Audio/AudioMixerMasterCommand.ts:27-33`).
            b"AMMO" => {
                need(4)?;
                Some(json!({"audio": {"master": {
                    "gain": classic_db(u16_at(b, 0)),
                    "balance": classic_balance(i16_at(b, 2)),
                }}}))
            }
            // u16 input, rsv, i64 source at 8, gain i32 at 20, balance i16 at
            // 40, fader gain i32 at 44, mix options at 48, mix option at 49,
            // gains in hundredths of a dB (Sofie
            // `Fairlight/FairlightMixerSourceCommand.ts:107-129`; Companion
            // `actions/fairlightAudio.ts:298, 472`).
            b"FASP" => {
                need(50)?;
                let input = u16_at(b, 0);
                let source = i64_at(b, 8);
                self.topology
                    .fairlight_sources
                    .insert((input, source), b[48]);
                Some(
                    json!({"fairlight": {"inputs": {input.to_string(): {"sources": {
                        source.to_string(): {
                            "gain": hundredths(i32_at(b, 20) as i64),
                            "balance": hundredths(i16_at(b, 40) as i64),
                            "fader_gain": hundredths(i32_at(b, 44) as i64),
                            "mix_options": fairlight_mixes(b[48]),
                            "mix_option": fairlight_mix(b[49]),
                        },
                    }}}}}),
                )
            }
            // Fader gain i32 at 12, follow fade to black at 16 (Sofie
            // `Fairlight/FairlightMixerMasterCommand.ts:41-49`).
            b"FAMP" => {
                need(17)?;
                Some(json!({"fairlight": {"master": {
                    "fader_gain": hundredths(i32_at(b, 12) as i64),
                    "follow_fade_to_black": b[16] != 0,
                }}}))
            }
            // Levels in hundredths of a dB, sent only while subscribed:
            // i64 source, u16 input, then the output levels and peaks at 32
            // (Sofie `Fairlight/FairlightMixerSourceLevelsCommand.ts:17-39`;
            // Companion `audioLevels.ts:12`).
            b"FMLv" => {
                need(40)?;
                let source = i64_at(b, 0);
                let input = u16_at(b, 8);
                Some(
                    json!({"fairlight": {"inputs": {input.to_string(): {"sources": {
                        source.to_string(): {"level": {
                            "left": hundredths(i16_at(b, 32) as i64),
                            "right": hundredths(i16_at(b, 34) as i64),
                            "left_peak": hundredths(i16_at(b, 36) as i64),
                            "right_peak": hundredths(i16_at(b, 38) as i64),
                        }},
                    }}}}}),
                )
            }
            // Master levels at 20 (Sofie `Fairlight/FairlightMixerMasterLevelsCommand.ts:9-28`).
            b"FDLv" => {
                need(28)?;
                Some(json!({"fairlight": {"master": {"level": {
                    "left": hundredths(i16_at(b, 20) as i64),
                    "right": hundredths(i16_at(b, 22) as i64),
                    "left_peak": hundredths(i16_at(b, 24) as i64),
                    "right_peak": hundredths(i16_at(b, 26) as i64),
                }}}}))
            }
            // Multiviewer, window, u16 source (Sofie
            // `Settings/MultiViewerSourceCommand.ts:39-47`). Constellation HD
            // models resend every window every frame (`:59-60`), so only
            // changes reach the state.
            b"MvIn" => {
                need(4)?;
                let source = u16_at(b, 2);
                let previous = self
                    .topology
                    .multiviewer_windows
                    .insert((b[0], b[1]), source);
                if previous == Some(source) {
                    return None;
                }
                Some(json!({"multiviewers": {one(b[0]): {"windows": {one(b[1]): {
                    "source": source,
                }}}}}))
            }
            b"TlSr" => {
                need(2)?;
                let count = u16_at(b, 0) as usize;
                let mut tally = Map::new();
                for i in 0..count {
                    let at = 2 + 3 * i;
                    if at + 3 > b.len() {
                        break;
                    }
                    let flags = b[at + 2];
                    tally.insert(
                        u16_at(b, at).to_string(),
                        json!({"program": flags & 1 != 0, "preview": flags & 2 != 0}),
                    );
                }
                Some(json!({"tally": tally}))
            }
            b"MPrp" => {
                need(8)?;
                let index = u16_at(b, 0);
                let used = b[2] != 0;
                self.topology.macros_used.insert(index, used);
                let name_len = u16_at(b, 4) as usize;
                let slot = (index as u32 + 1).to_string();
                if !used {
                    return Some(json!({"macros": {slot: null}}));
                }
                let name = b.get(8..8 + name_len).map(String::from_utf8_lossy)?;
                Some(json!({"macros": {slot: {"name": name}}}))
            }
            // Bit 0 running, bit 1 waiting; loop; u16 index (Sofie
            // `Macro/MacroRunStatusCommand.ts:8-15`).
            b"MRPr" => {
                need(4)?;
                let running = b[0] & 1 != 0;
                let index = u16_at(b, 2);
                Some(json!({
                    "macro_running": if running && index != 0xFFFF {
                        json!(index as u32 + 1)
                    } else {
                        Value::Null
                    },
                    "macro_waiting": b[0] & 2 != 0,
                    "macro_loop": b[1] != 0,
                }))
            }
            b"StRS" => {
                need(2)?;
                self.topology.encoder = true;
                let status = u16_at(b, 0);
                let state = if status & 32 != 0 {
                    "stopping"
                } else if status & 4 != 0 {
                    "streaming"
                } else if status & 2 != 0 {
                    "connecting"
                } else {
                    "idle"
                };
                self.streaming_active = status & (4 | 32) != 0;
                Some(json!({"streaming": {"state": state}}))
            }
            b"RTMS" => {
                need(2)?;
                self.topology.encoder = true;
                let status = u16_at(b, 0);
                let error = if status & 32768 != 0 {
                    "unknown"
                } else if status & 4 != 0 {
                    "media_full"
                } else if status & 8 != 0 {
                    "media_error"
                } else if status & 16 != 0 {
                    "media_unformatted"
                } else if status & 32 != 0 {
                    "dropping_frames"
                } else if status & 2 != 0 {
                    "none"
                } else {
                    "no_media"
                };
                self.recording_active = status & 1 != 0;
                let mut r = json!({"active": status & 1 != 0, "error": error});
                // Seconds of recording time left, 0xFFFFFFFF with no disk
                // (Sofie `Recording/RecordingStatusCommand.ts:49-52`).
                if b.len() >= 8 {
                    let left = u32_at(b, 4);
                    r["time_available"] = if left == u32::MAX {
                        Value::Null
                    } else {
                        json!(left)
                    };
                }
                Some(json!({"recording": r}))
            }
            // Durations, sent in answer to SRDR and RMDR.
            b"SRST" => {
                need(5)?;
                Some(json!({"streaming": {"duration": timecode(b)}}))
            }
            b"RTMR" => {
                need(5)?;
                Some(json!({"recording": {"duration": timecode(b)}}))
            }
            // Service name[64], url[512], key[512], bitrates (Sofie
            // `Streaming/StreamingServiceCommand.ts:40-46`). The key is not
            // put in the state.
            b"SRSU" => {
                need(576)?;
                Some(json!({"streaming": {
                    "service_name": text(&b[..64]),
                    "url": text(&b[64..576]),
                }}))
            }
            // Camera control: camera, category, parameter, type, the counts of
            // 8-, 16-, 32- and 64-bit values at 4, 6, 8, 10, values from 16
            // (Sofie `CameraControlCommand.ts:177-249`). Ids from
            // camera-control `ids.d.ts`.
            b"CCdP" => {
                need(16)?;
                let (camera, category, parameter, kind) = (b[0], b[1], b[2], b[3]);
                let float = |n: usize| -> Option<f64> {
                    (kind == CC_FLOAT && u16_at(b, 6) as usize >= n && b.len() >= 16 + 2 * n)
                        .then(|| i16_at(b, 16 + 2 * (n - 1)) as f64 / 2048.0)
                };
                let field = match (category, parameter) {
                    (0, 0) => json!({"focus": float(1)?}),
                    (0, 3) => json!({"iris": float(1)?}),
                    (0, 9) => json!({"zoom_speed": float(1)?}),
                    (1, 2) if kind == CC_SINT16 && u16_at(b, 6) >= 2 && b.len() >= 20 => {
                        json!({"white_balance": i16_at(b, 16), "tint": i16_at(b, 18)})
                    }
                    (1, 5) if kind == CC_SINT32 && u16_at(b, 8) >= 1 && b.len() >= 20 => {
                        json!({"exposure_us": i32_at(b, 16)})
                    }
                    (1, 13) if kind == CC_SINT8 && u16_at(b, 4) >= 1 && b.len() >= 17 => {
                        json!({"gain": b[16] as i8})
                    }
                    _ => return None,
                };
                Some(json!({"cameras": {camera.to_string(): field}}))
            }
            b"InCm" => {
                if self.phase == Phase::Loading {
                    self.phase = Phase::Ready;
                    self.connected = true;
                    self.hello_attempts = 0;
                    cx.connection(Connection::Connected);
                    // A new session does not remember the level subscription.
                    if self.monitor && self.levels_wanted && self.topology.fairlight {
                        self.send_commands(cx, None, &[(*b"SFLN", vec![1, 0, 0, 0])]);
                    }
                }
                None
            }
            _ => self.decode_keyers(name, b),
        }
    }

    fn check_me(&self, params: &Params) -> Result<u8, CommandError> {
        let me = int(params, "me").max(1);
        if me > self.topology.mes as i64 {
            return Err(invalid(format!(
                "M/E {me} does not exist; the switcher has {}",
                self.topology.mes
            )));
        }
        Ok((me - 1) as u8)
    }

    fn check_source(&self, params: &Params) -> Result<u16, CommandError> {
        self.source_param(params, "source")?
            .ok_or_else(|| invalid("source is required".into()))
    }

    /// An optional source parameter, which must be one of the switcher's.
    fn source_param(&self, params: &Params, name: &str) -> Result<Option<u16>, CommandError> {
        match opt_int(params, name) {
            None => Ok(None),
            Some(s)
                if (0..=65535).contains(&s) && self.topology.sources.contains_key(&(s as u16)) =>
            {
                Ok(Some(s as u16))
            }
            Some(s) => Err(invalid(format!("the switcher has no source {s}"))),
        }
    }

    fn check_index(&self, params: &Params, name: &str, count: u8) -> Result<u8, CommandError> {
        let n = int(params, name);
        if n < 1 || n > count as i64 {
            return Err(invalid(format!(
                "{name} {n} does not exist; the switcher has {count}"
            )));
        }
        Ok((n - 1) as u8)
    }

    fn check_keyer(&self, params: &Params) -> Result<(u8, u8), CommandError> {
        let me = self.check_me(params)?;
        let keyers = self.topology.usks.get(&me).copied().unwrap_or(0);
        Ok((me, self.check_index(params, "keyer", keyers)?))
    }

    /// A camera reachable over SDI camera control: an external input, on a
    /// switcher that has camera control.
    fn check_camera(&self, name: &str, params: &Params) -> Result<u8, CommandError> {
        if !self.topology.camera_control {
            return Err(unsupported(name, "this switcher (no camera control)"));
        }
        let camera = int(params, "camera");
        match self.topology.sources.get(&(camera as u16)) {
            Some(0) if (1..=255).contains(&camera) => Ok(camera as u8),
            _ => Err(invalid(format!("camera {camera} is not an external input"))),
        }
    }

    fn check_supersource(&self, name: &str, params: &Params) -> Result<(u8, u8), CommandError> {
        if self.topology.supersources == 0 {
            return Err(unsupported(name, "this switcher (no SuperSource)"));
        }
        let ssrc = self.check_index(params, "supersource", self.topology.supersources)?;
        if self.version < V2_28 && ssrc != 0 {
            return Err(invalid(
                "before protocol 2.28 there is one SuperSource".into(),
            ));
        }
        let boxes = self
            .topology
            .supersource_boxes
            .get(&ssrc)
            .copied()
            .unwrap_or(4);
        Ok((ssrc, boxes))
    }

    fn check_media_player(
        &self,
        params: &Params,
        clips: bool,
        name: &str,
    ) -> Result<u8, CommandError> {
        if clips && self.topology.clips == 0 {
            return Err(unsupported(name, "this switcher (no clip pool)"));
        }
        self.check_index(params, "player", self.topology.media_players)
    }

    /// A command's name and body, checked against the switcher's topology.
    fn build(&self, name: &str, params: &Params) -> Result<Out, CommandError> {
        let one =
            |n: &[u8; 4], body: Vec<u8>| -> Result<Out, CommandError> { Ok(vec![(*n, body)]) };
        match name {
            "cut" => one(b"DCut", vec![self.check_me(params)?, 0, 0, 0]),
            "auto" => one(b"DAut", vec![self.check_me(params)?, 0, 0, 0]),
            "fade_to_black" => one(b"FtbA", vec![self.check_me(params)?, 0, 0, 0]),
            "set_program" | "set_preview" => {
                let me = self.check_me(params)?;
                let [hi, lo] = self.check_source(params)?.to_be_bytes();
                let name = if name == "set_program" {
                    b"CPgI"
                } else {
                    b"CPvI"
                };
                one(name, vec![me, 0, hi, lo])
            }
            "set_transition_style" => {
                let me = self.check_me(params)?;
                let style = match params.get("style").and_then(Value::as_str) {
                    Some("mix") => 0,
                    Some("dip") => 1,
                    Some("wipe") => 2,
                    Some("dve") => 3,
                    Some("sting") => 4,
                    other => return Err(invalid(format!("unknown style {other:?}"))),
                };
                // Mask bit 0: set the style only.
                one(b"CTTp", vec![0x01, me, style, 0])
            }
            // Mask bit 1: the next transition's layers (Sofie
            // `TransitionPropertiesCommand.ts:23-31`; OpenSwitcher CTTp).
            // Layers not given keep their current selection.
            "set_next_transition" => {
                let me = self.check_me(params)?;
                let keyers = self.topology.usks.get(&me).copied().unwrap_or(0);
                let mut selection = self.topology.next_selection.get(&me).copied().unwrap_or(1);
                let mut any = false;
                for (bit, layer) in LAYERS.iter().enumerate() {
                    if let Some(on) = opt_bool(params, layer) {
                        if bit > keyers as usize {
                            return Err(invalid(format!(
                                "{layer} does not exist; M/E {} has {keyers} keyers",
                                me + 1
                            )));
                        }
                        any = true;
                        if on {
                            selection |= 1 << bit;
                        } else {
                            selection &= !(1 << bit);
                        }
                    }
                }
                if !any {
                    return Err(nothing_to_set());
                }
                if selection == 0 {
                    return Err(invalid(
                        "the next transition needs at least one layer".into(),
                    ));
                }
                one(b"CTTp", vec![0x02, me, 0, selection])
            }
            "set_transition_position" => {
                let me = self.check_me(params)?;
                let position = params
                    .get("position")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let [hi, lo] = ((position.clamp(0.0, 1.0) * 10_000.0).round() as u16).to_be_bytes();
                one(b"CTPs", vec![me, 0, hi, lo])
            }
            // M/E, preview on, 2 rsv (Sofie `TransitionPreviewCommand.ts:19-24`;
            // OpenSwitcher CTPr).
            "set_preview_transition" => {
                let me = self.check_me(params)?;
                one(b"CTPr", vec![me, flag(params, "enabled") as u8, 0, 0])
            }
            // M/E, rate, 2 rsv (Sofie `TransitionMixCommand.ts:16-21`;
            // OpenSwitcher CTMx).
            "set_mix_rate" => {
                let me = self.check_me(params)?;
                one(b"CTMx", vec![me, int(params, "rate") as u8, 0, 0])
            }
            // Mask (bit 0 rate, bit 1 source), M/E, rate, rsv, u16 source, 2
            // rsv (Sofie `TransitionDipCommand.ts:5-27`; OpenSwitcher CTDp).
            "set_dip" => {
                let me = self.check_me(params)?;
                let mut b = vec![0u8; 8];
                b[1] = me;
                if let Some(rate) = opt_int(params, "rate") {
                    b[0] |= 1;
                    b[2] = rate as u8;
                }
                if let Some(source) = self.source_param(params, "source")? {
                    b[0] |= 2;
                    put16(&mut b, 4, source);
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CTDp", b)
            }
            // u16 mask, M/E, rate, pattern, rsv, border width, border source,
            // symmetry, softness, x, y (u16 each), reverse, flip flop (Sofie
            // `TransitionWipeCommand.ts:5-48`; OpenSwitcher CTWp, which gives
            // the 0-10000 ranges; percent and 0-1 scaling from Companion
            // `actions/mixeffect/transition.ts:527-541`).
            "set_wipe" => {
                let me = self.check_me(params)?;
                let mut b = vec![0u8; 20];
                let mut mask = 0u16;
                b[2] = me;
                if let Some(rate) = opt_int(params, "rate") {
                    mask |= 1;
                    b[3] = rate as u8;
                }
                if let Some(p) = enum_param(params, "pattern", &PATTERNS)? {
                    mask |= 1 << 1;
                    b[4] = p;
                }
                for (bit, name, at, by) in [
                    (2, "border_width", 6, 100.0),
                    (4, "symmetry", 10, 100.0),
                    (5, "softness", 12, 100.0),
                    (6, "x", 14, 10_000.0),
                    (7, "y", 16, 10_000.0),
                ] {
                    if let Some(v) = scaled(params, name, by) {
                        mask |= 1 << bit;
                        put16(&mut b, at, v as u16);
                    }
                }
                if let Some(source) = self.source_param(params, "border_source")? {
                    mask |= 1 << 3;
                    put16(&mut b, 8, source);
                }
                if let Some(v) = opt_bool(params, "reverse") {
                    mask |= 1 << 8;
                    b[18] = v as u8;
                }
                if let Some(v) = opt_bool(params, "flip_flop") {
                    mask |= 1 << 9;
                    b[19] = v as u8;
                }
                if mask == 0 {
                    return Err(nothing_to_set());
                }
                put16(&mut b, 0, mask);
                one(b"CTWp", b)
            }
            // u16 mask, M/E, rate, logo rate, style, u16 fill, u16 key, enable
            // key, pre-multiplied, u16 clip, u16 gain, invert, reverse, flip
            // flop (Sofie `TransitionDVECommand.ts:5-52`). OpenSwitcher's CTDv
            // table puts the style at 4; Sofie's, consistent with its TDvP
            // decoder and OpenSwitcher's own TDvP table, puts it at 5.
            "set_dve_transition" => {
                if self.topology.dves == 0 {
                    return Err(unsupported(name, "this switcher (no DVE)"));
                }
                let me = self.check_me(params)?;
                let mut b = vec![0u8; 20];
                let mut mask = 0u16;
                b[2] = me;
                if let Some(rate) = opt_int(params, "rate") {
                    mask |= 1;
                    b[3] = rate as u8;
                }
                if let Some(s) = enum_param(params, "style", &DVE_STYLES)? {
                    mask |= 1 << 2;
                    b[5] = s;
                }
                if let Some(s) = self.source_param(params, "fill_source")? {
                    mask |= 1 << 3;
                    put16(&mut b, 6, s);
                }
                if let Some(s) = self.source_param(params, "key_source")? {
                    mask |= 1 << 4;
                    put16(&mut b, 8, s);
                }
                for (bit, name, at) in [
                    (5, "enable_key", 10),
                    (6, "pre_multiplied", 11),
                    (9, "invert_key", 16),
                    (10, "reverse", 17),
                    (11, "flip_flop", 18),
                ] {
                    if let Some(v) = opt_bool(params, name) {
                        mask |= 1 << bit;
                        b[at] = v as u8;
                    }
                }
                for (bit, name, at) in [(7, "clip", 12), (8, "gain", 14)] {
                    if let Some(v) = scaled(params, name, 10.0) {
                        mask |= 1 << bit;
                        put16(&mut b, at, v as u16);
                    }
                }
                if mask == 0 {
                    return Err(nothing_to_set());
                }
                put16(&mut b, 0, mask);
                one(b"CTDv", b)
            }
            // u16 mask, M/E, media player, pre-multiplied, rsv, u16 clip, u16
            // gain, invert, rsv, u16 preroll, clip duration, trigger point,
            // mix rate (Sofie `TransitionStingerCommand.ts:5-46`). The media
            // player counts from 1: every capture in Sofie's tests holds 1 or
            // 2 there.
            "set_stinger" => {
                if self.topology.stingers == 0 {
                    return Err(unsupported(name, "this switcher (no stinger)"));
                }
                let me = self.check_me(params)?;
                let mut b = vec![0u8; 20];
                let mut mask = 0u16;
                b[2] = me;
                if opt_int(params, "media_player").is_some() {
                    let player =
                        self.check_index(params, "media_player", self.topology.media_players)?;
                    mask |= 1;
                    b[3] = player + 1;
                }
                if let Some(v) = opt_bool(params, "pre_multiplied") {
                    mask |= 1 << 1;
                    b[4] = v as u8;
                }
                for (bit, name, at) in [(2, "clip", 6), (3, "gain", 8)] {
                    if let Some(v) = scaled(params, name, 10.0) {
                        mask |= 1 << bit;
                        put16(&mut b, at, v as u16);
                    }
                }
                if let Some(v) = opt_bool(params, "invert") {
                    mask |= 1 << 4;
                    b[10] = v as u8;
                }
                for (bit, name, at) in [
                    (5, "preroll", 12),
                    (6, "clip_duration", 14),
                    (7, "trigger_point", 16),
                    (8, "mix_rate", 18),
                ] {
                    if let Some(v) = opt_int(params, name) {
                        mask |= 1 << bit;
                        put16(&mut b, at, v as u16);
                    }
                }
                if mask == 0 {
                    return Err(nothing_to_set());
                }
                put16(&mut b, 0, mask);
                one(b"CTSt", b)
            }
            "set_aux" => {
                let aux = self.check_index(params, "aux", self.topology.auxes)?;
                let [hi, lo] = self.check_source(params)?.to_be_bytes();
                one(b"CAuS", vec![0x01, aux, hi, lo])
            }
            "dsk_auto" => {
                let dsk = self.check_index(params, "dsk", self.topology.dsks)?;
                if self.version >= V2_29 {
                    // Mask 0: toggle, without a direction.
                    one(b"DDsA", vec![0, dsk, 0, 0])
                } else {
                    one(b"DDsA", vec![dsk, 0, 0, 0])
                }
            }
            "dsk_on_air" => {
                let dsk = self.check_index(params, "dsk", self.topology.dsks)?;
                one(b"CDsL", vec![dsk, flag(params, "on_air") as u8, 0, 0])
            }
            // DSK, tie, 2 rsv (Sofie `DownstreamKeyTieCommand.ts:14-19`;
            // OpenSwitcher CDsT).
            "set_dsk_tie" => {
                let dsk = self.check_index(params, "dsk", self.topology.dsks)?;
                one(b"CDsT", vec![dsk, flag(params, "tie") as u8, 0, 0])
            }
            // DSK, rate, 2 rsv (Sofie `DownstreamKeyRateCommand.ts:14-19`;
            // OpenSwitcher CDsR).
            "set_dsk_rate" => {
                let dsk = self.check_index(params, "dsk", self.topology.dsks)?;
                one(b"CDsR", vec![dsk, int(params, "rate") as u8, 0, 0])
            }
            // DSK, rsv, u16 source, one command for fill and one for key
            // (Sofie `DownstreamKeyFillSourceCommand.ts:14-19`,
            // `DownstreamKeyCutSourceCommand.ts:14-19`; OpenSwitcher CDsF, CDsC).
            "set_dsk_sources" => {
                let dsk = self.check_index(params, "dsk", self.topology.dsks)?;
                let mut out = Vec::new();
                if let Some(s) = self.source_param(params, "fill")? {
                    let [hi, lo] = s.to_be_bytes();
                    out.push((*b"CDsF", vec![dsk, 0, hi, lo]));
                }
                if let Some(s) = self.source_param(params, "key")? {
                    let [hi, lo] = s.to_be_bytes();
                    out.push((*b"CDsC", vec![dsk, 0, hi, lo]));
                }
                if out.is_empty() {
                    return Err(nothing_to_set());
                }
                Ok(out)
            }
            // Mask, DSK, pre-multiplied, rsv, u16 clip, u16 gain, invert, 3 rsv
            // (OpenSwitcher CDsG; Sofie `DownstreamKeyGeneralCommand.ts:22-31`).
            // Clip and gain are 0-1000 for 0-100 % (Companion
            // `actions/mixeffect/upstreamKeyerLumaChroma.ts:84-85` for the
            // same fields on a luma key).
            "set_dsk_key" => {
                let dsk = self.check_index(params, "dsk", self.topology.dsks)?;
                let mut b = vec![0u8; 12];
                b[1] = dsk;
                if let Some(v) = opt_bool(params, "pre_multiplied") {
                    b[0] |= 1;
                    b[2] = v as u8;
                }
                for (bit, name, at) in [(1, "clip", 4), (2, "gain", 6)] {
                    if let Some(v) = scaled(params, name, 10.0) {
                        b[0] |= 1 << bit;
                        put16(&mut b, at, v as u16);
                    }
                }
                if let Some(v) = opt_bool(params, "invert") {
                    b[0] |= 1 << 3;
                    b[8] = v as u8;
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CDsG", b)
            }
            "usk_on_air" => {
                let (me, keyer) = self.check_keyer(params)?;
                one(b"CKOn", vec![me, keyer, flag(params, "on_air") as u8, 0])
            }
            // Mask (bit 0 type, bit 1 fly), M/E, keyer, type, fly, 3 rsv
            // (Sofie `Key/MixEffectKeyTypeSetCommand.ts:22-31`; OpenSwitcher CKTp).
            "set_usk_type" => {
                let (me, keyer) = self.check_keyer(params)?;
                let mut b = vec![0u8; 8];
                b[1] = me;
                b[2] = keyer;
                if let Some(t) = enum_param(params, "type", &KEY_TYPES)? {
                    if t == 3 && self.topology.dves == 0 {
                        return Err(unsupported(name, "this switcher (no DVE)"));
                    }
                    b[0] |= 1;
                    b[3] = t;
                }
                if let Some(fly) = opt_bool(params, "fly_enabled") {
                    if fly && self.topology.usk_can_fly.get(&(me, keyer)) == Some(&false) {
                        return Err(invalid(format!("keyer {} cannot fly", keyer + 1)));
                    }
                    b[0] |= 2;
                    b[4] = fly as u8;
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CKTp", b)
            }
            // M/E, keyer, u16 source; CKeF for fill, CKeC for key (Sofie
            // `Key/MixEffectKeyFillSourceSetCommand.ts:16-21`,
            // `MixEffectKeyCutSourceSetCommand.ts:16-21`; OpenSwitcher).
            "set_usk_sources" => {
                let (me, keyer) = self.check_keyer(params)?;
                let mut out = Vec::new();
                if let Some(s) = self.source_param(params, "fill")? {
                    let [hi, lo] = s.to_be_bytes();
                    out.push((*b"CKeF", vec![me, keyer, hi, lo]));
                }
                if let Some(s) = self.source_param(params, "key")? {
                    let [hi, lo] = s.to_be_bytes();
                    out.push((*b"CKeC", vec![me, keyer, hi, lo]));
                }
                if out.is_empty() {
                    return Err(nothing_to_set());
                }
                Ok(out)
            }
            // Mask, M/E, keyer, pre-multiplied, u16 clip, u16 gain, invert, 3
            // rsv (Sofie `Key/MixEffectKeyLumaCommand.ts:24-36`; OpenSwitcher
            // CKLm; percent x 10 from Companion
            // `actions/mixeffect/upstreamKeyerLumaChroma.ts:84-85`).
            "set_usk_luma" => {
                let (me, keyer) = self.check_keyer(params)?;
                let mut b = vec![0u8; 12];
                b[1] = me;
                b[2] = keyer;
                if let Some(v) = opt_bool(params, "pre_multiplied") {
                    b[0] |= 1;
                    b[3] = v as u8;
                }
                for (bit, name, at) in [(1, "clip", 4), (2, "gain", 6)] {
                    if let Some(v) = scaled(params, name, 10.0) {
                        b[0] |= 1 << bit;
                        put16(&mut b, at, v as u16);
                    }
                }
                if let Some(v) = opt_bool(params, "invert") {
                    b[0] |= 1 << 3;
                    b[8] = v as u8;
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CKLm", b)
            }
            // Mask, M/E, keyer, style, u16 size, symmetry, softness, x, y,
            // invert, rsv (Sofie `Key/MixEffectKeyPatternCommand.ts:28-43`;
            // scaling from Companion `actions/mixeffect/upstreamKeyerPattern.ts:60-78`).
            "set_usk_pattern" => {
                let (me, keyer) = self.check_keyer(params)?;
                let mut b = vec![0u8; 16];
                b[1] = me;
                b[2] = keyer;
                if let Some(s) = enum_param(params, "style", &PATTERNS)? {
                    b[0] |= 1;
                    b[3] = s;
                }
                for (bit, name, at, by) in [
                    (1, "size", 4, 100.0),
                    (2, "symmetry", 6, 100.0),
                    (3, "softness", 8, 100.0),
                    (4, "x", 10, 10_000.0),
                    (5, "y", 12, 10_000.0),
                ] {
                    if let Some(v) = scaled(params, name, by) {
                        b[0] |= 1 << bit;
                        put16(&mut b, at, v as u16);
                    }
                }
                if let Some(v) = opt_bool(params, "invert") {
                    b[0] |= 1 << 6;
                    b[14] = v as u8;
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CKPt", b)
            }
            // Mask always 1, M/E, rate, rsv (Sofie `FadeToBlackRateCommand.ts:15-21`;
            // OpenSwitcher FtbC).
            "set_fade_to_black_rate" => {
                let me = self.check_me(params)?;
                one(b"FtbC", vec![1, me, int(params, "rate") as u8, 0])
            }
            // Mask, index, hue 0-3599, saturation 0-1000, luma 0-1000
            // (OpenSwitcher CClV; Sofie `ColorGeneratorCommand.ts:21-29`).
            "set_color_generator" => {
                let index = (int(params, "generator") - 1).clamp(0, 255) as u8;
                if !self.topology.color_generators.contains(&index) {
                    return Err(invalid(format!(
                        "colour generator {} does not exist",
                        index as u32 + 1
                    )));
                }
                let mut b = vec![0u8; 8];
                b[1] = index;
                for (bit, name, at) in [(0, "hue", 2), (1, "saturation", 4), (2, "luma", 6)] {
                    if let Some(v) = scaled(params, name, 10.0) {
                        b[0] |= 1 << bit;
                        put16(&mut b, at, v as u16);
                    }
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CClV", b)
            }
            // Mask (bit 0 type, 1 still, 2 clip), player, type (1 still, 2
            // clip), still, clip, 3 rsv (Sofie `Media/MediaPlayerSourceCommand.ts:5-30`;
            // OpenSwitcher MPSS).
            "set_media_player_source" => {
                let player = self.check_index(params, "player", self.topology.media_players)?;
                let (still, clip) = (opt_int(params, "still"), opt_int(params, "clip"));
                let b = match (still, clip) {
                    (Some(_), None) => {
                        let s = self.check_index(params, "still", self.topology.stills)?;
                        vec![0x03, player, 1, s, 0, 0, 0, 0]
                    }
                    (None, Some(_)) => {
                        if self.topology.clips == 0 {
                            return Err(unsupported(name, "this switcher (no clip pool)"));
                        }
                        let c = self.check_index(params, "clip", self.topology.clips)?;
                        vec![0x05, player, 2, 0, c, 0, 0, 0]
                    }
                    _ => return Err(invalid("give either still or clip".into())),
                };
                one(b"MPSS", b)
            }
            // Mask (bit 0 playing, 1 loop, 2 at beginning, 3 frame), player,
            // playing, loop, at beginning, rsv, u16 frame (Sofie
            // `Media/MediaPlayerStatusCommand.ts:5-31`).
            "media_player_play" => {
                let player = self.check_media_player(params, true, name)?;
                one(b"SCPS", vec![0x01, player, 1, 0, 0, 0, 0, 0])
            }
            "media_player_stop" => {
                let player = self.check_media_player(params, true, name)?;
                if opt_bool(params, "rewind") == Some(true) {
                    one(b"SCPS", vec![0x05, player, 0, 0, 1, 0, 0, 0])
                } else {
                    one(b"SCPS", vec![0x01, player, 0, 0, 0, 0, 0, 0])
                }
            }
            "set_media_player_loop" => {
                let player = self.check_media_player(params, true, name)?;
                one(
                    b"SCPS",
                    vec![0x02, player, 0, flag(params, "loop") as u8, 0, 0, 0, 0],
                )
            }
            // From 2.28: u16 mask, SuperSource, box, enabled, rsv, u16 source,
            // i16 x, i16 y, u16 size, cropped, rsv, u16 crops; before 2.28 no
            // SuperSource byte and everything one byte earlier from the box
            // (Sofie `SuperSource/SuperSourceBoxParametersCommand.ts:6-55`).
            // Ranges and scaling from OpenSwitcher CSBP and Companion
            // `actions/superSource.ts:429-437`.
            "set_supersource_box" => {
                let (ssrc, boxes) = self.check_supersource(name, params)?;
                let bx = self.check_index(params, "box", boxes)?;
                let mut b = vec![0u8; 24];
                let mut mask = 0u16;
                let i = if self.version >= V2_28 {
                    b[2] = ssrc;
                    b[3] = bx;
                    if let Some(v) = opt_bool(params, "enabled") {
                        mask |= 1;
                        b[4] = v as u8;
                    }
                    2
                } else {
                    b[2] = bx;
                    if let Some(v) = opt_bool(params, "enabled") {
                        mask |= 1;
                        b[3] = v as u8;
                    }
                    0
                };
                if let Some(s) = self.source_param(params, "source")? {
                    mask |= 1 << 1;
                    put16(&mut b, i + 4, s);
                }
                for (bit, name, at, by) in [(2, "x", 6, 100.0), (3, "y", 8, 100.0)] {
                    if let Some(v) = scaled(params, name, by) {
                        mask |= 1 << bit;
                        put_i16(&mut b, i + at, v as i16);
                    }
                }
                if let Some(v) = scaled(params, "size", 1000.0) {
                    mask |= 1 << 4;
                    put16(&mut b, i + 10, v as u16);
                }
                if let Some(v) = opt_bool(params, "cropped") {
                    mask |= 1 << 5;
                    b[i + 12] = v as u8;
                }
                for (bit, name, at) in [
                    (6, "crop_top", 14),
                    (7, "crop_bottom", 16),
                    (8, "crop_left", 18),
                    (9, "crop_right", 20),
                ] {
                    if let Some(v) = scaled(params, name, 1000.0) {
                        mask |= 1 << bit;
                        put16(&mut b, i + at, v as u16);
                    }
                }
                if mask == 0 {
                    return Err(nothing_to_set());
                }
                put16(&mut b, 0, mask);
                one(b"CSBP", b)
            }
            // From 2.28: mask, SuperSource, u16 fill, u16 key, placement,
            // pre-multiplied, u16 clip, u16 gain, invert (16 bytes); before,
            // a u32 mask and the art from 4, with the border after it (36
            // bytes) (Sofie `SuperSource/SuperSourcePropertiesCommand.ts:6-103`;
            // OpenSwitcher CSSc; percent x 10 from Companion
            // `actions/superSource.ts:125-126`).
            "set_supersource_art" => {
                let (ssrc, _) = self.check_supersource(name, params)?;
                let (mut b, at) = if self.version >= V2_28 {
                    let mut b = vec![0u8; 16];
                    b[1] = ssrc;
                    (b, 2)
                } else {
                    (vec![0u8; 36], 4)
                };
                let mut mask = 0u32;
                if let Some(s) = self.source_param(params, "fill")? {
                    mask |= 1;
                    put16(&mut b, at, s);
                }
                if let Some(s) = self.source_param(params, "key")? {
                    mask |= 1 << 1;
                    put16(&mut b, at + 2, s);
                }
                if let Some(p) = enum_param(params, "placement", &["background", "foreground"])? {
                    mask |= 1 << 2;
                    b[at + 4] = p;
                }
                if let Some(v) = opt_bool(params, "pre_multiplied") {
                    mask |= 1 << 3;
                    b[at + 5] = v as u8;
                }
                for (bit, name, off) in [(4, "clip", 6), (5, "gain", 8)] {
                    if let Some(v) = scaled(params, name, 10.0) {
                        mask |= 1 << bit;
                        put16(&mut b, at + off, v as u16);
                    }
                }
                if let Some(v) = opt_bool(params, "invert") {
                    mask |= 1 << 6;
                    b[at + 10] = v as u8;
                }
                if mask == 0 {
                    return Err(nothing_to_set());
                }
                if self.version >= V2_28 {
                    b[0] = mask as u8;
                } else {
                    b[..4].copy_from_slice(&mask.to_be_bytes());
                }
                one(b"CSSc", b)
            }
            // Mask (bit 0 mix, 1 gain, 2 balance), rsv, u16 input, mix option,
            // rsv, u16 gain, i16 balance, rca-to-xlr, rsv (Sofie
            // `Audio/AudioMixerInputCommand.ts:8-34`; gain and balance
            // conversions `lib/atemUtil.ts:15-30`).
            "set_audio_input" => {
                if !self.topology.classic_audio {
                    return Err(unsupported(name, "this switcher (no classic audio mixer)"));
                }
                let input = int(params, "input");
                if !self.topology.classic_inputs.contains(&(input as u16)) {
                    return Err(invalid(format!("the audio mixer has no input {input}")));
                }
                let mut b = vec![0u8; 12];
                put16(&mut b, 2, input as u16);
                if let Some(m) = enum_param(params, "mix", &["off", "on", "afv"])? {
                    b[0] |= 1;
                    b[4] = m;
                }
                if let Some(db) = opt_num(params, "gain") {
                    b[0] |= 2;
                    put16(&mut b, 6, classic_raw(db));
                }
                if let Some(v) = scaled(params, "balance", 200.0) {
                    b[0] |= 4;
                    put_i16(&mut b, 8, v as i16);
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CAMI", b)
            }
            // Mask (bit 0 gain, 1 balance), rsv, u16 gain, i16 balance,
            // follow-FTB, rsv (Sofie `Audio/AudioMixerMasterCommand.ts:6-21`).
            "set_audio_master" => {
                if !self.topology.classic_audio {
                    return Err(unsupported(name, "this switcher (no classic audio mixer)"));
                }
                let mut b = vec![0u8; 8];
                if let Some(db) = opt_num(params, "gain") {
                    b[0] |= 1;
                    put16(&mut b, 2, classic_raw(db));
                }
                if let Some(v) = scaled(params, "balance", 200.0) {
                    b[0] |= 2;
                    put_i16(&mut b, 4, v as i16);
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CAMM", b)
            }
            // u16 mask, u16 input, 4 rsv, i64 source, frames delay, 3 rsv, i32
            // gain, i16 stereo simulation, eq enabled, rsv, i32 eq gain, i32
            // make-up gain, i16 balance, 2 rsv, i32 fader gain, mix option, 3
            // rsv; mask bits 1 gain, 6 balance, 7 fader gain, 8 mix option
            // (Sofie `Fairlight/FairlightMixerSourceCommand.ts:47-89`). Gains
            // in hundredths of a dB, balance in hundredths (Companion
            // `actions/fairlightAudio.ts:298, 472, 562`).
            "set_fairlight_source" => {
                if !self.topology.fairlight {
                    return Err(unsupported(name, "this switcher (no Fairlight mixer)"));
                }
                let input = int(params, "input");
                let source = int(params, "source");
                let Some(&options) = self.topology.fairlight_sources.get(&(input as u16, source))
                else {
                    return Err(invalid(format!(
                        "the Fairlight mixer has no source {source} on input {input}"
                    )));
                };
                let mut b = vec![0u8; 48];
                let mut mask = 0u16;
                put16(&mut b, 2, input as u16);
                b[8..16].copy_from_slice(&source.to_be_bytes());
                if let Some(v) = scaled(params, "gain", 100.0) {
                    mask |= 1 << 1;
                    put_i32(&mut b, 20, v as i32);
                }
                if let Some(v) = scaled(params, "balance", 100.0) {
                    mask |= 1 << 6;
                    put_i16(&mut b, 36, v as i16);
                }
                if let Some(v) = scaled(params, "fader_gain", 100.0) {
                    mask |= 1 << 7;
                    put_i32(&mut b, 40, v as i32);
                }
                if let Some(m) = enum_param(params, "mix", &["off", "on", "afv"])? {
                    let bit = 1u8 << m;
                    if options & bit == 0 {
                        return Err(invalid(format!(
                            "source {source} on input {input} does not offer mix {}",
                            fairlight_mix(bit)
                        )));
                    }
                    mask |= 1 << 8;
                    b[44] = bit;
                }
                if mask == 0 {
                    return Err(nothing_to_set());
                }
                put16(&mut b, 0, mask);
                one(b"CFSP", b)
            }
            // Mask (bit 3 fader gain, 4 follow FTB), eq enabled, 2 rsv, i32 eq
            // gain, i32 make-up gain, i32 fader gain, follow FTB, 3 rsv (Sofie
            // `Fairlight/FairlightMixerMasterCommand.ts:12-33`).
            "set_fairlight_master" => {
                if !self.topology.fairlight {
                    return Err(unsupported(name, "this switcher (no Fairlight mixer)"));
                }
                let mut b = vec![0u8; 20];
                if let Some(v) = scaled(params, "fader_gain", 100.0) {
                    b[0] |= 1 << 3;
                    put_i32(&mut b, 12, v as i32);
                }
                if let Some(v) = opt_bool(params, "follow_fade_to_black") {
                    b[0] |= 1 << 4;
                    b[16] = v as u8;
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CFMP", b)
            }
            // Send levels on or off, 3 rsv (Sofie
            // `Fairlight/FairlightMixerSendLevelsCommand.ts:10-14`).
            "set_audio_levels" => {
                if !self.topology.fairlight {
                    return Err(unsupported(name, "this switcher (no Fairlight mixer)"));
                }
                one(b"SFLN", vec![flag(params, "enabled") as u8, 0, 0, 0])
            }
            // Multiviewer, window, u16 source (Sofie
            // `Settings/MultiViewerSourceCommand.ts:19-25`).
            "set_multiviewer_window" => {
                let mv = (int(params, "multiviewer") - 1).clamp(0, 255) as u8;
                let window = (int(params, "window") - 1).clamp(0, 255) as u8;
                if !self
                    .topology
                    .multiviewer_windows
                    .contains_key(&(mv, window))
                {
                    return Err(invalid(format!(
                        "multiviewer {} has no window {}",
                        mv as u32 + 1,
                        window as u32 + 1
                    )));
                }
                let [hi, lo] = self.check_source(params)?.to_be_bytes();
                one(b"CMvI", vec![mv, window, hi, lo])
            }
            // Mask (bit 0 long, 1 short), rsv, u16 source, long[20], short[4],
            // u16 port, 2 rsv (Sofie `Inputs/InputPropertiesCommand.ts:7-32`).
            "set_input_name" => {
                let source = self.check_source(params)?;
                let mut b = vec![0u8; 32];
                put16(&mut b, 2, source);
                if let Some(long) = fits(params, "long_name", 20)? {
                    b[0] |= 1;
                    b[4..4 + long.len()].copy_from_slice(long.as_bytes());
                }
                if let Some(short) = fits(params, "short_name", 4)? {
                    b[0] |= 2;
                    b[24..24 + short.len()].copy_from_slice(short.as_bytes());
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CInL", b)
            }
            "run_macro" => {
                let slot = self.check_index(params, "macro", self.topology.macros)?;
                if self.topology.macros_used.get(&(slot as u16)) != Some(&true) {
                    return Err(invalid(format!("macro {} is empty", slot as u16 + 1)));
                }
                let [hi, lo] = (slot as u16).to_be_bytes();
                one(b"MAct", vec![hi, lo, 0, 0])
            }
            "stop_macro" => one(b"MAct", vec![0xFF, 0xFF, 1, 0]),
            // Continue is action 4 with index 0xFFFF (Sofie
            // `Macro/MacroActionCommand.ts:15-32`, `enums/index.ts:106-113`).
            "continue_macro" => one(b"MAct", vec![0xFF, 0xFF, 4, 0]),
            // Mask, loop, 2 rsv (Sofie `Macro/MacroRunStatusCommand.ts:25-37`).
            "set_macro_loop" => one(b"MRCP", vec![1, flag(params, "loop") as u8, 0, 0]),
            "start_streaming" | "stop_streaming" => {
                if self.version < V2_30 || !self.topology.encoder {
                    return Err(unsupported(
                        name,
                        "this switcher (no encoder, or protocol before 2.30)",
                    ));
                }
                one(b"StrR", vec![(name == "start_streaming") as u8, 0, 0, 0])
            }
            "start_recording" | "stop_recording" => {
                if self.version < V2_30 || !self.topology.encoder {
                    return Err(unsupported(
                        name,
                        "this switcher (no encoder, or protocol before 2.30)",
                    ));
                }
                one(b"RcTM", vec![(name == "start_recording") as u8, 0, 0, 0])
            }
            // Mask (bit 0 name, 1 url, 2 key), name[64] at 1, url[512] at 65,
            // key[512] at 577, u32 bitrates at 1092 and 1096; 1100 bytes
            // (Sofie `Streaming/StreamingServiceCommand.ts:7-29`). Strings
            // keep a terminating NUL.
            "set_streaming_service" => {
                if self.version < V2_30 || !self.topology.encoder {
                    return Err(unsupported(
                        name,
                        "this switcher (no encoder, or protocol before 2.30)",
                    ));
                }
                let mut b = vec![0u8; 1100];
                for (bit, field, at, max) in [
                    (0, "service_name", 1, 63),
                    (1, "url", 65, 511),
                    (2, "key", 577, 511),
                ] {
                    if let Some(s) = fits(params, field, max)? {
                        b[0] |= 1 << bit;
                        b[at..at + s.len()].copy_from_slice(s.as_bytes());
                    }
                }
                if b[0] == 0 {
                    return Err(nothing_to_set());
                }
                one(b"CRSS", b)
            }
            // Camera control (CCmd): category 0 lens, 1 video; lens 0 focus,
            // 3 aperture normalised, 9 continuous zoom speed; video 2 manual
            // white balance, 5 exposure in us, 13 gain in dB (camera-control
            // `ids.d.ts` and the data types in `commandSender/baseGenerator.ts`
            // lensFocus, lensIrisNormalised, lensSetContinuousZoomSpeed,
            // videoManualWhiteBalance, videoExposureUs, videoGain).
            "camera_iris" => {
                let cam = self.check_camera(name, params)?;
                let v = scaled(params, "iris", 2048.0).unwrap_or(0);
                one(b"CCmd", camera_body(cam, 0, 3, false, CC_FLOAT, &[v]))
            }
            "camera_focus" => {
                let cam = self.check_camera(name, params)?;
                let relative = opt_bool(params, "relative") == Some(true);
                let focus = opt_num(params, "focus").unwrap_or(0.0);
                if !relative && !(0.0..=1.0).contains(&focus) {
                    return Err(invalid("an absolute focus is 0 to 1".into()));
                }
                let v = (focus * 2048.0).round() as i64;
                one(b"CCmd", camera_body(cam, 0, 0, relative, CC_FLOAT, &[v]))
            }
            "camera_zoom" => {
                let cam = self.check_camera(name, params)?;
                let v = scaled(params, "speed", 2048.0).unwrap_or(0);
                one(b"CCmd", camera_body(cam, 0, 9, false, CC_FLOAT, &[v]))
            }
            "camera_gain" => {
                let cam = self.check_camera(name, params)?;
                one(
                    b"CCmd",
                    camera_body(cam, 1, 13, false, CC_SINT8, &[int(params, "gain")]),
                )
            }
            "camera_white_balance" => {
                let cam = self.check_camera(name, params)?;
                let values = [int(params, "kelvin"), int(params, "tint")];
                one(b"CCmd", camera_body(cam, 1, 2, false, CC_SINT16, &values))
            }
            "camera_shutter" => {
                let cam = self.check_camera(name, params)?;
                let values = [int(params, "exposure_us")];
                one(b"CCmd", camera_body(cam, 1, 5, false, CC_SINT32, &values))
            }
            other => {
                if let Some(out) = self.build_keyers(other, params)? {
                    return Ok(out);
                }
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                })
            }
        }
    }
}

impl Module for Atem {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.phase != Phase::Ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.build(name, params) {
            Ok(commands) => {
                if name == "set_audio_levels" {
                    self.levels_wanted = flag(params, "enabled");
                }
                self.send_commands(cx, Some(id), &commands)
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        if from.ip() != self.device.ip() {
            return;
        }
        self.receive(cx, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        self.lost(cx, &format!("socket error: {message}"));
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            HELLO if self.phase == Phase::Hello => self.hello(cx),
            SILENCE => self.lost(cx, "no data from the switcher for 5 s"),
            RESEND => {
                let now = cx.now();
                let mut give_up = false;
                for sent in self.in_flight.iter_mut() {
                    if now.saturating_sub(sent.sent_at) >= RESEND_AFTER {
                        if sent.attempts >= RESEND_ATTEMPTS {
                            give_up = true;
                            break;
                        }
                        let mut packet = sent.packet.clone();
                        // Mark it a retransmission.
                        packet[0] |= 0x04 << 3;
                        cx.udp_send(SOCKET, self.device, packet);
                        sent.sent_at = now;
                        sent.attempts += 1;
                        sent.resent = true;
                    }
                }
                if give_up {
                    self.lost(cx, "the switcher stopped acknowledging commands");
                } else if !self.in_flight.is_empty() {
                    cx.set_timer(RESEND, RESEND_AFTER);
                }
            }
            // Empty-bodied requests the switcher answers with SRST and RTMR
            // (Sofie `Streaming/StreamingDurationCommand.ts:6-16`,
            // `Recording/RecordingDurationCommand.ts:6-16`).
            DURATION => {
                self.duration_armed = false;
                if self.phase != Phase::Ready {
                    return;
                }
                let mut requests: Out = Vec::new();
                if self.streaming_active {
                    requests.push((*b"SRDR", Vec::new()));
                }
                if self.recording_active {
                    requests.push((*b"RMDR", Vec::new()));
                }
                if !requests.is_empty() {
                    self.send_commands(cx, None, &requests);
                }
                self.arm_duration(cx);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.udp_close(SOCKET);
    }
}

/// The source address a module compares against, for tests.
#[cfg(test)]
impl Atem {
    fn host(&self) -> std::net::IpAddr {
        self.device.ip()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));

    pub(super) fn atem() -> Atem {
        Atem::for_device(SocketAddr::new(HOST, PORT))
    }

    pub(super) fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    pub(super) fn feed(m: &mut Atem, now: Millis, packet: Vec<u8>) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.datagram(&mut cx, SOCKET, SocketAddr::new(m.host(), PORT), &packet);
        cx.take()
    }

    pub(super) fn state(actions: &[Action]) -> Value {
        let mut merged = json!({});
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(&mut merged, p);
            }
        }
        merged
    }

    /// A switcher packet: reliable, with the given id and commands.
    pub(super) fn reliable(session: u16, id: u16, commands: &[Vec<u8>]) -> Vec<u8> {
        let payload: Vec<u8> = commands.concat();
        let mut p = header(FLAG_RELIABLE, HEADER + payload.len(), session, 0, id);
        p.extend_from_slice(&payload);
        p
    }

    /// A switcher command with non-zero bytes where the header's unused bytes
    /// are, as real switchers send.
    pub(super) fn cmd(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut c = command(name, body);
        c[2] = 0xff;
        c[3] = 0xff;
        c
    }

    pub(super) fn syn_reply() -> Vec<u8> {
        let mut p = header(FLAG_SYN, HEADER + 8, 0x53ab, 0, 0);
        p.extend_from_slice(&[0x02, 0, 0, 0, 0, 0, 0, 0]);
        p
    }

    pub(super) fn input(id: u16, long: &str, short: &str, port: u8) -> Vec<u8> {
        let mut b = vec![0u8; 36];
        b[0..2].copy_from_slice(&id.to_be_bytes());
        b[2..2 + long.len()].copy_from_slice(long.as_bytes());
        b[22..22 + short.len()].copy_from_slice(short.as_bytes());
        b[32] = port;
        cmd(b"InPr", &b)
    }

    /// The initial dump of a small 1 M/E switcher on protocol 2.30, with a
    /// Fairlight mixer, two media players, a SuperSource, a DVE, a stinger
    /// and camera control.
    pub(super) fn dump() -> Vec<Vec<u8>> {
        let mut pin = vec![0u8; 44];
        pin[..13].copy_from_slice(b"ATEM Mini Pro");
        pin[40] = 14;
        let mut top = vec![0u8; 24];
        top[0] = 1; // M/Es
        top[1] = 3; // sources
        top[2] = 1; // DSKs
        top[3] = 1; // auxes
        top[5] = 2; // media players
        top[6] = 1; // multiviewers (from 2.30)
        top[9] = 1; // DVEs (offset 9 from 2.30)
        top[10] = 1; // stingers (offset 10 from 2.30)
        top[11] = 1; // SuperSources (offset 11 from 2.30)
        top[18] = 1; // camera control (offset 18 from 2.30)
        let mut mprp = vec![0, 0, 1, 0, 0, 4, 0, 0];
        mprp.extend_from_slice(b"Open");
        let mut kebp = vec![0u8; 20];
        kebp[4] = 1; // can fly
        put16(&mut kebp, 6, 1);
        put16(&mut kebp, 8, 2);
        vec![
            cmd(b"_ver", &[0, 2, 0, 30]),
            cmd(b"_pin", &pin),
            cmd(b"_top", &top),
            cmd(b"_MeC", &[0, 1, 0x72, 0x70]),
            cmd(b"_MAC", &[100, 0, 0, 0]),
            cmd(b"_mpl", &[20, 2, 0, 0]),
            cmd(b"_SSC", &[0, 0, 4, 0]),
            cmd(b"_FAC", &[1, 0, 0, 0]),
            input(0, "Black", "BLK", 1),
            input(1, "Camera 1", "CAM1", 0),
            input(2, "Camera 2", "CAM2", 0),
            cmd(b"PrgI", &[0, 0x18, 0, 1]),
            cmd(b"PrvI", &[0, 0, 0, 2, 0, 0, 0, 0]),
            cmd(b"TrSS", &[0, 0, 1, 0, 1, 0, 0, 0]),
            cmd(b"TlSr", &[0, 3, 0, 0, 0, 0, 1, 1, 0, 2, 2, 0]),
            cmd(b"MPrp", &mprp),
            cmd(b"StRS", &[0, 1, 0, 0]),
            cmd(b"ColV", &[0, 0, 0, 0, 0, 0, 0, 0]),
            cmd(b"ColV", &[1, 0, 0, 0, 0, 0, 0, 0]),
            cmd(b"KeBP", &kebp),
            cmd(b"MvIn", &[0, 2, 0, 1, 1, 1, 0, 0]),
            fasp(1, -65280, 0b011, 2),
            cmd(b"InCm", &[1, 0x50, 0x72, 0x70]),
        ]
    }

    /// A Fairlight source: input, source id, mix options offered, mix option.
    /// Bytes written as "00-1F-...", as the LibAtem samples in Sofie's
    /// `commands/__tests__/libatem-data.json` give them.
    pub(super) fn hex(s: &str) -> Vec<u8> {
        s.split('-')
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    /// The state one switcher packet of commands produces.
    pub(super) fn decoded(m: &mut Atem, commands: &[Vec<u8>]) -> Value {
        let id = (m.last_received + 1) % ID_MODULO;
        state(&feed(m, 30, reliable(0x8001, id, commands)))
    }

    pub(super) fn fasp(input: u16, source: i64, options: u8, mix: u8) -> Vec<u8> {
        let mut b = vec![0u8; 52];
        put16(&mut b, 0, input);
        b[8..16].copy_from_slice(&source.to_be_bytes());
        b[48] = options;
        b[49] = mix;
        cmd(b"FASP", &b)
    }

    /// Hello, the switcher's answer, and the dump in one packet.
    pub(super) fn ready() -> (Atem, Vec<Action>) {
        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        feed(&mut m, 10, syn_reply());
        let a = feed(&mut m, 20, reliable(0x8001, 1, &dump()));
        (m, a)
    }

    /// Every command the spec declares reaches a builder: given a value for
    /// each declared parameter, none is unknown to the module.
    #[test]
    fn every_spec_command_is_known_to_the_module() {
        let catalog = crate::catalog::Catalog::embedded();
        let spec = catalog.device("blackmagic-atem").unwrap();
        let (m, _) = ready();
        for (name, command) in &spec.commands {
            let mut params = Params::new();
            for (p, decl) in &command.params {
                let v = match decl.kind {
                    crate::catalog::ParamType::Int => {
                        json!(decl.min.unwrap_or(1.0).max(1.0) as i64)
                    }
                    crate::catalog::ParamType::Float => json!(decl.min.unwrap_or(0.0)),
                    crate::catalog::ParamType::Bool => json!(true),
                    crate::catalog::ParamType::Enum => {
                        json!(decl.values.as_ref().unwrap()[0])
                    }
                    _ => json!("x"),
                };
                params.insert(p.clone(), v);
            }
            match m.build(name, &params) {
                Err(CommandError::UnknownCommand { .. }) => {
                    panic!("{name} is in the spec but unknown to the module")
                }
                // With every parameter given, a setting command that finds
                // nothing to set reads names the spec does not declare.
                Err(CommandError::InvalidParams { message })
                    if message.starts_with("give at least") =>
                {
                    panic!("{name}: {message}")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn opens_with_the_documented_hello() {
        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Ephemeral
        }));
        assert_eq!(
            sent(&a),
            [vec![
                0x10, 0x14, 0x53, 0xab, 0, 0, 0, 0, 0, 0x3a, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0
            ]]
        );
        // Unanswered, the hello repeats.
        let mut cx = Cx::new(1_000);
        m.timer(&mut cx, HELLO);
        assert_eq!(sent(&cx.take()).len(), 1);
    }

    #[test]
    fn acknowledges_the_handshake_and_adopts_the_new_session() {
        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let a = feed(&mut m, 10, syn_reply());
        assert_eq!(
            sent(&a),
            [vec![0x80, 0x0c, 0x53, 0xab, 0, 0, 0, 0, 0, 0, 0, 0]]
        );

        // The first reliable packet carries the switcher's new session id,
        // which the acknowledgement echoes.
        let a = feed(
            &mut m,
            20,
            reliable(0x8001, 1, &[cmd(b"_ver", &[0, 2, 0, 30])]),
        );
        assert_eq!(
            sent(&a),
            [vec![0x80, 0x0c, 0x80, 0x01, 0, 1, 0, 0, 0, 0, 0, 0]]
        );
    }

    #[test]
    fn the_dump_builds_the_state_and_ends_in_connected() {
        let (_, a) = ready();
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let s = state(&a);
        assert_eq!(s["device"]["product"], "ATEM Mini Pro");
        assert_eq!(s["device"]["model_id"], 14);
        assert_eq!(s["device"]["protocol_version"], "2.30");
        assert_eq!(s["topology"]["mes"], 1);
        assert_eq!(s["topology"]["auxes"], 1);
        assert_eq!(
            s["sources"]["1"],
            json!({"long_name": "Camera 1", "short_name": "CAM1", "kind": "external"})
        );
        assert_eq!(s["mes"]["1"]["program"], 1);
        assert_eq!(s["mes"]["1"]["preview"], 2);
        assert_eq!(s["mes"]["1"]["keyers"], 1);
        assert_eq!(s["mes"]["1"]["transition"]["style"], "mix");
        assert_eq!(s["tally"]["1"], json!({"program": true, "preview": false}));
        assert_eq!(s["tally"]["2"], json!({"program": false, "preview": true}));
        assert_eq!(s["macros"]["1"]["name"], "Open");
        assert_eq!(s["streaming"]["state"], "idle");
    }

    #[test]
    fn a_gap_is_left_for_the_switcher_to_resend() {
        let (mut m, _) = ready();
        // Packet 3 before 2: dropped, and nothing acknowledged past 1.
        let a = feed(
            &mut m,
            30,
            reliable(0x8001, 3, &[cmd(b"PrgI", &[0, 0, 0, 2])]),
        );
        assert!(sent(&a).is_empty());
        assert_eq!(state(&a), json!({}));
        // Packet 1 again: already applied, acknowledged again.
        let a = feed(&mut m, 31, reliable(0x8001, 1, &[]));
        assert_eq!(sent(&a), [header(FLAG_ACK, HEADER, 0x8001, 1, 0)]);
        // Packet 2 in order is applied.
        let a = feed(
            &mut m,
            32,
            reliable(0x8001, 2, &[cmd(b"PrgI", &[0, 0, 0, 2])]),
        );
        assert_eq!(state(&a)["mes"]["1"]["program"], 2);
    }

    #[test]
    fn packet_ids_wrap_at_15_bits() {
        let (mut m, _) = ready();
        m.last_received = ID_MODULO - 1;
        let a = feed(
            &mut m,
            30,
            reliable(0x8001, 0, &[cmd(b"PrgI", &[0, 0, 0, 2])]),
        );
        assert_eq!(state(&a)["mes"]["1"]["program"], 2);
        assert_eq!(m.last_received, 0);
    }

    #[test]
    fn commands_are_reliable_packets_completed_by_the_switchers_ack() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(100);
        let params = json!({"me": 1, "source": 2}).as_object().unwrap().clone();
        m.command(&mut cx, 7, "set_program", &params);
        let a = cx.take();
        let packet = &sent(&a)[0];
        // Reliable flag, length 12 + 12, session, id 1, then CPgI.
        assert_eq!(&packet[..12], &header(FLAG_RELIABLE, 24, 0x8001, 0, 1)[..]);
        assert_eq!(
            &packet[12..],
            &[0, 12, 0, 0, b'C', b'P', b'g', b'I', 0, 0, 0, 2]
        );

        let a = feed(&mut m, 110, header(FLAG_ACK, HEADER, 0x8001, 1, 0));
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Ack)
        }));
    }

    #[test]
    fn a_packet_that_acknowledges_and_reports_applies_its_state_first() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(100);
        let params = json!({"me": 1, "source": 2}).as_object().unwrap().clone();
        m.command(&mut cx, 7, "set_program", &params);
        cx.take();
        // One packet: the ack of our command and the new program.
        let mut packet = reliable(0x8001, 2, &[cmd(b"PrgI", &[0, 0, 0, 2])]);
        packet[0] |= FLAG_ACK << 3;
        packet[4..6].copy_from_slice(&1u16.to_be_bytes());
        let a = feed(&mut m, 110, packet);
        let at = |f: fn(&Action) -> bool| a.iter().position(f).unwrap();
        assert!(
            at(|x| matches!(x, Action::State(_))) < at(|x| matches!(x, Action::Complete { .. }))
        );
        assert_eq!(state(&a)["mes"]["1"]["program"], 2);
    }

    #[test]
    fn unacknowledged_commands_are_resent_then_the_connection_is_dropped() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            1,
            "cut",
            &json!({"me": 1}).as_object().unwrap().clone(),
        );
        let first = sent(&cx.take())[0].clone();
        let mut now = 100;
        for _ in 1..RESEND_ATTEMPTS {
            now += RESEND_AFTER;
            let mut cx = Cx::new(now);
            m.timer(&mut cx, RESEND);
            let resent = sent(&cx.take());
            assert_eq!(resent.len(), 1);
            assert_eq!(resent[0][0], first[0] | 0x20, "marked as a retransmission");
            assert_eq!(&resent[0][1..], &first[1..]);
        }
        now += RESEND_AFTER;
        let mut cx = Cx::new(now);
        m.timer(&mut cx, RESEND);
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                id: 1,
                result: Err(CommandError::Transport { .. })
            }
        )));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
    }

    #[test]
    fn a_retransmit_request_resends_from_the_asked_packet() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(100);
        let me = json!({"me": 1}).as_object().unwrap().clone();
        m.command(&mut cx, 1, "cut", &me);
        m.command(&mut cx, 2, "auto", &me);
        cx.take();
        let mut request = header(FLAG_RETRANSMIT_REQUEST, HEADER, 0x8001, 0, 0);
        request[6..8].copy_from_slice(&2u16.to_be_bytes());
        let a = feed(&mut m, 120, request);
        let resent = sent(&a);
        assert_eq!(resent.len(), 1);
        assert_eq!(&resent[0][16..20], b"DAut");
    }

    #[test]
    fn commands_are_checked_against_the_switchers_topology() {
        let (mut m, _) = ready();
        let check = |m: &mut Atem, name: &str, params: Value| {
            let mut cx = Cx::new(100);
            m.command(&mut cx, 1, name, params.as_object().unwrap());
            let a = cx.take();
            assert!(sent(&a).is_empty(), "{name} sent nothing");
            match &a[..] {
                [Action::Complete { result: Err(e), .. }] => e.clone(),
                other => panic!("{other:?}"),
            }
        };
        assert!(matches!(
            check(&mut m, "cut", json!({"me": 2})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            check(&mut m, "set_program", json!({"me": 1, "source": 3010})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            check(&mut m, "set_aux", json!({"aux": 2, "source": 1})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            check(&mut m, "run_macro", json!({"macro": 2})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            check(&mut m, "usk_on_air", json!({"me": 1, "keyer": 2})),
            CommandError::InvalidParams { .. }
        ));
    }

    #[test]
    fn version_dependent_layouts() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            1,
            "dsk_auto",
            &json!({"dsk": 1}).as_object().unwrap().clone(),
        );
        // 2.29 and later: mask, DSK index, direction, reserved.
        assert_eq!(
            &sent(&cx.take())[0][12..],
            &[0, 12, 0, 0, b'D', b'D', b's', b'A', 0, 0, 0, 0]
        );

        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        feed(&mut m, 10, syn_reply());
        let mut dump = dump();
        dump[0] = cmd(b"_ver", &[0, 2, 0, 28]);
        // Before 2.30 _top has no multiviewer byte, and there is no encoder.
        dump.retain(|c| &c[4..8] != b"StRS");
        let a = feed(&mut m, 20, reliable(0x8001, 1, &dump));
        assert_eq!(state(&a)["topology"]["mes"], 1);
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            2,
            "dsk_auto",
            &json!({"dsk": 1}).as_object().unwrap().clone(),
        );
        assert_eq!(&sent(&cx.take())[0][20..], &[0, 0, 0, 0]);
        let mut cx = Cx::new(101);
        m.command(&mut cx, 3, "start_streaming", &Params::new());
        assert!(matches!(
            &cx.take()[..],
            [Action::Complete {
                result: Err(CommandError::UnsupportedForModel { .. }),
                ..
            }]
        ));
    }

    #[test]
    fn command_layouts() {
        let (mut m, _) = ready();
        let body = |m: &mut Atem, name: &str, params: Value| {
            let mut cx = Cx::new(100);
            m.command(&mut cx, 1, name, params.as_object().unwrap());
            sent(&cx.take())[0][12..].to_vec()
        };
        let b = body(
            &mut m,
            "set_transition_position",
            json!({"me": 1, "position": 0.5}),
        );
        assert_eq!(&b[4..], b"CTPs\0\0\x13\x88");
        let b = body(
            &mut m,
            "set_transition_style",
            json!({"me": 1, "style": "wipe"}),
        );
        assert_eq!(&b[4..], b"CTTp\x01\0\x02\0");
        let b = body(&mut m, "set_aux", json!({"aux": 1, "source": 2}));
        assert_eq!(&b[4..], b"CAuS\x01\0\0\x02");
        let b = body(&mut m, "run_macro", json!({"macro": 1}));
        assert_eq!(&b[4..], b"MAct\0\0\0\0");
        let b = body(&mut m, "stop_macro", json!({}));
        assert_eq!(&b[4..], b"MAct\xff\xff\x01\0");
        let b = body(&mut m, "start_streaming", json!({}));
        assert_eq!(&b[4..], b"StrR\x01\0\0\0");
        let b = body(
            &mut m,
            "usk_on_air",
            json!({"me": 1, "keyer": 1, "on_air": true}),
        );
        assert_eq!(&b[4..], b"CKOn\0\0\x01\0");
    }

    #[test]
    fn silence_reconnects_on_a_fresh_socket() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(10_000);
        m.timer(&mut cx, SILENCE);
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Ephemeral
        }));
        assert_eq!(sent(&a), [HELLO_PACKET.to_vec()]);
        // Commands wait for the new session's dump.
        let mut cx = Cx::new(10_001);
        m.command(
            &mut cx,
            5,
            "cut",
            &json!({"me": 1}).as_object().unwrap().clone(),
        );
        assert!(matches!(
            &cx.take()[..],
            [Action::Complete {
                id: 5,
                result: Err(CommandError::NotConnected)
            }]
        ));
    }

    #[test]
    fn datagrams_from_other_hosts_are_ignored() {
        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(10);
        m.datagram(
            &mut cx,
            SOCKET,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 99)), PORT),
            &syn_reply(),
        );
        assert!(cx.take().is_empty());
    }

    /// The one packet a command sends, after the packet header.
    pub(super) fn payload(m: &mut Atem, name: &str, params: Value) -> Vec<u8> {
        let mut cx = Cx::new(100);
        m.command(&mut cx, 1, name, params.as_object().unwrap());
        let a = cx.take();
        match sent(&a).first() {
            Some(p) => p[12..].to_vec(),
            None => panic!("{name} sent nothing: {a:?}"),
        }
    }

    pub(super) fn refused(m: &mut Atem, name: &str, params: Value) -> CommandError {
        let mut cx = Cx::new(100);
        m.command(&mut cx, 1, name, params.as_object().unwrap());
        let a = cx.take();
        assert!(sent(&a).is_empty(), "{name} sent nothing");
        match &a[..] {
            [Action::Complete { result: Err(e), .. }] => e.clone(),
            other => panic!("{other:?}"),
        }
    }

    pub(super) fn ready_with(dump: Vec<Vec<u8>>) -> Atem {
        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        feed(&mut m, 10, syn_reply());
        feed(&mut m, 20, reliable(0x8001, 1, &dump));
        assert_eq!(m.phase, Phase::Ready);
        m
    }

    #[test]
    fn transition_settings_layouts() {
        let (mut m, _) = ready();
        let p = payload(&mut m, "set_mix_rate", json!({"me": 1, "rate": 30}));
        assert_eq!(p, command(b"CTMx", &[0, 30, 0, 0]));
        let p = payload(&mut m, "set_dip", json!({"me": 1, "rate": 25, "source": 2}));
        assert_eq!(p, command(b"CTDp", &[3, 0, 25, 0, 0, 2, 0, 0]));
        let p = payload(
            &mut m,
            "set_wipe",
            json!({"me": 1, "rate": 50, "pattern": "circle_iris", "border_width": 12.5,
                   "border_source": 1, "symmetry": 50.0, "softness": 10.0, "x": 0.5,
                   "y": 0.25, "reverse": true, "flip_flop": false}),
        );
        assert_eq!(
            p,
            command(
                b"CTWp",
                &[
                    0x03, 0xFF, 0, 50, 7, 0, 0x04, 0xE2, 0, 1, 0x13, 0x88, 0x03, 0xE8, 0x13, 0x88,
                    0x09, 0xC4, 1, 0
                ]
            )
        );
        let p = payload(
            &mut m,
            "set_dve_transition",
            json!({"me": 1, "style": "push_left", "fill_source": 1, "key_source": 2, "clip": 50.0}),
        );
        assert_eq!(
            p,
            command(
                b"CTDv",
                &[0, 0x9C, 0, 0, 0, 27, 0, 1, 0, 2, 0, 0, 0x01, 0xF4, 0, 0, 0, 0, 0, 0]
            )
        );
        let p = payload(
            &mut m,
            "set_stinger",
            json!({"me": 1, "media_player": 2, "preroll": 10, "trigger_point": 20, "mix_rate": 5}),
        );
        assert_eq!(
            p,
            command(
                b"CTSt",
                &[0x01, 0xA1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0, 20, 0, 5]
            )
        );
        let p = payload(
            &mut m,
            "set_preview_transition",
            json!({"me": 1, "enabled": true}),
        );
        assert_eq!(p, command(b"CTPr", &[0, 1, 0, 0]));
        // The dump's next selection is the background; key 1 is added.
        let p = payload(
            &mut m,
            "set_next_transition",
            json!({"me": 1, "key1": true}),
        );
        assert_eq!(p, command(b"CTTp", &[2, 0, 0, 3]));
        let p = payload(
            &mut m,
            "set_next_transition",
            json!({"me": 1, "background": false, "key1": true}),
        );
        assert_eq!(p, command(b"CTTp", &[2, 0, 0, 2]));
        let p = payload(
            &mut m,
            "set_fade_to_black_rate",
            json!({"me": 1, "rate": 60}),
        );
        assert_eq!(p, command(b"FtbC", &[1, 0, 60, 0]));
    }

    #[test]
    fn keyer_layouts() {
        let (mut m, _) = ready();
        let p = payload(
            &mut m,
            "set_usk_type",
            json!({"me": 1, "keyer": 1, "type": "dve", "fly_enabled": true}),
        );
        assert_eq!(p, command(b"CKTp", &[3, 0, 0, 3, 1, 0, 0, 0]));
        // Fill and key go in one packet.
        let p = payload(
            &mut m,
            "set_usk_sources",
            json!({"me": 1, "keyer": 1, "fill": 1, "key": 2}),
        );
        assert_eq!(
            p,
            [
                command(b"CKeF", &[0, 0, 0, 1]),
                command(b"CKeC", &[0, 0, 0, 2])
            ]
            .concat()
        );
        let p = payload(
            &mut m,
            "set_usk_luma",
            json!({"me": 1, "keyer": 1, "clip": 25.5, "invert": true}),
        );
        assert_eq!(
            p,
            command(b"CKLm", &[10, 0, 0, 0, 0, 255, 0, 0, 1, 0, 0, 0])
        );
        let p = payload(
            &mut m,
            "set_usk_pattern",
            json!({"me": 1, "keyer": 1, "style": "diamond_iris", "size": 50.0, "x": 0.5}),
        );
        assert_eq!(
            p,
            command(
                b"CKPt",
                &[19, 0, 0, 6, 0x13, 0x88, 0, 0, 0, 0, 0x13, 0x88, 0, 0, 0, 0]
            )
        );
        let p = payload(&mut m, "set_dsk_sources", json!({"dsk": 1, "fill": 1}));
        assert_eq!(p, command(b"CDsF", &[0, 0, 0, 1]));
        let p = payload(&mut m, "set_dsk_sources", json!({"dsk": 1, "key": 2}));
        assert_eq!(p, command(b"CDsC", &[0, 0, 0, 2]));
        let p = payload(&mut m, "set_dsk_tie", json!({"dsk": 1, "tie": true}));
        assert_eq!(p, command(b"CDsT", &[0, 1, 0, 0]));
        let p = payload(&mut m, "set_dsk_rate", json!({"dsk": 1, "rate": 40}));
        assert_eq!(p, command(b"CDsR", &[0, 40, 0, 0]));
        let p = payload(
            &mut m,
            "set_dsk_key",
            json!({"dsk": 1, "pre_multiplied": true, "gain": 100.0}),
        );
        assert_eq!(
            p,
            command(b"CDsG", &[5, 0, 1, 0, 0, 0, 0x03, 0xE8, 0, 0, 0, 0])
        );
    }

    #[test]
    fn colour_media_and_supersource_layouts() {
        let (mut m, _) = ready();
        let p = payload(
            &mut m,
            "set_color_generator",
            json!({"generator": 2, "hue": 180.0, "saturation": 50.0, "luma": 75.5}),
        );
        assert_eq!(
            p,
            command(b"CClV", &[7, 1, 0x07, 0x08, 0x01, 0xF4, 0x02, 0xF3])
        );
        let p = payload(
            &mut m,
            "set_media_player_source",
            json!({"player": 2, "still": 5}),
        );
        assert_eq!(p, command(b"MPSS", &[3, 1, 1, 4, 0, 0, 0, 0]));
        let p = payload(
            &mut m,
            "set_media_player_source",
            json!({"player": 1, "clip": 2}),
        );
        assert_eq!(p, command(b"MPSS", &[5, 0, 2, 0, 1, 0, 0, 0]));
        let p = payload(&mut m, "media_player_play", json!({"player": 1}));
        assert_eq!(p, command(b"SCPS", &[1, 0, 1, 0, 0, 0, 0, 0]));
        let p = payload(
            &mut m,
            "media_player_stop",
            json!({"player": 2, "rewind": true}),
        );
        assert_eq!(p, command(b"SCPS", &[5, 1, 0, 0, 1, 0, 0, 0]));
        let p = payload(
            &mut m,
            "set_media_player_loop",
            json!({"player": 1, "loop": true}),
        );
        assert_eq!(p, command(b"SCPS", &[2, 0, 0, 1, 0, 0, 0, 0]));
        // From 2.28: mask, SuperSource, box, enabled, rsv, then the box at 6.
        let p = payload(
            &mut m,
            "set_supersource_box",
            json!({"supersource": 1, "box": 2, "enabled": true, "source": 1,
                   "x": -12.5, "y": 6.0, "size": 0.5}),
        );
        assert_eq!(
            p,
            command(
                b"CSBP",
                &[
                    0, 31, 0, 1, 1, 0, 0, 1, 0xFB, 0x1E, 0x02, 0x58, 0x01, 0xF4, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0
                ]
            )
        );
        let p = payload(
            &mut m,
            "set_supersource_art",
            json!({"supersource": 1, "fill": 1, "key": 2, "placement": "foreground", "clip": 50.0}),
        );
        assert_eq!(
            p,
            command(
                b"CSSc",
                &[23, 0, 0, 1, 0, 2, 1, 0, 0x01, 0xF4, 0, 0, 0, 0, 0, 0]
            )
        );
    }

    #[test]
    fn audio_multiviewer_names_macros_and_streaming_layouts() {
        let (mut m, _) = ready();
        let p = payload(
            &mut m,
            "set_fairlight_source",
            json!({"input": 1, "source": -65280, "fader_gain": -10.5, "mix": "on"}),
        );
        let mut b = vec![0u8; 48];
        b[0] = 0x01;
        b[1] = 0x80;
        b[3] = 1;
        b[8..16].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x00]);
        b[40..44].copy_from_slice(&[0xFF, 0xFF, 0xFB, 0xE6]);
        b[44] = 2;
        assert_eq!(p, command(b"CFSP", &b));
        let p = payload(
            &mut m,
            "set_fairlight_source",
            json!({"input": 1, "source": -65280, "gain": 3.0, "balance": -50.0}),
        );
        assert_eq!(&p[8..10], &[0x00, 0x42], "mask bits 1 and 6");
        assert_eq!(&p[28..32], &[0, 0, 0x01, 0x2C]);
        assert_eq!(&p[44..46], &[0xEC, 0x78]);
        let p = payload(
            &mut m,
            "set_fairlight_master",
            json!({"fader_gain": 0.0, "follow_fade_to_black": true}),
        );
        let mut b = vec![0u8; 20];
        b[0] = 0x18;
        b[16] = 1;
        assert_eq!(p, command(b"CFMP", &b));
        let p = payload(&mut m, "set_audio_levels", json!({"enabled": true}));
        assert_eq!(p, command(b"SFLN", &[1, 0, 0, 0]));
        assert!(m.levels_wanted);
        let p = payload(
            &mut m,
            "set_multiviewer_window",
            json!({"multiviewer": 1, "window": 3, "source": 2}),
        );
        assert_eq!(p, command(b"CMvI", &[0, 2, 0, 2]));
        let p = payload(
            &mut m,
            "set_input_name",
            json!({"source": 1, "long_name": "Host", "short_name": "HST"}),
        );
        let mut b = vec![0u8; 32];
        b[0] = 3;
        b[3] = 1;
        b[4..8].copy_from_slice(b"Host");
        b[24..27].copy_from_slice(b"HST");
        assert_eq!(p, command(b"CInL", &b));
        let p = payload(&mut m, "continue_macro", json!({}));
        assert_eq!(p, command(b"MAct", &[0xFF, 0xFF, 4, 0]));
        let p = payload(&mut m, "set_macro_loop", json!({"loop": true}));
        assert_eq!(p, command(b"MRCP", &[1, 1, 0, 0]));
        let p = payload(
            &mut m,
            "set_streaming_service",
            json!({"service_name": "YouTube", "key": "abc"}),
        );
        assert_eq!(p.len(), 8 + 1100);
        assert_eq!(&p[4..8], b"CRSS");
        let body = &p[8..];
        assert_eq!(body[0], 5);
        assert_eq!(&body[1..9], b"YouTube\0");
        assert_eq!(&body[65..66], &[0]);
        assert_eq!(&body[577..581], b"abc\0");
    }

    #[test]
    fn camera_control_layouts() {
        let (mut m, _) = ready();
        let head = |cam: u8, cat: u8, par: u8, rel: u8, kind: u8, counts: [u8; 8]| {
            let mut h = vec![cam, cat, par, rel, kind, 0];
            h.extend_from_slice(&counts);
            h.extend_from_slice(&[0, 0]);
            h
        };
        let p = payload(&mut m, "camera_iris", json!({"camera": 1, "iris": 0.5}));
        let mut b = head(1, 0, 3, 0, 0x80, [0, 0, 0, 1, 0, 0, 0, 0]);
        b.extend_from_slice(&[0x04, 0x00, 0, 0, 0, 0, 0, 0]);
        assert_eq!(p, command(b"CCmd", &b));
        let p = payload(
            &mut m,
            "camera_focus",
            json!({"camera": 2, "focus": -0.25, "relative": true}),
        );
        let mut b = head(2, 0, 0, 1, 0x80, [0, 0, 0, 1, 0, 0, 0, 0]);
        b.extend_from_slice(&[0xFE, 0x00, 0, 0, 0, 0, 0, 0]);
        assert_eq!(p, command(b"CCmd", &b));
        let p = payload(&mut m, "camera_zoom", json!({"camera": 1, "speed": -1.0}));
        assert_eq!(&p[24..26], &[0xF8, 0x00]);
        let p = payload(&mut m, "camera_gain", json!({"camera": 1, "gain": 12}));
        let mut b = head(1, 1, 13, 0, 1, [0, 1, 0, 0, 0, 0, 0, 0]);
        b.extend_from_slice(&[12, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(p, command(b"CCmd", &b));
        let p = payload(
            &mut m,
            "camera_white_balance",
            json!({"camera": 1, "kelvin": 5600, "tint": -10}),
        );
        let mut b = head(1, 1, 2, 0, 2, [0, 0, 0, 2, 0, 0, 0, 0]);
        b.extend_from_slice(&[0x15, 0xE0, 0xFF, 0xF6, 0, 0, 0, 0]);
        assert_eq!(p, command(b"CCmd", &b));
        let p = payload(
            &mut m,
            "camera_shutter",
            json!({"camera": 1, "exposure_us": 20000}),
        );
        let mut b = head(1, 1, 5, 0, 3, [0, 0, 0, 0, 0, 1, 0, 0]);
        b.extend_from_slice(&[0, 0, 0x4E, 0x20, 0, 0, 0, 0]);
        assert_eq!(p, command(b"CCmd", &b));
        // An absolute focus is 0 to 1; camera 0 is black, not a camera.
        assert!(matches!(
            refused(&mut m, "camera_focus", json!({"camera": 1, "focus": -0.5})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            refused(&mut m, "camera_iris", json!({"camera": 0, "iris": 0.5})),
            CommandError::InvalidParams { .. }
        ));
    }

    #[test]
    fn new_commands_are_checked_against_the_topology() {
        let (mut m, _) = ready();
        let bad = |m: &mut Atem, name: &str, params: Value| {
            assert!(
                matches!(refused(m, name, params), CommandError::InvalidParams { .. }),
                "{name}"
            );
        };
        bad(&mut m, "set_dip", json!({"me": 1}));
        bad(&mut m, "set_dip", json!({"me": 1, "source": 3010}));
        bad(
            &mut m,
            "set_usk_sources",
            json!({"me": 1, "keyer": 2, "fill": 1}),
        );
        bad(
            &mut m,
            "set_usk_sources",
            json!({"me": 1, "keyer": 1, "fill": 3010}),
        );
        bad(
            &mut m,
            "set_next_transition",
            json!({"me": 1, "key2": true}),
        );
        bad(
            &mut m,
            "set_next_transition",
            json!({"me": 1, "background": false}),
        );
        bad(&mut m, "set_dsk_rate", json!({"dsk": 2, "rate": 10}));
        bad(
            &mut m,
            "set_color_generator",
            json!({"generator": 3, "hue": 1.0}),
        );
        bad(
            &mut m,
            "set_media_player_source",
            json!({"player": 3, "still": 1}),
        );
        bad(
            &mut m,
            "set_media_player_source",
            json!({"player": 1, "still": 21}),
        );
        bad(
            &mut m,
            "set_media_player_source",
            json!({"player": 1, "clip": 3}),
        );
        bad(&mut m, "set_media_player_source", json!({"player": 1}));
        bad(&mut m, "set_stinger", json!({"me": 1, "media_player": 3}));
        bad(
            &mut m,
            "set_supersource_box",
            json!({"supersource": 1, "box": 5, "enabled": true}),
        );
        bad(
            &mut m,
            "set_supersource_box",
            json!({"supersource": 2, "box": 1, "enabled": true}),
        );
        bad(
            &mut m,
            "set_multiviewer_window",
            json!({"multiviewer": 1, "window": 1, "source": 1}),
        );
        bad(
            &mut m,
            "set_fairlight_source",
            json!({"input": 2, "source": -65280, "fader_gain": 0.0}),
        );
        // The dump's source offers off and on, not audio-follow-video.
        bad(
            &mut m,
            "set_fairlight_source",
            json!({"input": 1, "source": -65280, "mix": "afv"}),
        );
        bad(
            &mut m,
            "set_input_name",
            json!({"source": 1, "long_name": "A name of 21 bytes..."}),
        );
        bad(
            &mut m,
            "set_input_name",
            json!({"source": 9, "short_name": "X"}),
        );
        // Classic audio on a Fairlight switcher is unsupported, not invalid.
        assert!(matches!(
            refused(&mut m, "set_audio_input", json!({"input": 1, "gain": 0.0})),
            CommandError::UnsupportedForModel { .. }
        ));
    }

    #[test]
    fn new_state_decoders() {
        let (mut m, _) = ready();
        let mut twpp = vec![0u8; 20];
        twpp[1] = 25;
        twpp[2] = 7;
        put16(&mut twpp, 4, 1250);
        put16(&mut twpp, 6, 1);
        put16(&mut twpp, 8, 5000);
        put16(&mut twpp, 12, 5000);
        twpp[16] = 1;
        let mut tdvp = vec![0u8; 20];
        tdvp[1] = 30;
        tdvp[3] = 27;
        put16(&mut tdvp, 4, 1);
        put16(&mut tdvp, 10, 500);
        let mut tstp = vec![0u8; 20];
        tstp[1] = 2;
        put16(&mut tstp, 4, 500);
        put16(&mut tstp, 10, 2);
        put16(&mut tstp, 16, 5);
        let mut kept = vec![0u8; 16];
        kept[2] = 6;
        put16(&mut kept, 4, 5000);
        put16(&mut kept, 10, 2500);
        let mut dskp = vec![0u8; 20];
        dskp[1] = 1;
        dskp[2] = 30;
        put16(&mut dskp, 4, 250);
        let mut mpcs = vec![0u8; 68];
        mpcs[0] = 1;
        mpcs[1] = 1;
        mpcs[2..7].copy_from_slice(b"Intro");
        put16(&mut mpcs, 66, 120);
        let mut mpfe = vec![0u8; 24];
        put16(&mut mpfe, 2, 4);
        mpfe[4] = 1;
        mpfe[23] = 8;
        mpfe.extend_from_slice(b"logo.png");
        let mut ssbp = vec![0u8; 24];
        ssbp[1] = 1;
        ssbp[2] = 1;
        put16(&mut ssbp, 4, 2);
        put_i16(&mut ssbp, 6, -1250);
        put16(&mut ssbp, 10, 500);
        let mut ssrc = vec![0u8; 16];
        put16(&mut ssrc, 2, 1);
        ssrc[6] = 1;
        put16(&mut ssrc, 8, 1000);
        let mut famp = vec![0u8; 20];
        put_i32(&mut famp, 12, -1050);
        famp[16] = 1;
        let mut fmlv = vec![0u8; 40];
        fmlv[..8].copy_from_slice(&(-65280i64).to_be_bytes());
        put16(&mut fmlv, 8, 1);
        put_i16(&mut fmlv, 32, -2000);
        let mut srsu = vec![0u8; 1096];
        srsu[..7].copy_from_slice(b"YouTube");
        srsu[64..71].copy_from_slice(b"rtmp://");
        srsu[576..582].copy_from_slice(b"secret");
        let mut iris = vec![1, 0, 3, 0x80, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
        iris.extend_from_slice(&[0x04, 0x00, 0, 0, 0, 0, 0, 0]);
        let mut wb = vec![1, 1, 2, 0x02, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0];
        wb.extend_from_slice(&[0x15, 0xE0, 0xFF, 0xF6, 0, 0, 0, 0]);
        let a = feed(
            &mut m,
            30,
            reliable(
                0x8001,
                2,
                &[
                    cmd(b"TrSS", &[0, 2, 3, 3, 2, 0, 0, 0]),
                    cmd(b"TMxP", &[0, 30, 0, 0]),
                    cmd(b"TDpP", &[0, 25, 0, 2]),
                    cmd(b"TWpP", &twpp),
                    cmd(b"TDvP", &tdvp),
                    cmd(b"TStP", &tstp),
                    cmd(b"TrPr", &[0, 1, 0, 0]),
                    cmd(b"FtbP", &[0, 60, 0, 0]),
                    cmd(b"KeLm", &[0, 0, 1, 0, 0, 255, 0x03, 0xE8, 1, 0, 0, 0]),
                    cmd(b"KePt", &kept),
                    cmd(b"DskB", &[0, 0, 0, 1, 0, 2, 0, 0]),
                    cmd(b"DskP", &dskp),
                    cmd(b"ColV", &[1, 0, 0x07, 0x08, 0x01, 0xF4, 0x02, 0xF3]),
                    cmd(b"MPCE", &[1, 2, 0, 1]),
                    cmd(b"RCPS", &[1, 1, 1, 0, 0, 42, 0, 0]),
                    cmd(b"MPCS", &mpcs),
                    cmd(b"MPfe", &mpfe),
                    cmd(b"SSBP", &ssbp),
                    cmd(b"SSrc", &ssrc),
                    cmd(b"FAMP", &famp),
                    cmd(b"FMLv", &fmlv),
                    fasp(1, -65280, 0b111, 4),
                    cmd(b"MRPr", &[3, 1, 0, 0]),
                    cmd(b"SRST", &[1, 2, 3, 4, 0, 0, 0, 0]),
                    cmd(b"RTMS", &[0, 3, 0, 0, 0, 0, 0x0E, 0x10]),
                    cmd(b"SRSU", &srsu),
                    cmd(b"CCdP", &iris),
                    cmd(b"CCdP", &wb),
                ],
            ),
        );
        let s = state(&a);
        let me = &s["mes"]["1"];
        assert_eq!(me["transition"]["style"], "wipe");
        assert_eq!(me["transition"]["selection"], json!(["background", "key1"]));
        assert_eq!(me["transition"]["next_style"], "dve");
        assert_eq!(me["transition"]["next_selection"], json!(["key1"]));
        assert_eq!(me["transition"]["mix"]["rate"], 30);
        assert_eq!(me["transition"]["dip"], json!({"rate": 25, "source": 2}));
        assert_eq!(
            me["transition"]["wipe"],
            json!({"rate": 25, "pattern": "circle_iris", "border_width": 12.5,
                   "border_source": 1, "symmetry": 50.0, "softness": 0.0, "x": 0.5,
                   "y": 0.0, "reverse": true, "flip_flop": false})
        );
        assert_eq!(me["transition"]["dve"]["style"], "push_left");
        assert_eq!(me["transition"]["dve"]["rate"], 30);
        assert_eq!(me["transition"]["dve"]["clip"], 50.0);
        assert_eq!(me["transition"]["sting"]["media_player"], 2);
        assert_eq!(me["transition"]["sting"]["clip"], 50.0);
        assert_eq!(me["transition"]["sting"]["preroll"], 2);
        assert_eq!(me["transition"]["sting"]["mix_rate"], 5);
        assert_eq!(me["transition"]["preview"], true);
        assert_eq!(me["fade_to_black"]["rate"], 60);
        assert_eq!(
            me["usk"]["1"]["luma"],
            json!({"pre_multiplied": true, "clip": 25.5, "gain": 100.0, "invert": true})
        );
        assert_eq!(me["usk"]["1"]["pattern"]["style"], "diamond_iris");
        assert_eq!(me["usk"]["1"]["pattern"]["size"], 50.0);
        assert_eq!(me["usk"]["1"]["pattern"]["x"], 0.25);
        assert_eq!(s["dsks"]["1"]["fill_source"], 1);
        assert_eq!(s["dsks"]["1"]["key_source"], 2);
        assert_eq!(s["dsks"]["1"]["tie"], true);
        assert_eq!(s["dsks"]["1"]["rate"], 30);
        assert_eq!(s["dsks"]["1"]["clip"], 25.0);
        assert_eq!(
            s["colors"]["2"],
            json!({"hue": 180.0, "saturation": 50.0, "luma": 75.5})
        );
        assert_eq!(
            s["media_players"]["2"],
            json!({"source_type": "clip", "still": 1, "clip": 2, "playing": true,
                   "loop": true, "at_beginning": false, "clip_frame": 42})
        );
        assert_eq!(
            s["media_pool"]["clips"]["2"],
            json!({"name": "Intro", "frames": 120})
        );
        assert_eq!(s["media_pool"]["stills"]["5"], json!({"name": "logo.png"}));
        let bx = &s["supersources"]["1"]["boxes"]["2"];
        assert_eq!(bx["enabled"], true);
        assert_eq!(bx["source"], 2);
        assert_eq!(bx["x"], -12.5);
        assert_eq!(bx["size"], 0.5);
        assert_eq!(
            s["supersources"]["1"]["art"],
            json!({"fill_source": 1, "key_source": 0, "placement": "foreground",
                   "pre_multiplied": false, "clip": 100.0, "gain": 0.0, "invert": false})
        );
        assert_eq!(
            s["fairlight"]["master"],
            json!({"fader_gain": -10.5, "follow_fade_to_black": true})
        );
        let source = &s["fairlight"]["inputs"]["1"]["sources"]["-65280"];
        assert_eq!(source["level"]["left"], -20.0);
        assert_eq!(source["mix_option"], "afv");
        assert_eq!(source["mix_options"], json!(["off", "on", "afv"]));
        assert_eq!(s["macro_running"], 1);
        assert_eq!(s["macro_waiting"], true);
        assert_eq!(s["macro_loop"], true);
        assert_eq!(s["streaming"]["duration"], "01:02:03:04");
        assert_eq!(s["streaming"]["service_name"], "YouTube");
        assert_eq!(s["streaming"]["url"], "rtmp://");
        assert!(s["streaming"].get("key").is_none(), "the key stays out");
        assert_eq!(
            s["recording"],
            json!({"active": true, "error": "none", "time_available": 3600})
        );
        assert_eq!(s["cameras"]["1"]["iris"], 0.5);
        assert_eq!(s["cameras"]["1"]["white_balance"], 5600);
        assert_eq!(s["cameras"]["1"]["tint"], -10);
    }

    #[test]
    fn the_dump_describes_the_extra_topology() {
        let (_, a) = ready();
        let s = state(&a);
        assert_eq!(s["topology"]["media_players"], 2);
        assert_eq!(s["topology"]["stills"], 20);
        assert_eq!(s["topology"]["clips"], 2);
        assert_eq!(s["topology"]["dves"], 1);
        assert_eq!(s["topology"]["stingers"], 1);
        assert_eq!(s["topology"]["supersources"], 1);
        assert_eq!(s["topology"]["camera_control"], true);
        assert_eq!(s["topology"]["audio"], "fairlight");
        assert_eq!(s["mes"]["1"]["usk"]["1"]["can_fly"], true);
        assert_eq!(s["mes"]["1"]["usk"]["1"]["type"], "luma");
        assert_eq!(s["multiviewers"]["1"]["windows"]["3"]["source"], 1);
        assert_eq!(
            s["mes"]["1"]["transition"]["next_selection"],
            json!(["background"])
        );
    }

    #[test]
    fn repeated_multiviewer_windows_are_not_reported_again() {
        let (mut m, _) = ready();
        let a = feed(
            &mut m,
            30,
            reliable(0x8001, 2, &[cmd(b"MvIn", &[0, 2, 0, 1, 1, 1, 0, 0])]),
        );
        assert_eq!(state(&a), json!({}));
        let a = feed(
            &mut m,
            31,
            reliable(0x8001, 3, &[cmd(b"MvIn", &[0, 2, 0, 2, 1, 1, 0, 0])]),
        );
        assert_eq!(state(&a)["multiviewers"]["1"]["windows"]["3"]["source"], 2);
    }

    #[test]
    fn classic_audio_layouts() {
        let mut dump = dump();
        let fac = dump.iter().position(|c| &c[4..8] == b"_FAC").unwrap();
        dump[fac] = cmd(b"_AMC", &[8, 1, 0, 0]);
        let fa = dump.iter().position(|c| &c[4..8] == b"FASP").unwrap();
        dump[fa] = cmd(
            b"AMIP",
            &[0, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0x80, 0, 0xEC, 0x78, 0, 0],
        );
        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        feed(&mut m, 10, syn_reply());
        let a = feed(&mut m, 20, reliable(0x8001, 1, &dump));
        let s = state(&a);
        assert_eq!(s["topology"]["audio"], "classic");
        assert_eq!(
            s["audio"]["inputs"]["1"],
            json!({"source_type": "video", "mix_option": "on", "gain": 0.0, "balance": -25.0})
        );
        let p = payload(
            &mut m,
            "set_audio_input",
            json!({"input": 1, "mix": "afv", "gain": -6.0, "balance": -25.0}),
        );
        // -6 dB is 10^(-6/20) x 32768, floored: 16422.
        assert_eq!(
            p,
            command(b"CAMI", &[7, 0, 0, 1, 2, 0, 0x40, 0x26, 0xEC, 0x78, 0, 0])
        );
        let p = payload(&mut m, "set_audio_master", json!({"gain": 0.0}));
        assert_eq!(p, command(b"CAMM", &[1, 0, 0x80, 0, 0, 0, 0, 0]));
        assert!(matches!(
            refused(&mut m, "set_audio_input", json!({"input": 2, "gain": 0.0})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            refused(&mut m, "set_fairlight_master", json!({"fader_gain": 0.0})),
            CommandError::UnsupportedForModel { .. }
        ));
        let a = feed(
            &mut m,
            30,
            reliable(0x8001, 2, &[cmd(b"AMMO", &[0x40, 0x26, 0, 0, 0, 0, 0, 0])]),
        );
        assert_eq!(state(&a)["audio"]["master"]["gain"], -6.0);
    }

    #[test]
    fn supersource_before_protocol_2_28() {
        let mut dump = dump();
        dump[0] = cmd(b"_ver", &[0, 2, 0, 27]);
        // Before 2.30 _top has no multiviewer byte: SuperSources at 10.
        let mut top = vec![0u8; 24];
        top[0] = 1;
        top[1] = 3;
        top[2] = 1;
        top[3] = 1;
        top[5] = 2;
        top[8] = 1;
        top[9] = 1;
        top[10] = 1;
        dump[2] = cmd(b"_top", &top);
        let ssc = dump.iter().position(|c| &c[4..8] == b"_SSC").unwrap();
        dump[ssc] = cmd(b"_SSC", &[4, 0x50, 0x72, 0x70]);
        dump.retain(|c| &c[4..8] != b"StRS");
        let mut m = ready_with(dump);
        // Mask, box, enabled, then the source at 4.
        let p = payload(
            &mut m,
            "set_supersource_box",
            json!({"supersource": 1, "box": 1, "enabled": true, "source": 1}),
        );
        let mut b = vec![0u8; 24];
        b[1] = 3;
        b[3] = 1;
        b[5] = 1;
        assert_eq!(p, command(b"CSBP", &b));
        // A u32 mask and the art at 4, in 36 bytes.
        let p = payload(
            &mut m,
            "set_supersource_art",
            json!({"supersource": 1, "fill": 1}),
        );
        let mut b = vec![0u8; 36];
        b[3] = 1;
        b[5] = 1;
        assert_eq!(p, command(b"CSSc", &b));
        // Camera control was not offered in this topology.
        assert!(matches!(
            refused(&mut m, "camera_iris", json!({"camera": 1, "iris": 0.5})),
            CommandError::UnsupportedForModel { .. }
        ));
        let mut ssbp = vec![0u8; 20];
        ssbp[0] = 2;
        ssbp[1] = 1;
        put16(&mut ssbp, 2, 2);
        put16(&mut ssbp, 8, 1000);
        let mut ssrc = vec![0u8; 32];
        put16(&mut ssrc, 0, 2);
        ssrc[4] = 1;
        let a = feed(
            &mut m,
            30,
            reliable(0x8001, 2, &[cmd(b"SSBP", &ssbp), cmd(b"SSrc", &ssrc)]),
        );
        let s = state(&a);
        assert_eq!(s["supersources"]["1"]["boxes"]["3"]["source"], 2);
        assert_eq!(s["supersources"]["1"]["boxes"]["3"]["size"], 1.0);
        assert_eq!(s["supersources"]["1"]["art"]["fill_source"], 2);
        assert_eq!(s["supersources"]["1"]["art"]["placement"], "foreground");
    }

    #[test]
    fn durations_are_polled_while_streaming_or_recording() {
        let (mut m, _) = ready();
        let a = feed(
            &mut m,
            30,
            reliable(0x8001, 2, &[cmd(b"RTMS", &[0, 3, 0, 0, 0, 0, 0x0E, 0x10])]),
        );
        assert!(a.contains(&Action::SetTimer {
            key: DURATION,
            after: DURATION_EVERY
        }));
        let mut cx = Cx::new(530);
        m.timer(&mut cx, DURATION);
        let a = cx.take();
        assert_eq!(&sent(&a)[0][12..], &command(b"RMDR", &[])[..]);
        assert!(a.contains(&Action::SetTimer {
            key: DURATION,
            after: DURATION_EVERY
        }));
        // The request is reliable, and its acknowledgement completes no command.
        let a = feed(&mut m, 540, header(FLAG_ACK, HEADER, 0x8001, 1, 0));
        assert!(!a.iter().any(|x| matches!(x, Action::Complete { .. })));
        // Streaming as well: both requests in one packet.
        feed(
            &mut m,
            550,
            reliable(0x8001, 3, &[cmd(b"StRS", &[0, 4, 0, 0])]),
        );
        let mut cx = Cx::new(1_030);
        m.timer(&mut cx, DURATION);
        assert_eq!(
            &sent(&cx.take())[0][12..],
            &[command(b"SRDR", &[]), command(b"RMDR", &[])].concat()[..]
        );
        // Stopped: no more requests.
        feed(
            &mut m,
            1_040,
            reliable(
                0x8001,
                4,
                &[cmd(b"StRS", &[0, 1, 0, 0]), cmd(b"RTMS", &[0, 2, 0, 0])],
            ),
        );
        let mut cx = Cx::new(1_530);
        m.timer(&mut cx, DURATION);
        let a = cx.take();
        assert!(sent(&a).is_empty());
        assert!(!a.iter().any(|x| matches!(x, Action::SetTimer { .. })));
    }

    #[test]
    fn level_subscription_is_renewed_after_reconnecting() {
        let (mut m, _) = ready();
        payload(&mut m, "set_audio_levels", json!({"enabled": true}));
        let mut cx = Cx::new(10_000);
        m.timer(&mut cx, SILENCE);
        feed(&mut m, 10_010, syn_reply());
        let a = feed(&mut m, 10_020, reliable(0x8002, 1, &dump()));
        let packets = sent(&a);
        assert!(
            packets
                .iter()
                .any(|p| p.len() > 12 && p[12..] == command(b"SFLN", &[1, 0, 0, 0])[..]),
            "{packets:?}"
        );
    }

    #[test]
    fn opened_for_commands_only_it_polls_nothing_and_renews_nothing() {
        let mut m = atem();
        m.monitor = false;
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        feed(&mut m, 10, syn_reply());
        // The switcher sends its state regardless; only acknowledgements go back.
        let a = feed(&mut m, 20, reliable(0x8001, 1, &dump()));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(sent(&a).iter().all(|p| p.len() == HEADER));
        assert_eq!(state(&a)["device"]["product"], "ATEM Mini Pro");

        // Recording: no duration polling.
        let a = feed(
            &mut m,
            30,
            reliable(0x8001, 2, &[cmd(b"RTMS", &[0, 3, 0, 0, 0, 0, 0x0E, 0x10])]),
        );
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: DURATION, .. })));

        // Commands work, the level subscription among them, but it is not
        // renewed after reconnecting.
        payload(&mut m, "set_audio_levels", json!({"enabled": true}));
        let a = feed(&mut m, 110, header(FLAG_ACK, HEADER, 0x8001, 1, 0));
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));
        let mut cx = Cx::new(10_000);
        m.timer(&mut cx, SILENCE);
        feed(&mut m, 10_010, syn_reply());
        let a = feed(&mut m, 10_020, reliable(0x8002, 1, &dump()));
        assert!(sent(&a).iter().all(|p| p.len() == HEADER));
    }

    #[test]
    fn the_time_to_each_acknowledgement_is_reported() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(100);
        let params = json!({"me": 1, "source": 2}).as_object().unwrap().clone();
        m.command(&mut cx, 7, "set_program", &params);
        cx.take();
        let a = feed(&mut m, 125, header(FLAG_ACK, HEADER, 0x8001, 1, 0));
        assert!(a.contains(&Action::RoundTrip(25)));

        // A resent packet's acknowledgement times no send.
        let mut cx = Cx::new(200);
        m.command(&mut cx, 8, "set_program", &params);
        cx.take();
        let mut cx = Cx::new(200 + RESEND_AFTER);
        m.timer(&mut cx, RESEND);
        cx.take();
        let a = feed(&mut m, 290, header(FLAG_ACK, HEADER, 0x8001, 2, 0));
        assert!(a.contains(&Action::Complete {
            id: 8,
            result: Ok(Outcome::Ack)
        }));
        assert!(!a.iter().any(|x| matches!(x, Action::RoundTrip(_))));

        // Pushed state is not a reply.
        let a = feed(
            &mut m,
            300,
            reliable(0x8001, 2, &[cmd(b"PrgI", &[0, 0, 0, 1])]),
        );
        assert!(!a.iter().any(|x| matches!(x, Action::RoundTrip(_))));
    }
}
