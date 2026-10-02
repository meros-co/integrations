//! Allen & Heath consoles and processors over MIDI on TCP: dLive, Avantis,
//! AHM, Qu (Qu-16/24/32/Pac/SB), SQ, SQ+, Qu-5/6/7 and CQ.
//!
//! Every family speaks MIDI over a plain TCP connection (port 51325 for all
//! but a dLive Surface, 51328), but the messages differ by family, so each
//! has a dialect (see the specs' sources for the documents and pages):
//!
//! - dLive and Avantis ([`super::allenheath_dlive`]): the channel type is
//!   chosen by MIDI channel offset from a base channel N and the channel by
//!   note number; mutes are Note On, levels and assignments 7-bit NRPN,
//!   names, colours, sends and every "get" are SysEx with the header
//!   `F0 00 00 1A 50 10 01 00`.
//! - AHM ([`super::allenheath_ahm`]): the same shape on fixed MIDI channels
//!   0-3, with the header `F0 00 00 1A 50 12 01 00`.
//! - Qu-16/24/32/Pac/SB ([`super::allenheath_qu`]): Note On mutes, NRPN with
//!   the channel as parameter MSB and an index in the data LSB, and a SysEx
//!   "Get System State" after which the mixer pushes every parameter.
//! - SQ, SQ+, Qu-5/6/7 and CQ ([`super::allenheath_sq`]): every parameter is
//!   an NRPN number with a 14-bit value, read back by a "get" (data
//!   increment 7F).
//!
//! What is common lives here: the TCP session, the MIDI parser (running
//! status, SysEx, real-time bytes, [`super::allenheath_midi`]), reading the
//! console's state after connecting at a pace it can absorb, answering "get"
//! commands from the console's reply, and noticing a console that has gone.
//! MIDI has no acknowledgement, so a write is reported `unverified`; where
//! the family has a matching "get", it is sent after the write so the state
//! shows what the console actually did.
//!
//! dLive also listens with TLS/SSL encryption (MixRack 51327, Surface 51329,
//! dLive document p.1). There the first data sent is the login, "UserProfile,
//! UserPassword" with UserProfile 00 to 1F, and the console answers the six
//! characters "AuthOK" or drops the connection. The document gives no more
//! than that: the login is sent as the profile byte followed by the
//! password's bytes, with nothing between or after them. A connection dropped
//! after the login is a refusal, terminal like the core's other credential
//! refusals: repeated wrong logins are not sent on a schedule.

use std::collections::VecDeque;
use std::net::SocketAddr;

use serde_json::{Map, Value};

#[cfg(feature = "allenheath-ahm")]
use super::allenheath_ahm::Ahm;
#[cfg(feature = "allenheath-dlive")]
use super::allenheath_dlive::Dlive;
use super::allenheath_midi::{self as midi, Assembler, Event, Law, Parser, Update};
#[cfg(feature = "allenheath-qu")]
use super::allenheath_qu::Qu;
#[cfg(any(feature = "allenheath-sq", feature = "allenheath-cq"))]
use super::allenheath_sq::Sq;
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput, TlsTarget,
};

pub(crate) const DEFAULT_PORT: u16 = 51325;
const SOCKET: Key = "allenheath";

/// A "get" is answered within this, or the command times out.
const REPLY_TIMEOUT: Millis = 2_000;
/// Quiet this long, ask the console something it always answers.
const QUIET: Millis = 10_000;
/// No answer to that within this: the console is gone.
const PROBE_WAIT: Millis = 5_000;
/// Connected but nothing heard: tell the operator the usual causes.
const FIRST_WORD_WARNING: Millis = 5_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// State reads after connecting go out this many at a time, this often, so a
/// console is never handed thousands of requests at once.
const PACE_BATCH: usize = 10;
const PACE_EVERY: Millis = 20;
const KEEPALIVE_EVERY: Millis = 1_000;
/// "AuthOK" arrives within this after the login, or the attempt is dropped
/// and retried.
const LOGIN_TIMEOUT: Millis = 5_000;
/// The console's answer to an accepted login (dLive p.1).
const AUTH_OK: &[u8] = b"AuthOK";

const REPLY: Key = "reply";
const QUIET_TIMER: Key = "quiet";
const DEAD: Key = "dead";
const RETRY: Key = "retry";
const FIRST_WORD: Key = "first-word";
const PACE: Key = "pace";
const KEEPALIVE: Key = "keepalive";
const LOGIN: Key = "login";

