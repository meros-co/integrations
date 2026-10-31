//! Sony BRAVIA Professional Displays over their RS-232C serial control
//! protocol, reached through a serial-to-Ethernet converter, from the BRAVIA
//! Professional Displays Knowledge Center's "Serial Control" pages.
//!
//! - A control is 0x8C 0x00, the function, a length (the data bytes plus the
//!   checksum), the data and a checksum; a query is 0x83 0x00, the function,
//!   0xFF 0xFF and a checksum. The checksum is the low byte of the sum of the
//!   bytes before it.
//! - The display answers 0x70 and a status (0x00 completed, 0x01 above the
//!   upper limit, 0x02 below the lower limit, 0x03 canceled, 0x04 parse
//!   error); a completed query adds a size (its data bytes plus the checksum)
//!   and the data; a checksum ends every answer. How long an answer is
//!   depends on the request, so requests go one at a time and each answer is
//!   read against the one in flight.
//! - Nothing is pushed: monitored, power, input, volume and muting are asked
//!   every 5 s (the last three only while the display is on) and the ID and
//!   product information on connecting.
//!
//! Opened for commands only (`monitor` false), nothing is asked on connecting
//! and nothing polled; a power query every 10 s while idle is the liveness
//! check, and what answers carry still reaches state.

use std::collections::VecDeque;

use crate::{json, value::Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    PollStamps, TcpInput,
};

const SOCKET: Key = "bravia";
const REPLY_TIMER: Key = "reply";
const POLL_TIMER: Key = "poll";
const RETRY_TIMER: Key = "retry";

const CONTROL: u8 = 0x8C;
const QUERY: u8 = 0x83;
const ANSWER: u8 = 0x70;

const REPLY_TIMEOUT: Millis = 3_000;
const POLL_EVERY: Millis = 5_000;
const PROBE_EVERY: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

const POWER: u8 = 0x00;
const INPUT: u8 = 0x02;
const VOLUME: u8 = 0x05;
const MUTING: u8 = 0x06;
const ID: u8 = 0x6F;

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

/// A control frame: header, category, function, length, data, checksum.
pub(crate) fn control(function: u8, data: &[u8]) -> Vec<u8> {
    let mut out = vec![CONTROL, 0x00, function, data.len() as u8 + 1];
    out.extend_from_slice(data);
    out.push(checksum(&out));
    out
}

/// A query frame.
pub(crate) fn query(function: u8) -> Vec<u8> {
    let mut out = vec![QUERY, 0x00, function, 0xFF, 0xFF];
    out.push(checksum(&out));
    out
}

#[derive(Debug, Clone, PartialEq)]
enum Answer {
    Status(u8),
    Data(Vec<u8>),
}

