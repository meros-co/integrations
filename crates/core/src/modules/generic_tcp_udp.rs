//! Generic TCP client or UDP socket: send text or bytes to a device the
//! catalogue does not cover yet, and see what comes back.
//!
//! Nothing here is specific to a device. The operator chooses TCP or UDP, how
//! received data is split into messages, and what is appended to sent text.
//!
//! - Text to send may carry escapes for bytes a parameter cannot hold (the
//!   core rejects control characters in strings): `\r` CR, `\n` LF, `\t` TAB,
//!   `\0` NUL, `\\` a backslash, and `\xNN` the byte with hex value NN (two hex
//!   digits, so `\x03` is ETX and `\xff` is the byte 0xFF, not a character).
//!   Any other backslash sequence is refused rather than guessed at. The rest
//!   of the text is sent as UTF-8.
//! - Bytes to send may be given as hex: pairs of hex digits, optionally split
//!   by spaces, `:`, `,` or `-`, and each group optionally prefixed `0x`
//!   (`0x1` is one byte, 0x01).
//! - Received data is split at the `terminator` setting: `any` (CR, LF or
//!   CRLF, whichever the device uses), `cr`, `lf`, `crlf`, `custom` (the
//!   `custom_terminator` setting, with the escapes above) or `none`. The
//!   terminator is removed and empty messages are dropped. With `none`, a UDP
//!   datagram is one message, and over TCP whatever one read returns is one
//!   message. A UDP datagram always ends a message. Over TCP, text with no
//!   terminator yet is held until one arrives, until 64 KiB have gathered, or
//!   until the connection closes.
//! - TCP connects when the device is opened and reconnects with backoff (1 s
//!   doubling to 30 s) whenever the connection fails or the device closes it.
//!   UDP has no connection: the device is reported `unmonitored`, and alive
//!   whenever it sends something.

use std::collections::VecDeque;
use std::net::SocketAddr;

use regex::Regex;
use serde_json::{json, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const SOCKET: Key = "generic";
const RETRY: Key = "retry";
const WAIT: Key = "wait";
pub(super) const RETRY_MIN: Millis = 1_000;
pub(super) const RETRY_MAX: Millis = 30_000;
/// Received bytes held without a terminator before they are taken as a
/// message anyway.
const MAX_MESSAGE: usize = 65_536;
const DEFAULT_RECENT: usize = 20;

/// Decode the escapes documented above into bytes.
pub(super) fn unescape(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next() {
            Some('r') => out.push(b'\r'),
            Some('n') => out.push(b'\n'),
            Some('t') => out.push(b'\t'),
            Some('0') => out.push(0),
            Some('\\') => out.push(b'\\'),
            Some('x') => {
                let digits: String = chars.by_ref().take(2).collect();
                match (digits.len() == 2)
                    .then(|| u8::from_str_radix(&digits, 16).ok())
                    .flatten()
                {
                    Some(b) => out.push(b),
                    None => return Err("\\x must be followed by two hex digits".into()),
                }
            }
            Some(other) => {
                return Err(format!(
                    "unknown escape '\\{other}': the escapes are \\r \\n \\t \\0 \\\\ and \\xNN"
                ))
            }
            None => return Err("text ends in a lone backslash: write \\\\ for one".into()),
        }
    }
    Ok(out)
}

/// Parse hex bytes as documented above.
pub(super) fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    let mut digits = String::new();
    for group in text.split([' ', ':', ',', '-']).filter(|g| !g.is_empty()) {
        let (group, prefixed) = match group
            .strip_prefix("0x")
            .or_else(|| group.strip_prefix("0X"))
        {
            Some(rest) => (rest, true),
            None => (group, false),
        };
        if group.is_empty() || !group.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("'{group}' is not hex"));
        }
        if group.len() % 2 == 1 {
            if !prefixed {
                return Err(format!("'{group}' has an odd number of hex digits"));
            }
            digits.push('0');
        }
        digits.push_str(group);
    }
    Ok((0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).expect("checked hex"))
        .collect())
}

pub(super) fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Where received data is split into messages.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Split {
    None,
    /// CR, LF or CRLF.
    AnyLine,
    Sequence(Vec<u8>),
}

fn terminator(name: &str, custom: &str, setting: &str) -> Result<Option<Vec<u8>>, String> {
    Ok(match name {
        "none" => None,
        "cr" => Some(b"\r".to_vec()),
        "lf" => Some(b"\n".to_vec()),
        "crlf" => Some(b"\r\n".to_vec()),
        "custom" => {
            let bytes = unescape(custom).map_err(|e| format!("custom_terminator: {e}"))?;
            if bytes.is_empty() {
                return Err(format!(
                    "{setting} is custom but custom_terminator is empty"
                ));
            }
            Some(bytes)
        }
        other => return Err(format!("unknown {setting} '{other}'")),
    })
}

