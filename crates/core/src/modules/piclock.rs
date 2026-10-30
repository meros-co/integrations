//! PiClock studio clocks: the core is the clock's tally server, written from
//! PiClock's Tally Protocol document (TallyProtocol.md).
//!
//! - The clock connects to its tally servers on TCP 6254 (by default), so the
//!   core listens there and takes the connection from the clock's address.
//! - The server sends `CRYPT:<random string>`; the clock answers
//!   `AUTH:<sha512 digest>:<mac address>`, the digest being SHA-512 of the
//!   random string followed by the shared secret. A wrong digest is refused
//!   by closing the connection.
//! - Messages are UTF-8 lines `CMD:ARG1:ARG2...` ended by CR. The server must
//!   send something at least every 5 seconds or the clock drops the
//!   connection: `PING` every `ping_ms`, answered `PONG`. Every other command
//!   is answered `ACK`, or `NACK` when the clock does not understand it; they
//!   go one at a time.
//! - The clock sends `GPI:<state>` whenever an input changes and on each new
//!   connection: a 16-bit word, one bit per input.
//!
//! The protocol is one-directional apart from the GPIs: what a command sets
//! is not read back. Opened for commands only (`monitor` false) nothing
//! changes: the pings are what keeps the clock connected.

use std::collections::VecDeque;

use crate::{
    json,
    value::{Map, Value},
};
use sha2::{Digest, Sha512};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

const SOCKET: Key = "tally";
const PING_TIMER: Key = "ping";
const REPLY_TIMER: Key = "reply";
const RETRY_TIMER: Key = "retry";

pub(crate) const DEFAULT_PORT: u16 = 6254;

const REPLY_TIMEOUT: Millis = 3_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// Pings unanswered before the clock is taken to be gone.
const MISSED_PONGS: u32 = 3;

