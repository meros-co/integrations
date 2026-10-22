//! Epson projectors over ESC/VP21, on TCP 3629 (ESC/VP.net) or a serial
//! port.
//!
//! Protocol from Epson's ESC/VP21 Command User's Guide for Business
//! Projectors (Rev. A, 2013) and its ESC/VP21 command list workbook (Rev. T,
//! 2020), which gives each model family's commands; and Epson's support
//! article on sending ESC/VP.net commands (KA-01646) for the network
//! connection.
//!
//! - A set command is a name and a parameter (`PWR ON`), a get command a
//!   name and `?` (`PWR?`), each ended by CR; the projector answers a set
//!   with a colon and a get with `NAME=value` and CR, then a colon (guide
//!   §2.1, §2.2; the workbook's "How to send command"). A command it cannot
//!   take is answered `ERR` CR and a colon (§2.4). CR alone, the null
//!   command, is answered with a colon and shows the projector works (§2.3).
//! - One command at a time: the next goes only after the colon (§4). Power
//!   on can take 40 s, or 100 s when the lamp needs three tries, and a
//!   source change up to 25 s on old models (§4.2), so those wait longer.
//! - Over the network the controller first sends ESC/VP.net's 16-byte
//!   connect header (`ESC/VP.net`, 10h, 03h, four zero bytes) and the
//!   projector answers with a header beginning `ESC/VP.net`; ESC/VP21
//!   commands follow on the same connection (KA-01646). A projector whose
//!   Command Communication is set to Protected, or that has a Monitor
//!   Password, wants ESC/VP.net's password exchange, which Epson documents
//!   only to developers under confidentiality: it is not attempted, and such
//!   a projector is reported disconnected with that reason.
//! - The projector pushes nothing, so state is polled: power and the error
//!   status every 5 s, and while it is on the source, A/V mute and freeze;
//!   lamp hours and every other setting the model's family reads every
//!   minute while it is on (in standby most commands answer ERR, §3.1); the
//!   serial number on connecting and on `refresh`.
//! - The `link` setting chooses TCP or a serial port of the host (9600 bit/s,
//!   8 data bits, no parity, 1 stop bit and no flow control by default,
//!   guide appendix). A link that drops or a port that fails is opened again
//!   after 1 s, doubling to 30 s; three requests in a row unanswered close it.
//!
//! Opened for commands only (`monitor` false), nothing is polled and no set
//! is followed by a read; every 5 s the null command shows the projector is
//! there. Replies to commands still update the state, and every answered
//! request reports its round trip as the device's latency.

use std::collections::VecDeque;

use serde_json::{json, Value};

use super::epson_projector_commands::COMMANDS;
use crate::catalog::{DeviceSpec, Params};
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

pub(crate) const DEFAULT_PORT: u16 = 3629;
const SOCKET: Key = "escvp";
const REPLY: Key = "reply";
const POLL: Key = "poll";
const RETRY: Key = "retry";

/// The ESC/VP.net connect header of Epson's article KA-01646.
const CONNECT: [u8; 16] = [
    0x45, 0x53, 0x43, 0x2F, 0x56, 0x50, 0x2E, 0x6E, 0x65, 0x74, 0x10, 0x03, 0x00, 0x00, 0x00, 0x00,
];
const HANDSHAKE_TIMEOUT: Millis = 5_000;
const REPLY_TIMEOUT: Millis = 10_000;
/// §4.2 note 1: a colon within 40, 70 or 100 s as the lamp lights.
const POWER_TIMEOUT: Millis = 110_000;
/// §4.2: a source change takes up to 25 s on the oldest models.
const SOURCE_TIMEOUT: Millis = 30_000;
/// Calibrations and resets run before the colon comes.
const LONG_TIMEOUT: Millis = 60_000;
const POLL_EVERY: Millis = 5_000;
const SLOW_EVERY: u32 = 12;
const MISSES: u32 = 3;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// Far longer than any reply (SOURCELIST is the longest); anything longer
/// is not ESC/VP21.
const MAX_REPLY: usize = 16 * 1024;

