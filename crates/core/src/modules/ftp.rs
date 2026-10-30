//! A small FTP client (RFC 959) on the session's own TCP connections, for a
//! device that serves its media over FTP. It speaks the subset a file
//! manager needs: USER and PASS (anonymous when no user is given), TYPE I,
//! passive data connections (PASV, or EPSV from RFC 2428), MLSD (RFC 3659)
//! falling back to LIST, SIZE (RFC 3659), RETR, STOR, DELE, MKD, RNFR and
//! RNTO, ABOR and QUIT. Plain FTP only: no TLS.
//!
//! A [`Lane`] runs one operation at a time on a connection of its own:
//! connect, log in, do it, quit. It tells its owner what happened through
//! [`Event`]s, taken after each call.
//!
//! - Data connections go to the control connection's peer, whatever address
//!   a PASV reply names: servers behind NAT, or that bind every interface,
//!   name a private or zero address. Only the port is taken from the reply.
//! - Every reply is waited for at most [`REPLY_WITHIN`]; a data transfer may
//!   be silent at most [`STALL`] (no byte either way and no reply). Each
//!   timeout ends the operation, quits and closes both connections.
//! - A download's host file is created at its first byte (or at the end, for
//!   an empty file), so a refused RETR leaves nothing on the host; a download
//!   cut off leaves what arrived. It is checked against SIZE when the server
//!   answers SIZE.
//! - An upload's data is sent once the server has accepted STOR (its 1xx
//!   reply), through the session, which reads the host file a few chunks
//!   ahead of the socket.
//! - Before an upload or a rename, SIZE says whether the target exists, and
//!   an existing target is only replaced when the caller allows it.

use std::net::SocketAddr;

use crate::{json, value::Value};

use crate::module::{Cx, FileInput, Key, Level, Millis, TcpInput};

/// The longest wait for a reply.
pub(crate) const REPLY_WITHIN: Millis = 15_000;
/// The longest silence during a data transfer.
pub(crate) const STALL: Millis = 30_000;
/// The longest wait for QUIT's answer before closing anyway.
const QUIT_WITHIN: Millis = 2_000;
/// A control line longer than this is not FTP.
const MAX_LINE: usize = 64 << 10;

/// The keys a lane's connections, file and timer go by.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Keys {
    pub(crate) control: Key,
    pub(crate) data: Key,
    pub(crate) file: Key,
    pub(crate) timer: Key,
}

/// The login; an empty user logs in anonymously.
#[derive(Debug, Clone, Default)]
pub(crate) struct Login {
    pub(crate) user: String,
    pub(crate) password: String,
}

/// One operation. Paths are the server's, absolute, from `/`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Op {
    List {
        path: String,
    },
    Delete {
        path: String,
    },
    MakeDir {
        path: String,
    },
    Rename {
        from: String,
        to: String,
        overwrite: bool,
    },
    Download {
        remote: String,
        host: String,
    },
    Upload {
        host: String,
        remote: String,
        overwrite: bool,
    },
}

/// Why an operation did not succeed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Failure {
    /// Connecting, a dropped connection, a reply that is not FTP, the host
    /// file.
    Transport(String),
    /// The server refused the login.
    Login(String),
    /// The server refused a command: its reply code and text.
    Refused {
        code: u16,
        message: String,
    },
    /// The request cannot go ahead as asked (the target exists).
    Invalid(String),
    Timeout(String),
    Canceled,
}

impl Failure {
    pub(crate) fn message(&self) -> String {
        match self {
            Failure::Transport(m)
            | Failure::Login(m)
            | Failure::Invalid(m)
            | Failure::Timeout(m) => m.clone(),
            Failure::Refused { code, message } => format!("{code} {message}"),
            Failure::Canceled => "canceled".into(),
        }
    }
}

/// What a lane tells its owner.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    /// The operation's stage: `connecting`, `transferring`, `writing` (the
    /// host file, after the last byte).
    Status(&'static str),
    /// Bytes moved so far, and the whole when known.
    Progress { bytes: u64, total: Option<u64> },
    /// The operation ended: a listing's `{path, entries}`, a transfer's
    /// `{bytes}`, null for the rest.
    Done(Result<Value, Failure>),
}

/// One reply: its code and its text, the lines of a multi-line reply joined
/// by newlines.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reply {
    pub(crate) code: u16,
    pub(crate) text: String,
}

/// Control lines into replies.
#[derive(Debug, Default)]
struct Replies {
    buf: Vec<u8>,
    /// A multi-line reply begun: its code and lines so far.
    open: Option<(u16, Vec<String>)>,
}

