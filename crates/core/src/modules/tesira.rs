//! Biamp Tesira over the Tesira Text Protocol (TTP), Telnet on TCP 23.
//!
//! From Biamp's Tesira Text Protocol manual (v4.2) and its Cornerstone
//! articles on TTP and Telnet session negotiation (see the spec's sources):
//!
//! - The Telnet server negotiates options as soon as the connection opens and
//!   goes no further until each is answered. The module refuses all of them
//!   (`DO` answered `WON'T`, `WILL` answered `DON'T`), as Biamp describes for
//!   a raw connection, at any point in the session, and drops the Telnet
//!   bytes from what it reads.
//! - A protected system asks for a user name and password before the
//!   welcome banner; the module answers the prompts with the configured
//!   credentials only when they are asked for. Commands are sent only after
//!   "Welcome to the Tesira Text Protocol Server".
//! - Everything is addressed by the instance tags of the design, so commands
//!   take the tag as a parameter: `Level1 set level 1 -10`, answered `+OK`,
//!   `+OK "value":-10.000000` or `-ERR ...`. Answers do not say what they
//!   answer, so commands go one at a time and a missing answer resets the
//!   connection.
//! - Subscriptions push `! "publishToken":"<label>" "value":<v>` on change.
//!   They belong to the session: the module records each one with the label
//!   it chose and subscribes again after reconnecting. Values read with
//!   `get` and pushed by subscriptions land in state at
//!   `blocks.<tag>.<attribute>[.<index>...]`.
//!
//! Opened for commands only, it reads nothing on connecting, takes no
//! subscription from the settings and reads nothing back after a change.
//! Subscriptions the consumer asked for with `subscribe` are still made (and
//! made again after a reconnection), since they are commands. The serial
//! number is asked when the session is idle for 10 s in either mode, as the
//! liveness check: Tesira says nothing unasked otherwise.

use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

const PORT: u16 = 23;
const SOCKET: Key = "tesira";

/// How long a command may wait for its `+OK` or `-ERR`.
const REPLY_TIMEOUT: Millis = 5_000;
/// From connecting to the welcome banner, negotiation and login included.
const WELCOME_TIMEOUT: Millis = 15_000;
/// Ask something after this long without traffic, as the liveness check.
const IDLE_PROBE: Millis = 10_000;
/// Nothing at all from the server for this long: the connection is dead.
const SILENCE_TIMEOUT: Millis = 30_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

const REPLY: Key = "reply";
const WELCOME: Key = "welcome";
const IDLE: Key = "idle";
const SILENCE: Key = "silence";
const RETRY: Key = "retry";

const BANNER: &str = "Welcome to the Tesira Text Protocol Server";

const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;

// ── Telnet ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum TelnetState {
    #[default]
    Data,
    Iac,
    /// After IAC and a negotiation verb; the option byte comes next.
    Option(u8),
    Sub,
    SubIac,
}

/// Separates Telnet commands from data: answers every option negotiation
/// with a refusal and passes the rest on. CR NUL, Telnet's bare carriage
/// return, becomes CR.
#[derive(Debug, Default)]
struct Telnet {
    state: TelnetState,
    after_cr: bool,
}

impl Telnet {
    /// Returns the data bytes and the bytes to send back.
    fn feed(&mut self, input: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut data = Vec::with_capacity(input.len());
        let mut answer = Vec::new();
        for &b in input {
            self.state = match self.state {
                TelnetState::Data => {
                    if b == IAC {
                        TelnetState::Iac
                    } else {
                        if !(b == 0 && self.after_cr) {
                            data.push(b);
                        }
                        self.after_cr = b == b'\r';
                        TelnetState::Data
                    }
                }
                TelnetState::Iac => match b {
                    IAC => {
                        data.push(IAC);
                        TelnetState::Data
                    }
                    WILL | WONT | DO | DONT => TelnetState::Option(b),
                    SB => TelnetState::Sub,
                    // NOP, GA and the other two-byte commands carry nothing.
                    _ => TelnetState::Data,
                },
                TelnetState::Option(verb) => {
                    match verb {
                        DO => answer.extend_from_slice(&[IAC, WONT, b]),
                        WILL => answer.extend_from_slice(&[IAC, DONT, b]),
                        // WON'T and DON'T agree with what we want: nothing on.
                        _ => {}
                    }
                    TelnetState::Data
                }
                TelnetState::Sub => {
                    if b == IAC {
                        TelnetState::SubIac
                    } else {
                        TelnetState::Sub
                    }
                }
                TelnetState::SubIac => {
                    if b == SE {
                        TelnetState::Data
                    } else {
                        TelnetState::Sub
                    }
                }
            };
        }
        (data, answer)
    }
}

// ── TTP values ────────────────────────────────────────────────────────────

/// A TTP value: JSON-like, with values separated by spaces rather than
/// commas, and bare words for enumerations (`LINK_1_GB`). Returns the value
/// and the rest of the text.
fn parse_value(s: &str) -> Option<(Value, &str)> {
    let s = s.trim_start();
    let first = s.chars().next()?;
    match first {
        '"' => {
            let mut out = String::new();
            let mut chars = s.char_indices().skip(1);
            while let Some((i, c)) = chars.next() {
                match c {
                    '\\' => {
                        if let Some((_, n)) = chars.next() {
                            out.push(n);
                        }
                    }
                    '"' => return Some((Value::String(out), &s[i + 1..])),
                    c => out.push(c),
                }
            }
            None
        }
        '[' => {
            let mut rest = &s[1..];
            let mut items = Vec::new();
            loop {
                rest = rest.trim_start().trim_start_matches(',').trim_start();
                if let Some(r) = rest.strip_prefix(']') {
                    return Some((Value::Array(items), r));
                }
                let (v, r) = parse_value(rest)?;
                items.push(v);
                rest = r;
            }
        }
        '{' => {
            let mut rest = &s[1..];
            let mut map = Map::new();
            loop {
                rest = rest.trim_start().trim_start_matches(',').trim_start();
                if let Some(r) = rest.strip_prefix('}') {
                    return Some((Value::Object(map), r));
                }
                let (key, r) = parse_value(rest)?;
                let Value::String(key) = key else {
                    return None;
                };
                let r = r.trim_start().strip_prefix(':')?;
                let (v, r) = parse_value(r)?;
                map.insert(key, v);
                rest = r;
            }
        }
        _ => {
            let end = s
                .find(|c: char| c.is_whitespace() || matches!(c, ']' | '}' | ','))
                .unwrap_or(s.len());
            let word = &s[..end];
            if word.is_empty() {
                return None;
            }
            let value = match word {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                "null" => Value::Null,
                w => {
                    if let Ok(i) = w.parse::<i64>() {
                        json!(i)
                    } else if let Some(f) = w.parse::<f64>().ok().filter(|f| f.is_finite()) {
                        json!(f)
                    } else {
                        Value::String(w.to_string())
                    }
                }
            };
            Some((value, &s[end..]))
        }
    }
}