/// How a generated command is sent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    /// `NAME value`, the `value` or `args` parameter as given.
    Set,
    /// `NAME ON` / `OFF` / `INIT`.
    OnOff,
    /// `NAME`, alone.
    Action,
    /// `NAME?`.
    Get,
    /// `NAME? args`.
    GetArgs,
    /// `NAME`, answered with text (AMX).
    Raw,
}

fn lookup(name: &str) -> Option<(&'static str, Kind)> {
    COMMANDS
        .binary_search_by(|(n, _, _)| (*n).cmp(name))
        .ok()
        .map(|i| (COMMANDS[i].1, COMMANDS[i].2))
}

/// The state key of a keyword's reading: the command name without `get_`.
fn state_key(keyword: &str) -> Option<&'static str> {
    COMMANDS
        .iter()
        .find(|(n, k, kind)| *k == keyword && *kind == Kind::Get && n.starts_with("get_"))
        .map(|(n, _, _)| &n[4..])
}

#[derive(Debug, Clone)]
struct Job {
    command: Option<CommandId>,
    keyword: &'static str,
    line: String,
    /// A query, whose `NAME=value` lines are read into state.
    query: bool,
    /// Reads that follow a successful set when monitoring.
    follow: Vec<Job>,
}

impl Job {
    fn get(keyword: &'static str) -> Job {
        Job {
            command: None,
            keyword,
            line: format!("{keyword}?"),
            query: true,
            follow: Vec::new(),
        }
    }

    fn null() -> Job {
        Job {
            command: None,
            keyword: "",
            line: String::new(),
            query: false,
            follow: Vec::new(),
        }
    }

