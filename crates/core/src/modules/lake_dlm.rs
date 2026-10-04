//! Lab.gruppen Lake-enabled amplifiers and processors (D Series, PLM+, PLM,
//! LM 26 / LM 44) over Direct Lake Messaging (DLM) v3.4.
//!
//! Protocol from Lab.gruppen's "Direct Lake Messaging v3.4, 3rd party protocol
//! for Lake enabled products" (2015-06-09, marked PUBLIC). Section numbers
//! below are that document's.
//!
//! - UDP. In dynamic port mode (the default) packets go to the device's port
//!   6016 and it answers the sender's port; in fixed port mode they go to 6015
//!   and it answers on the host's port 6004, which nothing else may hold
//!   (4.1, 4.2). Lake Controller binds 6004 itself.
//! - Every packet is a 28-byte little-endian header (source and destination
//!   64-bit ids, source class 6 for a host, destination class 5 for a device
//!   or 0 for broadcast, total length, packet type, message id), a payload
//!   and a 4-byte footer, 560 bytes at most (5). The message id rises by one
//!   per packet from a source, and the answer carries it back.
//! - A command is a NUL-terminated text, `Path?args` to get, `Path=args` to
//!   set and `Path!args` to do (8.1), sent as packet type 701. A get is
//!   answered by a type 701 packet holding the value(s); a set or do by an
//!   acknowledgement (type 2) whose 32-bit result is -2 for success (6.2).
//! - A device is addressed by its 64-bit frame id (7.3). The module asks the
//!   device at the given address for it (`Dev.Network.ID?` with the
//!   broadcast id), unless the `frame_id` setting gives it, and takes it from
//!   the answer's source id.
//! - Nothing is pushed: state is read by polling (meters with
//!   `Dev.MD.FullBin`, a binary structure per family, Appendix B; parameters
//!   with gets).
//!
//! One request is in flight at a time, commands ahead of polls. Three
//! requests in a row without an answer drop the connection, which is probed
//! again after a pause.
//!
//! Opened for commands only, it reads and polls nothing: the frame id probe
//! finds the device, and `Dev.Power?` asked after 3 s without traffic is the
//! liveness check, as the protocol has no presence signal of its own.

use std::collections::VecDeque;
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};

const SOCKET: Key = "dlm";
const REPLY: Key = "reply";
const RETRY: Key = "retry";
const METERS: Key = "meters";
const PARAMETERS: Key = "parameters";
const LIVENESS: Key = "liveness";

const HEADER: usize = 28;
const FOOTER: usize = 4;
const MAX_PACKET: usize = 560;
const DLM_MSG: u16 = 701;
const ACK_MSG: u16 = 2;
const HOST_CLASS: u16 = 6;
const DEVICE_CLASS: u16 = 5;
const BROADCAST_CLASS: u16 = 0;
const BROADCAST_ID: (u32, u32) = (0xFFFF_FFFE, 0xFFFF_FFFD);
/// The source id this host presents ("MEROS DLM"); any value will do (FAQ 1).
const SOURCE_ID: (u32, u32) = (0x4D45_524F, 0x0000_D11A);

const ACK_SUCCESS: i32 = -2;
/// Dev.Power: the frame was already on, or already in standby.
const ACK_ALREADY_ON: i32 = -20;
const ACK_ALREADY_OFF: i32 = -21;

const REPLY_TIMEOUT: Millis = 1_000;
/// A preset recall answers late: it reloads every DSP parameter.
const SLOW_REPLY_TIMEOUT: Millis = 5_000;
const LIVENESS_EVERY: Millis = 1_000;
const QUIET_AFTER: Millis = 3_000;
const MISSES_TO_LOSE: u32 = 3;
const RETRY_AFTER: Millis = 2_000;

const DYNAMIC_PORT: u16 = 6016;
const FIXED_PORT: u16 = 6015;
const FIXED_REPLY_PORT: u16 = 6004;

/// The three families DLM distinguishes (2, 8.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Family {
    /// D Series and PLM+ (DLM version 3): modules A-D, four power channels.
    Plus,
    /// PLM 10000Q, 14000 and 20000Q (DLM versions 1 and 2): modules A-B.
    Legacy,
    /// LM 26 and LM 44 (DLM version 2): modules A-B, no amplifier.
    Lm,
}

impl Family {
    pub(crate) fn of(model: &str) -> Family {
        if model.starts_with("lm-") {
            Family::Lm
        } else if matches!(model, "plm-10000q" | "plm-14000" | "plm-20000q") {
            Family::Legacy
        } else {
            Family::Plus
        }
    }