/// One family's messages.
pub(crate) trait Dialect: Send {
    /// The model's TCP port.
    fn port(&self) -> u16 {
        DEFAULT_PORT
    }

    /// The model's TLS port, where its document gives one.
    fn tls_port(&self) -> Option<u16> {
        None
    }

    /// A command to the bytes that carry it.
    fn command(&mut self, name: &str, args: &Args) -> Result<Plan, CommandError>;

    /// One event from the console to state changes.
    fn event(&mut self, event: &Event) -> Vec<Update>;

    /// Requests that read the console's state, sent paced after connecting
    /// and on `refresh`.
    fn sync(&self) -> Vec<Vec<u8>>;

    /// A request the console always answers, sent when it has been quiet.
    fn probe(&self) -> Vec<u8>;

    /// Bytes sent on connecting and every second after (Qu's Active Sensing).
    fn keepalive(&self) -> Option<Vec<u8>> {
        None
    }

    /// Why a console that accepted the connection might say nothing.
    fn silence_hint(&self) -> &'static str;
}

/// What a command sends and how it completes.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub send: Vec<u8>,
    /// A read: completes when this state path arrives.
    pub wait: Option<Wait>,
    /// Reads sent after a write, so the state shows the console's result.
    pub then: Vec<Vec<u8>>,
}

impl Plan {
    pub(crate) fn write(send: Vec<u8>) -> Plan {
        Plan {
            send,
            ..Plan::default()
        }
    }

    pub(crate) fn write_then(send: Vec<u8>, then: Vec<u8>) -> Plan {
        Plan {
            send,
            wait: None,
            then: vec![then],
        }
    }

    /// A read answered by one state path, returned as its value.
    pub(crate) fn read(send: Vec<u8>, path: String) -> Plan {
        Plan {
            send,
            wait: Some(Wait {
                path,
                fields: Vec::new(),
            }),
            then: Vec::new(),
        }
    }

    /// A read answered by `path`, returned as an object of the sibling
    /// `fields` (such as `level_db` and `level_raw`).
    pub(crate) fn read_fields(send: Vec<u8>, path: String, fields: &[&str]) -> Plan {
        Plan {
            send,
            wait: Some(Wait {
                path,
                fields: fields.iter().map(|f| f.to_string()).collect(),
            }),
            then: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Wait {
    pub path: String,
    pub fields: Vec<String>,
}

/// Validated command parameters.
pub(crate) struct Args<'a>(pub &'a Params);

pub(crate) fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

pub(crate) fn unknown(name: &str) -> CommandError {
    CommandError::UnknownCommand {
        command: name.into(),
    }
}

impl Args<'_> {
    pub(crate) fn opt_str(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(Value::as_str)
    }

    pub(crate) fn str(&self, key: &str) -> Result<&str, CommandError> {
        self.opt_str(key)
            .ok_or_else(|| invalid(format!("'{key}' is required")))
    }

    pub(crate) fn opt_int(&self, key: &str) -> Option<i64> {
        self.0.get(key).and_then(Value::as_i64)
    }

    pub(crate) fn int(&self, key: &str) -> Result<i64, CommandError> {
        self.opt_int(key)
            .ok_or_else(|| invalid(format!("'{key}' is required")))
    }

    pub(crate) fn opt_float(&self, key: &str) -> Option<f64> {
        self.0.get(key).and_then(Value::as_f64)
    }

    pub(crate) fn flag(&self, key: &str) -> Result<bool, CommandError> {
        self.0
            .get(key)
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid(format!("'{key}' is required")))
    }

    /// A value given either in the document's unit (`<name>_db`, converted
    /// by `law`) or as the raw wire value (`<name>_raw`, 0..=`max_raw`).
    /// Exactly one of the two.
    pub(crate) fn level(&self, name: &str, law: &Law, max_raw: u16) -> Result<u16, CommandError> {
        self.value(&format!("{name}_db"), &format!("{name}_raw"), law, max_raw)
    }

    /// As [`Args::level`], with the parameter names given in full.
    pub(crate) fn value(
        &self,
        db_key: &str,
        raw_key: &str,
        law: &Law,
        max_raw: u16,
    ) -> Result<u16, CommandError> {
        match (self.opt_float(db_key), self.opt_int(raw_key)) {
            (Some(_), Some(_)) => Err(invalid(format!("give '{db_key}' or '{raw_key}', not both"))),
            (None, Some(raw)) if (0..=max_raw as i64).contains(&raw) => Ok(raw as u16),
            (None, Some(raw)) => Err(invalid(format!(
                "'{raw_key}' is {raw}, outside 0 to {max_raw}"
            ))),
            (Some(db), None) => law.encode(db).ok_or_else(|| {
                let (lo, hi) = law.range();
                invalid(format!(
                    "'{db_key}' is {db}, outside the documented range {lo} to {hi}"
                ))
            }),
            (None, None) => Err(invalid(format!("give '{db_key}' or '{raw_key}'"))),
        }
    }
}