    fn timeout(&self) -> Millis {
        match (self.keyword, self.query) {
            ("PWR", false) => POWER_TIMEOUT,
            ("SOURCE", false) => SOURCE_TIMEOUT,
            ("LENSCALB" | "LTCALB" | "INITALL" | "INITALL2" | "INITFACTORY", false) => LONG_TIMEOUT,
            _ => REPLY_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Opening,
    /// TCP: the connect header is sent, its answer awaited.
    Handshake,
    Open,
}

pub(crate) struct EpsonProjector {
    link: Link,
    tcp: bool,
    monitor: bool,
    /// Keywords the model's family reads (its `get_*` commands).
    reads: Vec<&'static str>,
    phase: Phase,
    reachable: bool,
    powered: bool,
    misses: u32,
    retry_after: Millis,
    buf: Vec<u8>,
    commands: VecDeque<Job>,
    polls: VecDeque<Job>,
    in_flight: Option<(Job, Millis)>,
    cycle: u32,
    need_identity: bool,
}

impl EpsonProjector {
    pub(crate) fn new(spec: &DeviceSpec, ctx: OpenContext) -> Result<EpsonProjector, String> {
        let model = spec
            .models
            .iter()
            .find(|m| m.id == ctx.model)
            .ok_or_else(|| format!("no Epson projector model '{}'", ctx.model))?;
        let reads = model
            .supports
            .iter()
            .filter_map(|c| match lookup(c) {
                Some((kw, Kind::Get)) => Some(kw),
                _ => None,
            })
            .collect();
        let link = Link::from_settings(&ctx, DEFAULT_PORT)?;
        Ok(EpsonProjector::with(link, reads, ctx.monitor))
    }

    fn with(link: Link, reads: Vec<&'static str>, monitor: bool) -> EpsonProjector {
        EpsonProjector {
            tcp: matches!(link, Link::Tcp(_)),
            link,
            monitor,
            reads,
            phase: Phase::Closed,
            reachable: false,
            powered: false,
            misses: 0,
            retry_after: RETRY_MIN,
            buf: Vec::new(),
            commands: VecDeque::new(),
            polls: VecDeque::new(),
            in_flight: None,
            cycle: 0,
            need_identity: true,
        }
    }

    fn queue_cycle(&mut self) {
        if !self.polls.is_empty() {
            return;
        }
        if !self.monitor {
            // Commands only: the null command, as the liveness check.
            self.polls.push_back(Job::null());
            return;
        }
        self.queue_reads(false);
    }

    /// Status always; the rest while the projector is on (or `all`).
    fn queue_reads(&mut self, all: bool) {
        let mut jobs = vec![Job::get("PWR"), Job::get("ERR")];
        let on = self.powered || all;
        if self.need_identity && self.reads.contains(&"SNO") {
            self.need_identity = false;
            jobs.push(Job::get("SNO"));
        }
        if on {
            for kw in ["SOURCE", "MUTE", "FREEZE"] {
                if self.reads.contains(&kw) {
                    jobs.push(Job::get(kw));
                }
            }
        }
        if self.cycle.is_multiple_of(SLOW_EVERY) || all {
            for kw in &self.reads {
                if ["PWR", "ERR", "SNO", "SOURCE", "MUTE", "FREEZE"].contains(kw) {
                    continue;
                }
                if *kw == "LAMP" || on {
                    jobs.push(Job::get(kw));
                }
            }
        }
        self.cycle = self.cycle.wrapping_add(1);
        self.polls.extend(jobs);
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || self.phase != Phase::Open {
            return;
        }
        let Some(job) = self.commands.pop_front().or_else(|| self.polls.pop_front()) else {
            return;
        };
        let mut out = job.line.clone().into_bytes();
        out.push(b'\r');
        cx.tcp_send(SOCKET, out);
        cx.set_timer(REPLY, job.timeout());
        self.in_flight = Some((job, cx.now()));
    }

    fn open_link(&mut self, cx: &mut Cx) {
        self.phase = Phase::Opening;
        self.buf.clear();
        cx.connection(Connection::Connecting);
        self.link.open(cx, SOCKET);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if self.phase != Phase::Closed {
            cx.tcp_close(SOCKET);
        }
        self.phase = Phase::Closed;
        self.buf.clear();
        self.misses = 0;
        for job in self
            .in_flight
            .take()
            .map(|(j, _)| j)
            .into_iter()
            .chain(self.commands.drain(..))
        {
            if let Some(id) = job.command {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        self.polls.clear();
        cx.cancel_timer(REPLY);
        cx.cancel_timer(POLL);
        if self.reachable {
            self.need_identity = true;
        }
        self.reachable = false;
        cx.log(Level::Debug, format!("Epson projector: {reason}"));
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// The link is ready for ESC/VP21: poll and send.
    fn ready(&mut self, cx: &mut Cx) {
        self.phase = Phase::Open;
        cx.cancel_timer(REPLY);
        self.queue_cycle();
        cx.set_timer(POLL, POLL_EVERY);
        self.pump(cx);
    }

    fn data(&mut self, cx: &mut Cx, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        if self.phase == Phase::Handshake {
            if self.buf.len() < 16 {
                return;
            }
            let header: Vec<u8> = self.buf.drain(..16).collect();
            if &header[..10] != b"ESC/VP.net" {
                self.lost(
                    cx,
                    "the projector did not answer the ESC/VP.net connect header".into(),
                );
                return;
            }
            cx.alive();
            self.ready(cx);
        }
        if self.phase != Phase::Open {
            return;
        }
        // A reply ends at a colon that starts a line.
        while let Some(end) = prompt(&self.buf) {
            let raw: Vec<u8> = self.buf.drain(..=end).collect();
            let text = String::from_utf8_lossy(&raw[..raw.len() - 1]).into_owned();
            self.reply(cx, &text);
            if self.phase != Phase::Open {
                return;
            }
        }
        if self.buf.len() > MAX_REPLY {
            self.lost(cx, "a reply longer than ESC/VP21 sends".into());
        }
    }

    fn reply(&mut self, cx: &mut Cx, text: &str) {
        let Some((job, sent_at)) = self.in_flight.take() else {
            // A colon nobody asked for (an echo after a reconnect).
            return;
        };
        cx.cancel_timer(REPLY);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        cx.alive();
        self.misses = 0;
        self.retry_after = RETRY_MIN;
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
        let lines: Vec<&str> = text
            .split(['\r', '\n'])
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let result = if lines.contains(&"ERR") {
            Err(CommandError::DeviceError {
                code: Some("ERR".into()),
                message: format!(
                    "{}: the projector answered ERR (not supported by the model, a value out of range, or not available now, such as in standby)",
                    if job.keyword.is_empty() { "null command" } else { job.keyword }
                ),
            })
        } else if job.query {
            let values: Vec<String> = lines
                .iter()
                .map(|l| match l.split_once('=') {
                    Some((_, v)) => v.to_string(),
                    None => l.to_string(),
                })
                .collect();
            let value = values.join("\n");
            if let Some(patch) = self.patch(job.keyword, &job.line, &value) {
                cx.state(patch);
            }
            Ok(Outcome::Value {
                value: json!(value),
            })
        } else if job.keyword == "AMX" {
            Ok(Outcome::Value {
                value: json!(lines.join("\n")),
            })
        } else {
            Ok(Outcome::Ack)
        };
        if result.is_ok() && self.monitor {
            for follow in job.follow.iter().rev() {
                self.polls.push_front(follow.clone());
            }
        }
        if let Some(id) = job.command {
            cx.complete(id, result);
        }
        self.pump(cx);
    }

    /// The state a query's value sets: the raw value under `settings`, and
    /// the decoded status keys.
    fn patch(&mut self, keyword: &str, line: &str, value: &str) -> Option<Value> {
        let mut patch = json!({});
        match keyword {
            "PWR" => {
                let (power, on) = match value {
                    "00" => ("standby_network_off", false),
                    "01" => ("on", true),
                    "02" => ("warming_up", true),
                    "03" => ("cooling_down", false),
                    "04" => ("standby_network_on", false),
                    "05" => ("abnormal_standby", false),
                    "07" => ("wireless_hd_standby", false),
                    "09" => ("av_standby", false),
                    _ => ("unknown", false),
                };
                self.powered = on;
                patch["power"] = json!(power);
                patch["power_code"] = json!(value);
            }
            "ERR" => {
                patch["error"] = json!({ "code": value, "text": error_text(value) });
            }
            "LAMP" => {
                // One number per lamp, separated by spaces on two-lamp models.
                let hours: Vec<Value> = value
                    .split_whitespace()
                    .filter_map(|h| h.parse::<u64>().ok().map(Value::from))
                    .collect();
                if let Some(first) = hours.first() {
                    patch["lamp_hours"] = first.clone();
                }
                if hours.len() > 1 {
                    patch["lamps_hours"] = Value::Array(hours);
                }
            }
            "ONTIME" => {
                if let Ok(h) = value.trim().parse::<u64>() {
                    patch["operating_hours"] = json!(h);
                }
            }
            "SNO" => patch["serial_number"] = json!(value),
            "SOURCE" => patch["source"] = json!(value),
            "MUTE" => patch["av_mute"] = json!(value == "ON"),
            "FREEZE" => patch["freeze"] = json!(value == "ON"),
            "SIGNAL" => {
                patch["signal"] = json!(match value {
                    "00" => "none",
                    "01" => "present",
                    "FF" => "unsupported",
                    _ => "unknown",
                })
            }
            _ => {}
        }
        // A plain query's value is kept as the projector sends it; a query
        // with arguments under its arguments.
        let args = line.split_once(' ').map(|(_, a)| a.trim()).unwrap_or("");
        if args.is_empty() {
            if let Some(key) = state_key(keyword) {
                patch["settings"] = json!({ key: value });
            }
        } else if let Some(name) = COMMANDS
            .iter()
            .find(|(_, k, kind)| *k == keyword && *kind == Kind::GetArgs)
            .map(|(n, _, _)| &n[4..])
        {
            patch["settings"] = json!({ name: { args.replace(' ', "_"): value } });
        }
        let empty = patch.as_object().is_none_or(|p| p.is_empty());
        (!empty).then_some(patch)
    }

    fn command_job(&self, name: &str, p: &Params) -> Result<Job, CommandError> {
        let text = |key: &str| {
            p.get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(format!("'{key}' is required")))
        };
        match name {
            "set_power" => {
                let on = p
                    .get("on")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| invalid("'on' is required"))?;
                let mut job = Job {
                    command: None,
                    keyword: "PWR",
                    line: format!("PWR {}", if on { "ON" } else { "OFF" }),
                    query: false,
                    follow: Vec::new(),
                };
                job.follow.push(Job::get("PWR"));
                return Ok(job);
            }
            "get_power" => return Ok(Job::get("PWR")),
            "get_errors" => return Ok(Job::get("ERR")),
            _ => {}
        }
        let (keyword, kind) = lookup(name).ok_or_else(|| CommandError::UnknownCommand {
            command: name.into(),
        })?;
        let checked = |v: &str| -> Result<String, CommandError> {
            if v.is_empty() || v.chars().any(|c| c.is_control()) {
                Err(invalid("the value is empty or holds a control character"))
            } else {
                Ok(v.to_string())
            }
        };
        let (line, query) = match kind {
            Kind::Set => {
                let v = p
                    .get("value")
                    .or_else(|| p.get("args"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("'value' is required"))?;
                (format!("{keyword} {}", checked(v)?), false)
            }
            Kind::OnOff => {
                let v = match text("value")? {
                    "on" => "ON",
                    "off" => "OFF",
                    "init" => "INIT",
                    other => return Err(invalid(format!("unknown value '{other}'"))),
                };
                (format!("{keyword} {v}"), false)
            }
            Kind::Action | Kind::Raw => (keyword.to_string(), false),
            Kind::Get => (format!("{keyword}?"), true),
            Kind::GetArgs => (format!("{keyword}? {}", checked(text("args")?)?), true),
        };
        let mut job = Job {
            command: None,
            keyword,
            line,
            query,
            follow: Vec::new(),
        };
        if matches!(kind, Kind::Set | Kind::OnOff) && state_key(keyword).is_some() {
            job.follow.push(Job::get(keyword));
        }
        Ok(job)
    }
}

/// The end of a reply: a colon at the start of a line.
fn prompt(buf: &[u8]) -> Option<usize> {
    buf.iter()
        .enumerate()
        .find(|(i, b)| **b == b':' && (*i == 0 || matches!(buf[i - 1], b'\r' | b'\n')))
        .map(|(i, _)| i)
}

/// The guide's error status codes (§5).
fn error_text(code: &str) -> &'static str {
    match code {
        "00" => "no error, or the error is recovered",
        "01" => "fan error",
        "03" => "lamp failure at power on",
        "04" => "high internal temperature",
        "06" => "lamp error",
        "07" => "lamp cover door open",
        "08" => "cinema filter error",
        "09" => "electric double-layer capacitor disconnected",
        "0A" => "auto iris error",
        "0B" => "subsystem error",
        "0C" => "low air flow",
        "0D" => "air filter air flow sensor error",
        "0E" => "power supply unit (ballast) error",
        "0F" => "shutter error",
        "10" => "cooling system (Peltier element) error",
        "11" => "cooling system (pump) error",
        "12" => "static iris error",
        "13" => "power supply unit (ballast) mismatch",
        "14" => "exhaust shutter error",
        "15" => "obstacle detected",
        "16" => "IF board discernment error",
        "17" => "stack projection communication error",
        "18" => "I2C error",
        _ => "an error the guide does not list",
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

impl Module for EpsonProjector {
    fn start(&mut self, cx: &mut Cx) {
        self.open_link(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if name == "refresh" {
            self.need_identity = true;
            self.polls.clear();
            self.cycle = 0;
            // Everything, once, even when opened for commands only.
            self.queue_reads(true);
            cx.complete(id, Ok(Outcome::Ack));
            self.pump(cx);
            return;
        }
        if self.phase == Phase::Closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.command_job(name, params) {
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
                if self.phase != Phase::Opening {
                    return;
                }
                self.buf.clear();
                if self.tcp {
                    self.phase = Phase::Handshake;
                    cx.tcp_send(SOCKET, CONNECT.to_vec());
                    cx.set_timer(REPLY, HANDSHAKE_TIMEOUT);
                } else {
                    self.ready(cx);
                }
            }
            TcpInput::Data(bytes) => {
                if matches!(self.phase, Phase::Handshake | Phase::Open) {
                    self.data(cx, &bytes);
                }
            }
            TcpInput::Closed { reason } => {
                if self.phase != Phase::Closed {
                    let reason = if self.phase == Phase::Handshake {
                        format!("{reason}; a projector set to Protected command communication or with a Monitor Password needs ESC/VP.net's password exchange, which is not available")
                    } else {
                        reason
                    };
                    self.phase = Phase::Opening;
                    self.lost(cx, reason);
                }
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.open_link(cx),
            POLL => {
                if self.phase == Phase::Open {
                    self.queue_cycle();
                    cx.set_timer(POLL, POLL_EVERY);
                    self.pump(cx);
                }
            }
            REPLY => {
                if self.phase == Phase::Handshake {
                    self.lost(
                        cx,
                        "no answer to the ESC/VP.net connect header; a projector set to Protected command communication or with a Monitor Password needs ESC/VP.net's password exchange, which is not available".into(),
                    );
                    return;
                }
                if let Some((job, _)) = self.in_flight.take() {
                    if let Some(id) = job.command {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    // A late colon would answer the next request: drop what
                    // has arrived.
                    self.buf.clear();
                    self.misses += 1;
                    if self.misses >= MISSES {
                        self.lost(cx, "the projector answered none of three requests".into());
                        return;
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn module(tcp: bool, monitor: bool, reads: Vec<&'static str>) -> EpsonProjector {
        let link = if tcp {
            Link::Tcp(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, 50)),
                DEFAULT_PORT,
            ))
        } else {
            Link::Serial(
                crate::module::SerialPort::from_settings(
                    json!({"serial_port": "COM3"}).as_object().unwrap(),
                )
                .unwrap(),
            )
        };
        EpsonProjector::with(link, reads, monitor)
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

    fn sent_text(actions: &[Action]) -> Vec<String> {
        sent(actions)
            .into_iter()
            .map(|d| String::from_utf8(d).unwrap())
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

    fn feed(m: &mut EpsonProjector, text: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(20);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(text.to_vec()));
        cx.take()
    }

    fn run(m: &mut EpsonProjector, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn opened(m: &mut EpsonProjector) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    /// Answer everything in flight with a bare colon until idle.
    fn drain(m: &mut EpsonProjector) {
        while m.in_flight.is_some() {
            feed(m, b":");
        }
    }

    #[test]
    fn the_command_table_is_sorted_and_unique() {
        for pair in COMMANDS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} then {}", pair[0].0, pair[1].0);
        }
        assert_eq!(lookup("set_brightness"), Some(("BRIGHT", Kind::Set)));
        assert_eq!(lookup("get_lamp_hours"), Some(("LAMP", Kind::Get)));
    }

    #[test]
    fn the_network_link_sends_the_connect_header_first() {
        let mut m = module(true, true, vec!["LAMP", "SOURCE"]);
        let a = opened(&mut m);
        assert_eq!(sent(&a), [CONNECT.to_vec()]);
        // The answer header, then the first poll.
        let mut answer = b"ESC/VP.net".to_vec();
        answer.extend_from_slice(&[0x10, 0x03, 0x00, 0x00, 0x20, 0x00]);
        let a = feed(&mut m, &answer);
        assert_eq!(sent_text(&a), ["PWR?\r"]);
        let a = feed(&mut m, b"PWR=01\r:");
        let s = state(&a);
        assert_eq!(s["power"], "on");
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(sent_text(&a), ["ERR?\r"]);
        let a = feed(&mut m, b"ERR=04\r:");
        assert_eq!(state(&a)["error"]["text"], "high internal temperature");
    }

    #[test]
    fn a_wrong_answer_to_the_header_drops_the_link() {
        let mut m = module(true, true, vec![]);
        opened(&mut m);
        let a = feed(&mut m, b"HTTP/1.1 400 Bad Req");
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
    }

    #[test]
    fn commands_are_sent_as_the_guide_writes_them() {
        let cases: &[(&str, Value, &str)] = &[
            ("set_power", json!({"on": true}), "PWR ON\r"),
            ("set_power", json!({"on": false}), "PWR OFF\r"),
            ("set_source", json!({"value": "30"}), "SOURCE 30\r"),
            ("set_av_mute", json!({"value": "on"}), "MUTE ON\r"),
            ("set_freeze", json!({"value": "init"}), "FREEZE INIT\r"),
            ("set_brightness", json!({"value": "INC"}), "BRIGHT INC\r"),
            (
                "set_quick_corner",
                json!({"args": "0 0 1919 0 1919 1199 0 1199"}),
                "QC 0 0 1919 0 1919 1199 0 1199\r",
            ),
            ("calibrate_lens", json!({}), "LENSCALB\r"),
            ("get_sharpness_of", json!({"args": "01"}), "SHARP? 01\r"),
            ("get_lamp_hours", json!({}), "LAMP?\r"),
            ("get_amx_beacon", json!({}), "AMX\r"),
        ];
        for (i, (name, p, line)) in cases.iter().enumerate() {
            let mut m = module(false, false, vec![]);
            opened(&mut m);
            drain(&mut m);
            let a = run(&mut m, i as u64 + 1, name, p.clone());
            assert_eq!(sent_text(&a), [line.to_string()], "{name}");
        }
    }

    #[test]
    fn a_query_reply_reaches_state_and_err_fails() {
        let mut m = module(false, false, vec![]);
        opened(&mut m);
        drain(&mut m);
        run(&mut m, 1, "get_brightness", json!({}));
        let a = feed(&mut m, b"BRIGHT=128\r:");
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!("128")
            }))
        );
        assert_eq!(state(&a)["settings"]["brightness"], "128");
        run(&mut m, 2, "get_sharpness_of", json!({"args": "01"}));
        let a = feed(&mut m, b"SHARP=5\r:");
        assert_eq!(state(&a)["settings"]["sharpness_of"]["01"], "5");
        run(&mut m, 3, "set_aspect", json!({"value": "99"}));
        let a = feed(&mut m, b"ERR\r:");
        assert!(matches!(
            completed(&a, 3),
            Some(Err(CommandError::DeviceError { .. }))
        ));
        // A reply split across reads.
        run(&mut m, 4, "get_lamp_hours", json!({}));
        feed(&mut m, b"LAMP=12");
        let a = feed(&mut m, b"34\r:");
        assert_eq!(state(&a)["lamp_hours"], 1234);
    }

    #[test]
    fn a_set_is_read_back_when_monitoring() {
        let mut m = module(false, true, vec![]);
        opened(&mut m);
        drain(&mut m);
        run(&mut m, 1, "set_brightness", json!({"value": "100"}));
        let a = feed(&mut m, b":");
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(sent_text(&a), ["BRIGHT?\r"]);
    }

    #[test]
    fn standby_reads_only_status_and_on_reads_the_rest() {
        let mut m = module(false, true, vec!["PWR", "LAMP", "SOURCE", "BRIGHT"]);
        let a = opened(&mut m);
        assert_eq!(sent_text(&a), ["PWR?\r"]);
        let mut lines = vec![];
        let mut a = feed(&mut m, b"PWR=04\r:");
        while let Some(l) = sent_text(&a).first().cloned() {
            lines.push(l);
            a = feed(&mut m, b":");
        }
        // Standby: errors and lamp hours only, nothing that answers ERR.
        assert_eq!(lines, ["ERR?\r", "LAMP?\r"]);
        m.powered = true;
        m.cycle = 0;
        m.queue_reads(false);
        let words: Vec<String> = m.polls.iter().map(|j| j.line.clone()).collect();
        assert!(words.contains(&"SOURCE?".to_string()));
        assert!(words.contains(&"BRIGHT?".to_string()));
    }

    #[test]
    fn opened_for_commands_only_it_sends_only_the_null_command() {
        let mut m = module(false, false, vec!["LAMP"]);
        let a = opened(&mut m);
        assert_eq!(sent_text(&a), ["\r"]);
        let a = feed(&mut m, b":");
        assert!(sent(&a).is_empty());
        let a = run(&mut m, 9, "set_power", json!({"on": true}));
        assert_eq!(sent_text(&a), ["PWR ON\r"]);
        let a = feed(&mut m, b":");
        assert_eq!(completed(&a, 9), Some(Ok(Outcome::Ack)));
        assert!(sent(&a).is_empty());
    }

    #[test]
    fn a_round_trip_is_reported() {
        let mut m = module(false, false, vec![]);
        opened(&mut m);
        let a = feed(&mut m, b":");
        assert!(a.contains(&Action::RoundTrip(20)));
    }

    #[test]
    fn three_unanswered_requests_reopen_the_link() {
        let mut m = module(false, false, vec![]);
        opened(&mut m);
        let mut cx = Cx::new(0);
        for _ in 0..3 {
            if m.in_flight.is_none() {
                m.timer(&mut cx, POLL);
            }
            m.timer(&mut cx, REPLY);
        }
        assert_eq!(m.phase, Phase::Closed);
    }
}