    fn modules(self) -> &'static [&'static str] {
        match self {
            Family::Plus => &["A", "B", "C", "D"],
            _ => &["A", "B"],
        }
    }

    fn power_channels(self) -> u32 {
        match self {
            Family::Lm => 0,
            _ => 4,
        }
    }

    /// Router inputs (Dev.Router.*): 2 on PLM, 4 on D Series & PLM+, 6 on LM.
    fn router_inputs(self) -> u32 {
        match self {
            Family::Plus => 4,
            Family::Legacy => 2,
            Family::Lm => 6,
        }
    }

    /// The meter request and the structure it answers with (Appendix B).
    fn meters(self) -> (&'static str, Meters) {
        match self {
            Family::Plus => ("Dev.MD.FullBin?3", Meters::V3),
            Family::Legacy => ("Dev.MD.FullBin?2", Meters::V2),
            Family::Lm => ("Dev.MD.FullBin?", Meters::Lm),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Meters {
    V2,
    V3,
    Lm,
}

/// How a get's value becomes state.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Parse {
    Bool,
    Number,
    Int,
    Text,
}

#[derive(Debug, Clone, PartialEq)]
enum Expect {
    /// An acknowledgement; on success the patch is applied.
    Ack(Option<Value>),
    /// Dev.Power: "already on" and "already off" are success too.
    Power(Option<Value>),
    /// A get: the value text.
    Value,
    /// The meter structure, decoded.
    Meters(Meters),
}

#[derive(Debug, Clone, PartialEq)]
enum Why {
    Probe,
    Liveness,
    Command { id: CommandId, expect: Expect },
    Read { path: Vec<String>, parse: Parse },
    Channels { module: usize },
    Meters(Meters),
}

#[derive(Debug, Clone)]
struct Request {
    text: String,
    why: Why,
    timeout: Millis,
}

struct InFlight {
    msg_id: u32,
    request: Request,
    sent: Millis,
}

pub(crate) struct LakeDlm {
    device: SocketAddr,
    bind: Bind,
    family: Family,
    model: String,
    monitor: bool,
    meter_poll: Millis,
    parameter_poll: Millis,
    /// The device's frame id, given or learnt from its answer.
    frame_id: Option<(u32, u32)>,
    connected: bool,
    socket_open: bool,
    next_msg_id: u32,
    in_flight: Option<InFlight>,
    commands: VecDeque<Request>,
    reads: VecDeque<Request>,
    misses: u32,
    last_heard: Millis,
    /// Output channels per module, from Mod.Out.Chans.
    channels: [u32; 4],
}

impl LakeDlm {
    pub(crate) fn new(ctx: OpenContext) -> Result<LakeDlm, String> {
        let setting = |name: &str| ctx.settings.get(name).cloned().unwrap_or(Value::Null);
        let fixed = setting("response_port").as_str() == Some("fixed");
        let port = ctx
            .port
            .unwrap_or(if fixed { FIXED_PORT } else { DYNAMIC_PORT });
        let frame_id = match setting("frame_id").as_str().filter(|s| !s.is_empty()) {
            Some(text) => Some(parse_frame_id(text).ok_or_else(|| {
                format!("frame_id '{text}' is not two 8-digit hex numbers joined by ':'")
            })?),
            None => None,
        };
        let ms = |name: &str, default: Millis| setting(name).as_u64().unwrap_or(default);
        Ok(LakeDlm {
            device: SocketAddr::new(ctx.host, port),
            bind: if fixed {
                Bind::Shared(FIXED_REPLY_PORT)
            } else {
                Bind::Ephemeral
            },
            family: Family::of(&ctx.model),
            model: ctx.model.clone(),
            monitor: ctx.monitor,
            meter_poll: ms("meter_poll_ms", 1_000),
            parameter_poll: ms("parameter_poll_ms", 5_000),
            frame_id,
            connected: false,
            socket_open: false,
            next_msg_id: 1,
            in_flight: None,
            commands: VecDeque::new(),
            reads: VecDeque::new(),
            misses: 0,
            last_heard: 0,
            channels: [0; 4],
        })
    }

    fn packet(&mut self, text: &str) -> (u32, Vec<u8>) {
        let msg_id = self.next_msg_id;
        // 0xFFFFFFFF asks for no answer (5.1), so the ids wrap before it.
        self.next_msg_id = if msg_id >= u32::MAX - 1 {
            1
        } else {
            msg_id + 1
        };
        let (dest, class) = match self.frame_id {
            Some(id) => (id, DEVICE_CLASS),
            None => (BROADCAST_ID, BROADCAST_CLASS),
        };
        (
            msg_id,
            encode(SOURCE_ID, dest, class, DLM_MSG, msg_id, text.as_bytes()),
        )
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || !self.socket_open {
            return;
        }
        let Some(request) = self.commands.pop_front().or_else(|| self.reads.pop_front()) else {
            return;
        };
        let (msg_id, data) = self.packet(&request.text);
        cx.udp_send(SOCKET, self.device, data);
        cx.set_timer(REPLY, request.timeout);
        self.in_flight = Some(InFlight {
            msg_id,
            request,
            sent: cx.now(),
        });
    }

    fn read(&mut self, text: String, path: &[&str], parse: Parse) {
        self.reads.push_back(Request {
            text,
            why: Why::Read {
                path: path.iter().map(|s| s.to_string()).collect(),
                parse,
            },
            timeout: REPLY_TIMEOUT,
        });
    }

    fn probe(&mut self, cx: &mut Cx) {
        self.reads.push_back(Request {
            text: "Dev.Network.ID?".into(),
            why: Why::Probe,
            timeout: REPLY_TIMEOUT,
        });
        self.pump(cx);
    }

    /// What is read once, when the device is found.
    fn identity(&mut self) {
        if self.family == Family::Plus {
            self.read(
                "Dev.ModelName?".into(),
                &["device", "model_name"],
                Parse::Text,
            );
        }
        self.read(
            "Dev.BundleVer?".into(),
            &["device", "bundle_version"],
            Parse::Text,
        );
        self.read(
            "Dev.Network.IPAddr?".into(),
            &["device", "ip_address"],
            Parse::Text,
        );
        self.read(
            "Dev.Network.MACAddr?".into(),
            &["device", "mac_address"],
            Parse::Text,
        );
        self.read(
            "Dev.Dante.Enabled?".into(),
            &["device", "dante_enabled"],
            Parse::Bool,
        );
        for (i, module) in self.family.modules().iter().enumerate() {
            self.reads.push_back(Request {
                text: format!("Mod.Out.Chans?{module}"),
                why: Why::Channels { module: i },
                timeout: REPLY_TIMEOUT,
            });
        }
    }

    /// The parameters read on each round of the parameter poll.
    fn parameters(&mut self) {
        self.read("Dev.Power?".into(), &["device", "power"], Parse::Bool);
        self.read(
            "Dev.FrameLabel?".into(),
            &["device", "frame_label"],
            Parse::Text,
        );
        self.read(
            "Dev.LatencyMatch?".into(),
            &["device", "latency_match"],
            Parse::Bool,
        );
        for pc in 1..=self.family.power_channels() {
            let n = pc.to_string();
            self.read(
                format!("Dev.Pwr.Mute?{pc}"),
                &["power_channels", &n, "mute"],
                Parse::Bool,
            );
            self.read(
                format!("Dev.Pwr.Attenuation?{pc}"),
                &["power_channels", &n, "attenuation_db"],
                Parse::Number,
            );
        }
        for input in 1..=self.family.router_inputs() {
            let n = input.to_string();
            self.read(
                format!("Dev.Router.InputMute?{input}"),
                &["router_inputs", &n, "mute"],
                Parse::Bool,
            );
            self.read(
                format!("Dev.Router.InputAct?{input}"),
                &["router_inputs", &n, "active_priority"],
                Parse::Int,
            );
        }
        for (i, m) in self.family.modules().iter().enumerate() {
            self.read(
                format!("Mod.Mod.Label?{m}"),
                &["modules", m, "label"],
                Parse::Text,
            );
            self.read(
                format!("Mod.In.Mute?{m}"),
                &["modules", m, "input", "mute"],
                Parse::Bool,
            );
            self.read(
                format!("Mod.In.Gain?{m}"),
                &["modules", m, "input", "gain_db"],
                Parse::Number,
            );
            self.read(
                format!("Mod.In.Delay?{m}"),
                &["modules", m, "input", "delay_ms"],
                Parse::Number,
            );
            for ch in 1..=self.channels[i] {
                let c = ch.to_string();
                for (path, field, parse) in [
                    ("Mod.Out.Mute", "mute", Parse::Bool),
                    ("Mod.Out.Gain", "gain_db", Parse::Number),
                    ("Mod.Out.Delay", "delay_ms", Parse::Number),
                    ("Mod.Out.Label", "label", Parse::Text),
                ] {
                    self.read(
                        format!("{path}?{m} {ch}"),
                        &["modules", m, "outputs", &c, field],
                        parse,
                    );
                }
            }
        }
    }

    fn meters(&mut self) {
        // One round at a time: a slow device is not asked again until it has
        // answered.
        let pending = self
            .reads
            .iter()
            .chain(self.in_flight.as_ref().map(|f| &f.request))
            .any(|r| matches!(r.why, Why::Meters(_)));
        if pending {
            return;
        }
        let (text, kind) = self.family.meters();
        self.reads.push_back(Request {
            text: text.into(),
            why: Why::Meters(kind),
            timeout: REPLY_TIMEOUT,
        });
        self.read(
            "Dev.MD.NoFaults?".into(),
            &["device", "no_faults"],
            Parse::Bool,
        );
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.misses = 0;
        self.last_heard = cx.now();
        cx.alive();
    }

    /// Nothing answers: fail what waits and probe again after a pause.
    fn lost(&mut self, cx: &mut Cx, reason: &str) {
        self.connected = false;
        self.in_flight = None;
        for r in self.commands.drain(..) {
            if let Why::Command { id, .. } = r.why {
                cx.complete(id, Err(CommandError::NotConnected));
            }
        }
        self.reads.clear();
        for key in [REPLY, METERS, PARAMETERS, LIVENESS] {
            cx.cancel_timer(key);
        }
        cx.connection(Connection::Disconnected {
            reason: reason.into(),
        });
        cx.set_timer(RETRY, RETRY_AFTER);
    }

    fn found(&mut self, cx: &mut Cx) {
        if self.connected {
            return;
        }
        self.connected = true;
        cx.connection(Connection::Connected);
        cx.set_timer(LIVENESS, LIVENESS_EVERY);
        if let Some(id) = self.frame_id {
            cx.state(json!({"device": {"frame_id": format_frame_id(id)}}));
        }
        if self.monitor {
            self.identity();
            if self.meter_poll > 0 {
                cx.set_timer(METERS, self.meter_poll);
            }
            // The first parameter round follows the channel counts.
            cx.set_timer(PARAMETERS, 0);
        }
    }

    fn reply(&mut self, cx: &mut Cx, request: Request, sent: Millis, reply: Reply) {
        cx.round_trip(cx.now().saturating_sub(sent));
        self.heard(cx);
        match request.why {
            Why::Probe => {
                if let Reply::Text { source, .. } = &reply {
                    // The answer's source id is the frame id (Appendix C).
                    if self.frame_id.is_none() && *source != BROADCAST_ID {
                        self.frame_id = Some(*source);
                    }
                }
                self.found(cx);
            }
            Why::Liveness => {
                if let Reply::Text { text, .. } = &reply {
                    if let Some(v) = parse_value(&strip_echo(&request.text, text), Parse::Bool) {
                        cx.state(json!({"device": {"power": v}}));
                    }
                }
            }
            Why::Read { path, parse } => {
                if let Reply::Text { text, .. } = &reply {
                    if let Some(v) = parse_value(&strip_echo(&request.text, text), parse) {
                        cx.state(nest(&path, v));
                    }
                }
            }
            Why::Channels { module } => {
                if let Reply::Text { text, .. } = &reply {
                    if let Some(n) = parse_value(&strip_echo(&request.text, text), Parse::Int)
                        .and_then(|v| v.as_u64())
                        .filter(|n| (1..=6).contains(n))
                    {
                        self.channels[module] = n as u32;
                        let m = self.family.modules()[module];
                        cx.state(json!({"modules": {m: {"output_channels": n}}}));
                    }
                }
            }
            Why::Meters(kind) => {
                if let Reply::Text { payload, .. } = &reply {
                    if let Some(patch) = decode_meters(kind, payload) {
                        cx.state(patch);
                    }
                }
            }
            Why::Command { id, expect } => {
                let outcome = match (expect, reply) {
                    (Expect::Ack(patch), Reply::Ack(ACK_SUCCESS)) => {
                        if let Some(p) = patch {
                            cx.state(p);
                        }
                        Ok(Outcome::Ack)
                    }
                    (
                        Expect::Power(patch),
                        Reply::Ack(ACK_SUCCESS | ACK_ALREADY_ON | ACK_ALREADY_OFF),
                    ) => {
                        if let Some(p) = patch {
                            cx.state(p);
                        }
                        Ok(Outcome::Ack)
                    }
                    (Expect::Value, Reply::Text { text, .. }) => {
                        let value = strip_echo(&request.text, &text);
                        let value = match value.parse::<f64>() {
                            Ok(n) if n.is_finite() => json!(n),
                            _ => json!(value),
                        };
                        Ok(Outcome::Value { value })
                    }
                    (Expect::Meters(kind), Reply::Text { payload, .. }) => {
                        match decode_meters(kind, &payload) {
                            Some(patch) => {
                                cx.state(patch.clone());
                                Ok(Outcome::Value { value: patch })
                            }
                            None => Err(CommandError::DeviceError {
                                code: None,
                                message: format!(
                                    "meter data of {} bytes does not match the structure",
                                    payload.len()
                                ),
                            }),
                        }
                    }
                    (_, Reply::Ack(code)) => Err(CommandError::DeviceError {
                        code: Some(code.to_string()),
                        message: ack_name(code).into(),
                    }),
                    (_, Reply::Text { text, .. }) => Err(CommandError::DeviceError {
                        code: None,
                        message: format!("unexpected answer: {text}"),
                    }),
                };
                cx.complete(id, outcome);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Reply {
    Ack(i32),
    Text {
        source: (u32, u32),
        text: String,
        payload: Vec<u8>,
    },
}

/// One packet (5.1-5.3): header, payload and a zero footer.
pub(crate) fn encode(
    source: (u32, u32),
    dest: (u32, u32),
    dest_class: u16,
    kind: u16,
    msg_id: u32,
    text: &[u8],
) -> Vec<u8> {
    let length = HEADER + text.len() + 1 + FOOTER;
    let mut p = Vec::with_capacity(length);
    for v in [source.0, source.1, dest.0, dest.1] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    p.extend_from_slice(&HOST_CLASS.to_le_bytes());
    p.extend_from_slice(&dest_class.to_le_bytes());
    p.extend_from_slice(&(length as u16).to_le_bytes());
    p.extend_from_slice(&kind.to_le_bytes());
    p.extend_from_slice(&msg_id.to_le_bytes());
    p.extend_from_slice(text);
    p.push(0);
    p.extend_from_slice(&[0; FOOTER]);
    p
}

/// A received packet: its message id and what it says, or None for one that
/// is not an answer (too short, a heartbeat or broadcast, another type).
fn decode(data: &[u8]) -> Option<(u32, Reply)> {
    if data.len() < HEADER + FOOTER {
        return None;
    }
    let u32_at = |i: usize| u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
    let u16_at = |i: usize| u16::from_le_bytes([data[i], data[i + 1]]);
    let source = (u32_at(0), u32_at(4));
    let dest = (u32_at(8), u32_at(12));
    if dest == BROADCAST_ID {
        // Heartbeats and meter broadcasts are not answers (Appendix C).
        return None;
    }
    let length = (u16_at(20) as usize).clamp(HEADER + FOOTER, data.len());
    let kind = u16_at(22);
    let msg_id = u32_at(24);
    let payload = &data[HEADER..length - FOOTER];
    match kind {
        ACK_MSG if payload.len() >= 4 => Some((
            msg_id,
            Reply::Ack(i32::from_le_bytes([
                payload[0], payload[1], payload[2], payload[3],
            ])),
        )),
        DLM_MSG => {
            let end = payload
                .iter()
                .position(|b| *b == 0)
                .unwrap_or(payload.len());
            Some((
                msg_id,
                Reply::Text {
                    source,
                    text: String::from_utf8_lossy(&payload[..end]).trim().to_string(),
                    payload: payload.to_vec(),
                },
            ))
        }
        _ => None,
    }
}

fn ack_name(code: i32) -> &'static str {
    match code {
        -3 => "the sender is not master of the device (ACK_NOTMASTER)",
        -4 => "invalid packet (ACK_INVALID_PACKET)",
        -5 => "communication with the DSP failed (ACK_DSP_ERROR)",
        -6 => "bad parameters (ACK_BAD_PARAM)",
        -20 => "the frame is already on",
        -21 => "the frame is already in standby",
        _ => "the device refused the command",
    }
}

pub(crate) fn parse_frame_id(text: &str) -> Option<(u32, u32)> {
    let (hi, lo) = text.trim().split_once(':')?;
    let hex = |s: &str| {
        (s.len() == 8)
            .then(|| u32::from_str_radix(s, 16).ok())
            .flatten()
    };
    Some((hex(hi)?, hex(lo)?))
}

fn format_frame_id(id: (u32, u32)) -> String {
    format!("{:08x}:{:08x}", id.0, id.1)
}

/// The value text of a get's answer. The document gives the answer as the
/// value(s) alone; a device that echoes the command (`Mod.Out.Gain=A 1
/// -3.00`) has the path, operator and the request's own arguments removed.
fn strip_echo(request: &str, reply: &str) -> String {
    let reply = reply.trim();
    let Some(op) = request.find(['?', '=', '!']) else {
        return reply.to_string();
    };
    let path = &request[..op];
    let Some(rest) = reply.strip_prefix(path) else {
        return reply.to_string();
    };
    let rest = rest.trim_start_matches(['?', '=', '!', ' ']);
    let mut rest = rest;
    for arg in request[op + 1..].split_whitespace() {
        match rest.strip_prefix(arg) {
            Some(r) => rest = r.trim_start(),
            None => break,
        }
    }
    rest.to_string()
}

/// A value as state: the first word for numbers and switches (a gain answers
/// with its group sum and limits after it), the whole text for text.
fn parse_value(text: &str, parse: Parse) -> Option<Value> {
    let first = text.split_whitespace().next();
    match parse {
        Parse::Text => Some(json!(text)),
        Parse::Bool => match first? {
            "1" => Some(json!(true)),
            "0" => Some(json!(false)),
            _ => None,
        },
        Parse::Int => first?.parse::<i64>().ok().map(|n| json!(n)),
        Parse::Number => first?
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(|n| json!(n)),
    }
}

/// `{"a": {"b": value}}` from a path.
fn nest(path: &[String], value: Value) -> Value {
    path.iter().rev().fold(value, |v, k| json!({ k: v }))
}

/// A level byte as dB: 0xFF is full scale, 0.5 dB a step down to 0x01 at
/// -127; 0x00 is below that, reported as -128 (Appendix B).
fn level_db(b: u8) -> f64 {
    if b == 0 {
        -128.0
    } else {
        (b as f64 - 255.0) * 0.5
    }
}

/// A gain reduction byte as dB: 0.1 dB a step (Appendix B, 9.1.2.6).
fn gr_db(b: u8) -> f64 {
    (b as f64 * 0.1 * 10.0).round() / 10.0
}

fn bit(v: u32, n: u32) -> bool {
    v >> n & 1 == 1
}

fn temperature(code: u32) -> &'static str {
    match code {
        0 => "ok",
        1 => "warning",
        _ => "fault",
    }
}

/// Amp Status (9.2.2.2): the low 16 bits of the first sAmpInfo word.
fn amp_status(w: u32) -> Value {
    json!({
        "power_on": bit(w, 0),
        "psu_failure": bit(w, 1),
        "pal": bit(w, 2),
        "audio_in_fault": bit(w, 3),
        "psu_temperature": temperature(w >> 4 & 3),
        "load_monitor_active": bit(w, 6),
        "protect": bit(w, 7),
        "dsp_temperature": temperature(w >> 8 & 3),
        "sense_warning": bit(w, 12),
    })
}

/// Channel Status (9.1.2.2, 9.2.2.3) and the meter word after it.
fn power_channel(status: u32, meters: u32) -> Value {
    json!({
        "service": bit(status, 31),
        "vhf": bit(status, 30),
        "short_circuit": bit(status, 29),
        "temperature": temperature(status >> 27 & 3),
        "open_load": bit(status, 26),
        "voltage_clip": bit(status, 25),
        "current_clip": bit(status, 24),
        "load_correct": bit(status, 23),
        "load_wrong": bit(status, 22),
        "load_not_verified": bit(status, 21),
        "cal_active": bit(status, 10),
        "voltage_rms_db": level_db((status & 0xFF) as u8),
        "power_db": level_db((meters >> 24) as u8),
        "voltage_db": level_db((meters >> 16) as u8),
        "current_db": level_db((meters >> 8) as u8),
        "gain_reduction_db": gr_db(meters as u8),
    })
}

/// The 60-byte sAmpInfo of versions 2 and 3: amp status and four channels.
fn amp_info(d: &[u8], patch: &mut Map<String, Value>) {
    let w = |i: usize| u32::from_le_bytes([d[i * 4], d[i * 4 + 1], d[i * 4 + 2], d[i * 4 + 3]]);
    patch.insert("amp".into(), amp_status(w(0) & 0xFFFF));
    let mut channels = Map::new();
    for c in 0..4 {
        channels.insert(
            (c + 1).to_string(),
            power_channel(w(1 + 2 * c), w(2 + 2 * c)),
        );
    }
    patch.insert("power_channels".into(), Value::Object(channels));
}

fn word(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// Decode Dev.MD.FullBin's answer into a state patch (Appendix B), or None
/// when it is too short for the structure.
fn decode_meters(kind: Meters, d: &[u8]) -> Option<Value> {
    let mut patch = Map::new();
    match kind {
        Meters::V3 => {
            if d.len() < 108 {
                return None;
            }
            amp_info(d, &mut patch);
            let s = word(d, 60);
            patch.insert(
                "status".into(),
                json!({
                    "link_1": bit(s, 0), "link_2": bit(s, 1), "controller_present": bit(s, 4),
                    "ad_data_fail": bit(s, 5), "clock_slip_dante": bit(s, 7), "clock_slip_aes_1_2": bit(s, 8),
                    "clock_slip_aes_3_4": bit(s, 9), "dante_leader": bit(s, 12), "dante_name_conflict": bit(s, 13),
                    "dante_module_fault": bit(s, 14), "dante_module_missing": bit(s, 15),
                    "dante_module_disabled": bit(s, 16), "dante_firmware_mismatch": bit(s, 17),
                    "ad_psu_fail": bit(s, 20), "dico_comm_fault": bit(s, 21),
                }),
            );
            let mut modules = Map::new();
            for (i, m) in ["A", "B", "C", "D"].iter().enumerate() {
                let connected = d[64 + i];
                let clip = d[68 + i];
                modules.insert(
                    m.to_string(),
                    json!({
                        "router_connected": (0..4).map(|b| connected >> b & 1 == 1).collect::<Vec<_>>(),
                        "clip": {
                            "outputs": (0..6).map(|b| clip >> b & 1 == 1).collect::<Vec<_>>(),
                            "router": clip >> 6 & 1 == 1,
                            "input": clip >> 7 & 1 == 1,
                        },
                        "input_meter": {"peak_dbfs": level_db(d[80 + i]), "rms_dbfs": level_db(d[84 + i])},
                    }),
                );
            }
            patch.insert("modules".into(), Value::Object(modules));
            let mut inputs = Map::new();
            for i in 0..4 {
                inputs.insert(
                    (i + 1).to_string(),
                    json!({"peak_dbfs": level_db(d[72 + i]), "rms_dbfs": level_db(d[76 + i])}),
                );
            }
            patch.insert("inputs".into(), Value::Object(inputs));
            let mut outputs = Map::new();
            for i in 0..4 {
                outputs.insert(
                    (i + 1).to_string(),
                    json!({
                        "peak_dbfs": level_db(d[88 + i]), "rms_dbfs": level_db(d[92 + i]),
                        "gain_reduction_peak_db": gr_db(d[96 + i]), "gain_reduction_rms_db": gr_db(d[100 + i]),
                    }),
                );
            }
            patch.insert("outputs".into(), Value::Object(outputs));
        }
        Meters::V2 => {
            if d.len() < 84 {
                return None;
            }
            amp_info(d, &mut patch);
            let s = word(d, 64);
            patch.insert(
                "status".into(),
                json!({
                    "link_1": bit(s, 0), "link_2": bit(s, 1), "controller_present": bit(s, 4),
                    "ad_data_fail": bit(s, 5), "clock_slip_aes": bit(s, 6), "clock_slip_dante": bit(s, 7),
                    "dante_leader": bit(s, 12),
                }),
            );
            let mut modules = Map::new();
            for (i, m) in ["A", "B"].iter().enumerate() {
                let clips = s >> (16 + 6 * i as u32) & 0x3F;
                modules.insert(
                    m.to_string(),
                    json!({
                        "input_meter": {"peak_dbfs": level_db(d[60 + 2 * i]), "rms_dbfs": level_db(d[61 + 2 * i])},
                        "inputs_connected": [bit(s, 8 + 2 * i as u32), bit(s, 9 + 2 * i as u32)],
                        "clip": {"outputs": (0..6).map(|b| clips >> b & 1 == 1).collect::<Vec<_>>()},
                    }),
                );
            }
            patch.insert("modules".into(), Value::Object(modules));
            patch.insert(
                "inputs".into(),
                json!({
                    "1": {"peak_dbfs": level_db(d[68]), "rms_dbfs": level_db(d[69])},
                    "2": {"peak_dbfs": level_db(d[70]), "rms_dbfs": level_db(d[71])},
                }),
            );
            let mut outputs = Map::new();
            for i in 0..4 {
                outputs.insert(
                    (i + 1).to_string(),
                    json!({"gain_reduction_peak_db": gr_db(d[72 + i]), "gain_reduction_rms_db": gr_db(d[76 + i])}),
                );
            }
            patch.insert("outputs".into(), Value::Object(outputs));
        }
        Meters::Lm => {
            if d.len() < 44 {
                return None;
            }
            let s = word(d, 0);
            patch.insert(
                "status".into(),
                json!({
                    "link_1": bit(s, 0), "link_2": bit(s, 1), "controller_present": bit(s, 4),
                    "ad_data_fail": bit(s, 5), "no_input_source": bit(s, 6), "clock_slip_aes_1": bit(s, 8),
                    "clock_slip_aes_2": bit(s, 9), "clock_slip_dante": bit(s, 10), "protective_mute": bit(s, 11),
                    "dante_leader": bit(s, 12), "fan_alarm": bit(s, 13), "temperature_warning": bit(s, 14),
                    "overtemperature": bit(s, 15),
                }),
            );
            // Peaks from byte 8, RMS values from byte 26, as the table lists them.
            let mut routers = Map::new();
            for i in 0..4 {
                routers.insert(
                    (i + 1).to_string(),
                    json!({"peak_dbfs": level_db(d[8 + i]), "rms_dbfs": level_db(d[26 + i])}),
                );
            }
            patch.insert("router_outputs".into(), Value::Object(routers));
            let mut modules = Map::new();
            for (i, m) in ["A", "B"].iter().enumerate() {
                modules.insert(
                    m.to_string(),
                    json!({"input_meter": {"peak_dbfs": level_db(d[12 + i]), "rms_dbfs": level_db(d[30 + i])}}),
                );
            }
            patch.insert("modules".into(), Value::Object(modules));
            let mut outputs = Map::new();
            for i in 0..6 {
                outputs.insert(
                    (i + 1).to_string(),
                    json!({
                        "peak_dbfs": level_db(d[14 + i]), "rms_dbfs": level_db(d[32 + i]),
                        "gain_reduction_peak_db": gr_db(d[20 + i]),
                    }),
                );
            }
            patch.insert("outputs".into(), Value::Object(outputs));
        }
    }
    Some(json!({ "meters": Value::Object(patch) }))
}

// ── Commands ─────────────────────────────────────────────────────────────

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn int(p: &Params, name: &str) -> Result<i64, CommandError> {
    p.get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(format!("'{name}' is required")))
}

fn num(p: &Params, name: &str) -> Result<f64, CommandError> {
    p.get(name)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .ok_or_else(|| invalid(format!("'{name}' is required")))
}

fn flag(p: &Params, name: &str) -> bool {
    p.get(name).and_then(Value::as_bool).unwrap_or(true)
}

fn word_param<'a>(p: &'a Params, name: &str) -> Result<&'a str, CommandError> {
    p.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{name}' is required")))
}

/// Free text (labels, preset names): no control characters, as it ends the
/// command text.
fn label<'a>(p: &'a Params, name: &str, max: usize) -> Result<&'a str, CommandError> {
    let text = word_param(p, name)?;
    if text.chars().any(char::is_control) || !text.is_ascii() {
        return Err(invalid(format!("'{name}' must be printable ASCII")));
    }
    if text.len() > max {
        return Err(invalid(format!("'{name}' is at most {max} characters")));
    }
    Ok(text)
}