struct Pending {
    id: CommandId,
    wait: Wait,
    deadline: Millis,
}

/// The login a TLS connection starts with (dLive p.1).
#[derive(Clone)]
pub(crate) struct Login {
    /// UserProfile, 00 to 1F.
    pub profile: u8,
    pub password: String,
}

impl Login {
    /// The settings' login, when `tls` is on.
    fn from_settings(settings: &Params) -> Result<Option<Login>, String> {
        if !setting_flag(settings, "tls", false) {
            return Ok(None);
        }
        let profile = match settings.get("user_profile").and_then(Value::as_i64) {
            Some(n @ 0..=0x1F) => n as u8,
            Some(n) => return Err(format!("user_profile {n} is outside 0 to 31 (00 to 1F)")),
            None => return Err("tls needs the user_profile setting".into()),
        };
        let password = settings
            .get("password")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Ok(Some(Login { profile, password }))
    }

    /// "UserProfile, UserPassword": the profile byte, then the password.
    fn message(&self) -> Vec<u8> {
        let mut out = vec![self.profile];
        out.extend_from_slice(self.password.as_bytes());
        out
    }
}

/// Where a TLS connection is in its login.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Auth {
    /// Plain TCP, or the login has been accepted.
    Done,
    /// The login was sent; "AuthOK" has not arrived yet.
    Waiting,
}

pub(crate) struct AllenHeath {
    device: SocketAddr,
    dialect: Box<dyn Dialect>,
    parser: Parser,
    assembler: Assembler,
    socket_open: bool,
    connected: bool,
    pending: VecDeque<Pending>,
    outbox: VecDeque<Vec<u8>>,
    retry_after: Millis,
    login: Option<Login>,
    auth: Auth,
    /// Bytes of the console's answer to the login so far.
    auth_reply: Vec<u8>,
    /// The console refused the login: nothing more is attempted.
    refused: Option<String>,
}

/// The dialect for a spec and model, or why there is none.
fn dialect(spec: &str, ctx: &OpenContext) -> Result<Box<dyn Dialect>, String> {
    Ok(match spec {
        #[cfg(feature = "allenheath-dlive")]
        "allenheath-dlive" => Box::new(Dlive::new(&ctx.model, &ctx.settings)?),
        #[cfg(feature = "allenheath-ahm")]
        "allenheath-ahm" => Box::new(Ahm::new(&ctx.settings)),
        #[cfg(feature = "allenheath-qu")]
        "allenheath-qu" => Box::new(Qu::new(&ctx.model, &ctx.settings)?),
        #[cfg(any(feature = "allenheath-sq", feature = "allenheath-cq"))]
        "allenheath-sq" | "allenheath-cq" => Box::new(Sq::new(&ctx.model, &ctx.settings)?),
        other => return Err(format!("no Allen & Heath dialect for '{other}'")),
    })
}

impl AllenHeath {
    pub(crate) fn new(spec: &str, ctx: OpenContext) -> Result<AllenHeath, String> {
        let dialect = dialect(spec, &ctx)?;
        let login = Login::from_settings(&ctx.settings)?;
        let port = match (ctx.port, &login) {
            (Some(port), _) => port,
            (None, None) => dialect.port(),
            (None, Some(_)) => dialect.tls_port().ok_or_else(|| {
                format!(
                    "model '{}' has no documented TLS port: set the port to use tls",
                    ctx.model
                )
            })?,
        };
        let mut m = AllenHeath::with_dialect(SocketAddr::new(ctx.host, port), dialect);
        m.login = login;
        Ok(m)
    }

    pub(crate) fn with_dialect(device: SocketAddr, dialect: Box<dyn Dialect>) -> AllenHeath {
        AllenHeath {
            device,
            dialect,
            parser: Parser::default(),
            assembler: Assembler::default(),
            socket_open: false,
            connected: false,
            pending: VecDeque::new(),
            outbox: VecDeque::new(),
            retry_after: RETRY_MIN,
            login: None,
            auth: Auth::Done,
            auth_reply: Vec::new(),
            refused: None,
        }
    }