/// The `"key":value` pairs after `+OK` or `!`.
fn parse_pairs(mut s: &str) -> Option<Vec<(String, Value)>> {
    let mut pairs = Vec::new();
    loop {
        s = s.trim_start();
        if s.is_empty() {
            return Some(pairs);
        }
        let (key, r) = parse_value(s)?;
        let Value::String(key) = key else {
            return None;
        };
        let r = r.trim_start().strip_prefix(':')?;
        let (v, r) = parse_value(r)?;
        pairs.push((key, v));
        s = r;
    }
}

/// What a `+OK` answer carries: nothing, its one value, or its fields.
fn ok_value(rest: &str) -> Option<Value> {
    let mut pairs = parse_pairs(rest)?;
    match pairs.len() {
        0 => None,
        1 => Some(pairs.remove(0).1),
        _ => Some(Value::Object(pairs.into_iter().collect())),
    }
}

/// A number as TTP takes it: no exponent, no trailing zeros.
fn number(f: f64) -> String {
    let text = format!("{f:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.to_string()
    }
}

/// An instance tag, quoted when it holds anything but letters, digits and
/// `_ . -` (tags may contain spaces, and then must be quoted).
fn tag(t: &str) -> String {
    if !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        t.to_string()
    } else {
        format!("\"{t}\"")
    }
}

/// The index words of a command, for the state path: `"1 2"` → `["1", "2"]`.
fn index_words(index: &str) -> Vec<String> {
    index
        .split_whitespace()
        .map(|w| w.trim_matches('"').to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Splits a line of TTP-style words, keeping a quoted word whole.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut current));
                    any = false;
                }
            }
            c => {
                current.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(current);
    }
    out
}

// ── State addressing ──────────────────────────────────────────────────────

/// Where a block attribute's value lives in state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Target {
    tag: String,
    attribute: String,
    index: Vec<String>,
}

impl Target {
    fn new(tag: &str, attribute: &str, index: &str) -> Target {
        Target {
            tag: tag.to_string(),
            attribute: attribute.to_string(),
            index: index_words(index),
        }
    }

    fn patch(&self, value: Value) -> Value {
        let mut v = value;
        for i in self.index.iter().rev() {
            v = json!({ i.clone(): v });
        }
        json!({"blocks": {self.tag.clone(): {self.attribute.clone(): v}}})
    }

    /// The words after the tag and command: `level 1`.
    fn address(&self) -> String {
        let mut s = self.attribute.clone();
        for i in &self.index {
            s.push(' ');
            s.push_str(i);
        }
        s
    }
}

/// One subscription this session keeps.
#[derive(Debug, Clone)]
struct Subscription {
    label: String,
    rate_ms: u64,
    /// From the `subscriptions` setting rather than a command.
    standing: bool,
}

// ── Requests ──────────────────────────────────────────────────────────────

#[derive(Debug)]
enum Purpose {
    /// A consumer's command. `target` receives a returned value; `readback`
    /// is read again after an `+OK` when it changed something.
    Command {
        id: CommandId,
        target: Option<Target>,
        readback: Option<Target>,
    },
    /// A read of the module's own, into state.
    Read(Option<Target>),
    /// `SESSION set verbose true`.
    Setup,
    /// A subscription being made: the consumer's (`id`) or a standing one.
    Subscribe {
        id: Option<CommandId>,
        target: Target,
    },
    Unsubscribe {
        id: CommandId,
        target: Target,
    },
    /// The idle liveness check.
    Probe,
}

#[derive(Debug)]
struct Request {
    line: String,
    purpose: Purpose,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// No connection, or waiting for one.
    Down,
    /// Connected: negotiating and logging in, until the banner.
    Opening,
    Ready,
}

pub(crate) struct Tesira {
    device: SocketAddr,
    username: String,
    password: String,
    monitor: bool,
    /// From the `subscriptions` setting: block attributes kept current.
    standing: Vec<(Target, u64)>,
    phase: Phase,
    socket_open: bool,
    telnet: Telnet,
    /// Received text not yet ending in a line break.
    partial: String,
    /// Credentials sent in this connection: user name, password.
    sent_user: bool,
    sent_password: bool,
    /// The credentials were refused: nothing more until the host reopens.
    refused: bool,
    queue: VecDeque<Request>,
    current: Option<(Request, Millis)>,
    subscriptions: BTreeMap<Target, Subscription>,
    /// Label → what it publishes.
    labels: BTreeMap<String, Target>,
    next_label: u64,
    retry_after: Millis,
}

