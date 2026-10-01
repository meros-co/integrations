//! PJLink Class 1 and Class 2: projector and display control over TCP 4352.
//!
//! Protocol from JBMIA's PJLink Specifications, Class 1 Version 1.04
//! (2013-12-10) and Class 2 Version 2.10 (2024-02-29), which includes Class 1.
//! Section numbers below are the Class 2 document's; Class 1's are the same
//! for chapters 2, 4.1-4.14 and 5, and its chapter 3 is Class 2's §3.1.
//!
//! - A command is `%<class><BODY> <param>` and CR, its response
//!   `%<class><BODY>=<param>` and CR (§2.1, §2.2). Commands defined in Class 1
//!   carry class `1`, those added by Class 2 carry `2`. INPT and INST exist in
//!   both; the `%2` form also accepts terminals A-Z and source 6 (§4.3, §4.4).
//! - The projector speaks first: `PJLINK 0` (no authentication, §5.2) or
//!   `PJLINK 1 <random>` (§5.1). With Class 1 authentication, the first
//!   command is prefixed by MD5(random + password) as 32 lowercase hex
//!   digits (Class 1 §5.1 (1-2), (1-3)). Version 2.10 adds SHA-256: the
//!   controller sends `PJLINK 2`, the projector answers `PJLINK 2
//!   <32 hex digits>`, and the first command is prefixed by the controller's
//!   own 16-byte random number (32 hex digits) and SHA-256 of the hex of
//!   both randoms XORed followed by the password (64 hex digits) (§5.1
//!   (1-2)-(1-6)). A projector that does not answer `PJLINK 2` as specified
//!   is reconnected with the old (MD5) procedure, as §5.1 step 5 directs.
//! - `PJLINK ERRA` refuses the password (§5.1 (1-8); Class 1 (1-4)). It is
//!   terminal: nothing is sent again until the device is opened again.
//! - One command at a time: a command sent before the previous response is
//!   not guaranteed (§5.3). Responses come within 2 s, model-dependent
//!   (§6 "Response method"). The projector closes a connection 30 s after its
//!   last response (§5.4) and the controller "must terminate the TCP
//!   connection as soon as the required command transmission is completed"
//!   (§5.4), so the module connects when it has work, runs the queue, and
//!   closes when it is empty.
//! - PJLink does not push over TCP, so state is polled: power, input, mute,
//!   errors (and freeze and input resolution on Class 2) every 5 s; lamp and
//!   filter hours every minute; identity (name, manufacturer, product,
//!   class, inputs and their names, serial number, software version,
//!   replacement models) on the first connection, after the projector has
//!   been unreachable, and on `refresh`.
//! - Class 2 projectors can send status notifications (`%2LKUP`, `%2ERST`,
//!   `%2POWR`, `%2INPT`) by UDP to port 4352 of a controller address
//!   registered on the projector (§3.3). With the `notifications` setting the
//!   module listens there and applies them.

use std::collections::{HashSet, VecDeque};
use std::hash::{BuildHasher, Hasher};
use std::net::SocketAddr;

use md5::Md5;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

pub(crate) const DEFAULT_PORT: u16 = 4352;
const SOCKET: Key = "pjlink";
const NOTIFY: Key = "pjlink-notify";

/// §6 gives 2 s, deferring to each projector's own specification; slower
/// projectors are allowed for.
const REPLY_TIMEOUT: Millis = 5_000;
const POLL_EVERY: Millis = 5_000;
/// Lamp and filter hours are read every this many polls (one minute).
const SLOW_EVERY: u32 = 12;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// A response is at most 2 + 4 + 1 + 128 + 1 bytes (§2.2); anything far
/// longer is not PJLink.
const MAX_LINE: usize = 1_024;

const REPLY: Key = "reply";
const POLL: Key = "poll";

/// Bodies added by Class 2, sent with class `2` (§2.1).
const CLASS2: &[&str] = &[
    "SNUM", "SVER", "INNM", "IRES", "RRES", "FILT", "RLMP", "RFIL", "SVOL", "MVOL", "FREZ",
];

const IDENTITY: &[&str] = &[
    "CLSS", "NAME", "INF1", "INF2", "INFO", "INST", "SNUM", "SVER", "RLMP", "RFIL", "RRES",
];
const STATUS: &[&str] = &["POWR", "INPT", "AVMT", "ERST", "FREZ", "IRES"];
const SLOW: &[&str] = &["LAMP", "FILT"];

#[derive(Debug, Clone, Copy, PartialEq)]
enum AuthMode {
    Auto,
    Sha256,
    Md5,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Connecting,
    /// Connected, waiting for `PJLINK 0` or `PJLINK 1 <random>`.
    Greeting,
    /// Sent `PJLINK 2`, waiting for `PJLINK 2 <random>`.
    Level,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Status {
    Connecting,
    Connected,
    Disconnected,
}

/// How a response is read.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Ack,
    Power,
    Input,
    Inputs,
    InputName,
    Mute,
    Errors,
    Lamps,
    Name,
    Manufacturer,
    Product,
    OtherInfo,
    Class,
    Serial,
    SoftwareVersion,
    InputResolution,
    RecommendedResolution,
    FilterHours,
    LampReplacement,
    FilterReplacement,
    Freeze,
}

fn query_op(body: &str) -> Op {
    match body {
        "POWR" => Op::Power,
        "INPT" => Op::Input,
        "INST" => Op::Inputs,
        "INNM" => Op::InputName,
        "AVMT" => Op::Mute,
        "ERST" => Op::Errors,
        "LAMP" => Op::Lamps,
        "NAME" => Op::Name,
        "INF1" => Op::Manufacturer,
        "INF2" => Op::Product,
        "INFO" => Op::OtherInfo,
        "CLSS" => Op::Class,
        "SNUM" => Op::Serial,
        "SVER" => Op::SoftwareVersion,
        "IRES" => Op::InputResolution,
        "RRES" => Op::RecommendedResolution,
        "FILT" => Op::FilterHours,
        "RLMP" => Op::LampReplacement,
        "RFIL" => Op::FilterReplacement,
        "FREZ" => Op::Freeze,
        _ => Op::Ack,
    }
}

#[derive(Debug, Clone)]
struct Job {
    command: Option<CommandId>,
    body: &'static str,
    param: String,
    op: Op,
    /// A query sent after a set succeeds, so the state shows the result.
    follow: Option<&'static str>,
}

impl Job {
    fn query(body: &'static str, command: Option<CommandId>) -> Job {
        Job {
            command,
            body,
            param: "?".into(),
            op: query_op(body),
            follow: None,
        }
    }

    fn set(body: &'static str, param: String, follow: Option<&'static str>) -> Job {
        Job {
            command: None,
            body,
            param,
            op: Op::Ack,
            follow,
        }
    }
}