fn code_of(line: &str) -> Option<u16> {
    let code: u16 = line.get(..3)?.parse().ok()?;
    (100..600).contains(&code).then_some(code)
}

impl Replies {
    fn feed(&mut self, data: &[u8]) -> Result<Vec<Reply>, String> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        while let Some(at) = self.buf.iter().position(|&b| b == b'\n') {
            let raw: Vec<u8> = self.buf.drain(..=at).collect();
            let line = String::from_utf8_lossy(&raw);
            let line = line.trim_end_matches(['\r', '\n']);
            match &mut self.open {
                Some((code, lines)) => {
                    // The last line repeats the code and a space (or ends).
                    let last = code_of(line) == Some(*code)
                        && matches!(line.as_bytes().get(3), None | Some(b' '));
                    if last {
                        lines.push(line.get(4..).unwrap_or("").to_string());
                        let reply = Reply {
                            code: *code,
                            text: lines.join("\n"),
                        };
                        self.open = None;
                        out.push(reply);
                    } else {
                        lines.push(line.to_string());
                    }
                }
                None => {
                    if line.is_empty() {
                        continue;
                    }
                    let Some(code) = code_of(line) else {
                        return Err(format!("not an FTP reply: {line}"));
                    };
                    let text = line.get(4..).unwrap_or("").to_string();
                    if line.as_bytes().get(3) == Some(&b'-') {
                        self.open = Some((code, vec![text]));
                    } else {
                        out.push(Reply { code, text });
                    }
                }
            }
        }
        if self.buf.len() > MAX_LINE {
            return Err("a reply line longer than 64 KiB".into());
        }
        Ok(out)
    }
}

/// The port of a PASV reply, `227 Entering Passive Mode (h1,h2,h3,h4,p1,p2)`,
/// and the address it names.
pub(crate) fn pasv_port(text: &str) -> Option<(String, u16)> {
    // The six numbers are the first run of comma-separated digits; servers
    // differ on the brackets around them.
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let run: String = text[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ',' || *c == ' ')
        .collect();
    let numbers: Vec<u16> = run
        .split(',')
        .map(|n| n.trim().parse::<u16>())
        .collect::<Result<_, _>>()
        .ok()?;
    if numbers.len() != 6 || numbers.iter().any(|&n| n > 255) {
        return None;
    }
    let port = numbers[4] * 256 + numbers[5];
    let address = format!(
        "{}.{}.{}.{}",
        numbers[0], numbers[1], numbers[2], numbers[3]
    );
    (port != 0).then_some((address, port))
}

/// The port of an EPSV reply, `229 Entering Extended Passive Mode (|||port|)`.
pub(crate) fn epsv_port(text: &str) -> Option<u16> {
    let open = text.find('(')?;
    let inner = &text[open + 1..text[open..].find(')')? + open];
    let delimiter = inner.chars().next()?;
    let parts: Vec<&str> = inner.split(delimiter).collect();
    // (d d d port d): empty, empty, empty, port, empty.
    let port: u16 = parts.get(3)?.parse().ok()?;
    (port != 0).then_some(port)
}

/// Byte ranges of the whitespace-separated fields of a line.
fn fields(line: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in line.char_indices() {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                out.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, line.len()));
    }
    out
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

fn entry(name: &str, kind: &str, size: Option<u64>, modified: Option<String>) -> Value {
    json!({ "name": name, "type": kind, "size": size, "modified": modified })
}

/// One MLSD line: `fact=value;...; name`.
fn mlsd_line(line: &str) -> Option<Option<Value>> {
    let (facts, name) = line.split_once(' ')?;
    if !facts.contains('=') {
        return None;
    }
    let mut kind = None;
    let mut size = None;
    let mut modified = None;
    for fact in facts.split(';').filter(|f| !f.is_empty()) {
        let (key, value) = fact.split_once('=')?;
        match key.to_ascii_lowercase().as_str() {
            "type" => kind = Some(value.to_ascii_lowercase()),
            "size" | "sizd" => size = value.parse::<u64>().ok(),
            "modify" => modified = Some(mlsd_time(value)),
            _ => {}
        }
    }
    let kind = match kind.as_deref() {
        Some("cdir" | "pdir") => return Some(None),
        Some("dir") => "folder",
        Some("file") => "file",
        Some(k) if k.starts_with("os.unix=slink") => "link",
        _ => "other",
    };
    if name == "." || name == ".." {
        return Some(None);
    }
    let size = if kind == "file" { size } else { None };
    Some(Some(entry(name, kind, size, modified)))
}