fn b01(v: bool) -> u8 {
    v as u8
}

impl LakeDlm {
    fn module(&self, p: &Params) -> Result<(usize, &'static str), CommandError> {
        let m = word_param(p, "module")?;
        self.family
            .modules()
            .iter()
            .position(|x| *x == m)
            .map(|i| (i, self.family.modules()[i]))
            .ok_or_else(|| {
                invalid(format!(
                    "this frame has modules {}",
                    self.family.modules().join(", ")
                ))
            })
    }

    /// `<Module> <Ch>`, or `#<power channel>` (8.1, D Series & PLM+), and the
    /// state path of that output when it is known.
    fn output(&self, p: &Params) -> Result<(String, Option<(String, String)>), CommandError> {
        if let Some(pc) = p.get("power_channel").and_then(Value::as_i64) {
            if self.family != Family::Plus {
                return Err(invalid(
                    "power_channel addressing is for D Series and PLM+ only",
                ));
            }
            return Ok((format!("#{pc}"), None));
        }
        let (_, m) = self.module(p)?;
        let ch = int(p, "channel")?;
        Ok((format!("{m} {ch}"), Some((m.to_string(), ch.to_string()))))
    }

    /// `<Module>`, or `#<power channel>`.
    fn module_target(&self, p: &Params) -> Result<(String, Option<String>), CommandError> {
        if let Some(pc) = p.get("power_channel").and_then(Value::as_i64) {
            if self.family != Family::Plus {
                return Err(invalid(
                    "power_channel addressing is for D Series and PLM+ only",
                ));
            }
            return Ok((format!("#{pc}"), None));
        }
        let (_, m) = self.module(p)?;
        Ok((m.to_string(), Some(m.to_string())))
    }