fn status_text(status: u8) -> &'static str {
    match status {
        0x01 => "the value is above the display's upper limit",
        0x02 => "the value is below the display's lower limit",
        0x03 => "the display canceled the command (not possible in its current state)",
        0x04 => "the display could not parse the command",
        _ => "the display answered an unknown status",
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Read {
    Power,
    Input,
    Volume,
    Muting,
    Id,
    ProductInfo(u8),
    Raw,
    /// A control: only the status matters.
    None,
}

#[derive(Debug, Clone)]
struct Job {
    command: Option<CommandId>,
    frame: Vec<u8>,
    is_query: bool,
    read: Read,
    /// After a control, read this function back.
    then: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Connecting,
    Ready,
}

pub(crate) struct SonyBraviaSerial {
    to: std::net::SocketAddr,
    monitor: bool,
    /// When each poll was last answered (`polled.<group>`, SPEC.md §8).
    stamps: PollStamps,
    phase: Phase,
    connected: bool,
    on: bool,
    buf: Vec<u8>,
    commands: VecDeque<Job>,
    polls: VecDeque<Job>,
    in_flight: Option<Job>,
    sent_at: Millis,
    retry_after: Millis,
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn int(p: &Params, key: &str) -> Result<u8, CommandError> {
    p.get(key)
        .and_then(Value::as_u64)
        .filter(|v| *v <= 255)
        .map(|v| v as u8)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn text<'a>(p: &'a Params, key: &str) -> Result<&'a str, CommandError> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn flag(p: &Params, key: &str) -> Result<bool, CommandError> {
    p.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn pick(p: &Params, key: &str, table: &[(&str, u8)]) -> Result<u8, CommandError> {
    let v = text(p, key)?;
    table
        .iter()
        .find(|(name, _)| *name == v)
        .map(|(_, code)| *code)
        .ok_or_else(|| invalid(format!("'{key}' has no value '{v}'")))
}

const UP_DOWN: &[(&str, u8)] = &[("up", 0x00), ("down", 0x01)];
const SIGN: &[(&str, u8)] = &[("plus", 0x00), ("minus", 0x01)];
const INPUTS: &[(&str, u8)] = &[
    ("video", 0x02),
    ("component", 0x03),
    ("hdmi", 0x04),
    ("pc", 0x05),
    ("shared", 0x07),
];
const PICTURE_MODES: &[(&str, u8)] = &[
    ("vivid", 0x00),
    ("standard", 0x01),
    ("cinema", 0x02),
    ("custom", 0x03),
    ("game", 0x08),
    ("graphics", 0x09),
];
const SOUND_MODES: &[(&str, u8)] = &[
    ("standard", 0x01),
    ("cinema", 0x04),
    ("sports", 0x05),
    ("music", 0x06),
    ("dialogue", 0x08),
];
const WIDE: &[(&str, u8)] = &[
    ("wide_zoom", 0x00),
    ("full", 0x01),
    ("zoom", 0x02),
    ("normal", 0x03),
    ("pc_normal", 0x05),
    ("pc_full1", 0x06),
    ("pc_full2", 0x07),
];
const FOUR_THREE: &[(&str, u8)] = &[("normal", 0x04), ("wide_zoom", 0x03), ("off", 0x00)];
/// SIRCS emulation: (button, category, data).
const KEYS: &[(&str, u8, u8)] = &[
    ("input", 0x01, 0x25),
    ("power", 0x01, 0x15),
    ("wide_mode", 0xA4, 0x3D),
    ("dot", 0x97, 0x1D),
    ("display", 0x01, 0x3A),
    ("return", 0x97, 0x23),
    ("options", 0x97, 0x36),
    ("home", 0x01, 0x60),
    ("up", 0x01, 0x74),
    ("down", 0x01, 0x75),
    ("left", 0x01, 0x34),
    ("right", 0x01, 0x33),
    ("select", 0x01, 0x65),
    ("num1", 0x01, 0x00),
    ("num2", 0x01, 0x01),
    ("num3", 0x01, 0x02),
    ("num4", 0x01, 0x03),
    ("num5", 0x01, 0x04),
    ("num6", 0x01, 0x05),
    ("num7", 0x01, 0x06),
    ("num8", 0x01, 0x07),
    ("num9", 0x01, 0x08),
    ("num0", 0x01, 0x09),
    ("cc", 0xA4, 0x10),
    ("volume_up", 0x01, 0x12),
    ("volume_down", 0x01, 0x13),
    ("muting", 0x01, 0x14),
    ("channel_up", 0x01, 0x10),
    ("channel_down", 0x01, 0x11),
    ("jump", 0x01, 0x3B),
];

impl SonyBraviaSerial {
    pub(crate) fn new(ctx: OpenContext) -> Result<SonyBraviaSerial, String> {
        if ctx.port.is_none() {
            return Err(
                "a serial-to-Ethernet converter has no standard port: give the converter's port"
                    .into(),
            );
        }
        let to = crate::module::tcp_address(&ctx, 0)?;
        Ok(SonyBraviaSerial {
            to,
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            phase: Phase::Closed,
            connected: false,
            on: false,
            buf: Vec::new(),
            commands: VecDeque::new(),
            polls: VecDeque::new(),
            in_flight: None,
            sent_at: 0,
            retry_after: RETRY_MIN,
        })
    }

    fn query_job(function: u8, read: Read) -> Job {
        Job {
            command: None,
            frame: query(function),
            is_query: true,
            read,
            then: None,
        }
    }

    fn control_job(function: u8, data: &[u8], then: Option<u8>) -> Job {
        Job {
            command: None,
            frame: control(function, data),
            is_query: false,
            read: Read::None,
            then,
        }
    }

    fn read_of(function: u8) -> Read {
        match function {
            POWER => Read::Power,
            INPUT => Read::Input,
            VOLUME => Read::Volume,
            MUTING => Read::Muting,
            _ => Read::Raw,
        }
    }

    fn command_job(name: &str, p: &Params) -> Result<Job, CommandError> {
        let set = |f: u8, data: &[u8]| Ok(Self::control_job(f, data, None));
        let set_read = |f: u8, data: &[u8]| Ok(Self::control_job(f, data, Some(f)));
        let get = |f: u8| Ok(Self::query_job(f, Self::read_of(f)));
        match name {
            "set_power" => set_read(POWER, &[u8::from(flag(p, "power_on")?)]),
            "get_power" => get(POWER),
            "set_standby_command" => set(0x01, &[u8::from(flag(p, "enabled")?)]),
            "set_input" => {
                let kind = pick(p, "kind", INPUTS)?;
                set_read(INPUT, &[kind, int(p, "number")?])
            }
            "toggle_input" => set_read(INPUT, &[0x00]),
            "get_input" => get(INPUT),
            "set_volume" => set_read(VOLUME, &[0x01, int(p, "volume")?]),
            "step_volume" => set_read(VOLUME, &[0x00, pick(p, "direction", UP_DOWN)?]),
            "get_volume" => get(VOLUME),
            "set_mute" => set_read(MUTING, &[0x01, u8::from(flag(p, "muted")?)]),
            "toggle_mute" => set_read(MUTING, &[0x00]),
            "get_mute" => get(MUTING),
            "set_language" => {
                let code = text(p, "language")?;
                if code.len() != 3 || !code.is_ascii() {
                    return Err(invalid("'language' is a three-letter code"));
                }
                let mut data = vec![0x00];
                data.extend_from_slice(code.as_bytes());
                set(0x07, &data)
            }
            "set_off_timer" => set(0x0C, &[0x01, int(p, "value")?]),
            "toggle_off_timer" => set(0x0C, &[0x00]),
            "set_picture_mode" => set(0x20, &[0x01, pick(p, "mode", PICTURE_MODES)?]),
            "toggle_picture_mode" => set(0x20, &[0x00]),
            "set_picture" => set(0x23, &[0x01, int(p, "value")?]),
            "step_picture" => set(0x23, &[0x00, pick(p, "direction", UP_DOWN)?]),
            "set_brightness" => set(0x24, &[0x01, int(p, "value")?]),
            "step_brightness" => set(0x24, &[0x00, pick(p, "direction", UP_DOWN)?]),
            "set_color" => set(0x25, &[0x01, int(p, "value")?]),
            "step_color" => set(0x25, &[0x00, pick(p, "direction", UP_DOWN)?]),
            "set_hue" => {
                let toward = pick(p, "toward", &[("red", 0x00), ("green", 0x01)])?;
                set(0x26, &[0x01, toward, int(p, "value")?])
            }
            "step_hue" => {
                let toward = pick(p, "toward", &[("red", 0x00), ("green", 0x01)])?;
                set(0x26, &[0x00, toward, pick(p, "direction", UP_DOWN)?])
            }
            "set_sharpness" => set(0x28, &[0x01, int(p, "value")?]),
            "step_sharpness" => set(0x28, &[0x00, pick(p, "direction", UP_DOWN)?]),
            "set_cine_motion" => set(0x2A, &[pick(p, "mode", &[("off", 0x00), ("auto", 0x01)])?]),
            "set_sound_mode" => set(0x30, &[0x01, pick(p, "mode", SOUND_MODES)?]),
            "toggle_sound_mode" => set(0x30, &[0x00]),
            "set_speaker_off" => set(0x36, &[0x01, u8::from(flag(p, "speaker_off")?)]),
            "toggle_speaker_off" => set(0x36, &[0x00]),
            "set_h_shift" => set(0x41, &[0x01, pick(p, "sign", SIGN)?, int(p, "value")?]),
            "step_h_shift" => set(0x41, &[0x00, pick(p, "direction", UP_DOWN)?]),
            "set_v_size" => set(0x42, &[0x01, pick(p, "sign", SIGN)?, int(p, "value")?]),
            "set_v_shift" => set(0x43, &[0x01, pick(p, "sign", SIGN)?, int(p, "value")?]),
            "step_v_shift" => set(0x43, &[0x00, pick(p, "direction", UP_DOWN)?]),
            "set_wide" => set(0x44, &[0x01, pick(p, "mode", WIDE)?]),
            "toggle_wide" => set(0x44, &[0x00]),
            "set_auto_wide" => set(0x45, &[0x01, u8::from(flag(p, "enabled")?)]),
            "toggle_auto_wide" => set(0x45, &[0x00]),
            "set_4_3_mode" => set(0x46, &[0x01, pick(p, "mode", FOUR_THREE)?]),
            "toggle_4_3_mode" => set(0x46, &[0x00]),
            "press_key" => {
                let key = text(p, "key")?;
                let (_, category, data) = KEYS
                    .iter()
                    .find(|(name, _, _)| *name == key)
                    .ok_or_else(|| invalid(format!("'key' has no value '{key}'")))?;
                set(0x67, &[*category, *data])
            }
            "get_id" => Ok(Self::query_job(ID, Read::Id)),
            "get_product_info" => {
                let part = p.get("part").and_then(Value::as_u64).unwrap_or(1) as u8;
                if !(1..=3).contains(&part) {
                    return Err(invalid("'part' is 1, 2 or 3"));
                }
                Ok(Self::query_job(0x6F - part, Read::ProductInfo(part)))
            }
            "send_raw" => {
                let function = int(p, "function")?;
                if text(p, "kind")? == "query" {
                    Ok(Self::query_job(function, Read::Raw))
                } else {
                    let data = unhex(p.get("data").and_then(Value::as_str).unwrap_or(""))
                        .ok_or_else(|| invalid("'data' is not hex bytes"))?;
                    let mut job = Self::control_job(function, &data, None);
                    job.read = Read::Raw;
                    Ok(job)
                }
            }
            other => Err(CommandError::UnknownCommand {
                command: other.to_string(),
            }),
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Ready || self.in_flight.is_some() {
            return;
        }
        let Some(job) = self.commands.pop_front().or_else(|| self.polls.pop_front()) else {
            return;
        };
        self.buf.clear();
        cx.tcp_send(SOCKET, job.frame.clone());
        self.sent_at = cx.now();
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        self.in_flight = Some(job);
    }

    fn queue_poll(&mut self, function: u8) {
        let frame = query(function);
        if self.polls.iter().any(|j| j.frame == frame)
            || self.in_flight.as_ref().is_some_and(|j| j.frame == frame)
        {
            return;
        }
        self.polls
            .push_back(Self::query_job(function, Self::read_of(function)));
    }

    fn poll(&mut self) {
        self.queue_poll(POWER);
        if self.on {
            self.queue_poll(INPUT);
            self.queue_poll(VOLUME);
            self.queue_poll(MUTING);
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        self.phase = Phase::Connecting;
        self.buf.clear();
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.to);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        let error = CommandError::Transport {
            message: reason.clone(),
        };
        if let Some(job) = self.in_flight.take() {
            if let Some(id) = job.command {
                cx.complete(id, Err(error.clone()));
            }
        }
        for job in self.commands.drain(..) {
            if let Some(id) = job.command {
                cx.complete(id, Err(error.clone()));
            }
        }
        self.polls.clear();
        cx.cancel_timer(REPLY_TIMER);
        self.phase = Phase::Closed;
        self.connected = false;
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// One whole answer at the front of the buffer, for the request in
    /// flight; `None` until it has all arrived. A bad checksum is a parse
    /// error.
    fn take_answer(&mut self, is_query: bool) -> Option<Result<Answer, String>> {
        let start = self.buf.iter().position(|b| *b == ANSWER)?;
        self.buf.drain(..start);
        let status = *self.buf.get(1)?;
        let total = if status == 0x00 && is_query {
            3 + *self.buf.get(2)? as usize
        } else {
            3
        };
        if self.buf.len() < total {
            return None;
        }
        let answer: Vec<u8> = self.buf.drain(..total).collect();
        if checksum(&answer[..total - 1]) != answer[total - 1] {
            return Some(Err(format!(
                "an answer with a bad checksum: {}",
                hex(&answer)
            )));
        }
        Some(Ok(if status == 0x00 && is_query {
            Answer::Data(answer[3..total - 1].to_vec())
        } else {
            Answer::Status(status)
        }))
    }

    fn apply(&mut self, cx: &mut Cx, read: Read, data: &[u8]) -> Value {
        let first = data.first().copied();
        let (state, value) = match read {
            Read::Power => {
                let on = first == Some(1);
                self.on = on;
                (json!({"power": on}), json!(on))
            }
            Read::Muting => {
                let muted = first == Some(1);
                (json!({"mute": muted}), json!(muted))
            }
            Read::Volume => (json!({"volume": first}), json!(first)),
            Read::Input => {
                let kind = first
                    .map(|k| {
                        INPUTS
                            .iter()
                            .find(|(_, c)| *c == k)
                            .map(|(n, _)| n.to_string())
                            .unwrap_or(format!("0x{k:02x}"))
                    })
                    .unwrap_or_default();
                let number = data.get(1).copied();
                let v = json!({"kind": kind, "number": number});
                (json!({"input": v}), v)
            }
            Read::Id => (
                json!({"id": hex(data)}),
                json!({"hex": hex(data), "text": printable(data)}),
            ),
            Read::ProductInfo(part) => (
                json!({"product_info": {part.to_string(): hex(data)}}),
                json!({"hex": hex(data), "text": printable(data)}),
            ),
            Read::Raw | Read::None => (Value::Null, json!({"status": 0, "data": hex(data)})),
        };
        if !state.is_null() {
            cx.state(state);
        }
        value
    }

    fn on_data(&mut self, cx: &mut Cx, data: &[u8]) {
        self.buf.extend_from_slice(data);
        loop {
            let Some(job) = self.in_flight.clone() else {
                self.buf.clear();
                return;
            };
            let Some(answer) = self.take_answer(job.is_query) else {
                return;
            };
            cx.cancel_timer(REPLY_TIMER);
            cx.round_trip(cx.now().saturating_sub(self.sent_at));
            cx.alive();
            if !self.connected {
                self.connected = true;
                self.retry_after = RETRY_MIN;
                cx.connection(Connection::Connected);
            }
            self.in_flight = None;
            if job.command.is_none() && self.monitor && answer.is_ok() {
                self.stamps.answered(cx, "status");
            }
            let result = match answer {
                Err(message) => Err(CommandError::DeviceError {
                    code: Some("0x04".into()),
                    message,
                }),
                Ok(Answer::Status(0)) => {
                    if let Some(f) = job.then {
                        if self.monitor || job.command.is_some() {
                            self.commands
                                .push_front(Self::query_job(f, Self::read_of(f)));
                        }
                    }
                    if job.read == Read::Raw {
                        Ok(Outcome::Value {
                            value: json!({"status": 0, "data": ""}),
                        })
                    } else {
                        Ok(Outcome::Ack)
                    }
                }
                Ok(Answer::Status(status)) if job.read == Read::Raw => Ok(Outcome::Value {
                    value: json!({"status": status, "data": ""}),
                }),
                Ok(Answer::Status(status)) => Err(CommandError::DeviceError {
                    code: Some(format!("0x{status:02x}")),
                    message: status_text(status).into(),
                }),
                Ok(Answer::Data(data)) => {
                    let value = self.apply(cx, job.read, &data);
                    Ok(Outcome::Value { value })
                }
            };
            match (job.command, result) {
                (Some(id), r) => cx.complete(id, r),
                (None, Err(e)) => cx.log(Level::Debug, format!("BRAVIA serial: {e}")),
                _ => {}
            }
            self.pump(cx);
        }
    }
}

fn printable(data: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(data)
        .trim_end_matches('\0')
        .to_string();
    (!text.is_empty() && text.chars().all(|c| !c.is_control())).then_some(text)
}

impl Module for SonyBraviaSerial {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.phase == Phase::Closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match Self::command_job(name, params) {
            Ok(mut job) => {
                job.command = Some(id);
                self.commands.push_back(job);
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                if self.phase != Phase::Connecting {
                    return;
                }
                self.phase = Phase::Ready;
                // A converter's connection shows nothing of the display: it
                // is reported connected once it answers.
                self.queue_poll(POWER);
                if self.monitor {
                    self.polls.push_back(Self::query_job(ID, Read::Id));
                    for part in 1..=3u8 {
                        self.polls
                            .push_back(Self::query_job(0x6F - part, Read::ProductInfo(part)));
                    }
                    cx.set_timer(POLL_TIMER, POLL_EVERY);
                } else {
                    cx.set_timer(POLL_TIMER, PROBE_EVERY);
                }
                self.pump(cx);
            }
            TcpInput::Data(data) => {
                if self.phase == Phase::Ready {
                    self.on_data(cx, &data);
                }
            }
            TcpInput::Closed { reason } => {
                if self.phase != Phase::Closed {
                    self.lost(cx, reason);
                }
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => {
                if self.phase == Phase::Closed {
                    self.open(cx);
                }
            }
            POLL_TIMER => {
                if self.phase != Phase::Ready {
                    return;
                }
                if self.monitor {
                    self.poll();
                    cx.set_timer(POLL_TIMER, POLL_EVERY);
                } else {
                    let idle = self.in_flight.is_none() && self.commands.is_empty();
                    if idle && cx.now().saturating_sub(self.sent_at) >= PROBE_EVERY {
                        self.queue_poll(POWER);
                    }
                    cx.set_timer(POLL_TIMER, PROBE_EVERY);
                }
                self.pump(cx);
            }
            REPLY_TIMER => {
                if let Some(job) = self.in_flight.take() {
                    self.buf.clear();
                    if let Some(id) = job.command {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    if self.connected {
                        self.connected = false;
                        cx.connection(Connection::Disconnected {
                            reason: "the display did not answer".into(),
                        });
                    }
                    self.pump(cx);
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Closed {
            cx.tcp_close(SOCKET);
        }
        self.phase = Phase::Closed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use std::net::{IpAddr, Ipv4Addr};

    fn module(monitor: bool, settings: Value) -> SonyBraviaSerial {
        let spec = Catalog::source_tree().devices["sony-bravia-serial"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        SonyBraviaSerial::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 9)),
            host_name: None,
            port: Some(4001),
            model: "bz".into(),
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

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn feed_at(m: &mut SonyBraviaSerial, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut SonyBraviaSerial, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn answer(status: u8) -> Vec<u8> {
        let mut a = vec![ANSWER, status];
        a.push(checksum(&a));
        a
    }

    fn data_answer(data: &[u8]) -> Vec<u8> {
        let mut a = vec![ANSWER, 0x00, data.len() as u8 + 1];
        a.extend_from_slice(data);
        a.push(checksum(&a));
        a
    }

    /// Opened for commands only, past its first power query.
    fn idle() -> SonyBraviaSerial {
        let mut m = module(false, json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = cx.take();
        assert_eq!(sent(&a), [query(POWER)]);
        let a = feed_at(&mut m, 0, &data_answer(&[1]));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        m
    }

    #[test]
    fn frames_are_the_documented_byte_sequences() {
        // The command definitions' examples.
        assert_eq!(control(0x00, &[0x00]), [0x8C, 0x00, 0x00, 0x02, 0x00, 0x8E]);
        assert_eq!(control(0x00, &[0x01]), [0x8C, 0x00, 0x00, 0x02, 0x01, 0x8F]);
        assert_eq!(query(0x00), [0x83, 0x00, 0x00, 0xFF, 0xFF, 0x81]);
        assert_eq!(query(0x6F), [0x83, 0x00, 0x6F, 0xFF, 0xFF, 0xF0]);
        assert_eq!(
            control(0x02, &[0x04, 0x02]),
            [0x8C, 0x00, 0x02, 0x03, 0x04, 0x02, 0x97]
        );
        assert_eq!(
            control(0x07, b"\x00eng"),
            [0x8C, 0x00, 0x07, 0x05, 0x00, 0x65, 0x6E, 0x67, 0xD2]
        );
        assert_eq!(
            control(0x67, &[0x01, 0x25]),
            [0x8C, 0x00, 0x67, 0x03, 0x01, 0x25, 0x1C]
        );
        assert_eq!(
            control(0x30, &[0x01, 0x08]),
            [0x8C, 0x00, 0x30, 0x03, 0x01, 0x08, 0xC8]
        );
    }

    #[test]
    fn commands_send_their_bytes_and_read_back() {
        let mut m = idle();
        let a = run(&mut m, 1, "set_input", json!({"kind": "hdmi", "number": 2}));
        assert_eq!(sent(&a), [control(0x02, &[0x04, 0x02])]);
        let a = feed_at(&mut m, 0, &answer(0x00));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        // The input is read back.
        assert_eq!(sent(&a), [query(INPUT)]);
        let a = feed_at(&mut m, 0, &data_answer(&[0x04, 0x02]));
        assert!(a.contains(&Action::State(
            json!({"input": {"kind": "hdmi", "number": 2}})
        )));
        // A refused value fails with its status.
        run(&mut m, 2, "set_picture", json!({"value": 200}));
        let a = feed_at(&mut m, 0, &answer(0x01));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { code: Some(c), .. })) if c == "0x01"
        ));
        let a = run(&mut m, 3, "press_key", json!({"key": "home"}));
        assert_eq!(sent(&a), [control(0x67, &[0x01, 0x60])]);
        feed_at(&mut m, 0, &answer(0x00));
        let a = run(&mut m, 4, "get_product_info", json!({"part": 2}));
        assert_eq!(sent(&a), [query(0x6D)]);
        let a = feed_at(&mut m, 0, &data_answer(b"FW-55BZ40L"));
        assert_eq!(
            completed(&a, 4),
            Some(Ok(Outcome::Value {
                value: json!({"hex": "46572d3535425a34304c", "text": "FW-55BZ40L"})
            }))
        );
    }

    #[test]
    fn opened_for_commands_only_it_asks_only_power() {
        let mut m = idle();
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, POLL_TIMER);
        assert_eq!(sent(&cx.take()), [query(POWER)]);
        // Monitored, it asks the ID and product information, then polls.
        let mut m = module(true, json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&cx.take()), [query(POWER)]);
        let a = feed_at(&mut m, 0, &data_answer(&[1]));
        assert_eq!(sent(&a), [query(ID)]);
    }

    #[test]
    fn the_time_to_each_answer_is_reported() {
        let mut m = idle();
        let mut cx = Cx::new(1_000);
        m.command(&mut cx, 1, "get_volume", json!({}).as_object().unwrap());
        let a = feed_at(&mut m, 1_025, &data_answer(&[30]));
        assert!(a.contains(&Action::RoundTrip(25)));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(30) }))
        );
    }
}