/// `YYYYMMDDHHMMSS[.sss]` (UTC) as ISO 8601.
fn mlsd_time(value: &str) -> String {
    let digits = value.split('.').next().unwrap_or("");
    if digits.len() == 14 && digits.bytes().all(|b| b.is_ascii_digit()) {
        format!(
            "{}-{}-{}T{}:{}:{}Z",
            &digits[..4],
            &digits[4..6],
            &digits[6..8],
            &digits[8..10],
            &digits[10..12],
            &digits[12..14]
        )
    } else {
        value.to_string()
    }
}

/// One LIST line, in the Unix `ls -l` form (with or without a group) or the
/// DOS form.
fn list_line(line: &str) -> Option<Option<Value>> {
    let f = fields(line);
    let text = |i: usize| f.get(i).map(|&(s, e)| &line[s..e]);
    let first = text(0)?;
    if first.eq_ignore_ascii_case("total") && f.len() == 2 {
        return Some(None);
    }
    let rest = |i: usize| f.get(i).map(|&(s, _)| &line[s..]);
    let kind_of = |c: char| match c {
        'd' => "folder",
        '-' => "file",
        'l' => "link",
        _ => "other",
    };
    // Unix: the month follows the size.
    let unix = (3..f.len().saturating_sub(3)).find(|&i| {
        let month = text(i).unwrap_or("").to_ascii_lowercase();
        MONTHS.contains(&month.as_str()) && text(i - 1).is_some_and(|s| s.parse::<u64>().is_ok())
    });
    if let (Some(i), Some(mode)) = (unix, first.chars().next()) {
        let kind = kind_of(mode);
        let mut name = rest(i + 3)?;
        if kind == "link" {
            name = name.split(" -> ").next().unwrap_or(name);
        }
        if name == "." || name == ".." {
            return Some(None);
        }
        let size = text(i - 1).and_then(|s| s.parse().ok());
        let modified = format!("{} {} {}", text(i)?, text(i + 1)?, text(i + 2)?);
        let size = if kind == "file" { size } else { None };
        return Some(Some(entry(name, kind, size, Some(modified))));
    }
    // DOS: date, time, <DIR> or size, name.
    let date = first;
    if f.len() >= 4 && date.contains('-') && date.bytes().next()?.is_ascii_digit() {
        let time = text(1)?;
        let what = text(2)?;
        let name = rest(3)?;
        let modified = Some(format!("{date} {time}"));
        return if what.eq_ignore_ascii_case("<dir>") {
            Some(Some(entry(name, "folder", None, modified)))
        } else {
            let size = what.parse::<u64>().ok()?;
            Some(Some(entry(name, "file", Some(size), modified)))
        };
    }
    None
}

/// A listing's entries, and the lines that could not be read.
pub(crate) fn parse_listing(data: &[u8], mlsd: bool) -> (Vec<Value>, Vec<String>) {
    let text = String::from_utf8_lossy(data);
    let mut entries = Vec::new();
    let mut unparsed = Vec::new();
    for line in text.lines().map(|l| l.trim_end_matches('\r')) {
        if line.trim().is_empty() {
            continue;
        }
        let read = if mlsd {
            mlsd_line(line)
        } else {
            list_line(line)
        };
        match read {
            Some(Some(e)) => entries.push(e),
            Some(None) => {}
            None => unparsed.push(line.to_string()),
        }
    }
    (entries, unparsed)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Greeting,
    User,
    Pass,
    Type,
    Size,
    Pasv,
    Epsv,
    DataOpening,
    Data,
    Rnfr,
    Rnto,
    Simple,
    /// A download's host file is being closed.
    Finishing,
    Quitting,
}

/// The data step of an operation.
#[derive(Debug, Default)]
struct Xfer {
    /// The server accepted the data command (its 1xx reply).
    accepted: bool,
    /// Its final positive reply has come.
    replied: bool,
    data_closed: bool,
    bytes: u64,
    total: Option<u64>,
    listing: Vec<u8>,
    /// A download's host file is open.
    file_open: bool,
    /// An upload's host file is being sent.
    sending: bool,
    /// An upload's host file has gone out whole.
    sent: bool,
}

/// One FTP connection, one operation at a time.
pub(crate) struct Lane {
    keys: Keys,
    to: SocketAddr,
    login: Login,
    phase: Phase,
    op: Option<Op>,
    replies: Replies,
    connected: bool,
    /// MLSD is still to be tried for a listing.
    mlsd: bool,
    xfer: Xfer,
    events: Vec<Event>,
}

/// A server path as sent: absolute, from `/`.
fn absolute(path: &str) -> String {
    format!("/{}", path.trim_start_matches('/'))
}