fn sha512_hex(text: &str) -> String {
    Sha512::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn nonce() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        // No system randomness: the time, hashed, still differs per
        // connection.
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        return sha512_hex(&t.to_string())[..32].to_string();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn bad(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn int(params: &Params, name: &str) -> Result<i64, CommandError> {
    params
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

fn float(params: &Params, name: &str) -> Result<f64, CommandError> {
    params
        .get(name)
        .and_then(Value::as_f64)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

fn flag(params: &Params, name: &str) -> Result<&'static str, CommandError> {
    params
        .get(name)
        .and_then(Value::as_bool)
        .map(|b| if b { "1" } else { "0" })
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

/// A text argument: never a line ending, and no colon unless it is the last
/// argument of its command (`last`).
fn text(params: &Params, name: &str, last: bool) -> Result<String, CommandError> {
    let t = params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(format!("'{name}' is required")))?;
    if t.contains('\r') || t.contains('\n') {
        return Err(bad(format!("'{name}' cannot hold a line break")));
    }
    if !last && t.contains(':') {
        return Err(bad(format!("'{name}' cannot hold ':'")));
    }
    Ok(t.to_string())
}

/// A color as the protocol writes it: `#RRGGBB`.
fn color(params: &Params, name: &str) -> Result<String, CommandError> {
    let t = text(params, name, false)?;
    let hex = t.trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(bad(format!("'{name}' must be RRGGBB")));
    }
    Ok(format!("#{}", hex.to_ascii_uppercase()))
}

/// A region command's prefix: the region's number.
fn region(params: &Params) -> Result<String, CommandError> {
    Ok(int(params, "region")?.to_string())
}

/// The line a command sends, without its CR.
fn line(name: &str, p: &Params) -> Result<String, CommandError> {
    Ok(match name {
        "set_global" => format!(
            "SETGLOBAL:{}:{}",
            flag(p, "landscape")?,
            flag(p, "screensaver")?
        ),
        "set_profile" => format!("SETPROFILE:{}", text(p, "name", false)?),
        "set_region_count" => format!("SETREGIONCOUNT:{}", int(p, "count")?),
        "store_font" => format!(
            "STOREFONT:{}:{}",
            text(p, "name", false)?,
            text(p, "data", true)?
        ),
        "clear_fonts" => "CLEARFONTS".into(),
        "set_fonts" => {
            let mut out = "SETFONTS".to_string();
            for key in ["tally", "tally_label", "status", "digital", "date", "hours"] {
                out.push(':');
                out.push_str(&text(p, key, false)?);
            }
            out
        }
        "store_image" => format!(
            "STOREIMAGE:{}:{}",
            text(p, "name", false)?,
            text(p, "image", true)?
        ),
        "clear_images" => "CLEARIMAGES".into(),
        "set_size" => format!(
            "{}SETSIZE:{}:{}",
            region(p)?,
            int(p, "rows")?,
            int(p, "columns")?
        ),
        "set_row" => format!(
            "{}SETROW:{}:{}",
            region(p)?,
            int(p, "row")?,
            int(p, "columns")?
        ),
        "set_tally" => format!(
            "{}SETTALLY:{}:{}:{}:{}:{}",
            region(p)?,
            int(p, "row")?,
            int(p, "column")?,
            color(p, "foreground")?,
            color(p, "background")?,
            text(p, "text", true)?
        ),
        "set_label" => format!(
            "{}SETLABEL:{}:{}:{}",
            region(p)?,
            int(p, "row")?,
            int(p, "column")?,
            text(p, "label", true)?
        ),
        "set_countdown" => {
            let target = int(p, "target_ms")?;
            let flash = int(p, "flash_seconds")?;
            format!(
                "{}SETCOUNTDOWN:{}:{}:{}:{}:{}:{}:{}:{}",
                region(p)?,
                int(p, "row")?,
                int(p, "column")?,
                color(p, "foreground")?,
                color(p, "background")?,
                target.div_euclid(1000),
                target.rem_euclid(1000) * 1000,
                if flash < 0 {
                    String::new()
                } else {
                    flash.to_string()
                },
                text(p, "label", true)?
            )
        }
        "set_layout" => {
            let mut out = format!("{}SETLAYOUT", region(p)?);
            for key in [
                "analogue_clock",
                "analogue_clock_local",
                "digital_clock_utc",
                "digital_clock_local",
                "date",
                "date_local",
            ] {
                out.push(':');
                out.push_str(flag(p, key)?);
            }
            // The unused legacy field.
            out.push(':');
            for key in ["numbers_present", "numbers_outside", "seconds_sweep"] {
                out.push(':');
                out.push_str(flag(p, key)?);
            }
            for key in [
                "clock_face_image",
                "hour_hand",
                "minute_hand",
                "second_hand",
            ] {
                out.push(':');
                out.push_str(&text(p, key, false)?);
            }
            out
        }
        "set_clocks" => {
            let mut out = format!(
                "{}SETCLOCKS:{}:{}",
                region(p)?,
                text(p, "date_timezone", false)?,
                text(p, "analogue_timezone", false)?
            );
            let clocks = text(p, "digital_clocks", false)?;
            for clock in clocks.split(',').map(str::trim).filter(|c| !c.is_empty()) {
                let (tz, label) = clock.split_once('=').unwrap_or((clock, ""));
                out.push_str(&format!(":{}:{}", tz.trim(), label.trim()));
            }
            out
        }
        "set_location" => format!(
            "{}SETLOCATION:{}:{}:{}:{}",
            region(p)?,
            float(p, "x")?,
            float(p, "y")?,
            float(p, "width")?,
            float(p, "height")?
        ),
        "set_font_size_zones" => {
            let mut out = format!("{}SETFONTSIZEZONES", region(p)?);
            for row in text(p, "zones", false)?.split(';') {
                out.push(':');
                out.push_str(row.trim());
            }
            out
        }
        "set_gpo" => format!("SETGPO:{}:{}", int(p, "output")?, flag(p, "on")?),
        "send" => text(p, "line", true)?,
        other => {
            return Err(CommandError::UnknownCommand {
                command: other.into(),
            })
        }
    })
}

#[derive(Debug, PartialEq)]
enum Phase {
    /// No clock connected.
    Waiting,
    /// CRYPT sent, its AUTH awaited.
    Challenged(String),
    Authenticated,
}

pub(crate) struct PiClock {
    port: u16,
    password: String,
    ping_ms: Millis,
    phase: Phase,
    buffer: Vec<u8>,
    queue: VecDeque<(CommandId, String)>,
    in_flight: Option<(CommandId, Millis)>,
    ping_sent: Option<Millis>,
    missed_pongs: u32,
    retry_after: Millis,
}

impl PiClock {
    pub(crate) fn new(ctx: OpenContext) -> Result<PiClock, String> {
        if ctx.host.is_unspecified() {
            return Err("give the clock's address as host: each clock is its own device".into());
        }
        let s = &ctx.settings;
        let port = s
            .get("listen_port")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_PORT, |p| p as u16);
        Ok(PiClock {
            port: ctx.port.unwrap_or(port),
            password: s
                .get("password")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            ping_ms: s
                .get("ping_ms")
                .and_then(Value::as_u64)
                .unwrap_or(2_000)
                .clamp(500, 4_500),
            phase: Phase::Waiting,
            buffer: Vec::new(),
            queue: VecDeque::new(),
            in_flight: None,
            ping_sent: None,
            missed_pongs: 0,
            retry_after: RETRY_MIN,
        })
    }

    fn send(cx: &mut Cx, line: &str) {
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\r');
        cx.tcp_send(SOCKET, bytes);
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || self.phase != Phase::Authenticated {
            return;
        }
        let Some((id, line)) = self.queue.pop_front() else {
            return;
        };
        Self::send(cx, &line);
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        self.in_flight = Some((id, cx.now()));
    }

    fn fail_all(&mut self, cx: &mut Cx, error: CommandError) {
        if let Some((id, _)) = self.in_flight.take() {
            cx.complete(id, Err(error.clone()));
        }
        for (id, _) in self.queue.drain(..) {
            cx.complete(id, Err(error.clone()));
        }
    }

    fn gone(&mut self, cx: &mut Cx, reason: String) {
        self.phase = Phase::Waiting;
        self.buffer.clear();
        self.ping_sent = None;
        self.missed_pongs = 0;
        cx.cancel_timer(PING_TIMER);
        cx.cancel_timer(REPLY_TIMER);
        self.fail_all(
            cx,
            CommandError::Transport {
                message: reason.clone(),
            },
        );
        cx.connection(Connection::Disconnected { reason });
    }

    fn received(&mut self, cx: &mut Cx, line: &str) {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return;
        }
        let (cmd, rest) = line.split_once(':').unwrap_or((line, ""));
        match cmd {
            "AUTH" => {
                let Phase::Challenged(challenge) = &self.phase else {
                    return;
                };
                let (digest, mac) = rest.split_once(':').unwrap_or((rest, ""));
                let expected = sha512_hex(&format!("{challenge}{}", self.password));
                if !digest.trim().eq_ignore_ascii_case(&expected) {
                    cx.log(
                        Level::Warning,
                        "the clock's AUTH digest does not match the password: connection closed",
                    );
                    cx.tcp_close(SOCKET);
                    self.phase = Phase::Waiting;
                    cx.connection(Connection::Unauthorized {
                        reason: "the clock's shared secret differs from the password setting"
                            .into(),
                    });
                    return;
                }
                self.phase = Phase::Authenticated;
                cx.state(json!({"mac_address": mac.trim().to_ascii_uppercase()}));
                cx.connection(Connection::Connected);
                cx.alive();
                cx.set_timer(PING_TIMER, self.ping_ms);
                self.pump(cx);
            }
            "PONG" => {
                if let Some(sent) = self.ping_sent.take() {
                    cx.round_trip(cx.now().saturating_sub(sent));
                }
                self.missed_pongs = 0;
                cx.alive();
            }
            "ACK" | "NACK" => {
                let Some((id, sent)) = self.in_flight.take() else {
                    return;
                };
                cx.cancel_timer(REPLY_TIMER);
                cx.round_trip(cx.now().saturating_sub(sent));
                cx.alive();
                let result = if cmd == "ACK" {
                    Ok(Outcome::Ack)
                } else {
                    Err(CommandError::DeviceError {
                        code: Some("NACK".into()),
                        message: "the clock does not understand the command (NACK)".into(),
                    })
                };
                cx.complete(id, result);
                self.pump(cx);
            }
            "GPI" => {
                let Ok(word) = rest.trim().parse::<u32>() else {
                    return;
                };
                let mut gpi = Map::new();
                for bit in 0..16 {
                    gpi.insert(bit.to_string(), json!(word & (1 << bit) != 0));
                }
                cx.state(json!({"gpi": gpi, "gpi_word": word}));
            }
            _ => cx.log(
                Level::Debug,
                format!("unexpected line from the clock: {line}"),
            ),
        }
    }
}