impl Tesira {
    pub(crate) fn new(ctx: OpenContext) -> Tesira {
        let text = |k: &str, default: &str| {
            ctx.settings
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or(default)
                .to_string()
        };
        let standing = parse_standing(&text("subscriptions", ""));
        let mut t = Tesira::for_device(SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)));
        t.username = text("username", "default");
        t.password = text("password", "default");
        t.monitor = ctx.monitor;
        // Commands only: standing subscriptions are the module's own reads.
        t.standing = if ctx.monitor { standing } else { Vec::new() };
        t
    }

    fn for_device(device: SocketAddr) -> Tesira {
        Tesira {
            device,
            username: "default".into(),
            password: "default".into(),
            monitor: true,
            standing: Vec::new(),
            phase: Phase::Down,
            socket_open: false,
            telnet: Telnet::default(),
            partial: String::new(),
            sent_user: false,
            sent_password: false,
            refused: false,
            queue: VecDeque::new(),
            current: None,
            subscriptions: BTreeMap::new(),
            labels: BTreeMap::new(),
            next_label: 1,
            retry_after: RETRY_MIN,
        }
    }

    fn connect(&mut self, cx: &mut Cx) {
        self.phase = Phase::Opening;
        self.telnet = Telnet::default();
        self.partial.clear();
        self.sent_user = false;
        self.sent_password = false;
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
        cx.set_timer(WELCOME, WELCOME_TIMEOUT);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if let Some((request, _)) = self.current.take() {
            self.fail(
                cx,
                request,
                CommandError::Transport {
                    message: reason.clone(),
                },
            );
        }
        for request in std::mem::take(&mut self.queue) {
            self.fail(
                cx,
                request,
                CommandError::Transport {
                    message: reason.clone(),
                },
            );
        }
        for key in [REPLY, WELCOME, IDLE, SILENCE] {
            cx.cancel_timer(key);
        }
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.phase = Phase::Down;
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// The credentials were refused: terminal until the host opens again.
    fn refuse(&mut self, cx: &mut Cx, reason: String) {
        self.refused = true;
        for request in std::mem::take(&mut self.queue) {
            self.fail(
                cx,
                request,
                CommandError::Auth {
                    message: reason.clone(),
                },
            );
        }
        for key in [REPLY, WELCOME, IDLE, SILENCE, RETRY] {
            cx.cancel_timer(key);
        }
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.phase = Phase::Down;
        cx.connection(Connection::Unauthorized { reason });
    }

    fn fail(&mut self, cx: &mut Cx, request: Request, error: CommandError) {
        match request.purpose {
            Purpose::Command { id, .. } | Purpose::Unsubscribe { id, .. } => {
                cx.complete(id, Err(error))
            }
            Purpose::Subscribe { id: Some(id), .. } => cx.complete(id, Err(error)),
            _ => {}
        }
    }

    fn send_line(&self, cx: &mut Cx, line: &str) {
        cx.tcp_send(SOCKET, format!("{line}\n"));
    }

    fn enqueue(&mut self, cx: &mut Cx, request: Request) {
        self.queue.push_back(request);
        self.pump(cx);
    }

    /// Send the next request when none is waiting for its answer.
    fn pump(&mut self, cx: &mut Cx) {
        if self.current.is_some() || self.phase != Phase::Ready {
            return;
        }
        let Some(request) = self.queue.pop_front() else {
            return;
        };
        self.send_line(cx, &request.line);
        self.current = Some((request, cx.now()));
        cx.set_timer(REPLY, REPLY_TIMEOUT);
        cx.cancel_timer(IDLE);
    }

    fn arm_idle(&self, cx: &mut Cx) {
        if self.current.is_none() && self.queue.is_empty() {
            cx.set_timer(IDLE, IDLE_PROBE);
        }
    }

    /// The banner arrived: the session is ready.
    fn ready(&mut self, cx: &mut Cx) {
        self.phase = Phase::Ready;
        self.retry_after = RETRY_MIN;
        cx.cancel_timer(WELCOME);
        cx.connection(Connection::Connected);
        // Answers are parsed in the verbose form, whatever the session
        // started with; it applies before any subscription is made.
        let mut setup = vec![Request {
            line: "SESSION set verbose true".into(),
            purpose: Purpose::Setup,
        }];
        if self.monitor {
            for (attribute, key) in [
                ("serialNumber", "serial_number"),
                ("version", "firmware"),
                ("hostname", "hostname"),
            ] {
                setup.push(Request {
                    line: format!("DEVICE get {attribute}"),
                    purpose: Purpose::Read(Some(Target {
                        tag: "\u{0}device".into(),
                        attribute: key.into(),
                        index: Vec::new(),
                    })),
                });
            }
            for (target, rate) in self.standing.clone() {
                self.subscriptions
                    .entry(target.clone())
                    .or_insert_with(|| Subscription {
                        label: String::new(),
                        rate_ms: rate,
                        standing: true,
                    });
            }
        }
        // Every subscription is made again: they died with the last session.
        let subs: Vec<Target> = self.subscriptions.keys().cloned().collect();
        for target in subs {
            let line = self.subscribe_line(&target);
            setup.push(Request {
                line,
                purpose: Purpose::Subscribe { id: None, target },
            });
        }
        // Ahead of anything a consumer queued while connecting.
        for request in setup.into_iter().rev() {
            self.queue.push_front(request);
        }
        self.pump(cx);
    }

    /// The label for a subscription, chosen once and kept, so a renewal
    /// replaces the subscription rather than adding one.
    fn label_for(&mut self, target: &Target) -> String {
        if let Some(s) = self.subscriptions.get(target) {
            if !s.label.is_empty() {
                return s.label.clone();
            }
        }
        let label = format!("meros{}", self.next_label);
        self.next_label += 1;
        self.labels.insert(label.clone(), target.clone());
        if let Some(s) = self.subscriptions.get_mut(target) {
            s.label = label.clone();
        }
        label
    }

    fn subscribe_line(&mut self, target: &Target) -> String {
        let label = self.label_for(target);
        let rate = self.subscriptions.get(target).map_or(0, |s| s.rate_ms);
        let mut line = format!(
            "{} subscribe {} {}",
            tag(&target.tag),
            target.address(),
            label
        );
        if rate > 0 {
            line.push_str(&format!(" {rate}"));
        }
        line
    }

    fn state_patch(&self, target: &Target, value: Value) -> Value {
        if let Some(key) = target.tag.strip_prefix('\u{0}') {
            return json!({ key: { target.attribute.clone(): value } });
        }
        target.patch(value)
    }

    // ── Inbound ───────────────────────────────────────────────────────────

    fn inbound(&mut self, cx: &mut Cx, data: &[u8]) {
        let (bytes, answer) = self.telnet.feed(data);
        if !answer.is_empty() {
            cx.tcp_send(SOCKET, answer);
        }
        self.partial.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(end) = self.partial.find(['\n', '\r']) {
            let line: String = self.partial.drain(..=end).collect();
            let line = line.trim_matches(|c: char| c.is_whitespace() || c == '\0');
            if !line.is_empty() {
                let line = line.to_string();
                self.line(cx, &line);
                if self.refused || self.phase == Phase::Down {
                    return;
                }
            }
        }
        if self.phase == Phase::Opening {
            self.prompt(cx);
        }
        // A line that never ends must not grow without limit.
        if self.partial.len() > 65_536 {
            self.partial.clear();
        }
    }

    /// A login prompt has no line ending: look at what is waiting.
    fn prompt(&mut self, cx: &mut Cx) {
        let tail = self.partial.trim().to_ascii_lowercase();
        let user = ["login:", "username:", "user name:", "user:"]
            .iter()
            .any(|p| tail.ends_with(p));
        let password = tail.ends_with("password:");
        if !user && !password {
            return;
        }
        // Asked again: the credentials were not accepted. They are never
        // offered twice, since repeated failures can lock an account.
        if (user && self.sent_user) || (password && self.sent_password) {
            self.refuse(cx, "the Tesira refused the user name or password".into());
            return;
        }
        self.partial.clear();
        let line = if user {
            self.sent_user = true;
            self.username.clone()
        } else {
            self.sent_password = true;
            self.password.clone()
        };
        self.send_line(cx, &line);
    }

    fn line(&mut self, cx: &mut Cx, line: &str) {
        if self.phase == Phase::Opening {
            if line.contains(BANNER) {
                self.ready(cx);
            } else {
                let lower = line.to_ascii_lowercase();
                if (self.sent_password || self.sent_user)
                    && ["incorrect", "denied", "failed", "invalid"]
                        .iter()
                        .any(|w| lower.contains(w))
                {
                    self.refuse(cx, format!("login refused: {line}"));
                }
            }
            return;
        }
        if let Some(rest) = line.strip_prefix('!') {
            // A subscription's publication, possibly with the subscribe's
            // own +OK on the same line.
            let (publication, ok) = match rest.trim_end().strip_suffix("+OK") {
                Some(p) => (p, true),
                None => (rest, false),
            };
            self.publication(cx, publication);
            if ok {
                self.answer(cx, "+OK");
            }
            return;
        }
        if line.starts_with("+OK") || line.starts_with('-') {
            self.answer(cx, line);
        } else if line.contains(BANNER) {
            // A banner again mid-session: the server started a new session.
            cx.log(Level::Info, "the Tesira sent its welcome banner again");
        }
    }

    fn publication(&mut self, cx: &mut Cx, text: &str) {
        let Some(pairs) = parse_pairs(text) else {
            return;
        };
        let label = pairs.iter().find_map(|(k, v)| match (k.as_str(), v) {
            ("publishToken", Value::String(s)) => Some(s.clone()),
            _ => None,
        });
        let value = pairs
            .iter()
            .find(|(k, _)| k == "value")
            .map(|(_, v)| v.clone());
        if let (Some(label), Some(value)) = (label, value) {
            if let Some(target) = self.labels.get(label.trim()) {
                let patch = self.state_patch(target, value);
                cx.state(patch);
            }
        }
    }

    fn answer(&mut self, cx: &mut Cx, line: &str) {
        let Some((request, sent_at)) = self.current.take() else {
            return;
        };
        cx.cancel_timer(REPLY);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        let result = match line.strip_prefix("+OK") {
            Some(rest) => Ok(ok_value(rest)),
            None => {
                let body = line.trim_start_matches('-');
                let code = body.split_whitespace().next().unwrap_or("").to_string();
                Err(CommandError::DeviceError {
                    code: Some(code),
                    message: line.to_string(),
                })
            }
        };
        match request.purpose {
            Purpose::Command {
                id,
                target,
                readback,
            } => match result {
                Ok(value) => {
                    if let (Some(target), Some(v)) = (&target, &value) {
                        let patch = self.state_patch(target, v.clone());
                        cx.state(patch);
                    }
                    if let Some(rb) = readback {
                        if self.monitor && !self.subscriptions.contains_key(&rb) {
                            // Next, so the state follows the change at once.
                            self.queue.push_front(Request {
                                line: format!("{} get {}", tag(&rb.tag), rb.address()),
                                purpose: Purpose::Read(Some(rb)),
                            });
                        }
                    }
                    cx.complete(
                        id,
                        Ok(match value {
                            Some(value) => Outcome::Value { value },
                            None => Outcome::Ack,
                        }),
                    );
                }
                Err(e) => cx.complete(id, Err(e)),
            },
            Purpose::Read(target) => match (result, target) {
                (Ok(Some(v)), Some(target)) => {
                    let patch = self.state_patch(&target, v);
                    cx.state(patch);
                }
                (Err(e), _) => cx.log(Level::Debug, format!("{}: {e}", request.line)),
                _ => {}
            },
            Purpose::Setup => {
                if let Err(e) = result {
                    cx.log(
                        Level::Warning,
                        format!("could not turn on verbose answers: {e}"),
                    );
                }
            }
            Purpose::Subscribe { id, target } => match result {
                Ok(_) => {
                    if let Some(id) = id {
                        cx.complete(id, Ok(Outcome::Ack));
                    }
                }
                Err(e) => {
                    if let Some(s) = self.subscriptions.remove(&target) {
                        self.labels.remove(&s.label);
                    }
                    match id {
                        Some(id) => cx.complete(id, Err(e)),
                        None => cx.log(
                            Level::Warning,
                            format!("subscription '{}' failed: {e}", request.line),
                        ),
                    }
                }
            },
            Purpose::Unsubscribe { id, target } => {
                if let Some(s) = self.subscriptions.remove(&target) {
                    self.labels.remove(&s.label);
                }
                cx.complete(
                    id,
                    match result {
                        Ok(_) => Ok(Outcome::Ack),
                        Err(e) => Err(e),
                    },
                );
            }
            Purpose::Probe => {}
        }
        self.pump(cx);
        self.arm_idle(cx);
    }

    // ── Commands ──────────────────────────────────────────────────────────

    fn command_line(&mut self, cx: &mut Cx, id: CommandId, name: &str, p: &Params) {
        let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let i = |k: &str| p.get(k).and_then(Value::as_i64).unwrap_or(0);
        let f = |k: &str| p.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let b = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
        let t = s("tag");
        let index = s("index");
        let device = |service: &str, value: String| -> String {
            if value.is_empty() {
                format!("DEVICE {service}")
            } else {
                format!("DEVICE {service} {value}")
            }
        };
        // The typed block commands: (attribute, index).
        let one = |attribute: &str, idx: String| Target::new(&t, attribute, &idx);
        let ch = i("channel").to_string();
        let xp = format!("{} {}", i("input"), i("output"));

        let (line, target, readback): (String, Option<Target>, Option<Target>) = match name {
            "get" => {
                let tg = one(&s("attribute"), index.clone());
                (format!("{} get {}", tag(&t), tg.address()), Some(tg), None)
            }
            "set" | "increment" | "decrement" => {
                let tg = one(&s("attribute"), index.clone());
                let value = if name == "set" {
                    s("value")
                } else {
                    number(f("amount"))
                };
                (
                    format!("{} {name} {} {value}", tag(&t), tg.address()),
                    None,
                    Some(tg),
                )
            }
            "toggle" => {
                let tg = one(&s("attribute"), index.clone());
                (
                    format!("{} toggle {}", tag(&t), tg.address()),
                    None,
                    Some(tg),
                )
            }
            "subscribe" => {
                let tg = one(&s("attribute"), index.clone());
                let rate = i("rate_ms").max(0) as u64;
                self.subscriptions
                    .entry(tg.clone())
                    .and_modify(|e| e.rate_ms = rate)
                    .or_insert_with(|| Subscription {
                        label: String::new(),
                        rate_ms: rate,
                        standing: false,
                    });
                let line = self.subscribe_line(&tg);
                self.enqueue(
                    cx,
                    Request {
                        line,
                        purpose: Purpose::Subscribe {
                            id: Some(id),
                            target: tg,
                        },
                    },
                );
                return;
            }
            "unsubscribe" => {
                let tg = one(&s("attribute"), index.clone());
                let Some(sub) = self.subscriptions.get(&tg) else {
                    cx.complete(
                        id,
                        Err(CommandError::InvalidParams {
                            message: format!(
                                "not subscribed to {} {} in this session",
                                t,
                                tg.address()
                            ),
                        }),
                    );
                    return;
                };
                let line = format!("{} unsubscribe {} {}", tag(&t), tg.address(), sub.label);
                self.enqueue(
                    cx,
                    Request {
                        line,
                        purpose: Purpose::Unsubscribe { id, target: tg },
                    },
                );
                return;
            }
            "service" => {
                let args = s("arguments");
                let line = if args.is_empty() {
                    format!("{} {}", tag(&t), s("service"))
                } else {
                    format!("{} {} {args}", tag(&t), s("service"))
                };
                (line, None, None)
            }
            "raw" => (s("command"), None, None),
            "get_aliases" => ("SESSION get aliases".into(), None, None),
            "recall_preset" => (device("recallPreset", i("preset").to_string()), None, None),
            "recall_preset_show_failures" => (
                device("recallPresetShowFailures", i("preset").to_string()),
                None,
                None,
            ),
            "recall_preset_by_name" => (
                device("recallPresetByName", format!("\"{}\"", s("name"))),
                None,
                None,
            ),
            "save_preset" => (device("savePreset", i("preset").to_string()), None, None),
            "save_preset_by_name" => (
                device("savePresetByName", format!("\"{}\"", s("name"))),
                None,
                None,
            ),
            "start_audio" => (device("startAudio", String::new()), None, None),
            "stop_audio" => (device("stopAudio", String::new()), None, None),
            "start_partition_audio" => (
                device("startPartitionAudio", i("partition").to_string()),
                None,
                None,
            ),
            "stop_partition_audio" => (
                device("stopPartitionAudio", i("partition").to_string()),
                None,
                None,
            ),
            "reboot" => (device("reboot", String::new()), None, None),
            "get_level" | "get_mute" => {
                let attr = if name == "get_level" { "level" } else { "mute" };
                let tg = one(attr, ch);
                (format!("{} get {}", tag(&t), tg.address()), Some(tg), None)
            }
            "set_level" => {
                let tg = one("level", ch);
                (
                    format!("{} set {} {}", tag(&t), tg.address(), number(f("level_db"))),
                    None,
                    Some(tg),
                )
            }
            "increment_level" | "decrement_level" => {
                let verb = name.trim_end_matches("_level");
                let tg = one("level", ch);
                (
                    format!(
                        "{} {verb} {} {}",
                        tag(&t),
                        tg.address(),
                        number(f("step_db"))
                    ),
                    None,
                    Some(tg),
                )
            }
            "set_mute" => {
                let tg = one("mute", ch);
                (
                    format!("{} set {} {}", tag(&t), tg.address(), b("muted")),
                    None,
                    Some(tg),
                )
            }
            "toggle_mute" => {
                let tg = one("mute", ch);
                (
                    format!("{} toggle {}", tag(&t), tg.address()),
                    None,
                    Some(tg),
                )
            }
            "select_source" => {
                let tg = one("sourceSelection", String::new());
                (
                    format!("{} set {} {}", tag(&t), tg.address(), i("source")),
                    None,
                    Some(tg),
                )
            }
            "get_source" => {
                let tg = one("sourceSelection", String::new());
                (format!("{} get {}", tag(&t), tg.address()), Some(tg), None)
            }
            "route" => {
                let tg = one("input", i("output").to_string());
                (
                    format!("{} set {} {}", tag(&t), tg.address(), i("input")),
                    None,
                    Some(tg),
                )
            }
            "get_route" => {
                let tg = one("input", i("output").to_string());
                (format!("{} get {}", tag(&t), tg.address()), Some(tg), None)
            }
            "set_crosspoint" => {
                let attr = if s("mixer") == "standard" {
                    "crosspoint"
                } else {
                    "crosspointLevelState"
                };
                let tg = one(attr, xp);
                (
                    format!("{} set {} {}", tag(&t), tg.address(), b("enabled")),
                    None,
                    Some(tg),
                )
            }
            "set_crosspoint_level" => {
                let tg = one("crosspointLevel", xp);
                (
                    format!("{} set {} {}", tag(&t), tg.address(), number(f("level_db"))),
                    None,
                    Some(tg),
                )
            }
            "set_logic_state" => {
                let tg = one("state", ch);
                (
                    format!("{} set {} {}", tag(&t), tg.address(), b("state")),
                    None,
                    Some(tg),
                )
            }
            "set_wall" => {
                let tg = one("wallState", i("wall").to_string());
                (
                    format!("{} set {} {}", tag(&t), tg.address(), b("closed")),
                    None,
                    Some(tg),
                )
            }
            "dial" => (
                format!(
                    "{} dial {} {} \"{}\"",
                    tag(&t),
                    i("line"),
                    i("call_appearance"),
                    s("number")
                ),
                None,
                None,
            ),
            "send_dtmf" => (
                format!("{} dtmf {} \"{}\"", tag(&t), i("line"), s("digits")),
                None,
                None,
            ),
            "end_call" | "answer_call" | "redial" | "hold_call" | "resume_call" | "off_hook"
            | "on_hook" | "flash_hook" => {
                let service = match name {
                    "end_call" => "end",
                    "answer_call" => "answer",
                    "redial" => "redial",
                    "hold_call" => "hold",
                    "resume_call" => "resume",
                    "off_hook" => "offHook",
                    "on_hook" => "onHook",
                    _ => "flash",
                };
                (
                    format!(
                        "{} {service} {} {}",
                        tag(&t),
                        i("line"),
                        i("call_appearance")
                    ),
                    None,
                    None,
                )
            }
            other => {
                cx.complete(
                    id,
                    Err(CommandError::UnknownCommand {
                        command: other.into(),
                    }),
                );
                return;
            }
        };
        self.enqueue(
            cx,
            Request {
                line,
                purpose: Purpose::Command {
                    id,
                    target,
                    readback,
                },
            },
        );
    }
}