/// Splits received bytes into messages.
#[derive(Debug)]
pub(super) struct Framer {
    split: Split,
    buf: Vec<u8>,
}

impl Framer {
    pub(super) fn new(split: Split) -> Framer {
        Framer {
            split,
            buf: Vec::new(),
        }
    }

    /// The first terminator in the buffer: where it starts and its length.
    fn find(&self) -> Option<(usize, usize)> {
        match &self.split {
            Split::None => None,
            Split::AnyLine => {
                let at = self.buf.iter().position(|b| *b == b'\r' || *b == b'\n')?;
                Some((at, 1))
            }
            Split::Sequence(seq) => self
                .buf
                .windows(seq.len())
                .position(|w| w == seq.as_slice())
                .map(|at| (at, seq.len())),
        }
    }

    /// Bytes from a stream. Returns the messages completed.
    pub(super) fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        if self.split == Split::None {
            return if bytes.is_empty() {
                Vec::new()
            } else {
                vec![bytes.to_vec()]
            };
        }
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some((at, len)) = self.find() {
            let message: Vec<u8> = self.buf.drain(..at + len).take(at).collect();
            if !message.is_empty() {
                out.push(message);
            }
        }
        if self.buf.len() >= MAX_MESSAGE {
            out.push(std::mem::take(&mut self.buf));
        }
        out
    }

    /// Whatever is held: the stream ended, or a datagram did.
    pub(super) fn flush(&mut self) -> Option<Vec<u8>> {
        (!self.buf.is_empty()).then(|| std::mem::take(&mut self.buf))
    }

    /// One datagram: its messages, the last one ended by the datagram itself.
    pub(super) fn datagram(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.buf.clear();
        let mut out = self.feed(bytes);
        out.extend(self.flush());
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug)]
struct Request {
    id: CommandId,
    deadline: Millis,
    pattern: Option<Regex>,
}

pub(crate) struct GenericTcpUdp {
    device: SocketAddr,
    protocol: Protocol,
    listen_port: Option<u16>,
    framer: Framer,
    send_terminator: Vec<u8>,
    recent_size: usize,
    recent: VecDeque<Value>,
    received: u64,
    /// TCP connected, or the UDP socket open.
    ready: bool,
    retry_after: Millis,
    requests: VecDeque<Request>,
}

fn str_setting<'a>(s: &'a Params, name: &str, default: &'a str) -> &'a str {
    s.get(name).and_then(Value::as_str).unwrap_or(default)
}