pub(crate) struct PjLink {
    device: SocketAddr,
    model_class: u8,
    /// What the projector answered to CLSS.
    reported_class: Option<u8>,
    password: String,
    auth: AuthMode,
    /// The projector did not answer `PJLINK 2`: use MD5 from now on.
    sha_unsupported: bool,
    notifications: bool,
    phase: Phase,
    status: Status,
    buf: Vec<u8>,
    /// The digest to put before the next command (the first of a session).
    prefix: Option<String>,
    commands: VecDeque<Job>,
    polls: VecDeque<Job>,
    in_flight: Option<Job>,
    /// Why the projector refused the password. Terminal.
    refused: Option<String>,
    retry_after: Millis,
    cycle: u32,
    need_identity: bool,
    /// Bodies the projector answered ERR1 (undefined) to: not polled again.
    unsupported: HashSet<&'static str>,
    inputs: Vec<String>,
    /// Fixed controller random number, for tests.
    fixed_random: Option<[u8; 16]>,
}

impl PjLink {
    pub(crate) fn new(ctx: OpenContext) -> Result<PjLink, String> {
        let model_class = match ctx.model.as_str() {
            "class-1" => 1,
            "class-2" => 2,
            other => return Err(format!("no PJLink model '{other}'")),
        };
        let s = &ctx.settings;
        let password = s
            .get("password")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let auth = match s.get("authentication").and_then(Value::as_str) {
            None | Some("auto") => AuthMode::Auto,
            Some("sha256") => AuthMode::Sha256,
            Some("md5") => AuthMode::Md5,
            Some(other) => return Err(format!("unknown authentication '{other}'")),
        };
        let notifications = s
            .get("notifications")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let port = ctx.port.unwrap_or(DEFAULT_PORT);
        Ok(PjLink::with(
            SocketAddr::new(ctx.host, port),
            model_class,
            password,
            auth,
            notifications,
        ))
    }

    fn with(
        device: SocketAddr,
        model_class: u8,
        password: String,
        auth: AuthMode,
        notifications: bool,
    ) -> PjLink {
        PjLink {
            device,
            model_class,
            reported_class: None,
            password,
            auth,
            sha_unsupported: false,
            notifications,
            phase: Phase::Closed,
            status: Status::Connecting,
            buf: Vec::new(),
            prefix: None,
            commands: VecDeque::new(),
            polls: VecDeque::new(),
            in_flight: None,
            refused: None,
            retry_after: RETRY_MIN,
            cycle: 0,
            need_identity: true,
            unsupported: HashSet::new(),
            inputs: Vec::new(),
            fixed_random: None,
        }
    }

    /// The class in use: the model's, lowered to what the projector reports.
    fn class(&self) -> u8 {
        self.reported_class
            .map_or(self.model_class, |c| c.min(self.model_class))
    }

    fn line(&self, job: &Job) -> String {
        let class = match job.body {
            "INPT" | "INST" if self.class() >= 2 => '2',
            body if CLASS2.contains(&body) => '2',
            _ => '1',
        };
        format!("%{class}{} {}", job.body, job.param)
    }

    fn queue_cycle(&mut self) {
        if !self.polls.is_empty() {
            return;
        }
        let mut bodies: Vec<&'static str> = Vec::new();
        if self.need_identity {
            self.need_identity = false;
            bodies.extend(IDENTITY);
        }
        bodies.extend(STATUS);
        if self.cycle.is_multiple_of(SLOW_EVERY) {
            bodies.extend(SLOW);
        }
        self.cycle = self.cycle.wrapping_add(1);
        for body in bodies {
            self.polls.push_back(Job::query(body, None));
        }
    }

    /// Whether a poll is worth sending to this projector.
    fn wanted(&self, job: &Job) -> bool {
        !(self.unsupported.contains(job.body) || (CLASS2.contains(&job.body) && self.class() < 2))
    }

    /// Send the next job, opening the connection if needed, or close an idle
    /// one (§5.4).
    fn pump(&mut self, cx: &mut Cx) {
        if self.refused.is_some() || self.in_flight.is_some() {
            return;
        }
        while let Some(job) = self.polls.front() {
            if self.wanted(job) {
                break;
            }
            self.polls.pop_front();
        }
        if self.commands.is_empty() && self.polls.is_empty() {
            if self.phase == Phase::Ready {
                cx.tcp_close(SOCKET);
                self.phase = Phase::Closed;
                self.buf.clear();
                self.prefix = None;
            }
            return;
        }
        match self.phase {
            Phase::Closed => {
                self.phase = Phase::Connecting;
                cx.tcp_open(SOCKET, self.device);
            }
            Phase::Ready => {
                let job = match self.commands.pop_front() {
                    Some(job) => job,
                    None => self.polls.pop_front().expect("checked above"),
                };
                let mut out = self.prefix.take().unwrap_or_default();
                out.push_str(&self.line(&job));
                out.push('\r');
                cx.tcp_send(SOCKET, out.into_bytes());
                cx.set_timer(REPLY, REPLY_TIMEOUT);
                self.in_flight = Some(job);
            }
            _ => {}
        }
    }

    fn set_status(&mut self, cx: &mut Cx, status: Status, reason: &str) {
        if self.status == status {
            return;
        }
        self.status = status;
        cx.connection(match status {
            Status::Connecting => Connection::Connecting,
            Status::Connected => Connection::Connected,
            Status::Disconnected => Connection::Disconnected {
                reason: reason.into(),
            },
        });
    }

    fn close_socket(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Closed {
            cx.tcp_close(SOCKET);
        }
        self.phase = Phase::Closed;
        self.buf.clear();
        self.prefix = None;
        cx.cancel_timer(REPLY);
    }