    /// The text a command sends and what it expects back.
    fn command_text(&self, name: &str, p: &Params) -> Result<(String, Expect), CommandError> {
        let ack = |text: String| Ok((text, Expect::Ack(None)));
        let get = |text: String| Ok((text, Expect::Value));
        let set_out = |path: &str, value: String, field: &str, state: Value| {
            let (target, at) = self.output(p)?;
            let patch = at.map(|(m, c)| json!({"modules": {m: {"outputs": {c: {field: state}}}}}));
            Ok((format!("{path}={target} {value}"), Expect::Ack(patch)))
        };
        let set_in = |path: &str, value: String, field: &str, state: Value| {
            let (target, at) = self.module_target(p)?;
            let patch = at.map(|m| json!({"modules": {m: {"input": {field: state}}}}));
            Ok((format!("{path}={target} {value}"), Expect::Ack(patch)))
        };
        match name {
            // Frame
            "set_power" => {
                let on = flag(p, "powered");
                Ok((
                    format!("Dev.Power={}", b01(on)),
                    Expect::Power(Some(json!({"device": {"power": on}}))),
                ))
            }
            "get_power" => get("Dev.Power?".into()),
            "get_model_name" => get("Dev.ModelName?".into()),
            "get_bundle_version" => get("Dev.BundleVer?".into()),
            "get_hardware_id" => get("Dev.Network.ID?".into()),
            "get_preset_name" => get(format!("Dev.Preset.Name?{}", int(p, "preset")?)),
            "recall_preset" => Ok((
                format!("Dev.Preset.Recall!{}", int(p, "preset")?),
                Expect::Ack(None),
            )),
            "store_preset" => ack(format!(
                "Dev.Preset.Store!{} {}",
                int(p, "preset")?,
                label(p, "name", 64)?
            )),
            "set_frame_label" => {
                let l = label(p, "label", 30)?;
                Ok((
                    format!("Dev.FrameLabel={l}"),
                    Expect::Ack(Some(json!({"device": {"frame_label": l}}))),
                ))
            }
            "set_redundancy" => ack(format!("Dev.Network.Redund={}", b01(flag(p, "enabled")))),
            "factory_reset" => ack("Dev.Reset.Factory!".into()),
            "soft_reset" => ack("Dev.Reset.Soft!".into()),
            "reset_to_contour" => ack("Dev.Reset.Contour!".into()),
            "reset_to_mesa" => ack("Dev.Reset.Mesa!".into()),
            "set_iso_float" => ack(format!("Dev.IsoFloat={}", b01(flag(p, "grounded")))),
            "set_iso_float_inputs" => {
                ack(format!("Dev.IsoFloatInputs={}", b01(flag(p, "grounded"))))
            }
            "set_iso_float_outputs" => {
                ack(format!("Dev.IsoFloatOutputs={}", b01(flag(p, "grounded"))))
            }
            "set_aes_termination" => ack(format!(
                "Dev.AesLoopTermination={}",
                b01(flag(p, "terminated"))
            )),
            "set_breaker_current" => ack(format!(
                "Dev.Fuse.NominalCurrent={:.1}",
                num(p, "current_a")?
            )),
            "set_breaker_type" => {
                let t = match word_param(p, "type")? {
                    "conservative" => 0,
                    "fast" => 1,
                    "universal" => 2,
                    other => return Err(invalid(format!("unknown breaker type '{other}'"))),
                };
                ack(format!("Dev.Fuse.Type={t}"))
            }
            "set_latency_match" => {
                let on = flag(p, "enabled");
                Ok((
                    format!("Dev.LatencyMatch={}", b01(on)),
                    Expect::Ack(Some(json!({"device": {"latency_match": on}}))),
                ))
            }
            "get_latency" => get(format!("Dev.Latency?{}", int(p, "power_channel")?)),
            "get_route" => {
                let (_, m) = self.module(p)?;
                get(format!("Dev.Route?{m} {}", int(p, "channel")?))
            }
            "get_bridge_mode" => get(format!("Dev.BridgeMode?{}", int(p, "pair")?)),
            "set_load_speakers" => ack(format!(
                "Dev.Load.Speakers={} {}",
                int(p, "power_channel")?,
                int(p, "count")?
            )),
            "set_power_channel_attenuation" => {
                let pc = int(p, "power_channel")?;
                let db = num(p, "attenuation_db")?;
                if (db * 4.0).fract() != 0.0 {
                    return Err(invalid("attenuation_db is in 0.25 dB steps"));
                }
                Ok((
                    format!("Dev.Pwr.Attenuation={pc} {db:.2}"),
                    Expect::Ack(Some(
                        json!({"power_channels": {pc.to_string(): {"attenuation_db": db}}}),
                    )),
                ))
            }
            "set_power_channel_mute" => {
                let pc = int(p, "power_channel")?;
                let muted = flag(p, "muted");
                Ok((
                    format!("Dev.Pwr.Mute={pc} {}", b01(muted)),
                    Expect::Ack(Some(
                        json!({"power_channels": {pc.to_string(): {"mute": muted}}}),
                    )),
                ))
            }
            "set_gpi_config" => {
                let (closed, opened) = (
                    word_param(p, "closed_action")?,
                    word_param(p, "opened_action")?,
                );
                let group = |a: &str| match a {
                    "ToggleMute" | "Mute" | "Unmute" => Some(0),
                    "ToggleStandby" | "Standby" | "On" => Some(1),
                    "Recall99" | "Recall100" => Some(2),
                    _ => None,
                };
                if let (Some(a), Some(b)) = (group(closed), group(opened)) {
                    if a != b {
                        return Err(invalid(
                            "both actions must be of the same kind: mute, standby or preset recall",
                        ));
                    }
                }
                ack(format!(
                    "Dev.GPI.Config={} {closed} {opened}",
                    int(p, "gpi")?
                ))
            }
            "get_gpi_state" => get(format!("Dev.GPI.State?{}", int(p, "gpi")?)),
            "set_gpo_config" => ack(format!(
                "Dev.GPO.Config={} {}",
                int(p, "gpo")?,
                word_param(p, "indication")?
            )),
            "get_gpo_state" => get(format!("Dev.GPO.State?{}", int(p, "gpo")?)),
            "set_dante_break_in" => {
                let tx = int(p, "transmitter")?;
                let source = word_param(p, "source")?;
                let text = if source == "Probe" {
                    format!(
                        "Dev.Dante.BreakIn={tx} Probe {} {} {}",
                        int(p, "channel")?,
                        word_param(p, "probe")?,
                        int(p, "power_channel")?
                    )
                } else {
                    format!("Dev.Dante.BreakIn={tx} {source} {}", int(p, "channel")?)
                };
                ack(text)
            }
            "set_router_input" => {
                let mut text = format!(
                    "Dev.Router.InputTypSel={} {} {} {}",
                    int(p, "input")?,
                    int(p, "priority")?,
                    word_param(p, "type")?,
                    int(p, "channel")?
                );
                if let Some(s) = p.get("sensitivity").and_then(Value::as_f64) {
                    text.push_str(&format!(" {s:.2}"));
                }
                ack(text)
            }
            "get_router_input" => get(format!(
                "Dev.Router.InputTypSel?{} {}",
                int(p, "input")?,
                int(p, "priority")?
            )),
            "force_input_priority" => ack(format!(
                "Dev.Router.ForceInputPriority={} {}",
                int(p, "input")?,
                int(p, "priority")?
            )),
            "set_router_input_mute" => {
                let input = int(p, "input")?;
                let muted = flag(p, "muted");
                Ok((
                    format!("Dev.Router.InputMute={input} {}", b01(muted)),
                    Expect::Ack(Some(
                        json!({"router_inputs": {input.to_string(): {"mute": muted}}}),
                    )),
                ))
            }
            "get_input_sample_rate" => get(format!(
                "Dev.Route.InputSR?{} {}",
                word_param(p, "type")?,
                int(p, "channel")?
            )),
            "get_meters" => {
                let (text, kind) = self.family.meters();
                Ok((text.into(), Expect::Meters(kind)))
            }
            "get_no_faults" => get("Dev.MD.NoFaults?".into()),
            "set_load_pilot" => ack(format!(
                "Dev.LoadPilot.Enable={} {}",
                int(p, "power_channel")?,
                b01(flag(p, "enabled"))
            )),
            "get_load_pilot_readings" => get(format!(
                "Dev.LoadPilot.Readings?{}",
                int(p, "power_channel")?
            )),
            "set_load_pilot_signal" => ack(format!(
                "Dev.LoadPilot.Signal={} 10 24000 {:.3} {:.3}",
                int(p, "power_channel")?,
                num(p, "tone_1_amplitude_v")?,
                num(p, "tone_2_amplitude_v")?
            )),
            "set_load_pilot_thresholds" => ack(format!(
                "Dev.LoadPilot.Threshold={} {:.2} {:.2} {:.2} {:.2}",
                int(p, "power_channel")?,
                num(p, "tone_1_lower_ohm")?,
                num(p, "tone_2_lower_ohm")?,
                num(p, "tone_1_upper_ohm")?,
                num(p, "tone_2_upper_ohm")?
            )),
            "set_pilot_tone" => {
                // PLM 20000Q uses the second pilot tone generator (PTG2).
                let ptg = if self.model == "plm-20000q" {
                    "PTG2"
                } else {
                    "PTG"
                };
                ack(format!(
                    "Dev.{ptg}.Active={} {}",
                    int(p, "power_channel")?,
                    b01(flag(p, "active"))
                ))
            }
            "get_pilot_tone_impedance" => {
                let ptg = if self.model == "plm-20000q" {
                    "PTG2"
                } else {
                    "PTG"
                };
                get(format!("Dev.{ptg}.Impedance?{}", int(p, "power_channel")?))
            }

            // Module outputs
            "set_output_mute" => {
                let m = flag(p, "muted");
                set_out("Mod.Out.Mute", b01(m).to_string(), "mute", json!(m))
            }
            "set_output_gain" => {
                let g = num(p, "gain_db")?;
                set_out("Mod.Out.Gain", format!("{g:.2}"), "gain_db", json!(g))
            }
            "set_output_delay" => {
                let d = num(p, "delay_ms")?;
                set_out("Mod.Out.Delay", format!("{d:.2}"), "delay_ms", json!(d))
            }
            "set_output_polarity" => {
                let positive = !flag(p, "inverted");
                set_out(
                    "Mod.Out.Phase",
                    b01(positive).to_string(),
                    "polarity_positive",
                    json!(positive),
                )
            }
            "set_output_label" => {
                let l = label(p, "label", 32)?;
                set_out("Mod.Out.Label", l.to_string(), "label", json!(l))
            }
            "set_output_max_rms_level" => {
                let v = num(p, "level_db")?;
                set_out(
                    "Mod.Out.MaxRMSLvl",
                    format!("{v:.2}"),
                    "max_rms_level_db",
                    json!(v),
                )
            }
            "set_output_max_rms_corner" => {
                let v = num(p, "corner_db")?;
                set_out(
                    "Mod.Out.MaxRMSCor",
                    format!("{v:.2}"),
                    "max_rms_corner_db",
                    json!(v),
                )
            }
            "set_output_max_rms_attack" => {
                let v = num(p, "attack_ms")?;
                set_out(
                    "Mod.Out.MaxRMSAtk",
                    format!("{v:.2}"),
                    "max_rms_attack_ms",
                    json!(v),
                )
            }
            "set_output_max_rms_release" => {
                let v = num(p, "release_ms")?;
                set_out(
                    "Mod.Out.MaxRMSRel",
                    format!("{v:.2}"),
                    "max_rms_release_ms",
                    json!(v),
                )
            }
            "set_output_max_peak_level" => {
                let v = num(p, "level_db")?;
                set_out(
                    "Mod.Out.MaxPeakLvl",
                    format!("{v:.2}"),
                    "max_peak_level_db",
                    json!(v),
                )
            }
            "set_output_amp_gain" => {
                let v = int(p, "gain_db")?;
                set_out("Mod.Out.AmpGain", v.to_string(), "amp_gain_db", json!(v))
            }
            "set_output_vpl" => {
                let v = num(p, "vpl_v")?;
                set_out("Mod.Out.AmpVPL", format!("{v:.1}"), "vpl_v", json!(v))
            }
            "set_output_vpl_profile" => {
                let profile = match word_param(p, "profile")? {
                    "universal" => 0,
                    "sub_lf" => 1,
                    "sub" => 2,
                    "lf" => 3,
                    "mf" => 4,
                    "hf" => 5,
                    other => return Err(invalid(format!("unknown VPL profile '{other}'"))),
                };
                set_out(
                    "Mod.Out.VPLProfile",
                    profile.to_string(),
                    "vpl_profile",
                    json!(profile),
                )
            }
            "get_output_parameter" => {
                let (target, _) = self.output(p)?;
                get(format!("Mod.Out.{}?{target}", word_param(p, "parameter")?))
            }
            "get_output_channels" => {
                let (_, m) = self.module(p)?;
                get(format!("Mod.Out.Chans?{m}"))
            }

            // Module inputs and modules
            "set_input_mute" => {
                let m = flag(p, "muted");
                set_in("Mod.In.Mute", b01(m).to_string(), "mute", json!(m))
            }
            "set_input_gain" => {
                let g = num(p, "gain_db")?;
                set_in("Mod.In.Gain", format!("{g:.2}"), "gain_db", json!(g))
            }
            "set_input_delay" => {
                let d = num(p, "delay_ms")?;
                set_in("Mod.In.Delay", format!("{d:.2}"), "delay_ms", json!(d))
            }
            "set_input_polarity" => {
                let positive = !flag(p, "inverted");
                set_in(
                    "Mod.In.Phase",
                    b01(positive).to_string(),
                    "polarity_positive",
                    json!(positive),
                )
            }
            "set_input_label" => {
                let l = label(p, "label", 32)?;
                set_in("Mod.In.Label", l.to_string(), "label", json!(l))
            }
            "set_input_mixer_gain" => {
                let (target, _) = self.module_target(p)?;
                ack(format!(
                    "Mod.In.MixerGain={target} {} {:.2}",
                    int(p, "router")?,
                    num(p, "gain_db")?
                ))
            }
            "set_module_label" => {
                let (target, at) = self.module_target(p)?;
                let l = label(p, "label", 32)?;
                let patch = at.map(|m| json!({"modules": {m: {"label": l}}}));
                Ok((format!("Mod.Mod.Label={target} {l}"), Expect::Ack(patch)))
            }
            "set_module_selected" => {
                let (target, _) = self.module_target(p)?;
                ack(format!(
                    "Mod.Mod.Selected={target} {}",
                    b01(flag(p, "selected"))
                ))
            }

            // Anything else in the document
            "query" => {
                let path = word_param(p, "path")?;
                let args = p.get("args").and_then(Value::as_str).unwrap_or("");
                get(format!("{path}?{args}"))
            }
            "send_message" => {
                let text = label(p, "message", 500)?;
                let expect = if text.contains('?') {
                    Expect::Value
                } else {
                    Expect::Ack(None)
                };
                Ok((text.to_string(), expect))
            }
            other => Err(CommandError::UnknownCommand {
                command: other.into(),
            }),
        }
    }
}