/// The `subscriptions` setting: entries separated by `;` or line breaks, each
/// `<tag> <attribute> [index ...] [@<ms>]`, a tag with spaces in quotes.
fn parse_standing(text: &str) -> Vec<(Target, u64)> {
    let mut out = Vec::new();
    for entry in text.split([';', '\n']) {
        let mut w = words(entry);
        let mut rate = 0;
        if let Some(last) = w.last() {
            if let Some(ms) = last.strip_prefix('@').and_then(|m| m.parse().ok()) {
                rate = ms;
                w.pop();
            }
        }
        if w.len() < 2 {
            continue;
        }
        out.push((
            Target {
                tag: w[0].clone(),
                attribute: w[1].clone(),
                index: w[2..].to_vec(),
            },
            rate,
        ));
    }
    out
}

impl Module for Tesira {
    fn start(&mut self, cx: &mut Cx) {
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.refused {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: "the Tesira refused the configured credentials".into(),
                }),
            );
            return;
        }
        if self.phase != Phase::Ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        // Quotes would end a quoted tag or value early.
        for key in ["tag", "name", "number", "digits"] {
            if params
                .get(key)
                .and_then(Value::as_str)
                .is_some_and(|v| v.contains('"'))
            {
                cx.complete(
                    id,
                    Err(CommandError::InvalidParams {
                        message: format!("'{key}' cannot contain a double quote"),
                    }),
                );
                return;
            }
        }
        self.command_line(cx, id, name, params);
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        if self.refused {
            return;
        }
        match input {
            TcpInput::Connected => {
                self.socket_open = true;
                cx.set_timer(SILENCE, SILENCE_TIMEOUT);
            }
            TcpInput::Data(data) => {
                cx.alive();
                cx.set_timer(SILENCE, SILENCE_TIMEOUT);
                self.inbound(cx, &data);
            }
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if self.refused {
            return;
        }
        match key {
            RETRY => self.connect(cx),
            WELCOME => self.lost(
                cx,
                if self.sent_user || self.sent_password {
                    "no welcome banner after logging in".into()
                } else {
                    "no welcome banner: is Telnet enabled on the Tesira?".into()
                },
            ),
            SILENCE => self.lost(cx, "nothing from the Tesira for 30 s".into()),
            REPLY => {
                // Answers are matched in order: a late one would be taken for
                // the next request's. Start the session afresh.
                self.lost(cx, "no answer within the timeout".into());
            }
            IDLE if self.phase == Phase::Ready
                && self.current.is_none()
                && self.queue.is_empty() =>
            {
                self.enqueue(
                    cx,
                    Request {
                        line: "DEVICE get serialNumber".into(),
                        purpose: Purpose::Probe,
                    },
                );
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.socket_open {
            cx.tcp_close(SOCKET);
            self.socket_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn tesira(settings: Value, monitor: bool) -> Tesira {
        Tesira::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 40)),
            port: None,
            model: "tesiraforte".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        })
    }

    fn sent(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(String::from_utf8_lossy(data).into_owned()),
                _ => None,
            })
            .collect()
    }

    fn raw_sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    fn feed(m: &mut Tesira, now: Millis, data: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(data.to_vec()));
        cx.take()
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

    fn completed(actions: &[Action]) -> Vec<(CommandId, crate::module::CommandResult)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Complete { id, result } => Some((*id, result.clone())),
                _ => None,
            })
            .collect()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    /// Connected, past the banner, and the verbose setup answered.
    fn ready(settings: Value, monitor: bool) -> (Tesira, Vec<Action>) {
        let mut m = tesira(settings, monitor);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let mut a = cx.take();
        a.extend(feed(
            &mut m,
            1,
            b"\r\nWelcome to the Tesira Text Protocol Server...\r\n",
        ));
        (m, a)
    }

    /// Answer whatever is in flight with `+OK` until the queue is empty,
    /// returning everything sent.
    fn drain(m: &mut Tesira, now: Millis) -> Vec<String> {
        let mut lines = Vec::new();
        while let Some((request, _)) = &m.current {
            lines.push(request.line.clone());
            feed(m, now, b"+OK\r\n");
        }
        lines
    }

    #[test]
    fn telnet_options_are_refused_and_dropped_from_the_text() {
        let mut t = Telnet::default();
        // Biamp's example: DO terminal type, DO terminal speed, WILL suppress
        // go ahead, then the banner after CR LF.
        let (data, answer) = t.feed(&[IAC, DO, 0x18, IAC, DO, 0x20, IAC, WILL, 0x03, b'H', b'i']);
        assert_eq!(data, b"Hi");
        assert_eq!(answer, [IAC, WONT, 0x18, IAC, WONT, 0x20, IAC, DONT, 0x03]);
        // Split across reads, a subnegotiation, an escaped 255 and CR NUL.
        let (data, answer) = t.feed(&[IAC]);
        assert!(data.is_empty() && answer.is_empty());
        let (data, answer) = t.feed(&[
            DO, 0x01, IAC, SB, 0x18, 1, IAC, SE, IAC, IAC, b'\r', 0, b'x',
        ]);
        assert_eq!(data, [IAC, b'\r', b'x']);
        assert_eq!(answer, [IAC, WONT, 0x01]);
    }

    #[test]
    fn values_parse_from_ttp_answers() {
        assert_eq!(ok_value(""), None);
        assert_eq!(ok_value(r#" "value":-10.000000"#), Some(json!(-10.0)));
        assert_eq!(ok_value(r#" "value":4"#), Some(json!(4)));
        assert_eq!(
            ok_value(r#" "value":[false true false]"#),
            Some(json!([false, true, false]))
        );
        assert_eq!(
            ok_value(r#" "list":["123" "AudioMeter1" "DEVICE"]"#),
            Some(json!(["123", "AudioMeter1", "DEVICE"]))
        );
        assert_eq!(
            ok_value(r#" "value":{"units":MILLISECOND "delay":47.3}"#),
            Some(json!({"units": "MILLISECOND", "delay": 47.3}))
        );
        assert_eq!(
            ok_value(r#" "value":"my \"test\" string""#),
            Some(json!("my \"test\" string"))
        );
        assert_eq!(
            ok_value(r#" "time":"12:00" "line":"2""#),
            Some(json!({"time": "12:00", "line": "2"}))
        );
        assert_eq!(number(-10.0), "-10");
        assert_eq!(number(-3.5), "-3.5");
        assert_eq!(number(0.1 + 0.2), "0.3");
        assert_eq!(tag("Level1"), "Level1");
        assert_eq!(tag("my level 2"), "\"my level 2\"");
    }

    #[test]
    fn negotiation_then_banner_then_verbose_and_reads() {
        let mut m = tesira(json!({}), true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = cx.take();
        assert!(a.contains(&Action::Connection(Connection::Connecting)));
        let a = feed(&mut m, 1, &[IAC, DO, 0x18, IAC, WILL, 0x01]);
        assert_eq!(raw_sent(&a), vec![vec![IAC, WONT, 0x18, IAC, DONT, 0x01]]);
        assert!(!a.contains(&Action::Connection(Connection::Connected)));
        // Commands wait for the banner.
        let mut cx = Cx::new(2);
        m.command(&mut cx, 9, "get_aliases", &Params::new());
        assert_eq!(
            completed(&cx.take()),
            vec![(9, Err(CommandError::NotConnected))]
        );

        let a = feed(
            &mut m,
            3,
            b"\r\nWelcome to the Tesira Text Protocol Server...\r\n",
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(sent(&a), ["SESSION set verbose true\n"]);
        let a = feed(&mut m, 4, b"+OK\r\n");
        assert_eq!(sent(&a), ["DEVICE get serialNumber\n"]);
        let a = feed(&mut m, 5, b"+OK \"value\":\"01842224\"\r\n");
        assert_eq!(state(&a), json!({"device": {"serial_number": "01842224"}}));
        assert_eq!(sent(&a), ["DEVICE get version\n"]);
    }

    #[test]
    fn a_protected_system_is_logged_into_when_it_asks() {
        let mut m = tesira(json!({"username": "ctl", "password": "pw"}), true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        let a = feed(&mut m, 1, b"\r\nlogin: ");
        assert_eq!(sent(&a), ["ctl\n"]);
        let a = feed(&mut m, 2, b"\r\nPass");
        assert!(sent(&a).is_empty());
        let a = feed(&mut m, 3, b"word: ");
        assert_eq!(sent(&a), ["pw\n"]);
        let a = feed(
            &mut m,
            4,
            b"\r\nWelcome to the Tesira Text Protocol Server...\r\n",
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));

        // Asked for the user name again: refused, and terminal.
        let mut m = tesira(json!({"username": "ctl", "password": "bad"}), true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        feed(&mut m, 1, b"login: ");
        feed(&mut m, 2, b"Password: ");
        let a = feed(&mut m, 3, b"\r\nlogin: ");
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(sent(&a).is_empty());
        let mut cx = Cx::new(4);
        m.timer(&mut cx, RETRY);
        assert!(cx.take().is_empty(), "no retry after a refusal");
    }

    #[test]
    fn commands_are_encoded_and_answers_become_results_and_state() {
        let (mut m, _) = ready(json!({}), true);
        drain(&mut m, 2);
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "get_level",
            &params(json!({"tag": "Level1", "channel": 2})),
        );
        assert_eq!(sent(&cx.take()), ["Level1 get level 2\n"]);
        let a = feed(&mut m, 12, b"+OK \"value\":-10.000000\r\n");
        assert_eq!(
            completed(&a),
            vec![(
                1,
                Ok(Outcome::Value {
                    value: json!(-10.0)
                })
            )]
        );
        assert_eq!(
            state(&a),
            json!({"blocks": {"Level1": {"level": {"2": -10.0}}}})
        );
        assert!(a.contains(&Action::RoundTrip(2)));

        // A change is read back when nothing publishes it.
        let mut cx = Cx::new(20);
        m.command(
            &mut cx,
            2,
            "set_mute",
            &params(json!({"tag": "my level 2", "channel": 1, "muted": true})),
        );
        assert_eq!(sent(&cx.take()), ["\"my level 2\" set mute 1 true\n"]);
        let a = feed(&mut m, 21, b"+OK\r\n");
        assert_eq!(completed(&a), vec![(2, Ok(Outcome::Ack))]);
        assert_eq!(sent(&a), ["\"my level 2\" get mute 1\n"]);

        let a = feed(&mut m, 22, b"+OK \"value\":true\r\n");
        assert_eq!(
            state(&a),
            json!({"blocks": {"my level 2": {"mute": {"1": true}}}})
        );

        let mut cx = Cx::new(30);
        for (id, name, p) in [
            (3, "recall_preset", json!({"preset": 1001})),
            (
                4,
                "set_crosspoint",
                json!({"tag": "Mixer1", "input": 1, "output": 2, "enabled": true}),
            ),
            (
                5,
                "route",
                json!({"tag": "Router1", "output": 1, "input": 3}),
            ),
            (
                6,
                "dial",
                json!({"tag": "Dialer1", "line": 1, "call_appearance": 1, "number": "1,5036417287"}),
            ),
            (
                7,
                "set",
                json!({"tag": "Delay1", "attribute": "unitsDelay", "index": "", "value": "{\"units\":MILLISECOND \"delay\":4}"}),
            ),
        ] {
            m.command(&mut cx, id, name, &params(p));
        }
        cx.take();
        let lines = drain(&mut m, 31);
        assert_eq!(
            lines,
            [
                "DEVICE recallPreset 1001",
                "Mixer1 set crosspointLevelState 1 2 true",
                "Mixer1 get crosspointLevelState 1 2",
                "Router1 set input 1 3",
                "Router1 get input 1",
                "Dialer1 dial 1 1 \"1,5036417287\"",
                "Delay1 set unitsDelay {\"units\":MILLISECOND \"delay\":4}",
                "Delay1 get unitsDelay",
            ]
        );
    }

    #[test]
    fn errors_carry_their_code() {
        let (mut m, _) = ready(json!({}), true);
        drain(&mut m, 2);
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "set_mute",
            &params(json!({"tag": "level3", "channel": 3, "muted": true})),
        );
        let a = feed(
            &mut m,
            11,
            b"-ERR address not found: {\"deviceId\":0 \"classCode\":0 \"instanceNum\":0}\r\n",
        );
        assert!(matches!(
            &completed(&a)[..],
            [(1, Err(CommandError::DeviceError { code: Some(c), .. }))] if c == "ERR"
        ));
        // Nothing is read back after a failure.
        assert!(sent(&a).is_empty());
    }

    #[test]
    fn subscriptions_publish_into_state_and_are_made_again_after_reconnecting() {
        let (mut m, _) = ready(
            json!({"subscriptions": "Level1 level 1 @250; \"Room Mics\" mutes"}),
            true,
        );
        let lines = drain(&mut m, 2);
        assert_eq!(
            lines,
            [
                "SESSION set verbose true",
                "DEVICE get serialNumber",
                "DEVICE get version",
                "DEVICE get hostname",
                "Level1 subscribe level 1 meros1 250",
                "\"Room Mics\" subscribe mutes meros2",
            ]
        );
        let a = feed(
            &mut m,
            3,
            b"! \"publishToken\":\"meros1\" \"value\":-98.099998\r\n! \"publishToken\":\"meros2\" \"value\":[false true]\r\n",
        );
        assert_eq!(
            state(&a),
            json!({"blocks": {
                "Level1": {"level": {"1": -98.099998}},
                "Room Mics": {"mutes": [false, true]},
            }})
        );

        // The consumer's own, with the first value and +OK on one line.
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "subscribe",
            &params(
                json!({"tag": "LogicMeter1", "attribute": "state", "index": "2", "rate_ms": 0}),
            ),
        );
        assert_eq!(sent(&cx.take()), ["LogicMeter1 subscribe state 2 meros3\n"]);
        let a = feed(
            &mut m,
            11,
            b"! \"publishToken\":\"meros3\" \"value\":false +OK\r\n",
        );
        assert_eq!(completed(&a), vec![(1, Ok(Outcome::Ack))]);
        assert_eq!(
            state(&a),
            json!({"blocks": {"LogicMeter1": {"state": {"2": false}}}})
        );

        // A subscribed attribute is not read back after a change.
        let mut cx = Cx::new(20);
        m.command(
            &mut cx,
            2,
            "set_level",
            &params(json!({"tag": "Level1", "channel": 1, "level_db": -20.0})),
        );
        cx.take();
        let a = feed(&mut m, 21, b"+OK\r\n");
        assert!(sent(&a).is_empty());

        // Reconnected: every subscription again, with the same labels.
        let mut cx = Cx::new(30);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        m.timer(&mut cx, RETRY);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        feed(
            &mut m,
            31,
            b"Welcome to the Tesira Text Protocol Server...\r\n",
        );
        let lines = drain(&mut m, 32);
        assert_eq!(
            &lines[4..],
            [
                "Level1 subscribe level 1 meros1 250",
                "LogicMeter1 subscribe state 2 meros3",
                "\"Room Mics\" subscribe mutes meros2",
            ]
        );

        // Unsubscribing uses the label it was made with.
        let mut cx = Cx::new(40);
        m.command(
            &mut cx,
            3,
            "unsubscribe",
            &params(json!({"tag": "LogicMeter1", "attribute": "state", "index": "2"})),
        );
        assert_eq!(
            sent(&cx.take()),
            ["LogicMeter1 unsubscribe state 2 meros3\n"]
        );
        let a = feed(&mut m, 41, b"+OK\r\n");
        assert_eq!(completed(&a), vec![(3, Ok(Outcome::Ack))]);
    }

    #[test]
    fn opened_for_commands_only_it_reads_nothing_of_its_own() {
        let (mut m, a) = ready(json!({"subscriptions": "Level1 level 1"}), false);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // Only the session setup, which answers depend on.
        assert_eq!(drain(&mut m, 2), ["SESSION set verbose true"]);

        // A change is not read back.
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "set_level",
            &params(json!({"tag": "Level1", "channel": 1, "level_db": -6.5})),
        );
        assert_eq!(sent(&cx.take()), ["Level1 set level 1 -6.5\n"]);
        let a = feed(&mut m, 14, b"+OK\r\n");
        assert_eq!(completed(&a), vec![(1, Ok(Outcome::Ack))]);
        assert!(a.contains(&Action::RoundTrip(4)));
        assert!(sent(&a).is_empty());

        // The liveness check when idle.
        let mut cx = Cx::new(10_014);
        m.timer(&mut cx, IDLE);
        assert_eq!(sent(&cx.take()), ["DEVICE get serialNumber\n"]);
        let a = feed(&mut m, 10_020, b"+OK \"value\":\"01842224\"\r\n");
        assert!(a.contains(&Action::RoundTrip(6)));
        assert_eq!(state(&a), json!({}));
    }

    #[test]
    fn an_unanswered_command_resets_the_session() {
        let (mut m, _) = ready(json!({}), true);
        drain(&mut m, 2);
        let mut cx = Cx::new(10);
        m.command(&mut cx, 1, "get_aliases", &Params::new());
        m.command(
            &mut cx,
            2,
            "toggle_mute",
            &params(json!({"tag": "Mute1", "channel": 1})),
        );
        cx.take();
        let mut cx = Cx::new(5_010);
        m.timer(&mut cx, REPLY);
        let a = cx.take();
        let done = completed(&a);
        assert_eq!(done.len(), 2);
        assert!(done
            .iter()
            .all(|(_, r)| matches!(r, Err(CommandError::Transport { .. }))));
        assert!(a.iter().any(|x| matches!(x, Action::TcpClose { .. })));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
    }

    #[test]
    fn standing_subscriptions_parse() {
        let s = parse_standing(
            "Level1 level 1 @250;\n\"Room Mics\" mutes; bad ;Mixer1 crosspointLevel 2 3",
        );
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].0.address(), "level 1");
        assert_eq!(s[0].1, 250);
        assert_eq!(s[1].0.tag, "Room Mics");
        assert_eq!(s[2].0.index, ["2", "3"]);
    }
}