impl Lane {
    pub(crate) fn new(keys: Keys, to: SocketAddr, login: Login) -> Lane {
        Lane {
            keys,
            to,
            login,
            phase: Phase::Idle,
            op: None,
            replies: Replies::default(),
            connected: false,
            mlsd: true,
            xfer: Xfer::default(),
            events: Vec::new(),
        }
    }

    /// Where the next operation connects, and how it logs in.
    pub(crate) fn configure(&mut self, to: SocketAddr, login: Login) {
        self.to = to;
        self.login = login;
    }

    pub(crate) fn idle(&self) -> bool {
        self.phase == Phase::Idle
    }

    pub(crate) fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub(crate) fn owns(&self, key: Key) -> bool {
        key == self.keys.control
            || key == self.keys.data
            || key == self.keys.file
            || key == self.keys.timer
    }

    /// Start an operation; the lane must be idle.
    pub(crate) fn start(&mut self, cx: &mut Cx, op: Op) {
        debug_assert!(self.idle());
        self.op = Some(op);
        self.replies = Replies::default();
        self.connected = false;
        self.mlsd = true;
        self.xfer = Xfer::default();
        self.phase = Phase::Greeting;
        self.events.push(Event::Status("connecting"));
        cx.tcp_open(self.keys.control, self.to);
        cx.set_timer(self.keys.timer, REPLY_WITHIN);
    }

    fn send(&mut self, cx: &mut Cx, phase: Phase, line: &str) {
        self.phase = phase;
        cx.tcp_send(self.keys.control, format!("{line}\r\n"));
        cx.set_timer(self.keys.timer, REPLY_WITHIN);
    }

    /// End the operation, once.
    fn finish(&mut self, cx: &mut Cx, result: Result<Value, Failure>) {
        if self.op.take().is_none() {
            return;
        }
        if result.is_err() && self.xfer.file_open {
            // A download cut off keeps what arrived.
            cx.file_close(self.keys.file);
            self.xfer.file_open = false;
        }
        self.events.push(Event::Done(result));
    }

    /// End the operation and quit politely.
    fn fail(&mut self, cx: &mut Cx, failure: Failure) {
        self.finish(cx, Err(failure));
        self.quit(cx);
    }

    /// End the operation on a control connection that cannot carry a QUIT.
    fn fail_hard(&mut self, cx: &mut Cx, failure: Failure) {
        self.finish(cx, Err(failure));
        self.reset(cx);
    }

    fn quit(&mut self, cx: &mut Cx) {
        cx.tcp_close(self.keys.data);
        if self.connected {
            self.phase = Phase::Quitting;
            cx.tcp_send(self.keys.control, "QUIT\r\n");
            cx.set_timer(self.keys.timer, QUIT_WITHIN);
        } else {
            self.reset(cx);
        }
    }

    fn reset(&mut self, cx: &mut Cx) {
        cx.tcp_close(self.keys.data);
        cx.tcp_close(self.keys.control);
        cx.cancel_timer(self.keys.timer);
        self.phase = Phase::Idle;
        self.connected = false;
    }

    /// Cancel the operation running, if any.
    pub(crate) fn cancel(&mut self, cx: &mut Cx) {
        if self.op.is_none() {
            return;
        }
        let transferring = matches!(self.phase, Phase::DataOpening | Phase::Data);
        self.finish(cx, Err(Failure::Canceled));
        cx.tcp_close(self.keys.data);
        if transferring && self.connected {
            cx.tcp_send(self.keys.control, "ABOR\r\n");
        }
        self.quit(cx);
    }

    /// Close everything at once, the device closing.
    pub(crate) fn stop(&mut self, cx: &mut Cx) {
        self.finish(cx, Err(Failure::Canceled));
        if self.connected {
            cx.tcp_send(self.keys.control, "QUIT\r\n");
        }
        self.reset(cx);
    }