impl Module for LakeDlm {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.udp_open(SOCKET, self.bind);
        self.socket_open = true;
        self.probe(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.command_text(name, params) {
            Ok((text, expect)) => {
                if text.len() + 1 > MAX_PACKET - HEADER - FOOTER {
                    cx.complete(
                        id,
                        Err(invalid("the command is longer than a DLM packet holds")),
                    );
                    return;
                }
                let timeout = if name == "recall_preset" || name == "store_preset" {
                    SLOW_REPLY_TIMEOUT
                } else {
                    REPLY_TIMEOUT
                };
                self.commands.push_back(Request {
                    text,
                    why: Why::Command { id, expect },
                    timeout,
                });
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        if from.ip() != self.device.ip() {
            return;
        }
        let Some((msg_id, reply)) = decode(data) else {
            return;
        };
        let matches = self.in_flight.as_ref().is_some_and(|f| f.msg_id == msg_id);
        if !matches {
            return;
        }
        let InFlight { request, sent, .. } = self.in_flight.take().expect("checked");
        cx.cancel_timer(REPLY);
        self.reply(cx, request, sent, reply);
        self.pump(cx);
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        cx.log(Level::Warning, format!("DLM socket: {message}"));
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            REPLY => {
                let Some(flight) = self.in_flight.take() else {
                    return;
                };
                if let Why::Command { id, .. } = flight.request.why {
                    cx.complete(id, Err(CommandError::Timeout));
                }
                self.misses += 1;
                if !self.connected {
                    // The probe went unanswered: try again shortly.
                    cx.set_timer(RETRY, RETRY_AFTER);
                    return;
                }
                if self.misses >= MISSES_TO_LOSE {
                    self.lost(cx, "no answer to three requests in a row");
                    return;
                }
                self.pump(cx);
            }
            RETRY => {
                if !self.connected && self.in_flight.is_none() {
                    self.probe(cx);
                }
            }
            LIVENESS => {
                if !self.connected {
                    return;
                }
                let idle =
                    self.in_flight.is_none() && self.commands.is_empty() && self.reads.is_empty();
                if idle && cx.now().saturating_sub(self.last_heard) >= QUIET_AFTER {
                    self.reads.push_back(Request {
                        text: "Dev.Power?".into(),
                        why: Why::Liveness,
                        timeout: REPLY_TIMEOUT,
                    });
                    self.pump(cx);
                }
                cx.set_timer(LIVENESS, LIVENESS_EVERY);
            }
            METERS => {
                if !self.connected {
                    return;
                }
                self.meters();
                self.pump(cx);
                cx.set_timer(METERS, self.meter_poll);
            }
            PARAMETERS => {
                if !self.connected {
                    return;
                }
                // A round still being read (or the identity, which gives the
                // channel counts) is not doubled: look again shortly.
                let pending = self
                    .reads
                    .iter()
                    .any(|r| matches!(r.why, Why::Read { .. } | Why::Channels { .. }));
                if pending {
                    cx.set_timer(PARAMETERS, 500);
                    return;
                }
                self.parameters();
                self.pump(cx);
                if self.parameter_poll > 0 {
                    cx.set_timer(PARAMETERS, self.parameter_poll);
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.udp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Action, CommandResult};
    use std::net::{IpAddr, Ipv4Addr};

    const DEVICE: Ipv4Addr = Ipv4Addr::new(169, 254, 10, 20);
    const FRAME: (u32, u32) = (0x3d00_0011, 0xd6ed_9201);

    fn context(model: &str, monitor: bool, settings: Value) -> OpenContext {
        OpenContext {
            host: IpAddr::V4(DEVICE),
            host_name: None,
            port: None,
            model: model.into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        }
    }

    fn sent(actions: &[Action]) -> Vec<(SocketAddr, Vec<u8>)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { to, data, .. } => Some((*to, data.clone())),
                _ => None,
            })
            .collect()
    }

    fn text_of(packet: &[u8]) -> String {
        let payload = &packet[HEADER..packet.len() - FOOTER];
        String::from_utf8(payload[..payload.len() - 1].to_vec()).unwrap()
    }

    fn msg_id(packet: &[u8]) -> u32 {
        u32::from_le_bytes([packet[24], packet[25], packet[26], packet[27]])
    }

    fn answer(msg_id: u32, payload: &[u8]) -> Vec<u8> {
        let mut p = encode(FRAME, SOURCE_ID, HOST_CLASS, DLM_MSG, msg_id, payload);
        // The device's packet carries its own class; only ids matter here.
        p[16..18].copy_from_slice(&DEVICE_CLASS.to_le_bytes());
        p
    }

    fn ack(msg_id: u32, result: i32) -> Vec<u8> {
        let mut p = encode(FRAME, SOURCE_ID, HOST_CLASS, ACK_MSG, msg_id, &[]);
        // Payload is the 32-bit result, not a text.
        p.truncate(HEADER);
        p.extend_from_slice(&result.to_le_bytes());
        p.extend_from_slice(&[0; FOOTER]);
        let len = p.len() as u16;
        p[20..22].copy_from_slice(&len.to_le_bytes());
        p
    }

    fn from() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(DEVICE), DYNAMIC_PORT)
    }

    fn completed(actions: &[Action]) -> Vec<(CommandId, CommandResult)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Complete { id, result } => Some((*id, result.clone())),
                _ => None,
            })
            .collect()
    }

    fn state(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    /// Start and answer the frame id probe.
    fn found(model: &str, monitor: bool) -> (LakeDlm, Vec<Action>) {
        let mut m = LakeDlm::new(context(model, monitor, json!({}))).unwrap();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let probe = sent(&cx.take()).remove(0).1;
        let mut cx = Cx::new(5);
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &answer(msg_id(&probe), b"3d000011:d6ed9201\0"),
        );
        (m, cx.take())
    }

    #[test]
    fn packets_follow_the_documented_layout() {
        // Appendix D's broadcast "Dev.Power=1" with no answer asked, from
        // source 0:1: the bytes the document gives.
        let p = encode(
            (0, 1),
            BROADCAST_ID,
            BROADCAST_CLASS,
            DLM_MSG,
            u32::MAX,
            b"Dev.Power=1",
        );
        let expected: Vec<u8> = [
            "00000000",
            "01000000",
            "FEFFFFFF",
            "FDFFFFFF",
            "0600",
            "0000",
            "2C00",
            "BD02",
            "FFFFFFFF",
            "4465762E506F7765723D3100",
            "00000000",
        ]
        .concat()
        .as_bytes()
        .chunks(2)
        .map(|h| u8::from_str_radix(std::str::from_utf8(h).unwrap(), 16).unwrap())
        .collect();
        assert_eq!(p, expected);
    }

    #[test]
    fn the_frame_id_is_asked_for_and_then_used() {
        let mut m = LakeDlm::new(context("d-80-4l", true, json!({}))).unwrap();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Ephemeral
        }));
        let (to, probe) = sent(&a).remove(0);
        assert_eq!(to, from());
        assert_eq!(text_of(&probe), "Dev.Network.ID?");
        // Broadcast id and class, unicast to the address.
        assert_eq!(
            &probe[8..16],
            &[0xFE, 0xFF, 0xFF, 0xFF, 0xFD, 0xFF, 0xFF, 0xFF]
        );
        assert_eq!(&probe[18..20], &[0, 0]);

        let mut cx = Cx::new(7);
        m.datagram(
            &mut cx,
            SOCKET,
            from(),
            &answer(msg_id(&probe), b"3d000011:d6ed9201\0"),
        );
        let a = cx.take();
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(7)));
        assert!(state(&a).contains(&json!({"device": {"frame_id": "3d000011:d6ed9201"}})));
        // Now addressed to the frame, class 5, with the next message id.
        let (_, next) = sent(&a).remove(0);
        assert_eq!(&next[8..16], &[0x11, 0, 0, 0x3d, 0x01, 0x92, 0xed, 0xd6]);
        assert_eq!(&next[18..20], &[5, 0]);
        assert_eq!(msg_id(&next), msg_id(&probe) + 1);
        assert_eq!(text_of(&next), "Dev.ModelName?");
    }

    #[test]
    fn a_given_frame_id_and_fixed_port_are_used() {
        let mut m = LakeDlm::new(context(
            "plm-20k44",
            true,
            json!({"frame_id": "3d000011:d6ed9201", "response_port": "fixed"}),
        ))
        .unwrap();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Shared(6004)
        }));
        let (to, probe) = sent(&a).remove(0);
        assert_eq!(to.port(), 6015);
        assert_eq!(&probe[8..12], &FRAME.0.to_le_bytes());
        assert!(LakeDlm::new(context("plm-20k44", true, json!({"frame_id": "nope"}))).is_err());
    }

    #[test]
    fn sets_are_acknowledged_and_update_state() {
        let (mut m, _) = found("d-120-4l", false);
        let mut cx = Cx::new(10);
        let p = json!({"module": "B", "channel": 2, "gain_db": -3.0});
        m.command(&mut cx, 1, "set_output_gain", p.as_object().unwrap());
        let (_, pkt) = sent(&cx.take()).remove(0);
        assert_eq!(text_of(&pkt), "Mod.Out.Gain=B 2 -3.00");
        let mut cx = Cx::new(14);
        m.datagram(&mut cx, SOCKET, from(), &ack(msg_id(&pkt), ACK_SUCCESS));
        let a = cx.take();
        assert_eq!(completed(&a), [(1, Ok(Outcome::Ack))]);
        assert!(a.contains(&Action::RoundTrip(4)));
        assert!(
            state(&a).contains(&json!({"modules": {"B": {"outputs": {"2": {"gain_db": -3.0}}}}}))
        );

        // By power channel (D Series & PLM+), and a refusal.
        let mut cx = Cx::new(20);
        let p = json!({"power_channel": 3, "muted": true});
        m.command(&mut cx, 2, "set_output_mute", p.as_object().unwrap());
        let (_, pkt) = sent(&cx.take()).remove(0);
        assert_eq!(text_of(&pkt), "Mod.Out.Mute=#3 1");
        let mut cx = Cx::new(25);
        m.datagram(&mut cx, SOCKET, from(), &ack(msg_id(&pkt), -6));
        assert!(matches!(
            &completed(&cx.take())[0].1,
            Err(CommandError::DeviceError { code: Some(c), .. }) if c == "-6"
        ));

        // Power: "already on" is success.
        let mut cx = Cx::new(30);
        m.command(
            &mut cx,
            3,
            "set_power",
            json!({"powered": true}).as_object().unwrap(),
        );
        let (_, pkt) = sent(&cx.take()).remove(0);
        assert_eq!(text_of(&pkt), "Dev.Power=1");
        let mut cx = Cx::new(31);
        m.datagram(&mut cx, SOCKET, from(), &ack(msg_id(&pkt), ACK_ALREADY_ON));
        assert_eq!(completed(&cx.take()), [(3, Ok(Outcome::Ack))]);
    }

    #[test]
    fn gets_return_the_value_with_or_without_an_echo() {
        let (mut m, _) = found("d-80-4l", false);
        for (n, reply) in [
            "-3.00 -3.00 -100.00 20.00",
            "Mod.Out.Gain=A 1 -3.00 -3.00 -100.00 20.00",
        ]
        .iter()
        .enumerate()
        {
            let mut cx = Cx::new(10);
            let p = json!({"module": "A", "channel": 1, "parameter": "Gain"});
            m.command(
                &mut cx,
                n as u64,
                "get_output_parameter",
                p.as_object().unwrap(),
            );
            let (_, pkt) = sent(&cx.take()).remove(0);
            assert_eq!(text_of(&pkt), "Mod.Out.Gain?A 1");
            let mut cx = Cx::new(12);
            let mut payload = reply.as_bytes().to_vec();
            payload.push(0);
            m.datagram(&mut cx, SOCKET, from(), &answer(msg_id(&pkt), &payload));
            assert_eq!(
                completed(&cx.take()),
                [(
                    n as u64,
                    Ok(Outcome::Value {
                        value: json!("-3.00 -3.00 -100.00 20.00")
                    })
                )]
            );
        }
        assert_eq!(
            parse_value("-3.00 -3.00 -100.00 20.00", Parse::Number),
            Some(json!(-3.0))
        );
    }

    #[test]
    fn monitored_it_reads_identity_then_parameters_and_meters() {
        let (mut m, a) = found("d-80-4l", true);
        assert!(a.contains(&Action::SetTimer {
            key: METERS,
            after: 1_000
        }));
        assert!(a.contains(&Action::SetTimer {
            key: PARAMETERS,
            after: 0
        }));
        // Identity reads go one at a time, each after the last answer.
        let mut texts = vec![text_of(&sent(&a)[0].1)];
        let mut last = sent(&a)[0].1.clone();
        for t in 0..20 {
            let mut cx = Cx::new(10 + t);
            let reply: &[u8] = if text_of(&last).starts_with("Mod.Out.Chans") {
                b"2\0"
            } else {
                b"x\0"
            };
            m.datagram(&mut cx, SOCKET, from(), &answer(msg_id(&last), reply));
            let a = cx.take();
            let Some((_, next)) = sent(&a).into_iter().next() else {
                break;
            };
            texts.push(text_of(&next));
            last = next;
        }
        assert_eq!(
            texts,
            [
                "Dev.ModelName?",
                "Dev.BundleVer?",
                "Dev.Network.IPAddr?",
                "Dev.Network.MACAddr?",
                "Dev.Dante.Enabled?",
                "Mod.Out.Chans?A",
                "Mod.Out.Chans?B",
                "Mod.Out.Chans?C",
                "Mod.Out.Chans?D"
            ]
        );
        // The parameter round then reads each module's two outputs.
        let mut cx = Cx::new(100);
        m.timer(&mut cx, PARAMETERS);
        let first = sent(&cx.take()).remove(0).1;
        assert_eq!(text_of(&first), "Dev.Power?");
        assert!(m.reads.iter().any(|r| r.text == "Mod.Out.Gain?D 2"));
        assert!(!m.reads.iter().any(|r| r.text == "Mod.Out.Gain?D 3"));
        let mut cx = Cx::new(105);
        m.datagram(&mut cx, SOCKET, from(), &answer(msg_id(&first), b"1\0"));
        assert!(state(&cx.take()).contains(&json!({"device": {"power": true}})));
    }

    #[test]
    fn opened_for_commands_only_it_polls_nothing_but_checks_liveness() {
        let (mut m, a) = found("d-80-4l", false);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(sent(&a).is_empty());
        assert!(!a.iter().any(|x| matches!(
            x,
            Action::SetTimer {
                key: METERS | PARAMETERS,
                ..
            }
        )));
        let mut cx = Cx::new(5 + QUIET_AFTER);
        m.timer(&mut cx, LIVENESS);
        let (_, pkt) = sent(&cx.take()).remove(0);
        assert_eq!(text_of(&pkt), "Dev.Power?");
        let mut cx = Cx::new(5 + QUIET_AFTER + 3);
        m.datagram(&mut cx, SOCKET, from(), &answer(msg_id(&pkt), b"0\0"));
        let a = cx.take();
        assert!(a.contains(&Action::RoundTrip(3)));
        assert!(state(&a).contains(&json!({"device": {"power": false}})));
    }

    #[test]
    fn three_unanswered_requests_lose_the_device() {
        let (mut m, _) = found("lm-26", false);
        for n in 0..3 {
            let mut cx = Cx::new(100 + n);
            m.command(&mut cx, n, "get_power", &Params::new());
            cx.take();
            let mut cx = Cx::new(2_000 + n);
            m.timer(&mut cx, REPLY);
            let a = cx.take();
            assert_eq!(completed(&a), [(n, Err(CommandError::Timeout))]);
            let lost = a
                .iter()
                .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. })));
            assert_eq!(lost, n == 2);
        }
        let mut cx = Cx::new(3_000);
        m.command(&mut cx, 9, "get_power", &Params::new());
        assert_eq!(
            completed(&cx.take()),
            [(9, Err(CommandError::NotConnected))]
        );
    }

    #[test]
    fn modules_are_checked_against_the_family() {
        let (mut m, _) = found("plm-10000q", false);
        let mut cx = Cx::new(10);
        let p = json!({"module": "C", "channel": 1, "muted": true});
        m.command(&mut cx, 1, "set_output_mute", p.as_object().unwrap());
        assert!(matches!(
            completed(&cx.take())[0].1,
            Err(CommandError::InvalidParams { .. })
        ));
        let mut cx = Cx::new(11);
        m.command(
            &mut cx,
            2,
            "set_pilot_tone",
            json!({"power_channel": 2, "active": true})
                .as_object()
                .unwrap(),
        );
        assert_eq!(text_of(&sent(&cx.take())[0].1), "Dev.PTG.Active=2 1");
    }

    #[test]
    fn version_3_meters_decode() {
        let mut d = vec![0u8; 108];
        // Amp status: power on, protect.
        d[0..4].copy_from_slice(&0x0081u32.to_le_bytes());
        // Channel A status: short circuit, temperature warning; meters.
        d[4..8].copy_from_slice(&((1u32 << 29) | (1 << 27) | 0xFE).to_le_bytes());
        d[8..12].copy_from_slice(&u32::from_le_bytes([10, 0xFF, 0x01, 0]).to_le_bytes());
        // uStatus: link 1, Dante leader.
        d[60..64].copy_from_slice(&((1u32) | (1 << 12)).to_le_bytes());
        d[65] = 0b0011; // module B routers 1 and 2 connected
        d[68] = 0b1000_0001; // module A output 1 and input clip
        d[72] = 0xFE; // physical input 1 peak -0.5 dBFS
        d[96] = 34; // output 1 peak gain reduction 3.4 dB
        let patch = decode_meters(Meters::V3, &d).unwrap();
        let mtr = &patch["meters"];
        assert_eq!(mtr["amp"]["power_on"], true);
        assert_eq!(mtr["amp"]["protect"], true);
        let a = &mtr["power_channels"]["1"];
        assert_eq!(a["short_circuit"], true);
        assert_eq!(a["temperature"], "warning");
        assert_eq!(a["voltage_rms_db"], -0.5);
        assert_eq!(a["gain_reduction_db"], 1.0);
        assert_eq!(a["power_db"], -128.0);
        assert_eq!(a["voltage_db"], -127.0);
        assert_eq!(a["current_db"], 0.0);
        assert_eq!(mtr["status"]["dante_leader"], true);
        assert_eq!(
            mtr["modules"]["B"]["router_connected"],
            json!([true, true, false, false])
        );
        assert_eq!(mtr["modules"]["A"]["clip"]["outputs"][0], true);
        assert_eq!(mtr["modules"]["A"]["clip"]["input"], true);
        assert_eq!(mtr["inputs"]["1"]["peak_dbfs"], -0.5);
        assert_eq!(mtr["inputs"]["2"]["peak_dbfs"], -128.0);
        assert_eq!(mtr["outputs"]["1"]["gain_reduction_peak_db"], 3.4);
        assert!(decode_meters(Meters::V3, &d[..100]).is_none());
    }

    #[test]
    fn heartbeats_and_strangers_are_ignored() {
        let (mut m, _) = found("d-80-4l", false);
        let mut cx = Cx::new(10);
        m.command(&mut cx, 1, "get_power", &Params::new());
        let (_, pkt) = sent(&cx.take()).remove(0);
        // A broadcast with the same message id is not the answer.
        let mut hb = encode(FRAME, BROADCAST_ID, 0, 4, msg_id(&pkt), b"");
        hb[22..24].copy_from_slice(&DLM_MSG.to_le_bytes());
        let mut cx = Cx::new(11);
        m.datagram(&mut cx, SOCKET, from(), &hb);
        // Nor is an answer from another address.
        let other = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(169, 254, 10, 21)), DYNAMIC_PORT);
        m.datagram(&mut cx, SOCKET, other, &answer(msg_id(&pkt), b"1\0"));
        assert!(completed(&cx.take()).is_empty());
    }

    #[test]
    fn frame_ids_parse() {
        assert_eq!(parse_frame_id("3d000011:d6ed9201"), Some(FRAME));
        assert_eq!(parse_frame_id("3D000011:D6ED9201"), Some(FRAME));
        assert_eq!(parse_frame_id("3d0011:d6ed9201"), None);
        assert_eq!(strip_echo("Dev.Power?", "Dev.Power=1"), "1");
        assert_eq!(strip_echo("Dev.Power?", "1"), "1");
    }
}