    /// The projector cannot be reached or stopped answering: fail what is
    /// waiting, and try again with backoff.
    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if let Some(Job {
            command: Some(id), ..
        }) = self.in_flight.take()
        {
            cx.complete(
                id,
                Err(CommandError::Transport {
                    message: reason.clone(),
                }),
            );
        }
        for job in self.commands.drain(..) {
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
        self.close_socket(cx);
        if self.status != Status::Disconnected {
            self.need_identity = true;
        }
        self.set_status(cx, Status::Disconnected, &reason);
        cx.log(Level::Debug, format!("PJLink: {reason}"));
        cx.set_timer(POLL, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// The projector refused the password. Never presented again.
    fn refuse(&mut self, cx: &mut Cx, reason: String) {
        if self.refused.is_some() {
            return;
        }
        let fail = |cx: &mut Cx, job: Job| {
            if let Some(id) = job.command {
                cx.complete(
                    id,
                    Err(CommandError::Auth {
                        message: reason.clone(),
                    }),
                );
            }
        };
        if let Some(job) = self.in_flight.take() {
            fail(cx, job);
        }
        for job in self.commands.drain(..) {
            fail(cx, job);
        }
        self.polls.clear();
        self.close_socket(cx);
        cx.cancel_timer(POLL);
        if self.notifications {
            cx.udp_close(NOTIFY);
        }
        cx.log(
            Level::Warning,
            format!("{reason}; nothing more is sent until the device is opened again with a corrected password"),
        );
        cx.connection(Connection::Unauthorized {
            reason: reason.clone(),
        });
        self.refused = Some(reason);
    }

    /// The projector did not take up `PJLINK 2`: reconnect with MD5 (§5.1
    /// step 5), unless SHA-256 was required.
    fn sha_fallback(&mut self, cx: &mut Cx, why: &str) {
        if self.auth == AuthMode::Sha256 {
            self.refuse(
                cx,
                format!("the projector does not offer PJLink 2.10 SHA-256 authentication ({why})"),
            );
            return;
        }
        self.sha_unsupported = true;
        cx.log(
            Level::Info,
            format!("the projector does not offer PJLink 2.10 SHA-256 authentication ({why}); reconnecting with MD5"),
        );
        self.close_socket(cx);
        self.pump(cx);
    }

    fn controller_random(&self) -> [u8; 16] {
        self.fixed_random.unwrap_or_else(random16)
    }

    fn greeting(&mut self, cx: &mut Cx, line: &str) {
        if line.eq_ignore_ascii_case("PJLINK 0") {
            self.ready(cx, None);
        } else if let Some(random) = strip_prefix_ci(line, "PJLINK 1 ") {
            if self.password.is_empty() {
                self.refuse(
                    cx,
                    "the projector requires a password (PJLINK 1) and none is set".into(),
                );
            } else if self.auth != AuthMode::Md5 && !self.sha_unsupported {
                cx.tcp_send(SOCKET, b"PJLINK 2\r".to_vec());
                cx.set_timer(REPLY, REPLY_TIMEOUT);
                self.phase = Phase::Level;
            } else {
                let digest = md5_digest(random, &self.password);
                self.ready(cx, Some(digest));
            }
        } else if line.eq_ignore_ascii_case("PJLINK ERRA") {
            self.refuse(
                cx,
                "the projector refused the password (PJLINK ERRA)".into(),
            );
        } else {
            self.lost(cx, format!("unexpected greeting '{line}'"));
        }
    }

    fn level(&mut self, cx: &mut Cx, line: &str) {
        let random = strip_prefix_ci(line, "PJLINK 2 ").and_then(parse_hex16);
        match random {
            Some(projector) => {
                let controller = self.controller_random();
                let prefix = sha256_prefix(&projector, &controller, &self.password);
                self.ready(cx, Some(prefix));
            }
            None => self.sha_fallback(cx, &format!("answered '{line}'")),
        }
    }

    fn ready(&mut self, cx: &mut Cx, prefix: Option<String>) {
        cx.cancel_timer(REPLY);
        self.phase = Phase::Ready;
        self.prefix = prefix;
        self.pump(cx);
    }

    fn on_line(&mut self, cx: &mut Cx, line: &str) {
        match self.phase {
            Phase::Greeting => self.greeting(cx, line),
            Phase::Level => self.level(cx, line),
            Phase::Ready => {
                if line.eq_ignore_ascii_case("PJLINK ERRA") {
                    self.refuse(
                        cx,
                        "the projector refused the password (PJLINK ERRA)".into(),
                    );
                    return;
                }
                let Some((body, param)) = parse_response(line) else {
                    cx.log(Level::Debug, format!("PJLink: ignored '{line}'"));
                    return;
                };
                let matches = self
                    .in_flight
                    .as_ref()
                    .is_some_and(|j| j.body.eq_ignore_ascii_case(body));
                if !matches {
                    cx.log(Level::Debug, format!("PJLink: unrequested '{line}'"));
                    return;
                }
                let job = self.in_flight.take().expect("checked above");
                self.answered(cx, job, param);
            }
            Phase::Closed | Phase::Connecting => {}
        }
    }

    fn answered(&mut self, cx: &mut Cx, job: Job, param: &str) {
        cx.cancel_timer(REPLY);
        self.retry_after = RETRY_MIN;
        self.set_status(cx, Status::Connected, "");
        let result = if let Some(code) = error_code(param) {
            self.on_error(cx, &job, code);
            Err(device_error(job.body, code))
        } else {
            match interpret(job.op, &job.param, param) {
                Err(e) => Err(CommandError::DeviceError {
                    code: None,
                    message: format!("{}: {e}", job.body),
                }),
                Ok((value, patch)) => {
                    if job.op == Op::Class {
                        self.reported_class = value.as_u64().map(|c| c.min(9) as u8);
                    }
                    let patch = self.complete_patch(job.op, patch);
                    if patch.as_object().is_some_and(|p| !p.is_empty()) {
                        cx.state(patch);
                    }
                    self.after_inputs(&job);
                    Ok(value)
                }
            }
        };
        if result.is_ok() {
            if let Some(follow) = job.follow {
                self.polls.push_front(Job::query(follow, None));
            }
        }
        if let Some(id) = job.command {
            cx.complete(
                id,
                result.map(|v| match job.op {
                    Op::Ack => Outcome::Ack,
                    _ => Outcome::Value { value: v },
                }),
            );
        }
        self.pump(cx);
    }

    /// Side effects of an error response on state and polling.
    fn on_error(&mut self, cx: &mut Cx, job: &Job, code: &str) {
        if code != "ERR1" || job.op == Op::Ack {
            return;
        }
        match job.body {
            "LAMP" => cx.state(json!({"lamp_count": 0})),
            "FILT" => cx.state(json!({"filter": {"installed": false}})),
            _ => {}
        }
        if job.command.is_none() {
            self.unsupported.insert(job.body);
        }
    }

    /// After a polled input list (Class 2), read each input's name (§4.17).
    fn after_inputs(&mut self, job: &Job) {
        if job.command.is_some() || job.op != Op::Inputs || self.class() < 2 {
            return;
        }
        for code in self.inputs.iter().rev() {
            self.polls.push_front(Job {
                param: format!("?{code}"),
                ..Job::query("INNM", None)
            });
        }
    }

    /// Patches that need the module's memory: inputs no longer listed are
    /// removed.
    fn complete_patch(&mut self, op: Op, patch: Value) -> Value {
        if op != Op::Inputs {
            return patch;
        }
        let mut inputs = patch["inputs"].as_object().cloned().unwrap_or_default();
        let listed: Vec<String> = inputs.keys().cloned().collect();
        for old in &self.inputs {
            if !inputs.contains_key(old) {
                inputs.insert(old.clone(), Value::Null);
            }
        }
        self.inputs = listed;
        json!({ "inputs": inputs })
    }

    fn data(&mut self, cx: &mut Cx, data: &[u8]) {
        if !matches!(self.phase, Phase::Greeting | Phase::Level | Phase::Ready) {
            return;
        }
        cx.alive();
        self.buf.extend_from_slice(data);
        while let Some(end) = self.buf.iter().position(|&b| b == b'\r' || b == b'\n') {
            let raw: Vec<u8> = self.buf.drain(..=end).collect();
            let line = String::from_utf8_lossy(&raw[..raw.len() - 1]).into_owned();
            if line.is_empty() {
                continue;
            }
            self.on_line(cx, &line);
            // A refusal, loss or reconnection discards the rest.
            if self.phase == Phase::Closed || self.phase == Phase::Connecting {
                self.buf.clear();
                return;
            }
        }
        if self.buf.len() > MAX_LINE {
            self.lost(cx, "a line longer than PJLink allows".into());
        }
    }

    fn command_job(&self, name: &str, p: &Params) -> Result<Job, CommandError> {
        let flag = |key: &str| {
            p.get(key)
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid(format!("'{key}' is required")))
        };
        let text = |key: &str| {
            p.get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(format!("'{key}' is required")))
        };
        let job = match name {
            "set_power" => Job::set("POWR", bit(flag("on")?), Some("POWR")),
            "set_input" => {
                let code = text("input")?;
                self.check_input(code)?;
                Job::set("INPT", code.into(), Some("INPT"))
            }
            "set_mute" => {
                let target = match text("target")? {
                    "video" => '1',
                    "audio" => '2',
                    "video_and_audio" => '3',
                    other => return Err(invalid(format!("unknown target '{other}'"))),
                };
                let muted = bit(flag("muted")?);
                Job::set("AVMT", format!("{target}{muted}"), Some("AVMT"))
            }
            "adjust_speaker_volume" | "adjust_microphone_volume" => {
                let step = match text("direction")? {
                    "up" => "1",
                    "down" => "0",
                    other => return Err(invalid(format!("unknown direction '{other}'"))),
                };
                let body = if name == "adjust_speaker_volume" {
                    "SVOL"
                } else {
                    "MVOL"
                };
                Job::set(body, step.into(), None)
            }
            "set_freeze" => Job::set("FREZ", bit(flag("frozen")?), Some("FREZ")),
            "get_input_name" => {
                let code = text("input")?;
                self.check_input(code)?;
                Job {
                    param: format!("?{code}"),
                    ..Job::query("INNM", None)
                }
            }
            _ => match query_body(name) {
                Some(body) => Job::query(body, None),
                None => {
                    return Err(CommandError::UnknownCommand {
                        command: name.into(),
                    })
                }
            },
        };
        if CLASS2.contains(&job.body) && self.class() < 2 {
            return Err(CommandError::DeviceError {
                code: None,
                message: format!(
                    "{}: the projector reports Class 1 (CLSS), which has no {}",
                    name, job.body
                ),
            });
        }
        Ok(job)
    }