impl Module for PiClock {
    fn start(&mut self, cx: &mut Cx) {
        cx.tcp_listen(SOCKET, self.port);
        cx.connection(Connection::Disconnected {
            reason: "waiting for the clock to connect".into(),
        });
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match line(name, params) {
            Ok(line) => {
                if self.phase != Phase::Authenticated {
                    cx.complete(id, Err(CommandError::NotConnected));
                    return;
                }
                self.queue.push_back((id, line));
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                // A newer connection from the clock replaces any before it.
                self.fail_all(
                    cx,
                    CommandError::Transport {
                        message: "the clock connected again".into(),
                    },
                );
                self.buffer.clear();
                self.retry_after = RETRY_MIN;
                let challenge = nonce();
                Self::send(cx, &format!("CRYPT:{challenge}"));
                self.phase = Phase::Challenged(challenge);
                cx.connection(Connection::Connecting);
            }
            TcpInput::Data(bytes) => {
                self.buffer.extend_from_slice(&bytes);
                while let Some(end) = self.buffer.iter().position(|b| *b == b'\r' || *b == b'\n') {
                    let raw: Vec<u8> = self.buffer.drain(..=end).collect();
                    let line = String::from_utf8_lossy(&raw).into_owned();
                    self.received(cx, &line);
                }
            }
            TcpInput::Closed { reason } => {
                self.gone(cx, format!("the clock's connection closed: {reason}"));
            }
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        cx.log(Level::Warning, message.to_string());
        cx.connection(Connection::Disconnected {
            reason: message.to_string(),
        });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            PING_TIMER if self.phase == Phase::Authenticated => {
                if self.ping_sent.is_some() {
                    self.missed_pongs += 1;
                    if self.missed_pongs >= MISSED_PONGS {
                        cx.tcp_close(SOCKET);
                        self.gone(cx, "the clock answered none of three pings".into());
                        return;
                    }
                }
                Self::send(cx, "PING");
                self.ping_sent = Some(cx.now());
                cx.set_timer(PING_TIMER, self.ping_ms);
            }
            REPLY_TIMER => {
                if let Some((id, _)) = self.in_flight.take() {
                    cx.complete(id, Err(CommandError::Timeout));
                }
                self.pump(cx);
            }
            RETRY_TIMER => self.start(cx),
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn module() -> PiClock {
        let spec = Catalog::source_tree().devices["piclock"].clone();
        let settings = validate(
            &spec.settings,
            json!({"password": "secret"}).as_object().unwrap(),
        )
        .unwrap();
        PiClock::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 50)),
            host_name: None,
            port: None,
            model: "piclock".into(),
            channels: None,
            settings,
            monitor: true,
        })
        .unwrap()
    }

    fn lines(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(String::from_utf8(data.clone()).unwrap()),
                _ => None,
            })
            .collect()
    }

    fn feed(m: &mut PiClock, now: Millis, text: &str) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(text.as_bytes().to_vec()));
        cx.take()
    }

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    /// Start, accept the clock's connection and answer the challenge.
    fn authenticated(m: &mut PiClock, password: &str) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::TcpListen { port: 6254, .. })));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let sent = lines(&cx.take());
        let challenge = sent[0]
            .strip_prefix("CRYPT:")
            .unwrap()
            .trim_end()
            .to_string();
        assert_eq!(challenge.len(), 32);
        let digest = sha512_hex(&format!("{challenge}{password}"));
        feed(m, 1, &format!("AUTH:{digest}:b827eb123456\r"))
    }

    fn run(m: &mut PiClock, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(2);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    #[test]
    fn the_clock_is_challenged_and_authenticated() {
        let mut m = module();
        let a = authenticated(&mut m, "secret");
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let mut state = json!({});
        for x in &a {
            if let Action::State(p) = x {
                merge_patch(&mut state, p);
            }
        }
        assert_eq!(state["mac_address"], json!("B827EB123456"));
    }

    #[test]
    fn a_wrong_secret_closes_the_connection() {
        let mut m = module();
        let a = authenticated(&mut m, "wrong");
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(a.iter().any(|x| matches!(x, Action::TcpClose { .. })));
        let a = run(&mut m, 1, "clear_images", json!({}));
        assert_eq!(completed(&a, 1), Some(Err(CommandError::NotConnected)));
    }

    #[test]
    fn commands_are_sent_as_the_document_writes_them_and_acknowledged() {
        let mut m = module();
        authenticated(&mut m, "secret");
        let a = run(
            &mut m,
            1,
            "set_tally",
            json!({"region": 0, "row": 1, "column": 2, "foreground": "ffffff",
                   "background": "#ff0000", "text": "ON AIR: Studio 1"}),
        );
        assert_eq!(
            lines(&a),
            ["0SETTALLY:1:2:#FFFFFF:#FF0000:ON AIR: Studio 1\r"]
        );
        // A second command waits for the first's answer.
        let a = run(&mut m, 2, "set_gpo", json!({"output": 3, "on": true}));
        assert!(lines(&a).is_empty());
        let a = feed(&mut m, 5, "ACK\r");
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert!(a.contains(&Action::RoundTrip(3)));
        assert_eq!(lines(&a), ["SETGPO:3:1\r"]);
        let a = feed(&mut m, 6, "NACK\r");
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
        let p = |name: &str, v: Value| line(name, v.as_object().unwrap()).unwrap();
        assert_eq!(
            p(
                "set_countdown",
                json!({"region": 1, "row": 0, "column": 0, "foreground": "000000",
                "background": "00ff00", "target_ms": 1_793_188_800_250i64, "flash_seconds": -1,
                "label": "VT"})
            ),
            "1SETCOUNTDOWN:0:0:#000000:#00FF00:1793188800:250000::VT"
        );
        assert_eq!(
            p(
                "set_clocks",
                json!({"region": 0, "date_timezone": "LOCAL", "analogue_timezone": "",
                "digital_clocks": "Europe/London=LON, UTC=UTC"})
            ),
            "0SETCLOCKS:LOCAL::Europe/London:LON:UTC:UTC"
        );
        assert_eq!(
            p(
                "set_layout",
                json!({"region": 0, "analogue_clock": true, "analogue_clock_local": true,
                "digital_clock_utc": false, "digital_clock_local": true, "date": true,
                "date_local": true, "numbers_present": true, "numbers_outside": false,
                "seconds_sweep": true, "clock_face_image": "", "hour_hand": "#000000",
                "minute_hand": "", "second_hand": ""})
            ),
            "0SETLAYOUT:1:1:0:1:1:1::1:0:1::#000000::"
        );
        assert!(line("set_profile", json!({"name": "a:b"}).as_object().unwrap()).is_err());
    }

    #[test]
    fn pings_keep_the_link_and_gpis_are_read() {
        let mut m = module();
        authenticated(&mut m, "secret");
        let mut cx = Cx::new(10);
        m.timer(&mut cx, PING_TIMER);
        assert_eq!(lines(&cx.take()), ["PING\r"]);
        let a = feed(&mut m, 14, "PONG\rGPI:5\r");
        assert!(a.contains(&Action::RoundTrip(4)));
        let mut state = json!({});
        for x in &a {
            if let Action::State(p) = x {
                merge_patch(&mut state, p);
            }
        }
        assert_eq!(state["gpi"]["0"], json!(true));
        assert_eq!(state["gpi"]["1"], json!(false));
        assert_eq!(state["gpi"]["2"], json!(true));
        assert_eq!(state["gpi_word"], json!(5));
        // Three pings unanswered: the clock is gone.
        let mut last = Vec::new();
        for t in 0..4 {
            let mut cx = Cx::new(20 + t);
            m.timer(&mut cx, PING_TIMER);
            last = cx.take();
        }
        assert!(last
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
    }

    #[test]
    fn every_command_of_the_spec_makes_a_line() {
        let spec = Catalog::source_tree().devices["piclock"].clone();
        for (name, command) in &spec.commands {
            let mut params = Params::new();
            for (p, decl) in &command.params {
                let value = match (&decl.default, decl.kind) {
                    (Some(v), _) => v.clone(),
                    (None, crate::catalog::ParamType::Bool) => json!(true),
                    (None, crate::catalog::ParamType::Float) => json!(0.5),
                    (None, crate::catalog::ParamType::Int) => json!(decl.min.unwrap_or(0.0) as i64),
                    _ if ["foreground", "background"].contains(&p.as_str()) => json!("#00FF00"),
                    _ => json!("Text"),
                };
                params.insert(p.clone(), value);
            }
            let made = line(name, &params);
            assert!(made.is_ok(), "{name}: {made:?}");
        }
    }
}