impl GenericTcpUdp {
    pub(crate) fn new(ctx: OpenContext) -> Result<GenericTcpUdp, String> {
        let port = ctx
            .port
            .ok_or("a generic TCP/UDP device has no standard port: give the device's port")?;
        let s = &ctx.settings;
        let protocol = match str_setting(s, "protocol", "tcp") {
            "tcp" => Protocol::Tcp,
            "udp" => Protocol::Udp,
            other => return Err(format!("unknown protocol '{other}'")),
        };
        let custom = str_setting(s, "custom_terminator", "");
        let split = match str_setting(s, "terminator", "any") {
            "any" => Split::AnyLine,
            name => match terminator(name, custom, "terminator")? {
                Some(seq) => Split::Sequence(seq),
                None => Split::None,
            },
        };
        let send_terminator = terminator(
            str_setting(s, "send_terminator", "none"),
            custom,
            "send_terminator",
        )?
        .unwrap_or_default();
        let listen_port = s
            .get("listen_port")
            .and_then(Value::as_u64)
            .map(|p| p as u16);
        if listen_port.is_some() && protocol == Protocol::Tcp {
            return Err("listen_port applies to UDP only".into());
        }
        let recent_size = s
            .get("recent_size")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_RECENT, |n| n as usize);
        Ok(GenericTcpUdp {
            device: SocketAddr::new(ctx.host, port),
            protocol,
            listen_port,
            framer: Framer::new(split),
            send_terminator,
            recent_size,
            recent: VecDeque::new(),
            received: 0,
            ready: false,
            retry_after: RETRY_MIN,
            requests: VecDeque::new(),
        })
    }

    fn open(&mut self, cx: &mut Cx) {
        match self.protocol {
            Protocol::Tcp => {
                cx.connection(Connection::Connecting);
                cx.tcp_open(SOCKET, self.device);
            }
            Protocol::Udp => {
                let bind = self.listen_port.map_or(Bind::Ephemeral, Bind::Shared);
                cx.udp_open(SOCKET, bind);
                self.ready = true;
                cx.connection(Connection::Unmonitored);
            }
        }
    }

    /// The bytes a command sends: text (with escapes and the send terminator)
    /// or hex.
    fn payload(&self, params: &Params) -> Result<Vec<u8>, String> {
        let text = params.get("text").and_then(Value::as_str);
        let hex = params.get("hex").and_then(Value::as_str);
        match (text, hex) {
            (Some(text), None) => {
                let mut bytes = unescape(text)?;
                bytes.extend_from_slice(&self.send_terminator);
                Ok(bytes)
            }
            (None, Some(hex)) => parse_hex(hex),
            _ => Err("give either 'text' or 'hex'".into()),
        }
    }

    fn send(&mut self, cx: &mut Cx, data: Vec<u8>) {
        match self.protocol {
            Protocol::Tcp => cx.tcp_send(SOCKET, data),
            Protocol::Udp => cx.udp_send(SOCKET, self.device, data),
        }
    }

    fn arm(&self, cx: &mut Cx) {
        match self.requests.iter().map(|r| r.deadline).min() {
            Some(at) => cx.set_timer(WAIT, at.saturating_sub(cx.now()).max(1)),
            None => cx.cancel_timer(WAIT),
        }
    }

    fn received(&mut self, cx: &mut Cx, message: Vec<u8>) {
        let text = String::from_utf8_lossy(&message).into_owned();
        let entry = json!({"text": text, "hex": to_hex(&message)});
        self.received += 1;
        self.recent.push_back(entry.clone());
        while self.recent.len() > self.recent_size {
            self.recent.pop_front();
        }
        cx.state(json!({
            "last_message": entry,
            "recent": Value::Array(self.recent.iter().cloned().collect()),
            "received_count": self.received,
        }));
        let answers = self
            .requests
            .iter()
            .position(|r| r.pattern.as_ref().is_none_or(|p| p.is_match(&text)));
        if let Some(at) = answers {
            let request = self.requests.remove(at).expect("found");
            cx.complete(request.id, Ok(Outcome::Value { value: entry }));
            self.arm(cx);
        }
    }

    fn fail_requests(&mut self, cx: &mut Cx, error: CommandError) {
        for request in self.requests.drain(..) {
            cx.complete(request.id, Err(error.clone()));
        }
        cx.cancel_timer(WAIT);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.ready = false;
        self.fail_requests(
            cx,
            CommandError::Transport {
                message: reason.clone(),
            },
        );
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }
}