    /// Input codes: source 1-5 and terminal 1-9 in Class 1 (§4.3), source
    /// 1-6 and terminal 1-9 or A-Z in Class 2.
    fn check_input(&self, code: &str) -> Result<(), CommandError> {
        let b = code.as_bytes();
        let ok = b.len() == 2
            && if self.class() >= 2 {
                (b'1'..=b'6').contains(&b[0])
                    && ((b[1].is_ascii_digit() && b[1] != b'0') || b[1].is_ascii_uppercase())
            } else {
                (b'1'..=b'5').contains(&b[0]) && (b'1'..=b'9').contains(&b[1])
            };
        if ok {
            Ok(())
        } else if self.class() >= 2 {
            Err(invalid(format!(
                "input '{code}' is not a Class 2 input: source 1-6 then terminal 1-9 or A-Z"
            )))
        } else {
            Err(invalid(format!(
                "input '{code}' is not a Class 1 input: source 1-5 then terminal 1-9"
            )))
        }
    }
}

impl Module for PjLink {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        if self.notifications {
            cx.udp_open(NOTIFY, Bind::Shared(DEFAULT_PORT));
        }
        self.queue_cycle();
        self.pump(cx);
        cx.set_timer(POLL, POLL_EVERY);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if let Some(reason) = &self.refused {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: reason.clone(),
                }),
            );
            return;
        }
        if name == "refresh" {
            self.need_identity = true;
            self.polls.clear();
            self.cycle = 0;
            self.queue_cycle();
            cx.complete(id, Ok(Outcome::Ack));
            self.pump(cx);
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
        if self.refused.is_some() {
            return;
        }
        match input {
            TcpInput::Connected => {
                if self.phase == Phase::Connecting {
                    self.phase = Phase::Greeting;
                    cx.set_timer(REPLY, REPLY_TIMEOUT);
                }
            }
            TcpInput::Data(data) => self.data(cx, &data),
            TcpInput::Closed { reason } => match self.phase {
                Phase::Closed => {}
                Phase::Level => {
                    self.phase = Phase::Closed;
                    self.sha_fallback(cx, &format!("closed the connection: {reason}"));
                }
                Phase::Ready
                    if self.in_flight.is_none()
                        && self.commands.is_empty()
                        && self.polls.is_empty() =>
                {
                    self.phase = Phase::Closed;
                }
                _ => {
                    self.phase = Phase::Closed;
                    self.lost(cx, reason);
                }
            },
        }
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, _from: SocketAddr, data: &[u8]) {
        if socket != NOTIFY || self.refused.is_some() {
            return;
        }
        cx.alive();
        let text = String::from_utf8_lossy(data);
        for line in text.split(['\r', '\n']).filter(|l| !l.is_empty()) {
            match notification(line) {
                Some(Notice::State(patch)) => cx.state(patch),
                Some(Notice::Linkup(patch)) => {
                    cx.state(patch);
                    self.need_identity = true;
                    self.queue_cycle();
                }
                Some(Notice::Poll) => self.queue_cycle(),
                None => cx.log(
                    Level::Debug,
                    format!("PJLink: ignored notification '{line}'"),
                ),
            }
        }
        self.pump(cx);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        let _ = socket;
        cx.log(
            Level::Warning,
            format!("cannot receive PJLink notifications on UDP {DEFAULT_PORT}: {message}; polling continues"),
        );
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if self.refused.is_some() {
            return;
        }
        match key {
            POLL => {
                self.queue_cycle();
                cx.set_timer(POLL, POLL_EVERY);
                self.pump(cx);
            }
            REPLY => match self.phase {
                Phase::Greeting => {
                    self.lost(cx, "connected, but no PJLINK greeting".into());
                }
                Phase::Level => self.sha_fallback(cx, "no answer to PJLINK 2"),
                Phase::Ready => {
                    if let Some(Job {
                        command: Some(id), ..
                    }) = self.in_flight.take()
                    {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.lost(cx, "the projector stopped answering".into());
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Closed {
            cx.tcp_close(SOCKET);
        }
        if self.notifications {
            cx.udp_close(NOTIFY);
        }
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn bit(on: bool) -> String {
    if on { "1" } else { "0" }.into()
}

/// The PJLink body a query command sends.
fn query_body(name: &str) -> Option<&'static str> {
    Some(match name {
        "get_power" => "POWR",
        "get_input" => "INPT",
        "get_inputs" => "INST",
        "get_mute" => "AVMT",
        "get_errors" => "ERST",
        "get_lamps" => "LAMP",
        "get_name" => "NAME",
        "get_manufacturer" => "INF1",
        "get_product_name" => "INF2",
        "get_other_info" => "INFO",
        "get_class" => "CLSS",
        "get_serial_number" => "SNUM",
        "get_software_version" => "SVER",
        "get_input_resolution" => "IRES",
        "get_recommended_resolution" => "RRES",
        "get_filter_hours" => "FILT",
        "get_lamp_replacement_models" => "RLMP",
        "get_filter_replacement_models" => "RFIL",
        "get_freeze" => "FREZ",
        _ => return None,
    })
}

fn strip_prefix_ci<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let head = line.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &line[prefix.len()..])
        .filter(|rest| !rest.is_empty())
}

/// `%<class><BODY>=<param>` (§2.2) to its body and parameter.
fn parse_response(line: &str) -> Option<(&str, &str)> {
    let b = line.as_bytes();
    if b.len() < 7 || b[0] != b'%' || b[6] != b'=' || !b[..7].is_ascii() {
        return None;
    }
    Some((&line[2..6], &line[7..]))
}

/// ERR1-ERR4 (§2.3, §2.4).
fn error_code(param: &str) -> Option<&'static str> {
    match param {
        "ERR1" => Some("ERR1"),
        "ERR2" => Some("ERR2"),
        "ERR3" => Some("ERR3"),
        "ERR4" => Some("ERR4"),
        _ => None,
    }
}

/// An error response, with the meaning chapter 4 gives it for the command.
fn device_error(body: &str, code: &str) -> CommandError {
    let meaning = match (body, code) {
        ("LAMP", "ERR1") => "no lamp",
        ("FILT", "ERR1") => "no filter",
        ("SVOL", "ERR1") => "no speaker installed",
        ("MVOL", "ERR1") => "no microphone installed",
        ("FREZ", "ERR1") => "freeze is not supported",
        ("INPT", "ERR2") => "nonexistent input source",
        ("AVMT", "ERR2") => "out of parameter: the projector has no separate audio or video mute",
        (_, "ERR1") => "undefined command: the projector does not support it",
        (_, "ERR2") => "out of parameter",
        (_, "ERR3") => {
            "unavailable time: the projector cannot accept it now (standby, warm-up, cooling, input switching or lamp ignition)"
        }
        _ => "projector/display failure",
    };
    CommandError::DeviceError {
        code: Some(code.into()),
        message: format!("{body}: {meaning}"),
    }
}

/// The result and state patch for a successful response.
fn interpret(op: Op, sent: &str, p: &str) -> Result<(Value, Value), String> {
    let unexpected = || format!("unexpected response '{p}'");
    Ok(match op {
        Op::Ack => {
            if p.eq_ignore_ascii_case("OK") {
                (Value::Bool(true), json!({}))
            } else {
                return Err(unexpected());
            }
        }
        Op::Power => {
            let v = power(p).ok_or_else(unexpected)?;
            (json!(v), json!({ "power": v }))
        }
        Op::Input => {
            let v = input(p).ok_or_else(unexpected)?;
            (v.clone(), json!({ "input": v }))
        }
        Op::Inputs => {
            let mut list = Vec::new();
            let mut map = Map::new();
            for code in p.split(' ').filter(|c| !c.is_empty()) {
                let v = input(code).ok_or_else(unexpected)?;
                map.insert(
                    code.to_string(),
                    json!({"source": v["source"], "terminal": v["terminal"]}),
                );
                list.push(v);
            }
            (Value::Array(list), json!({ "inputs": map }))
        }
        Op::InputName => {
            let code = sent.trim_start_matches('?');
            (json!(p), json!({ "inputs": { code: { "name": p } } }))
        }
        Op::Mute => {
            let v = mute(p).ok_or_else(unexpected)?;
            (v.clone(), json!({ "mute": v }))
        }
        Op::Errors => {
            let v = errors(p).ok_or_else(unexpected)?;
            (v.clone(), json!({ "errors": v }))
        }
        Op::Lamps => {
            let lamps = lamps(p).ok_or_else(unexpected)?;
            let mut map = Map::new();
            for (i, lamp) in lamps.iter().enumerate() {
                map.insert((i + 1).to_string(), lamp.clone());
            }
            (
                Value::Array(lamps.clone()),
                json!({ "lamp_count": lamps.len(), "lamps": map }),
            )
        }
        Op::Name => (json!(p), json!({ "name": p })),
        Op::Manufacturer => (json!(p), json!({ "info": { "manufacturer": p } })),
        Op::Product => (json!(p), json!({ "info": { "product": p } })),
        Op::OtherInfo => (json!(p), json!({ "info": { "other": p } })),
        Op::Serial => (json!(p), json!({ "info": { "serial_number": p } })),
        Op::SoftwareVersion => (json!(p), json!({ "info": { "software_version": p } })),
        Op::Class => {
            let class: u64 = p
                .parse()
                .ok()
                .filter(|_| p.len() == 1)
                .ok_or_else(unexpected)?;
            (json!(class), json!({ "info": { "class": class } }))
        }
        Op::InputResolution => {
            let v = match p {
                "-" => json!({"status": "none", "horizontal": null, "vertical": null}),
                "*" => json!({"status": "unknown", "horizontal": null, "vertical": null}),
                _ => {
                    let (h, v) = resolution(p).ok_or_else(unexpected)?;
                    json!({"status": "present", "horizontal": h, "vertical": v})
                }
            };
            (v.clone(), json!({ "signal": v }))
        }
        Op::RecommendedResolution => {
            let (h, v) = resolution(p).ok_or_else(unexpected)?;
            let v = json!({"horizontal": h, "vertical": v});
            (v.clone(), json!({ "recommended_resolution": v }))
        }
        Op::FilterHours => {
            let hours = hours(p).ok_or_else(unexpected)?;
            (
                json!(hours),
                json!({ "filter": { "hours": hours, "installed": true } }),
            )
        }
        Op::LampReplacement | Op::FilterReplacement => {
            let models: Vec<&str> = p.split(' ').filter(|m| !m.is_empty()).collect();
            let key = if op == Op::LampReplacement {
                "lamp"
            } else {
                "filter"
            };
            (
                json!(models),
                json!({ "replacement_models": { key: models } }),
            )
        }
        Op::Freeze => {
            let v = match p {
                "1" => true,
                "0" => false,
                _ => return Err(unexpected()),
            };
            (json!(v), json!({ "freeze": v }))
        }
    })
}

/// POWR ? (§4.2).
fn power(p: &str) -> Option<&'static str> {
    Some(match p {
        "0" => "off",
        "1" => "on",
        "2" => "cooling",
        "3" => "warming",
        _ => return None,
    })
}