    /// The console refused the login: fail everything and stop. Only opening
    /// the device again, with corrected settings, tries again.
    fn refuse(&mut self, cx: &mut Cx, reason: String) {
        for p in self.pending.drain(..) {
            cx.complete(
                p.id,
                Err(CommandError::Auth {
                    message: reason.clone(),
                }),
            );
        }
        for key in [
            REPLY,
            QUIET_TIMER,
            DEAD,
            FIRST_WORD,
            PACE,
            KEEPALIVE,
            LOGIN,
            RETRY,
        ] {
            cx.cancel_timer(key);
        }
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.connected = false;
        self.outbox.clear();
        cx.log(
            Level::Warning,
            format!("{reason}; no further attempts until the device is opened again"),
        );
        cx.connection(Connection::Unauthorized {
            reason: reason.clone(),
        });
        self.refused = Some(reason);
    }

    /// The connection is up and, with TLS, logged in: read the console.
    fn begin(&mut self, cx: &mut Cx) {
        if let Some(bytes) = self.dialect.keepalive() {
            cx.tcp_send(SOCKET, bytes);
            cx.set_timer(KEEPALIVE, KEEPALIVE_EVERY);
        }
        self.queue_sync(cx);
        cx.set_timer(QUIET_TIMER, QUIET);
        cx.set_timer(FIRST_WORD, FIRST_WORD_WARNING);
    }