impl Module for GenericTcpUdp {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        let invalid = |message: String| Err(CommandError::InvalidParams { message });
        match name {
            "send_text" | "send_hex" | "request" => {}
            other => {
                cx.complete(
                    id,
                    Err(CommandError::UnknownCommand {
                        command: other.into(),
                    }),
                );
                return;
            }
        }
        let data = match self.payload(params) {
            Ok(data) if data.is_empty() => {
                cx.complete(id, invalid("nothing to send".into()));
                return;
            }
            Ok(data) => data,
            Err(message) => {
                cx.complete(id, invalid(message));
                return;
            }
        };
        let pattern = match params.get("match").and_then(Value::as_str) {
            Some(p) => match Regex::new(p) {
                Ok(re) => Some(re),
                Err(e) => {
                    cx.complete(id, invalid(format!("match: {e}")));
                    return;
                }
            },
            None => None,
        };
        if !self.ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        self.send(cx, data);
        if name == "request" {
            let timeout = params
                .get("timeout_ms")
                .and_then(Value::as_u64)
                .unwrap_or(2_000);
            self.requests.push_back(Request {
                id,
                deadline: cx.now() + timeout,
                pattern,
            });
            self.arm(cx);
        } else {
            // Neither TCP nor UDP says whether the device acted on it.
            cx.complete(id, Ok(Outcome::Unverified));
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, _from: SocketAddr, data: &[u8]) {
        cx.alive();
        for message in self.framer.datagram(data) {
            self.received(cx, message);
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        cx.log(Level::Warning, format!("UDP socket: {message}"));
        self.lost(cx, message.to_string());
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.ready = true;
                self.retry_after = RETRY_MIN;
                self.framer.flush();
                cx.connection(Connection::Connected);
            }
            TcpInput::Data(bytes) => {
                cx.alive();
                for message in self.framer.feed(&bytes) {
                    self.received(cx, message);
                }
            }
            TcpInput::Closed { reason } => {
                // A device that answers and then closes, without a
                // terminator, has still answered.
                if let Some(rest) = self.framer.flush() {
                    self.received(cx, rest);
                }
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.open(cx),
            WAIT => {
                let now = cx.now();
                let (due, keep): (Vec<Request>, Vec<Request>) =
                    self.requests.drain(..).partition(|r| r.deadline <= now);
                self.requests = keep.into();
                for request in due {
                    cx.complete(request.id, Err(CommandError::Timeout));
                }
                self.arm(cx);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        match self.protocol {
            Protocol::Tcp => cx.tcp_close(SOCKET),
            Protocol::Udp => cx.udp_close(SOCKET),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn module(settings: Value) -> GenericTcpUdp {
        GenericTcpUdp::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)),
            port: Some(5000),
            model: "generic".into(),
            channels: None,
            settings: params(settings),
            monitor: true,
        })
        .unwrap()
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } | Action::UdpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    fn completion(actions: &[Action], id: CommandId) -> Option<crate::module::CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    #[test]
    fn escapes() {
        assert_eq!(unescape(r"PWR ON\r\n").unwrap(), b"PWR ON\r\n");
        assert_eq!(unescape(r"\x02go\x03").unwrap(), b"\x02go\x03");
        assert_eq!(unescape(r"a\tb\0c\\d").unwrap(), b"a\tb\0c\\d");
        assert_eq!(unescape(r"\xFF").unwrap(), vec![0xff]);
        assert_eq!(unescape("é").unwrap(), "é".as_bytes());
        assert!(unescape(r"\q").is_err());
        assert!(unescape(r"\x4").is_err());
        assert!(unescape(r"\xZZ").is_err());
        assert!(unescape(r"end\").is_err());
    }

    #[test]
    fn hex() {
        assert_eq!(
            parse_hex("81 01 04 00 02 FF").unwrap(),
            b"\x81\x01\x04\x00\x02\xff"
        );
        assert_eq!(parse_hex("810104").unwrap(), b"\x81\x01\x04");
        assert_eq!(parse_hex("0x81,0x1:aa-bb").unwrap(), b"\x81\x01\xaa\xbb");
        assert_eq!(parse_hex("").unwrap(), b"");
        assert!(parse_hex("abc").is_err());
        assert!(parse_hex("zz").is_err());
        assert_eq!(to_hex(b"\x00\xffA"), "00ff41");
    }

    #[test]
    fn framing() {
        let mut f = Framer::new(Split::AnyLine);
        assert_eq!(
            f.feed(b"one\r\ntwo\rthr"),
            vec![b"one".to_vec(), b"two".to_vec()]
        );
        assert_eq!(f.feed(b"ee\n"), vec![b"three".to_vec()]);

        let mut f = Framer::new(Split::Sequence(b"\r\n".to_vec()));
        assert_eq!(f.feed(b"a\rb\r"), Vec::<Vec<u8>>::new());
        assert_eq!(f.feed(b"\nc"), vec![b"a\rb".to_vec()]);
        assert_eq!(f.flush(), Some(b"c".to_vec()));

        let mut f = Framer::new(Split::Sequence(b";".to_vec()));
        assert_eq!(
            f.datagram(b"VFL:a;VFL:b"),
            vec![b"VFL:a".to_vec(), b"VFL:b".to_vec()]
        );

        let mut f = Framer::new(Split::None);
        assert_eq!(f.feed(b"\x01\x02"), vec![b"\x01\x02".to_vec()]);
        assert_eq!(f.datagram(b"x\r\n"), vec![b"x\r\n".to_vec()]);

        // Held text without a terminator is let go at 64 KiB.
        let mut f = Framer::new(Split::AnyLine);
        assert_eq!(f.feed(&vec![b'x'; MAX_MESSAGE]).len(), 1);
    }

    #[test]
    fn settings_are_checked() {
        let open = |settings: Value| {
            GenericTcpUdp::new(OpenContext {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: Some(1),
                model: "generic".into(),
                channels: None,
                settings: params(settings),
                monitor: true,
            })
        };
        assert!(open(json!({"terminator": "custom"})).is_err());
        assert!(open(json!({"listen_port": 9000})).is_err());
        let m = open(json!({"terminator": "custom", "custom_terminator": r"\x03", "send_terminator": "custom"})).unwrap();
        assert_eq!(m.framer.split, Split::Sequence(vec![3]));
        assert_eq!(m.send_terminator, vec![3]);
        let no_port = GenericTcpUdp::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: None,
            model: "generic".into(),
            channels: None,
            settings: Params::new(),
            monitor: true,
        });
        assert!(no_port.is_err());
    }