/// An input number: source 1-6, terminal 1-9 or A-Z (§4.3, §4.4).
fn input(code: &str) -> Option<Value> {
    let b = code.as_bytes();
    if b.len() != 2 {
        return None;
    }
    let source = match b[0] {
        b'1' => "rgb",
        b'2' => "video",
        b'3' => "digital",
        b'4' => "storage",
        b'5' => "network",
        b'6' => "internal",
        _ => return None,
    };
    let t = b[1].to_ascii_uppercase();
    if !((b'1'..=b'9').contains(&t) || t.is_ascii_uppercase()) {
        return None;
    }
    Some(json!({
        "code": code.to_ascii_uppercase(),
        "source": source,
        "terminal": (t as char).to_string(),
    }))
}

/// AVMT ? (§4.6): 11 video, 21 audio, 31 both muted, 30 neither. 10 and 20
/// fit the documented character ranges but are not in its table; each says
/// only that one is not muted.
fn mute(p: &str) -> Option<Value> {
    Some(match p {
        "11" => json!({"video": true, "audio": false}),
        "21" => json!({"video": false, "audio": true}),
        "31" => json!({"video": true, "audio": true}),
        "30" => json!({"video": false, "audio": false}),
        "10" => json!({"video": false}),
        "20" => json!({"audio": false}),
        _ => return None,
    })
}