    /// The console's answer to the login, as it arrives.
    fn login_reply(&mut self, cx: &mut Cx, data: &[u8]) {
        self.auth_reply.extend_from_slice(data);
        let n = self.auth_reply.len().min(AUTH_OK.len());
        if self.auth_reply[..n] != AUTH_OK[..n] {
            let got = String::from_utf8_lossy(&self.auth_reply).into_owned();
            let reason = format!(
                "the console answered the login with {got:?}, not \"AuthOK\" (user profile {})",
                self.login.as_ref().map_or(0, |l| l.profile)
            );
            self.refuse(cx, reason);
            return;
        }
        if self.auth_reply.len() < AUTH_OK.len() {
            return;
        }
        cx.cancel_timer(LOGIN);
        self.auth = Auth::Done;
        let rest = self.auth_reply.split_off(AUTH_OK.len());
        self.auth_reply.clear();
        cx.log(Level::Info, "the console accepted the login");
        self.begin(cx);
        // AuthOK is the console's first word.
        self.data(cx, &rest);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        for p in self.pending.drain(..) {
            cx.complete(
                p.id,
                Err(CommandError::Transport {
                    message: reason.clone(),
                }),
            );
        }
        for key in [REPLY, QUIET_TIMER, DEAD, FIRST_WORD, PACE, KEEPALIVE] {
            cx.cancel_timer(key);
        }
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.connected = false;
        self.outbox.clear();
        self.parser = Parser::default();
        self.assembler = Assembler::default();
        self.auth = Auth::Done;
        self.auth_reply.clear();
        cx.cancel_timer(LOGIN);
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn arm_reply_timer(&self, cx: &mut Cx) {
        match self.pending.iter().map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    fn queue_sync(&mut self, cx: &mut Cx) {
        let was_idle = self.outbox.is_empty();
        self.outbox.extend(self.dialect.sync());
        if was_idle && !self.outbox.is_empty() {
            self.pace(cx);
        }
    }

    fn pace(&mut self, cx: &mut Cx) {
        for _ in 0..PACE_BATCH {
            match self.outbox.pop_front() {
                Some(bytes) => cx.tcp_send(SOCKET, bytes),
                None => break,
            }
        }
        if !self.outbox.is_empty() {
            cx.set_timer(PACE, PACE_EVERY);
        }
    }

    fn data(&mut self, cx: &mut Cx, data: &[u8]) {
        cx.alive();
        if !self.connected {
            self.connected = true;
            self.retry_after = RETRY_MIN;
            cx.cancel_timer(FIRST_WORD);
            cx.connection(Connection::Connected);
        }
        cx.cancel_timer(DEAD);
        cx.set_timer(QUIET_TIMER, QUIET);
        let mut groups: Vec<Vec<Update>> = Vec::new();
        for msg in self.parser.feed(data) {
            let Some(event) = self.assembler.feed(msg) else {
                continue;
            };
            let group = self.dialect.event(&event);
            if !group.is_empty() {
                groups.push(group);
            }
        }
        if groups.is_empty() {
            return;
        }
        // State first, so a caller reading the snapshot after its read
        // completes sees what the reply carried.
        let updates: Vec<Update> = groups.iter().flatten().cloned().collect();
        cx.state(midi::patch(&updates));
        for group in &groups {
            self.resolve(cx, group);
        }
    }

    /// Complete the reads that one console message answers.
    fn resolve(&mut self, cx: &mut Cx, group: &[Update]) {
        let mut answered = false;
        let mut i = 0;
        while i < self.pending.len() {
            let path = &self.pending[i].wait.path;
            if let Some((_, value)) = group.iter().find(|(p, _)| p == path) {
                let p = self.pending.remove(i).unwrap();
                let result = if p.wait.fields.is_empty() {
                    value.clone()
                } else {
                    let parent = p.wait.path.rsplit_once('.').map(|(a, _)| a).unwrap_or("");
                    let mut obj = Map::new();
                    for field in &p.wait.fields {
                        let full = format!("{parent}.{field}");
                        let v = group
                            .iter()
                            .find(|(p, _)| *p == full)
                            .map(|(_, v)| v.clone())
                            .unwrap_or(Value::Null);
                        obj.insert(field.clone(), v);
                    }
                    Value::Object(obj)
                };
                cx.complete(p.id, Ok(Outcome::Value { value: result }));
                answered = true;
            } else {
                i += 1;
            }
        }
        if answered {
            self.arm_reply_timer(cx);
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        if self.refused.is_some() {
            return;
        }
        cx.connection(Connection::Connecting);
        if self.login.is_some() {
            // The document says nothing of the console's certificate; a
            // console addressed by IP has none a public root vouches for.
            cx.tcp_open_tls(
                SOCKET,
                TlsTarget {
                    to: self.device,
                    server_name: self.device.ip().to_string(),
                    accept_invalid_certs: true,
                },
            );
        } else {
            cx.tcp_open(SOCKET, self.device);
        }
    }
}

impl Module for AllenHeath {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
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
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let args = Args(params);
        let plan = match name {
            "refresh" => {
                self.queue_sync(cx);
                cx.complete(id, Ok(Outcome::Ack));
                return;
            }
            "raw_midi" => match args
                .str("hex")
                .and_then(|h| midi::parse_hex(h).map_err(invalid))
            {
                Ok(bytes) if bytes[0] >= 0x80 => Ok(Plan::write(bytes)),
                Ok(_) => Err(invalid("MIDI starts with a status byte (80-FF)")),
                Err(e) => Err(e),
            },
            _ => self.dialect.command(name, &args),
        };
        let plan = match plan {
            Ok(plan) => plan,
            Err(e) => {
                cx.complete(id, Err(e));
                return;
            }
        };
        cx.tcp_send(SOCKET, plan.send);
        for read in plan.then {
            cx.tcp_send(SOCKET, read);
        }
        match plan.wait {
            Some(wait) => {
                self.pending.push_back(Pending {
                    id,
                    wait,
                    deadline: cx.now() + REPLY_TIMEOUT,
                });
                self.arm_reply_timer(cx);
            }
            None => cx.complete(id, Ok(Outcome::Unverified)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.socket_open = true;
                match &self.login {
                    // "the first data sent to the dLive should be" the login.
                    Some(login) => {
                        cx.tcp_send(SOCKET, login.message());
                        self.auth = Auth::Waiting;
                        self.auth_reply.clear();
                        cx.set_timer(LOGIN, LOGIN_TIMEOUT);
                    }
                    None => self.begin(cx),
                }
            }
            TcpInput::Data(data) => {
                if self.refused.is_some() {
                    return;
                }
                if self.auth == Auth::Waiting {
                    self.login_reply(cx, &data);
                } else {
                    self.data(cx, &data);
                }
            }
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                if self.refused.is_some() {
                    return;
                }
                if self.auth == Auth::Waiting {
                    // "otherwise the connection will be dropped" (p.1).
                    let profile = self.login.as_ref().map_or(0, |l| l.profile);
                    self.refuse(
                        cx,
                        format!(
                            "the console dropped the connection after the login, without \
                             \"AuthOK\": the user profile ({profile}) or password was refused \
                             ({reason})"
                        ),
                    );
                    return;
                }
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.open(cx),
            LOGIN => {
                if self.auth == Auth::Waiting {
                    self.lost(
                        cx,
                        "the console did not answer the login within 5 s".to_string(),
                    );
                }
            }
            PACE => {
                if self.socket_open {
                    self.pace(cx);
                }
            }
            KEEPALIVE => {
                if let (true, Some(bytes)) = (self.socket_open, self.dialect.keepalive()) {
                    cx.tcp_send(SOCKET, bytes);
                    cx.set_timer(KEEPALIVE, KEEPALIVE_EVERY);
                }
            }
            FIRST_WORD => {
                let hint = self.dialect.silence_hint();
                cx.log(
                    Level::Warning,
                    format!("connected but the console has not answered; {hint}"),
                );
            }
            QUIET_TIMER => {
                if self.socket_open {
                    cx.tcp_send(SOCKET, self.dialect.probe());
                    cx.set_timer(DEAD, PROBE_WAIT);
                }
            }
            DEAD => {
                let reason = if self.connected {
                    "the console stopped answering".to_string()
                } else {
                    "the console accepted the connection but never answered".to_string()
                };
                self.lost(cx, reason);
            }
            REPLY => {
                let now = cx.now();
                while let Some(i) = self.pending.iter().position(|p| p.deadline <= now) {
                    let p = self.pending.remove(i).unwrap();
                    cx.complete(p.id, Err(CommandError::Timeout));
                }
                self.arm_reply_timer(cx);
            }
            _ => {}
        }
    }
}

/// The model's base MIDI channel setting (1-16 in the settings, 0-15 on the
/// wire).
pub(crate) fn midi_channel(settings: &Params, max: i64) -> Result<u8, String> {
    match settings.get("midi_channel").and_then(Value::as_i64) {
        Some(n) if (1..=max).contains(&n) => Ok((n - 1) as u8),
        Some(n) => Err(format!("midi_channel {n} is outside 1 to {max}")),
        None => Err("set midi_channel to the console's MIDI channel".into()),
    }
}

pub(crate) fn setting_flag(settings: &Params, key: &str, default: bool) -> bool {
    settings
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

#[cfg(test)]
pub(crate) mod testing {
    //! Helpers shared by the dialects' tests.

    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use serde_json::json;
    use std::net::{IpAddr, Ipv4Addr};

    pub(crate) fn module(dialect: Box<dyn Dialect>) -> AllenHeath {
        AllenHeath::with_dialect(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 20)), DEFAULT_PORT),
            dialect,
        )
    }