    #[test]
    fn tcp_send_request_and_reconnect() {
        let mut m = module(json!({"send_terminator": "cr"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::Connection(Connection::Connecting)));

        // Not connected yet.
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "send_text", &params(json!({"text": "PWR?"})));
        assert_eq!(
            completion(&cx.take(), 1),
            Some(Err(CommandError::NotConnected))
        );

        let mut cx = Cx::new(10);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert!(cx
            .take()
            .contains(&Action::Connection(Connection::Connected)));

        let mut cx = Cx::new(20);
        m.command(
            &mut cx,
            2,
            "send_text",
            &params(json!({"text": r"PWR ON\x21"})),
        );
        let a = cx.take();
        assert_eq!(sent(&a), vec![b"PWR ON!\r".to_vec()]);
        assert_eq!(completion(&a, 2), Some(Ok(Outcome::Unverified)));

        let mut cx = Cx::new(30);
        m.command(&mut cx, 3, "send_hex", &params(json!({"hex": "02 41 03"})));
        assert_eq!(sent(&cx.take()), vec![b"\x02A\x03".to_vec()]);

        // A request is answered by the next message matching it.
        let mut cx = Cx::new(40);
        m.command(
            &mut cx,
            4,
            "request",
            &params(json!({"text": "PWR?", "match": "^PWR=", "timeout_ms": 500})),
        );
        let a = cx.take();
        assert!(a.contains(&Action::SetTimer {
            key: WAIT,
            after: 500
        }));
        let mut cx = Cx::new(50);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(b"NOISE\r\nPWR=1\r\n".to_vec()),
        );
        let a = cx.take();
        assert_eq!(
            completion(&a, 4),
            Some(Ok(Outcome::Value {
                value: json!({"text": "PWR=1", "hex": "5057523d31"})
            }))
        );
        let mut state = json!({});
        for action in &a {
            if let Action::State(p) = action {
                crate::session::merge_patch(&mut state, p);
            }
        }
        assert_eq!(state["last_message"]["text"], "PWR=1");
        assert_eq!(state["recent"].as_array().unwrap().len(), 2);
        assert_eq!(state["received_count"], 2);

        // An unanswered request times out.
        let mut cx = Cx::new(60);
        m.command(
            &mut cx,
            5,
            "request",
            &params(json!({"text": "X", "timeout_ms": 100})),
        );
        cx.take();
        let mut cx = Cx::new(160);
        m.timer(&mut cx, WAIT);
        assert_eq!(completion(&cx.take(), 5), Some(Err(CommandError::Timeout)));

        // Closing flushes what was held, fails waiting requests and retries.
        let mut cx = Cx::new(200);
        m.command(
            &mut cx,
            6,
            "request",
            &params(json!({"text": "Y", "match": "^Z"})),
        );
        cx.take();
        let mut cx = Cx::new(210);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(b"partial".to_vec()));
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        let a = cx.take();
        assert!(matches!(
            completion(&a, 6),
            Some(Err(CommandError::Transport { .. }))
        ));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::State(p) if p["last_message"]["text"] == "partial")));
        let mut cx = Cx::new(1_210);
        m.timer(&mut cx, RETRY);
        assert!(cx
            .take()
            .iter()
            .any(|x| matches!(x, Action::TcpOpen { .. })));
        // A second failure backs off further.
        let mut cx = Cx::new(1_300);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "refused".into(),
            },
        );
        assert!(cx.take().contains(&Action::SetTimer {
            key: RETRY,
            after: 2 * RETRY_MIN
        }));
    }

    #[test]
    fn udp_with_a_fixed_listen_port() {
        let mut m = module(
            json!({"protocol": "udp", "listen_port": 9100, "terminator": "none", "recent_size": 1}),
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Shared(9100)
        }));
        assert!(a.contains(&Action::Connection(Connection::Unmonitored)));
        let from = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)), 5000);
        let mut cx = Cx::new(10);
        m.datagram(&mut cx, SOCKET, from, b"a\r\n");
        m.datagram(&mut cx, SOCKET, from, b"b");
        let patches: Vec<Value> = cx
            .take()
            .into_iter()
            .filter_map(|a| match a {
                Action::State(p) => Some(p),
                _ => None,
            })
            .collect();
        assert_eq!(patches[0]["last_message"]["hex"], "610d0a");
        assert_eq!(patches[1]["recent"], json!([{"text": "b", "hex": "62"}]));
    }
}