/// ERST ? (§4.7): fan, lamp, temperature, cover open, filter, other.
fn errors(p: &str) -> Option<Value> {
    let b = p.as_bytes();
    if b.len() != 6 {
        return None;
    }
    let mut map = Map::new();
    let names = [
        "fan",
        "lamp",
        "temperature",
        "cover_open",
        "filter",
        "other",
    ];
    for (name, c) in names.iter().zip(b) {
        let v = match c {
            b'0' => "ok",
            b'1' => "warning",
            b'2' => "error",
            _ => return None,
        };
        map.insert(name.to_string(), json!(v));
    }
    Some(Value::Object(map))
}

/// LAMP ? (§4.8): pairs of hours (0-99999) and on (1) or off (0), 1-8 lamps.
fn lamps(p: &str) -> Option<Vec<Value>> {
    let parts: Vec<&str> = p.split(' ').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() || !parts.len().is_multiple_of(2) || parts.len() > 16 {
        return None;
    }
    parts
        .chunks(2)
        .map(|pair| {
            let hours = hours(pair[0])?;
            let on = match pair[1] {
                "1" => true,
                "0" => false,
                _ => return None,
            };
            Some(json!({"hours": hours, "on": on}))
        })
        .collect()
}

/// One- to five-digit hours (§4.8, §4.20).
fn hours(p: &str) -> Option<u32> {
    (!p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()))
        .then(|| p.parse().ok())
        .flatten()
}

/// `<horizontal>x<vertical>` (§4.18, §4.19).
fn resolution(p: &str) -> Option<(u64, u64)> {
    let (h, v) = p.split_once(['x', 'X'])?;
    let num = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    Some((num(h)?, num(v)?))
}

enum Notice {
    State(Value),
    Linkup(Value),
    Poll,
}