    pub(crate) fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        if socket == self.keys.control {
            self.control(cx, input);
        } else if socket == self.keys.data {
            self.data(cx, input);
        }
    }

    fn control(&mut self, cx: &mut Cx, input: TcpInput) {
        if self.phase == Phase::Idle {
            return;
        }
        match input {
            TcpInput::Connected => self.connected = true,
            TcpInput::Data(data) => match self.replies.feed(&data) {
                Ok(replies) => {
                    for reply in replies {
                        self.reply(cx, reply);
                        if self.phase == Phase::Idle {
                            break;
                        }
                    }
                }
                Err(e) => self.fail_hard(cx, Failure::Transport(e)),
            },
            TcpInput::Closed { reason } => {
                if self.phase == Phase::Quitting {
                    return self.reset(cx);
                }
                let message = if self.connected {
                    format!("the FTP server closed the connection: {reason}")
                } else {
                    format!("cannot connect to the FTP server at {}: {reason}", self.to)
                };
                self.connected = false;
                self.fail_hard(cx, Failure::Transport(message));
            }
        }
    }

    fn refused(reply: &Reply) -> Failure {
        Failure::Refused {
            code: reply.code,
            message: reply.text.clone(),
        }
    }

    fn reply(&mut self, cx: &mut Cx, r: Reply) {
        // A preliminary reply is followed by another; only the data step
        // acts on one.
        if r.code < 200 && self.phase != Phase::Data {
            return;
        }
        match self.phase {
            Phase::Idle | Phase::Finishing => {}
            Phase::Quitting => {
                if r.code == 221 || r.code == 421 {
                    self.reset(cx);
                }
            }
            Phase::Greeting => match r.code {
                220 => {
                    let user = if self.login.user.is_empty() {
                        "anonymous".to_string()
                    } else {
                        self.login.user.clone()
                    };
                    self.send(cx, Phase::User, &format!("USER {user}"));
                }
                _ => self.fail(
                    cx,
                    Failure::Transport(format!(
                        "the FTP server refused the session: {} {}",
                        r.code, r.text
                    )),
                ),
            },
            Phase::User => match r.code {
                230 => self.send(cx, Phase::Type, "TYPE I"),
                331 | 332 => {
                    let password = if self.login.user.is_empty() && self.login.password.is_empty() {
                        "anonymous@".to_string()
                    } else {
                        self.login.password.clone()
                    };
                    self.send(cx, Phase::Pass, &format!("PASS {password}"));
                }
                _ => self.fail(cx, Failure::Login(format!("{} {}", r.code, r.text))),
            },
            Phase::Pass => match r.code {
                230 | 202 => self.send(cx, Phase::Type, "TYPE I"),
                _ => self.fail(cx, Failure::Login(format!("{} {}", r.code, r.text))),
            },
            Phase::Type => match r.code {
                200..=299 => self.begin(cx),
                _ => self.fail(cx, Self::refused(&r)),
            },
            Phase::Size => self.sized(cx, &r),
            Phase::Pasv => match (r.code, pasv_port(&r.text)) {
                (227, Some((address, port))) => {
                    if address != self.to.ip().to_string() {
                        cx.log(
                            Level::Debug,
                            format!(
                                "FTP passive reply names {address}; connecting to {} instead",
                                self.to.ip()
                            ),
                        );
                    }
                    self.open_data(cx, port);
                }
                (227, None) => self.fail(
                    cx,
                    Failure::Transport(format!("cannot read the passive reply: {}", r.text)),
                ),
                _ => self.send(cx, Phase::Epsv, "EPSV"),
            },
            Phase::Epsv => match (r.code, epsv_port(&r.text)) {
                (229, Some(port)) => self.open_data(cx, port),
                (229, None) => self.fail(
                    cx,
                    Failure::Transport(format!("cannot read the passive reply: {}", r.text)),
                ),
                _ => self.fail(cx, Self::refused(&r)),
            },
            Phase::DataOpening => {
                if r.code >= 400 {
                    self.fail(cx, Self::refused(&r));
                }
            }
            Phase::Data => self.data_reply(cx, &r),
            Phase::Rnfr => match r.code {
                350 => {
                    let to = match &self.op {
                        Some(Op::Rename { to, .. }) => absolute(to),
                        _ => return,
                    };
                    self.send(cx, Phase::Rnto, &format!("RNTO {to}"));
                }
                _ => self.fail(cx, Self::refused(&r)),
            },
            Phase::Rnto | Phase::Simple => match r.code {
                200..=299 => {
                    self.finish(cx, Ok(Value::Null));
                    self.quit(cx);
                }
                _ => self.fail(cx, Self::refused(&r)),
            },
        }
    }

    /// Logged in, binary: the operation itself.
    fn begin(&mut self, cx: &mut Cx) {
        let Some(op) = self.op.clone() else {
            return self.quit(cx);
        };
        match op {
            Op::List { .. } => self.passive(cx),
            Op::Delete { path } => {
                self.send(cx, Phase::Simple, &format!("DELE {}", absolute(&path)))
            }
            Op::MakeDir { path } => {
                self.send(cx, Phase::Simple, &format!("MKD {}", absolute(&path)))
            }
            Op::Rename { to: target, .. }
            | Op::Download { remote: target, .. }
            | Op::Upload { remote: target, .. } => {
                self.send(cx, Phase::Size, &format!("SIZE {}", absolute(&target)))
            }
        }
    }

    /// SIZE's answer: the download's length, or whether an upload's or a
    /// rename's target exists.
    fn sized(&mut self, cx: &mut Cx, r: &Reply) {
        // Some(size): there, with its size when the reply reads as one;
        // None: not there. Anything else: the server cannot say.
        let found = match r.code {
            213 => Some(r.text.trim().parse::<u64>().ok()),
            550 => None,
            _ => {
                return self.size_unknown(cx, r);
            }
        };
        let Some(op) = self.op.clone() else {
            return self.quit(cx);
        };
        match (op, found) {
            (Op::Download { .. }, Some(size)) => {
                self.xfer.total = size;
                self.passive(cx);
            }
            (Op::Download { remote, .. }, None) => self.fail(
                cx,
                Failure::Refused {
                    code: r.code,
                    message: format!("{} is not on the device: {}", absolute(&remote), r.text),
                },
            ),
            (
                Op::Upload {
                    overwrite, remote, ..
                },
                Some(_),
            ) => {
                if overwrite {
                    self.passive(cx);
                } else {
                    self.fail(
                        cx,
                        Failure::Invalid(format!(
                            "{} already exists; overwrite replaces it",
                            absolute(&remote)
                        )),
                    );
                }
            }
            (
                Op::Rename {
                    overwrite,
                    to,
                    from,
                },
                Some(_),
            ) => {
                if overwrite {
                    self.send(cx, Phase::Rnfr, &format!("RNFR {}", absolute(&from)));
                } else {
                    self.fail(
                        cx,
                        Failure::Invalid(format!(
                            "{} already exists; overwrite replaces it",
                            absolute(&to)
                        )),
                    );
                }
            }
            (Op::Upload { .. }, None) => self.passive(cx),
            (Op::Rename { from, .. }, None) => {
                self.send(cx, Phase::Rnfr, &format!("RNFR {}", absolute(&from)))
            }
            _ => self.quit(cx),
        }
    }

    /// SIZE answered neither a size nor "not found".
    fn size_unknown(&mut self, cx: &mut Cx, r: &Reply) {
        let Some(op) = self.op.clone() else {
            return self.quit(cx);
        };
        let unknown = |target: &str| {
            Failure::Invalid(format!(
                "the server cannot say whether {} exists (SIZE answered {} {}); overwrite goes ahead anyway",
                absolute(target),
                r.code,
                r.text
            ))
        };
        match op {
            Op::Download { .. } => self.passive(cx),
            Op::Upload {
                overwrite: true, ..
            } => self.passive(cx),
            Op::Rename {
                overwrite: true,
                from,
                ..
            } => self.send(cx, Phase::Rnfr, &format!("RNFR {}", absolute(&from))),
            Op::Upload { remote, .. } => self.fail(cx, unknown(&remote)),
            Op::Rename { to, .. } => self.fail(cx, unknown(&to)),
            _ => self.quit(cx),
        }
    }

    /// Ask for a passive data connection: PASV, or EPSV over IPv6.
    fn passive(&mut self, cx: &mut Cx) {
        if self.to.is_ipv6() {
            self.send(cx, Phase::Epsv, "EPSV");
        } else {
            self.send(cx, Phase::Pasv, "PASV");
        }
    }

    fn open_data(&mut self, cx: &mut Cx, port: u16) {
        self.phase = Phase::DataOpening;
        cx.tcp_open(self.keys.data, SocketAddr::new(self.to.ip(), port));
        cx.set_timer(self.keys.timer, REPLY_WITHIN);
    }

    /// The data connection is open: the command that uses it.
    fn data_command(&self) -> Option<String> {
        Some(match self.op.as_ref()? {
            Op::List { path } if self.mlsd => format!("MLSD {}", absolute(path)),
            Op::List { path } => format!("LIST {}", absolute(path)),
            Op::Download { remote, .. } => format!("RETR {}", absolute(remote)),
            Op::Upload { remote, .. } => format!("STOR {}", absolute(remote)),
            _ => return None,
        })
    }

    fn data(&mut self, cx: &mut Cx, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                if self.phase != Phase::DataOpening {
                    return;
                }
                let Some(command) = self.data_command() else {
                    return self.quit(cx);
                };
                self.send(cx, Phase::Data, &command);
                cx.set_timer(self.keys.timer, STALL);
            }
            TcpInput::Data(data) => {
                if self.phase != Phase::Data {
                    return;
                }
                cx.set_timer(self.keys.timer, STALL);
                match self.op.clone() {
                    Some(Op::List { .. }) => {
                        self.xfer.listing.extend_from_slice(&data);
                    }
                    Some(Op::Download { host, .. }) => {
                        if !self.xfer.file_open {
                            cx.file_open(self.keys.file, host);
                            self.xfer.file_open = true;
                        }
                        self.xfer.bytes += data.len() as u64;
                        cx.file_write(self.keys.file, data);
                        self.events.push(Event::Progress {
                            bytes: self.xfer.bytes,
                            total: self.xfer.total,
                        });
                    }
                    _ => {}
                }
            }
            TcpInput::Closed { reason } => match self.phase {
                Phase::DataOpening => self.fail(
                    cx,
                    Failure::Transport(format!(
                        "cannot open the data connection to {}: {reason}",
                        self.to.ip()
                    )),
                ),
                Phase::Data => {
                    self.xfer.data_closed = true;
                    let upload = matches!(self.op, Some(Op::Upload { .. }));
                    if upload && self.xfer.accepted && !self.xfer.sent {
                        return self.fail(
                            cx,
                            Failure::Transport(format!(
                                "the data connection dropped after {} bytes: {reason}",
                                self.xfer.bytes
                            )),
                        );
                    }
                    self.complete(cx);
                }
                _ => {}
            },
        }
    }

    fn data_reply(&mut self, cx: &mut Cx, r: &Reply) {
        if r.code < 200 {
            if self.xfer.accepted {
                return;
            }
            self.xfer.accepted = true;
            self.events.push(Event::Status("transferring"));
            if let Some(Op::Upload { host, .. }) = &self.op {
                let host = host.clone();
                self.xfer.sending = true;
                cx.tcp_send_file(self.keys.data, self.keys.file, host);
            }
            cx.set_timer(self.keys.timer, STALL);
            return;
        }
        if r.code < 300 {
            self.xfer.replied = true;
            return self.complete(cx);
        }
        let listing = matches!(self.op, Some(Op::List { .. }));
        if listing && self.mlsd && !self.xfer.accepted && matches!(r.code, 500 | 501 | 502 | 504) {
            // No MLSD here: LIST on a new data connection.
            self.mlsd = false;
            cx.tcp_close(self.keys.data);
            self.xfer = Xfer::default();
            return self.passive(cx);
        }
        if self.xfer.accepted {
            self.fail(
                cx,
                Failure::Transport(format!(
                    "the transfer was cut off after {} bytes: {} {}",
                    self.xfer.bytes, r.code, r.text
                )),
            );
        } else {
            self.fail(cx, Self::refused(r));
        }
    }

    /// The data step ends when the server's final reply and the end of the
    /// data have both come.
    fn complete(&mut self, cx: &mut Cx) {
        if !self.xfer.replied {
            if self.xfer.data_closed {
                // The last reply is still on its way.
                cx.set_timer(self.keys.timer, REPLY_WITHIN);
            }
            return;
        }
        match self.op.clone() {
            Some(Op::List { path }) if self.xfer.data_closed => {
                let (entries, unparsed) = parse_listing(&self.xfer.listing, self.mlsd);
                let mut value = json!({ "path": path, "entries": entries });
                if !unparsed.is_empty() {
                    value["unparsed"] = json!(unparsed);
                }
                self.finish(cx, Ok(value));
                self.quit(cx);
            }
            Some(Op::Download { host, .. }) if self.xfer.data_closed => {
                if let Some(total) = self.xfer.total.filter(|&t| t != self.xfer.bytes) {
                    return self.fail(
                        cx,
                        Failure::Transport(format!(
                            "received {} of {total} bytes",
                            self.xfer.bytes
                        )),
                    );
                }
                if !self.xfer.file_open {
                    cx.file_open(self.keys.file, host);
                    self.xfer.file_open = true;
                }
                cx.file_close(self.keys.file);
                self.phase = Phase::Finishing;
                self.events.push(Event::Status("writing"));
                cx.set_timer(self.keys.timer, STALL);
            }
            Some(Op::Upload { .. }) if self.xfer.sent => {
                let bytes = self.xfer.bytes;
                self.finish(cx, Ok(json!({ "bytes": bytes })));
                self.quit(cx);
            }
            _ => {}
        }
    }

    pub(crate) fn file(&mut self, cx: &mut Cx, input: FileInput) {
        if self.op.is_none() {
            return;
        }
        let upload = matches!(self.op, Some(Op::Upload { .. }));
        match input {
            FileInput::Opened { bytes } if upload => {
                self.xfer.total = Some(bytes);
                self.events.push(Event::Progress {
                    bytes: 0,
                    total: Some(bytes),
                });
            }
            FileInput::Progress { bytes } if upload => {
                self.xfer.bytes = bytes;
                cx.set_timer(self.keys.timer, STALL);
                self.events.push(Event::Progress {
                    bytes,
                    total: self.xfer.total,
                });
            }
            FileInput::Closed { bytes } if upload => {
                self.xfer.bytes = bytes;
                self.xfer.sent = true;
                self.events.push(Event::Progress {
                    bytes,
                    total: self.xfer.total,
                });
                cx.set_timer(self.keys.timer, STALL);
                self.complete(cx);
            }
            FileInput::Closed { .. } if self.phase == Phase::Finishing => {
                self.xfer.file_open = false;
                let bytes = self.xfer.bytes;
                self.finish(cx, Ok(json!({ "bytes": bytes })));
                self.quit(cx);
            }
            FileInput::Failed { message } => {
                self.xfer.file_open = false;
                self.fail(cx, Failure::Transport(format!("host file: {message}")));
            }
            _ => {}
        }
    }

    pub(crate) fn timer(&mut self, cx: &mut Cx) {
        let failure = match self.phase {
            Phase::Idle => return,
            Phase::Quitting => return self.reset(cx),
            Phase::Data | Phase::Finishing => {
                Failure::Timeout("nothing from the FTP server for 30 s".into())
            }
            _ => Failure::Timeout("no reply from the FTP server within 15 s".into()),
        };
        self.fail(cx, failure);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_line_replies_are_joined() {
        let mut r = Replies::default();
        let got = r
            .feed(b"220-Welcome\r\n  to the deck\r\n220 Ready\r\n331 Pass")
            .unwrap();
        assert_eq!(
            got,
            [Reply {
                code: 220,
                text: "Welcome\n  to the deck\nReady".into()
            }]
        );
        assert_eq!(
            r.feed(b"word required\r\n").unwrap(),
            [Reply {
                code: 331,
                text: "Password required".into()
            }]
        );
        assert!(r.feed(b"hello\r\n").is_err());
    }

    #[test]
    fn passive_replies_give_the_port() {
        assert_eq!(
            pasv_port("Entering Passive Mode (0,0,0,0,195,80)."),
            Some(("0.0.0.0".into(), 50000))
        );
        assert_eq!(
            pasv_port("Entering Passive Mode 192,168,1,20,4,1"),
            Some(("192.168.1.20".into(), 1025))
        );
        assert_eq!(pasv_port("Entering Passive Mode (1,2,3)"), None);
        assert_eq!(
            epsv_port("Entering Extended Passive Mode (|||50001|)"),
            Some(50001)
        );
        assert_eq!(epsv_port("Extended (!!!7!)"), Some(7));
        assert_eq!(epsv_port("no port"), None);
    }

    #[test]
    fn listings_are_read_in_each_form() {
        let mlsd = b"type=cdir;modify=20240101000000; .\r\ntype=dir;modify=20240101000000; Clips\r\ntype=file;size=1048576;modify=20240102030405.123; Clip 01.mov\r\n";
        let (entries, unparsed) = parse_listing(mlsd, true);
        assert!(unparsed.is_empty());
        assert_eq!(
            entries,
            [
                json!({"name": "Clips", "type": "folder", "size": null, "modified": "2024-01-01T00:00:00Z"}),
                json!({"name": "Clip 01.mov", "type": "file", "size": 1048576, "modified": "2024-01-02T03:04:05Z"}),
            ]
        );

        let unix = b"total 2\r\ndrwxr-xr-x 1 ftp ftp 0 Jan 01 12:00 sd1\r\n-rw-r--r-- 1 ftp 2048 Feb  3  2024 A  B.mov\r\nlrwxrwxrwx 1 ftp ftp 4 Mar 01 10:00 usb -> /mnt\r\nsomething else\r\n";
        let (entries, unparsed) = parse_listing(unix, false);
        assert_eq!(
            entries,
            [
                json!({"name": "sd1", "type": "folder", "size": null, "modified": "Jan 01 12:00"}),
                json!({"name": "A  B.mov", "type": "file", "size": 2048, "modified": "Feb 3 2024"}),
                json!({"name": "usb", "type": "link", "size": null, "modified": "Mar 01 10:00"}),
            ]
        );
        assert_eq!(unparsed, ["something else"]);

        let dos = b"01-02-24  03:04PM       <DIR>          Clips\r\n01-02-24  03:04PM              123 a b.mp4\r\n";
        let (entries, _) = parse_listing(dos, false);
        assert_eq!(
            entries,
            [
                json!({"name": "Clips", "type": "folder", "size": null, "modified": "01-02-24 03:04PM"}),
                json!({"name": "a b.mp4", "type": "file", "size": 123, "modified": "01-02-24 03:04PM"}),
            ]
        );
    }
}