    pub(crate) fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    pub(crate) fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn state(actions: &[Action]) -> Value {
        let mut merged = json!({});
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(&mut merged, p);
            }
        }
        merged
    }

    /// Opened, connected, and heard from (so commands are accepted).
    pub(crate) fn connected(
        dialect: Box<dyn Dialect>,
        first_word: &[u8],
    ) -> (AllenHeath, Vec<Action>) {
        let mut m = module(dialect);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(first_word.to_vec()));
        (m, cx.take())
    }

    pub(crate) fn feed(m: &mut AllenHeath, now: Millis, data: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(data.to_vec()));
        cx.take()
    }

    pub(crate) fn run(m: &mut AllenHeath, now: Millis, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, 1, name, &params(p));
        cx.take()
    }

    /// The bytes one command sends, or its error.
    pub(crate) fn bytes(m: &mut AllenHeath, name: &str, p: Value) -> Result<Vec<u8>, CommandError> {
        let actions = run(m, 100, name, p);
        for a in &actions {
            if let Action::Complete { result: Err(e), .. } = a {
                return Err(e.clone());
            }
        }
        Ok(sent(&actions).concat())
    }

    /// Every byte the module sends until its sync queue is empty.
    pub(crate) fn drain_sync(m: &mut AllenHeath, actions: &[Action]) -> Vec<Vec<u8>> {
        let mut out = sent(actions);
        let mut now = 0;
        while !m.outbox.is_empty() {
            now += PACE_EVERY;
            let mut cx = Cx::new(now);
            m.timer(&mut cx, PACE);
            out.extend(sent(&cx.take()));
        }
        out
    }

    pub(crate) fn hex(s: &str) -> Vec<u8> {
        midi::parse_hex(s).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::module::Action;
    use serde_json::json;

    /// A dialect with one read and one write, to test the session alone.
    struct Fake;

    impl Dialect for Fake {
        fn command(&mut self, name: &str, args: &Args) -> Result<Plan, CommandError> {
            match name {
                "get" => Ok(Plan::read_fields(
                    vec![0xF0, 0x01, 0xF7],
                    "x.level_raw".into(),
                    &["level_db", "level_raw"],
                )),
                "set" => Ok(Plan::write_then(
                    vec![0x90, args.int("n")? as u8, 0x7F],
                    vec![0xF0, 0x01, 0xF7],
                )),
                other => Err(unknown(other)),
            }
        }
        fn event(&mut self, event: &Event) -> Vec<Update> {
            match event {
                Event::Note { note, .. } => vec![
                    ("x.level_raw".into(), json!(*note)),
                    ("x.level_db".into(), json!(-3.5)),
                ],
                _ => Vec::new(),
            }
        }
        fn sync(&self) -> Vec<Vec<u8>> {
            (0..25u8).map(|i| vec![0xF0, i, 0xF7]).collect()
        }
        fn probe(&self) -> Vec<u8> {
            vec![0xF0, 0x7E, 0xF7]
        }
        fn silence_hint(&self) -> &'static str {
            "check the fake"
        }
    }

    #[test]
    fn sync_is_paced_and_connection_waits_for_the_console() {
        let mut m = module(Box::new(Fake));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = cx.take();
        assert_eq!(sent(&a).len(), PACE_BATCH, "first batch only");
        assert!(!a.contains(&Action::Connection(Connection::Connected)));
        // Commands wait for the console's first word.
        let a = run(&mut m, 5, "get", json!({}));
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Err(CommandError::NotConnected)
        }));
        let all = drain_sync(&mut m, &[]);
        assert_eq!(all.len(), 15);
        let a = feed(&mut m, 100, &[0x90, 0x01, 0x7F]);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
    }

    #[test]
    fn reads_complete_from_the_reply_and_writes_are_unverified() {
        let (mut m, _) = connected(Box::new(Fake), &[0xFE]);
        let a = run(&mut m, 10, "get", json!({}));
        assert_eq!(sent(&a), [vec![0xF0, 0x01, 0xF7]]);
        let a = feed(&mut m, 20, &[0x90, 0x05, 0x7F]);
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value {
                value: json!({"level_db": -3.5, "level_raw": 5})
            })
        }));
        assert_eq!(state(&a)["x"]["level_raw"], 5);
        let at = |f: fn(&Action) -> bool| a.iter().position(f).unwrap();
        assert!(
            at(|x| matches!(x, Action::State(_))) < at(|x| matches!(x, Action::Complete { .. })),
            "the reply's state lands before the read completes"
        );

        let a = run(&mut m, 30, "set", json!({"n": 9}));
        assert_eq!(sent(&a), [vec![0x90, 9, 0x7F], vec![0xF0, 0x01, 0xF7]]);
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Unverified)
        }));

        // An unanswered read times out.
        run(&mut m, 40, "get", json!({}));
        let mut cx = Cx::new(40 + REPLY_TIMEOUT);
        m.timer(&mut cx, REPLY);
        assert!(cx.take().contains(&Action::Complete {
            id: 1,
            result: Err(CommandError::Timeout)
        }));
    }

    #[test]
    fn raw_midi_and_refresh() {
        let (mut m, _) = connected(Box::new(Fake), &[0xFE]);
        assert_eq!(
            bytes(&mut m, "raw_midi", json!({"hex": "B0 63 00"})).unwrap(),
            [0xB0, 0x63, 0x00]
        );
        assert!(matches!(
            bytes(&mut m, "raw_midi", json!({"hex": "63 00"})),
            Err(CommandError::InvalidParams { .. })
        ));
        // A refresh while the first read is still going adds to it.
        let a = run(&mut m, 10, "refresh", json!({}));
        assert!(sent(&a).is_empty());
        assert_eq!(drain_sync(&mut m, &a).len(), 15 + 25);
        let a = run(&mut m, 10, "refresh", json!({}));
        assert_eq!(sent(&a).len(), PACE_BATCH);
    }

    #[test]
    fn a_quiet_console_is_probed_then_dropped() {
        let (mut m, _) = connected(Box::new(Fake), &[0xFE]);
        let mut cx = Cx::new(QUIET);
        m.timer(&mut cx, QUIET_TIMER);
        assert_eq!(sent(&cx.take()), [vec![0xF0, 0x7E, 0xF7]]);
        // An answer cancels the deadline.
        let a = feed(&mut m, QUIET + 100, &[0xFE]);
        assert!(a.contains(&Action::CancelTimer { key: DEAD }));
        let mut cx = Cx::new(QUIET + PROBE_WAIT);
        m.timer(&mut cx, DEAD);
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Connection(Connection::Disconnected { reason }) if reason.contains("stopped")
        )));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
    }

    fn dlive_tls(model: &str, extra: Value) -> Result<AllenHeath, String> {
        let mut settings = json!({"midi_channel": 1, "sync_preamps": false});
        for (k, v) in extra.as_object().unwrap() {
            settings[k] = v.clone();
        }
        AllenHeath::new(
            "allenheath-dlive",
            OpenContext {
                host: "10.0.0.20".parse().unwrap(),
                port: None,
                model: model.into(),
                channels: None,
                settings: params(settings),
            },
        )
    }

    fn secure_mixrack() -> AllenHeath {
        dlive_tls(
            "dlive-mixrack",
            json!({"tls": true, "user_profile": 3, "password": "pw"}),
        )
        .unwrap()
    }

    #[test]
    fn tls_logs_in_first_and_reads_the_console_after_auth_ok() {
        let mut m = secure_mixrack();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx.take().contains(&Action::TcpOpenTls {
            socket: SOCKET,
            target: TlsTarget {
                to: "10.0.0.20:51327".parse().unwrap(),
                server_name: "10.0.0.20".into(),
                accept_invalid_certs: true,
            },
        }));
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = cx.take();
        // The login alone: the profile byte, then the password.
        assert_eq!(sent(&a), [vec![0x03, b'p', b'w']]);
        assert!(a.contains(&Action::SetTimer {
            key: LOGIN,
            after: LOGIN_TIMEOUT
        }));
        // Commands wait for the login.
        let a = run(&mut m, 20, "refresh", json!({}));
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Err(CommandError::NotConnected)
        }));
        // "AuthOK" may arrive in pieces, with MIDI after it.
        let a = feed(&mut m, 30, b"Auth");
        assert!(sent(&a).is_empty());
        let a = feed(&mut m, 40, b"OK\xFE");
        assert!(a.contains(&Action::CancelTimer { key: LOGIN }));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(!sent(&a).is_empty(), "the state is read");
    }

    #[test]
    fn a_dropped_login_is_a_terminal_refusal() {
        let mut m = secure_mixrack();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let mut cx = Cx::new(50);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "closed by the device".into(),
            },
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: RETRY, .. })));
        let a = run(&mut m, 60, "refresh", json!({}));
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                result: Err(CommandError::Auth { .. }),
                ..
            }
        )));
        let mut cx = Cx::new(70);
        m.timer(&mut cx, RETRY);
        assert!(cx.take().is_empty(), "nothing is attempted again");

        // An answer other than AuthOK is a refusal too.
        let mut m = secure_mixrack();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = feed(&mut m, 10, b"Denied");
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(a.contains(&Action::TcpClose { socket: SOCKET }));
    }

    #[test]
    fn a_tls_handshake_failure_or_a_silent_login_is_retried() {
        let mut m = secure_mixrack();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "tls: handshake failed".into(),
            },
        );
        assert!(cx.take().contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        let mut cx = Cx::new(RETRY_MIN);
        m.timer(&mut cx, RETRY);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let mut cx = Cx::new(RETRY_MIN + LOGIN_TIMEOUT);
        m.timer(&mut cx, LOGIN);
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Connection(Connection::Disconnected { reason }) if reason.contains("login")
        )));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: RETRY, .. })));
    }

    #[test]
    fn tls_ports_and_settings() {
        let surface = dlive_tls("dlive-surface", json!({"tls": true, "user_profile": 0})).unwrap();
        assert_eq!(surface.device.port(), 51329);
        let plain = dlive_tls("dlive-surface", json!({})).unwrap();
        assert_eq!(plain.device.port(), 51328);
        assert!(plain.login.is_none());
        // The Avantis documents give no TLS port.
        assert!(dlive_tls("avantis", json!({"tls": true, "user_profile": 0})).is_err());
        assert!(dlive_tls("dlive-mixrack", json!({"tls": true, "user_profile": 32})).is_err());
        assert!(dlive_tls("dlive-mixrack", json!({"tls": true})).is_err());
    }

    #[test]
    fn level_arguments_take_db_or_raw_but_not_both() {
        static T: &[(f64, u16)] = &[(-10.0, 0), (10.0, 100)];
        let law = Law::Table(T);
        let p = params(json!({"level_db": 0.0}));
        assert_eq!(Args(&p).level("level", &law, 127), Ok(50));
        let p = params(json!({"level_raw": 127}));
        assert_eq!(Args(&p).level("level", &law, 127), Ok(127));
        let p = params(json!({"level_raw": 128}));
        assert!(Args(&p).level("level", &law, 127).is_err());
        let p = params(json!({"level_db": 11.0}));
        assert!(Args(&p).level("level", &law, 127).is_err());
        let p = params(json!({"level_db": 1.0, "level_raw": 1}));
        assert!(Args(&p).level("level", &law, 127).is_err());
        let p = params(json!({}));
        assert!(Args(&p).level("level", &law, 127).is_err());
    }
}