/// A status notification (§3.3.2).
fn notification(line: &str) -> Option<Notice> {
    let (body, param) = parse_response(line)?;
    match body.to_ascii_uppercase().as_str() {
        "LKUP" => Some(Notice::Linkup(json!({ "mac_address": param }))),
        "ERST" => errors(param).map(|v| Notice::State(json!({ "errors": v }))),
        "INPT" => input(param).map(|v| Notice::State(json!({ "input": v }))),
        // 0 is standby or cooling, 1 on or warm-up: ask which.
        "POWR" if param == "0" || param == "1" => Some(Notice::Poll),
        _ => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_hex16(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Class 1 §5.1 (1-3): MD5 of the projector's random number followed by the
/// password, as 32 lowercase hex digits.
fn md5_digest(random: &str, password: &str) -> String {
    hex(&Md5::digest(format!("{random}{password}").as_bytes()))
}

/// Class 2 v2.10 §5.1 (1-4), (1-5): the controller's random number, then
/// SHA-256 of the hex of both random numbers XORed followed by the password.
fn sha256_prefix(projector: &[u8; 16], controller: &[u8; 16], password: &str) -> String {
    let mut xor = [0u8; 16];
    for (i, b) in xor.iter_mut().enumerate() {
        *b = projector[i] ^ controller[i];
    }
    let digest = Sha256::digest(format!("{}{password}", hex(&xor)).as_bytes());
    format!("{}{}", hex(controller), hex(&digest))
}

/// 16 unpredictable bytes. `RandomState` is keyed from the operating
/// system's random source, and SipHash's output cannot be predicted without
/// the key; the core has no other random source.
fn random16() -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, chunk) in out.chunks_mut(8).enumerate() {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_usize(i);
        chunk.copy_from_slice(&h.finish().to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    const PASSWORD: &str = "JBMIAProjectorLink";

    fn module(class: u8, password: &str, auth: AuthMode) -> PjLink {
        let mut m = PjLink::with(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 30)), DEFAULT_PORT),
            class,
            password.into(),
            auth,
            false,
        );
        // Class 2 v2.10 §5.1: the controller random number of the example.
        m.fixed_random = Some(parse_hex16("c14b279e603d8a5f17e849ba360f2dc5").unwrap());
        m
    }

    fn sent(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(String::from_utf8(data.clone()).unwrap()),
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

    fn feed(m: &mut PjLink, data: &str) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(data.as_bytes().to_vec()));
        cx.take()
    }

    fn run(m: &mut PjLink, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    /// Started, connected and greeted; the first poll has gone out.
    fn greeted(m: &mut PjLink, greeting: &str) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(greeting.as_bytes().to_vec()),
        );
        cx.take()
    }

    /// Answer every poll with ERR3 until the module goes idle.
    fn drain(m: &mut PjLink) {
        while let Some(job) = m.in_flight.clone() {
            feed(m, &format!("%1{}=ERR3\r", job.body));
        }
    }

    #[test]
    fn spec_example_digests() {
        // Class 1 v1.04 §5.1 (1-3).
        assert_eq!(
            md5_digest("498e4a67", PASSWORD),
            "5d8409bc1c3fa39749434aa3a5c38682"
        );
        // Class 2 v2.10 §5.1 (1-4), (1-5).
        let p = parse_hex16("3db25e10f69c47a85adb24cf361897e0").unwrap();
        let c = parse_hex16("c14b279e603d8a5f17e849ba360f2dc5").unwrap();
        assert_eq!(
            sha256_prefix(&p, &c, PASSWORD),
            "c14b279e603d8a5f17e849ba360f2dc5\
             70ab129eac6924d4c129c9c4fcf45be42e1e776325a66db67e5b6eee50f2a692"
        );
    }

    #[test]
    fn md5_authentication_prefixes_the_first_command_only() {
        let mut m = module(1, PASSWORD, AuthMode::Md5);
        let a = greeted(&mut m, "PJLINK 1 498e4a67\r");
        // The first poll is CLSS, carrying the digest (Class 1 §5.1 (1-2)).
        assert_eq!(sent(&a), ["5d8409bc1c3fa39749434aa3a5c38682%1CLSS ?\r"]);
        let a = feed(&mut m, "%1CLSS=1\r");
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(sent(&a), ["%1NAME ?\r"]);
        assert_eq!(state(&a)["info"]["class"], 1);
    }

    #[test]
    fn sha256_authentication_matches_the_v2_10_example() {
        let mut m = module(2, PASSWORD, AuthMode::Auto);
        // §5.1 (1-1) then (1-2).
        let a = greeted(&mut m, "PJLINK 1 498e4a67\r");
        assert_eq!(sent(&a), ["PJLINK 2\r"]);
        // (1-3), then the command as in (1-4), here a power on.
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            7,
            "set_power",
            json!({"on": true}).as_object().unwrap(),
        );
        let a = feed(&mut m, "PJLINK 2 3db25e10f69c47a85adb24cf361897e0\r");
        assert_eq!(
            sent(&a),
            ["c14b279e603d8a5f17e849ba360f2dc5\
              70ab129eac6924d4c129c9c4fcf45be42e1e776325a66db67e5b6eee50f2a692%1POWR 1\r"]
        );
        // §4.1 OK, then the follow-up query.
        let a = feed(&mut m, "%1POWR=OK\r");
        assert_eq!(completed(&a, 7), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a), ["%1POWR ?\r"]);
        let a = feed(&mut m, "%1POWR=3\r");
        assert_eq!(state(&a)["power"], "warming");
    }

    #[test]
    fn a_projector_without_sha256_is_reconnected_with_md5() {
        let mut m = module(1, PASSWORD, AuthMode::Auto);
        greeted(&mut m, "PJLINK 1 498e4a67\r");
        // An older projector reads PJLINK 2 as a bad digest.
        let a = feed(&mut m, "PJLINK ERRA\r");
        assert!(a.contains(&Action::TcpClose { socket: SOCKET }));
        assert!(a.iter().any(|x| matches!(x, Action::TcpOpen { .. })));
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"PJLINK 1 498e4a67\r".to_vec()),
        );
        assert_eq!(
            sent(&cx.take()),
            ["5d8409bc1c3fa39749434aa3a5c38682%1CLSS ?\r"]
        );
    }

    #[test]
    fn erra_is_terminal() {
        let mut m = module(1, "wrong", AuthMode::Md5);
        greeted(&mut m, "PJLINK 1 498e4a67\r");
        let a = feed(&mut m, "PJLINK ERRA\r");
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(a.contains(&Action::TcpClose { socket: SOCKET }));
        assert!(a.contains(&Action::CancelTimer { key: POLL }));
        // Nothing is sent again: not on a poll, not for a command.
        let mut cx = Cx::new(0);
        m.timer(&mut cx, POLL);
        assert!(cx.take().is_empty());
        let a = run(&mut m, 3, "get_power", json!({}));
        assert!(matches!(
            completed(&a, 3),
            Some(Err(CommandError::Auth { .. }))
        ));
        assert!(sent(&a).is_empty());
        assert!(!a.iter().any(|x| matches!(x, Action::TcpOpen { .. })));
    }

    #[test]
    fn a_password_is_required_when_the_projector_asks() {
        let mut m = module(1, "", AuthMode::Auto);
        let a = greeted(&mut m, "PJLINK 1 498e4a67\r");
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(sent(&a).is_empty());
    }

    #[test]
    fn command_lines_are_byte_exact() {
        let mut m = module(2, "", AuthMode::Auto);
        greeted(&mut m, "PJLINK 0\r");
        feed(&mut m, "%1CLSS=2\r");
        drain(&mut m);
        let cases: &[(&str, Value, &str)] = &[
            // §4.1, §4.2
            ("set_power", json!({"on": true}), "%1POWR 1\r"),
            ("set_power", json!({"on": false}), "%1POWR 0\r"),
            ("get_power", json!({}), "%1POWR ?\r"),
            // §4.3 (Class 2 form), §4.4
            ("set_input", json!({"input": "31"}), "%2INPT 31\r"),
            ("set_input", json!({"input": "6A"}), "%2INPT 6A\r"),
            ("get_input", json!({}), "%2INPT ?\r"),
            // §4.5
            (
                "set_mute",
                json!({"target": "video", "muted": true}),
                "%1AVMT 11\r",
            ),
            (
                "set_mute",
                json!({"target": "audio", "muted": false}),
                "%1AVMT 20\r",
            ),
            (
                "set_mute",
                json!({"target": "video_and_audio", "muted": true}),
                "%1AVMT 31\r",
            ),
            ("get_mute", json!({}), "%1AVMT ?\r"),
            ("get_errors", json!({}), "%1ERST ?\r"),
            ("get_lamps", json!({}), "%1LAMP ?\r"),
            ("get_inputs", json!({}), "%2INST ?\r"),
            ("get_name", json!({}), "%1NAME ?\r"),
            ("get_manufacturer", json!({}), "%1INF1 ?\r"),
            ("get_product_name", json!({}), "%1INF2 ?\r"),
            ("get_other_info", json!({}), "%1INFO ?\r"),
            ("get_class", json!({}), "%1CLSS ?\r"),
            ("get_serial_number", json!({}), "%2SNUM ?\r"),
            ("get_software_version", json!({}), "%2SVER ?\r"),
            // §4.17's example.
            ("get_input_name", json!({"input": "31"}), "%2INNM ?31\r"),
            ("get_input_resolution", json!({}), "%2IRES ?\r"),
            ("get_recommended_resolution", json!({}), "%2RRES ?\r"),
            ("get_filter_hours", json!({}), "%2FILT ?\r"),
            ("get_lamp_replacement_models", json!({}), "%2RLMP ?\r"),
            ("get_filter_replacement_models", json!({}), "%2RFIL ?\r"),
            (
                "adjust_speaker_volume",
                json!({"direction": "up"}),
                "%2SVOL 1\r",
            ),
            (
                "adjust_microphone_volume",
                json!({"direction": "down"}),
                "%2MVOL 0\r",
            ),
            ("set_freeze", json!({"frozen": true}), "%2FREZ 1\r"),
            ("get_freeze", json!({}), "%2FREZ ?\r"),
        ];
        for (i, (name, params, wire)) in cases.iter().enumerate() {
            let id = i as CommandId + 100;
            let a = run(&mut m, id, name, params.clone());
            // A new connection for each, since the last closed when idle.
            let mut cx = Cx::new(0);
            assert!(
                a.iter().any(|x| matches!(x, Action::TcpOpen { .. })),
                "{name}"
            );
            m.tcp(&mut cx, SOCKET, TcpInput::Connected);
            m.tcp(&mut cx, SOCKET, TcpInput::Data(b"PJLINK 0\r".to_vec()));
            assert_eq!(sent(&cx.take()), [*wire], "{name}");
            let body = &wire[2..6];
            feed(&mut m, &format!("%{}{body}=ERR4\r", &wire[1..2]));
            drain(&mut m);
        }
    }

    #[test]
    fn class_1_refuses_class_2_inputs_and_commands() {
        let mut m = module(1, "", AuthMode::Auto);
        greeted(&mut m, "PJLINK 0\r");
        let a = run(&mut m, 1, "set_input", json!({"input": "6A"}));
        assert!(matches!(
            completed(&a, 1),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        // A Class 2 model on a projector that reports Class 1.
        let mut m = module(2, "", AuthMode::Auto);
        greeted(&mut m, "PJLINK 0\r");
        feed(&mut m, "%1CLSS=1\r");
        let a = run(&mut m, 2, "get_freeze", json!({}));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { code: None, .. }))
        ));
        // And INPT drops to %1.
        drain(&mut m);
        run(&mut m, 3, "get_input", json!({}));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(b"PJLINK 0\r".to_vec()));
        assert_eq!(sent(&cx.take()), ["%1INPT ?\r"]);
    }

    #[test]
    fn responses_become_values_and_state() {
        let r = |op, p| interpret(op, "?", p).unwrap();
        assert_eq!(r(Op::Power, "0").0, "off");
        assert_eq!(r(Op::Power, "2").0, "cooling");
        assert_eq!(
            r(Op::Input, "11").0,
            json!({"code": "11", "source": "rgb", "terminal": "1"})
        );
        // §4.3: source 3 is DIGITAL (§4.17's table calls 31 "RGB1").
        assert_eq!(r(Op::Input, "31").0["source"], "digital");
        assert_eq!(r(Op::Input, "6Z").0["source"], "internal");
        assert_eq!(r(Op::Mute, "31").0, json!({"video": true, "audio": true}));
        assert_eq!(r(Op::Mute, "21").0, json!({"video": false, "audio": true}));
        assert_eq!(
            r(Op::Errors, "000120").0,
            json!({"fan": "ok", "lamp": "ok", "temperature": "ok",
                   "cover_open": "warning", "filter": "error", "other": "ok"})
        );
        // §4.8: two lamps.
        let (v, patch) = r(Op::Lamps, "12345 1 99999 0");
        assert_eq!(
            v,
            json!([{"hours": 12345, "on": true}, {"hours": 99999, "on": false}])
        );
        assert_eq!(patch["lamp_count"], 2);
        assert_eq!(patch["lamps"]["2"]["hours"], 99999);
        // §4.10: no name is an empty parameter.
        assert_eq!(r(Op::Name, "").0, "");
        assert_eq!(r(Op::Class, "2").0, 2);
        assert_eq!(
            r(Op::InputResolution, "1920x1080").0,
            json!({"status": "present", "horizontal": 1920, "vertical": 1080})
        );
        assert_eq!(r(Op::InputResolution, "-").0["status"], "none");
        assert_eq!(r(Op::InputResolution, "*").0["status"], "unknown");
        assert_eq!(r(Op::FilterHours, "250").0, 250);
        assert_eq!(r(Op::LampReplacement, "").0, json!([]));
        assert_eq!(
            r(Op::FilterReplacement, "AB-1 AB-2").0,
            json!(["AB-1", "AB-2"])
        );
        assert_eq!(r(Op::Freeze, "1").0, true);
        let (v, patch) = interpret(Op::InputName, "?31", "PC").unwrap();
        assert_eq!(v, "PC");
        assert_eq!(patch["inputs"]["31"]["name"], "PC");
        assert!(interpret(Op::Power, "?", "7").is_err());
        assert!(interpret(Op::Ack, "?", "NG").is_err());
        assert!(interpret(Op::Lamps, "?", "123456 1").is_err());
    }

    #[test]
    fn error_responses_map_to_device_errors() {
        let mut m = module(1, "", AuthMode::Auto);
        greeted(&mut m, "PJLINK 0\r");
        drain(&mut m);
        run(&mut m, 5, "set_input", json!({"input": "59"}));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(b"PJLINK 0\r".to_vec()));
        let a = feed(&mut m, "%1INPT=ERR2\r");
        match completed(&a, 5) {
            Some(Err(CommandError::DeviceError { code, message })) => {
                assert_eq!(code.as_deref(), Some("ERR2"));
                assert!(message.contains("nonexistent input source"));
            }
            other => panic!("{other:?}"),
        }
        // No follow-up query after a failure; the connection closes when idle.
        assert!(sent(&a).is_empty());
        assert!(a.contains(&Action::TcpClose { socket: SOCKET }));
    }

    #[test]
    fn a_lamp_less_projector_is_not_asked_again() {
        let mut m = module(1, "", AuthMode::Auto);
        greeted(&mut m, "PJLINK 0\r");
        let mut lamp_state = json!({});
        while let Some(job) = m.in_flight.clone() {
            let reply = if job.body == "LAMP" { "ERR1" } else { "ERR3" };
            let a = feed(&mut m, &format!("%1{}={reply}\r", job.body));
            merge_patch(&mut lamp_state, &state(&a));
        }
        assert_eq!(lamp_state["lamp_count"], 0);
        assert!(m.unsupported.contains("LAMP"));
    }

    #[test]
    fn inputs_and_their_names_are_read_on_class_2() {
        let mut m = module(2, "", AuthMode::Auto);
        greeted(&mut m, "PJLINK 0\r");
        let mut all = json!({});
        let mut lines = Vec::new();
        while let Some(job) = m.in_flight.clone() {
            let reply = match (job.body, job.param.as_str()) {
                ("CLSS", _) => "2".to_string(),
                ("INST", _) => "11 31 5A".to_string(),
                ("INNM", "?31") => "PC".to_string(),
                ("INNM", "?11") => "VGA".to_string(),
                ("INNM", _) => "Media".to_string(),
                _ => "ERR3".to_string(),
            };
            lines.push(m.line(&job));
            let a = feed(&mut m, &format!("%2{}={reply}\r", job.body));
            merge_patch(&mut all, &state(&a));
        }
        assert!(lines.contains(&"%2INNM ?5A".to_string()));
        assert_eq!(all["inputs"]["31"]["name"], "PC");
        assert_eq!(all["inputs"]["5A"]["source"], "network");
        assert_eq!(all["inputs"]["11"]["name"], "VGA");
    }

    #[test]
    fn a_silent_projector_times_out_and_is_retried_with_backoff() {
        let mut m = module(1, "", AuthMode::Auto);
        greeted(&mut m, "PJLINK 0\r");
        drain(&mut m);
        run(&mut m, 9, "get_power", json!({}));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(b"PJLINK 0\r".to_vec()));
        let mut cx = Cx::new(REPLY_TIMEOUT);
        m.timer(&mut cx, REPLY);
        let a = cx.take();
        assert_eq!(completed(&a, 9), Some(Err(CommandError::Timeout)));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::SetTimer {
            key: POLL,
            after: RETRY_MIN
        }));
        // A refused connection doubles the wait.
        let mut cx = Cx::new(0);
        m.timer(&mut cx, POLL);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "refused".into(),
            },
        );
        assert!(cx.take().contains(&Action::SetTimer {
            key: POLL,
            after: RETRY_MIN * 2
        }));
    }

    #[test]
    fn notifications_update_state() {
        // §3.3.2.
        match notification("%2ERST=000020") {
            Some(Notice::State(v)) => assert_eq!(v["errors"]["filter"], "error"),
            _ => panic!(),
        }
        match notification("%2INPT=32") {
            Some(Notice::State(v)) => assert_eq!(v["input"]["code"], "32"),
            _ => panic!(),
        }
        match notification("%2LKUP=00:11:22:33:44:55") {
            Some(Notice::Linkup(v)) => assert_eq!(v["mac_address"], "00:11:22:33:44:55"),
            _ => panic!(),
        }
        assert!(matches!(notification("%2POWR=1"), Some(Notice::Poll)));
        assert!(notification("%2SRCH").is_none());
    }
}
