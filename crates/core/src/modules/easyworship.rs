//! Softouch EasyWorship's remote protocol, the one the EasyWorship Remote app
//! uses: JSON objects over TCP, one per CRLF-ended line, to the port
//! EasyWorship advertises over Bonjour (`_ezwremote._tcp`). Softouch does not
//! publish it; this follows the MIT-licensed Bitfocus Companion module (see
//! the spec's sources).
//!
//! On connecting the module asks to pair (`{"action":"connect", "uid", ...}`)
//! and EasyWorship answers `paired`, or `notPaired` until the request is
//! approved on the EasyWorship computer; the request goes again every
//! [`PAIR_RETRY`] until it is. Every message EasyWorship sends carries a
//! `requestrev`, which the module keeps and sends back in a `heartbeat` after
//! each message (but `notPaired`), in every command, and every
//! [`KEEPALIVE`] while paired. `status` messages carry the logo, black and
//! clear flags and the live item, and become the state. The logo, black and
//! clear commands send back the whole status with the flag changed, since
//! EasyWorship replaces its state from it. Nothing is acknowledged, so
//! commands are unverified.
//!
//! Opened for commands only it behaves the same: it asks for nothing beyond
//! pairing, which commands need, and EasyWorship's status messages arrive
//! unasked and still update the state. The pairing request is the one
//! request answered directly, so it alone reports a round trip.

use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Millis, Module, OpenContext, Outcome, TcpInput,
};

const SOCKET: Key = "easyworship";
const RETRY: Key = "retry";
const PAIR: Key = "pair";
const HEARTBEAT: Key = "heartbeat";

/// The pairing request again while EasyWorship has not answered `paired`.
const PAIR_RETRY: Millis = 15_000;
/// A heartbeat while paired and quiet, as the Companion module sends.
const KEEPALIVE: Millis = 30_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// A line longer than this is not EasyWorship's; the connection is dropped.
const MAX_LINE: usize = 1 << 20;
/// The device type the Companion module pairs as.
const DEVICE_TYPE: u64 = 8;

/// The status fields EasyWorship needs back when a flag changes, in the
/// order the Companion module sends them.
const ECHOED: [&str; 9] = [
    "rectype",
    "pres_rowid",
    "slide_rowid",
    "pres_no",
    "slide_no",
    "schedulerev",
    "liverev",
    "imagehash",
    "permissions",
];

pub(crate) struct EasyWorship {
    device: SocketAddr,
    uid: String,
    name: String,
    socket_open: bool,
    paired: bool,
    buffer: Vec<u8>,
    /// The last `requestrev` EasyWorship sent.
    requestrev: Option<String>,
    /// The last status: logo, black, clear and the echoed fields.
    logo: bool,
    black: bool,
    clear: bool,
    echoed: Map<String, Value>,
    /// When the pairing request in flight was sent.
    pair_sent: Option<Millis>,
    retry_after: Millis,
}

impl EasyWorship {
    pub(crate) fn new(ctx: &OpenContext, port: u16) -> EasyWorship {
        let setting = |name: &str| {
            ctx.settings
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let name = match setting("device_name") {
            n if n.is_empty() => "Meros".to_string(),
            n => n,
        };
        // EasyWorship remembers a paired controller by its uid, so it has to
        // stay the same from one connection to the next.
        let uid = match setting("client_id") {
            id if id.is_empty() => format!("meros-{name}"),
            id => id,
        };
        EasyWorship {
            device: SocketAddr::new(ctx.host, port),
            uid,
            name,
            socket_open: false,
            paired: false,
            buffer: Vec::new(),
            requestrev: None,
            logo: false,
            black: false,
            clear: false,
            echoed: Map::new(),
            pair_sent: None,
            retry_after: RETRY_MIN,
        }
    }

    fn send(&self, cx: &mut Cx, message: &Value) {
        let mut line = message.to_string();
        line.push_str("\r\n");
        cx.tcp_send(SOCKET, line);
    }

    fn pair(&mut self, cx: &mut Cx) {
        let request = json!({
            "action": "connect",
            "uid": self.uid,
            "device_name": self.name,
            "device_type": DEVICE_TYPE,
            "requestrev": "0",
        });
        self.send(cx, &request);
        self.pair_sent = Some(cx.now());
        cx.set_timer(PAIR, PAIR_RETRY);
    }

    /// `{"action": action, "requestrev": rev}`, the revision left out until
    /// EasyWorship has sent one.
    fn action(&self, action: &str) -> Value {
        let mut m = Map::new();
        m.insert("action".into(), json!(action));
        if let Some(rev) = &self.requestrev {
            m.insert("requestrev".into(), json!(rev));
        }
        Value::Object(m)
    }

    /// The whole status with the flags as given.
    fn status(&self, logo: bool, black: bool, clear: bool) -> Value {
        let mut m = Map::new();
        m.insert("action".into(), json!("status"));
        m.insert("logo".into(), json!(logo));
        m.insert("black".into(), json!(black));
        m.insert("clear".into(), json!(clear));
        for key in ECHOED {
            if let Some(v) = self.echoed.get(key) {
                m.insert(key.into(), v.clone());
            }
        }
        if let Some(rev) = &self.requestrev {
            m.insert("requestrev".into(), json!(rev));
        }
        Value::Object(m)
    }

    fn feed(&mut self, cx: &mut Cx, data: &[u8]) {
        self.buffer.extend_from_slice(data);
        while let Some(end) = self.buffer.windows(2).position(|w| w == b"\r\n") {
            let line: Vec<u8> = self.buffer.drain(..end + 2).collect();
            let text = String::from_utf8_lossy(&line[..end]).into_owned();
            if !text.trim().is_empty() {
                self.message(cx, &text);
            }
        }
        if self.buffer.len() > MAX_LINE {
            self.lost(
                cx,
                "EasyWorship sent a line too long to be a message".into(),
            );
        }
    }

    fn message(&mut self, cx: &mut Cx, text: &str) {
        let Ok(Value::Object(m)) = serde_json::from_str::<Value>(text) else {
            cx.log(
                crate::module::Level::Debug,
                format!("not a JSON object: {text}"),
            );
            return;
        };
        let Some(action) = m.get("action").and_then(Value::as_str) else {
            return;
        };
        match m.get("requestrev") {
            Some(Value::String(s)) => self.requestrev = Some(s.clone()),
            Some(Value::Number(n)) => self.requestrev = Some(n.to_string()),
            _ => {}
        }
        match action {
            "paired" => {
                if let Some(sent) = self.pair_sent.take() {
                    cx.round_trip(cx.now().saturating_sub(sent));
                }
                cx.cancel_timer(PAIR);
                self.paired = true;
                self.retry_after = RETRY_MIN;
                cx.connection(Connection::Connected);
                cx.state(json!({"paired": true}));
                cx.set_timer(HEARTBEAT, KEEPALIVE);
            }
            "notPaired" => {
                self.pair_sent = None;
                self.paired = false;
                cx.connection(Connection::Unauthorized {
                    reason: "EasyWorship has not paired this controller: approve it on the \
                             EasyWorship computer"
                        .into(),
                });
                cx.state(json!({"paired": false}));
                cx.cancel_timer(HEARTBEAT);
                // No heartbeat for notPaired: EasyWorship refuses it.
                return;
            }
            "status" => self.apply_status(cx, &m),
            _ => {}
        }
        let heartbeat = self.action("heartbeat");
        self.send(cx, &heartbeat);
    }

    fn apply_status(&mut self, cx: &mut Cx, m: &Map<String, Value>) {
        // EasyWorship's status is the truth: a flag it leaves out is off.
        self.logo = m.get("logo") == Some(&Value::Bool(true));
        self.black = m.get("black") == Some(&Value::Bool(true));
        self.clear = m.get("clear") == Some(&Value::Bool(true));
        for key in ECHOED {
            if let Some(v) = m.get(key).filter(|v| v.is_number() || v.is_string()) {
                self.echoed.insert(key.into(), v.clone());
            }
        }
        let mut live = Map::new();
        for (key, path) in [
            ("pres_no", "presentation"),
            ("slide_no", "slide"),
            ("rectype", "record_type"),
            ("pres_rowid", "presentation_id"),
            ("slide_rowid", "slide_id"),
        ] {
            if let Some(n) = m.get(key).and_then(Value::as_i64) {
                live.insert(path.into(), json!(n));
            }
        }
        let mut state = json!({
            "display": {"logo": self.logo, "black": self.black, "clear": self.clear},
            "live": live,
        });
        for (key, path) in [
            ("schedulerev", "schedule_revision"),
            ("liverev", "live_revision"),
        ] {
            match m.get(key) {
                Some(Value::String(s)) => state[path] = json!(s),
                Some(Value::Number(n)) => state[path] = json!(n.to_string()),
                _ => {}
            }
        }
        cx.state(state);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.paired = false;
        self.pair_sent = None;
        self.buffer.clear();
        for key in [PAIR, HEARTBEAT] {
            cx.cancel_timer(key);
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// The message for a command, or why it cannot be sent.
    fn command_message(&self, name: &str, params: &Params) -> Option<Value> {
        let int = |k: &str| params.get(k).and_then(Value::as_i64);
        let flag = |k: &str| params.get(k).and_then(Value::as_bool);
        let simple = match name {
            "next_slide" => "nextSlide",
            "previous_slide" => "prevSlide",
            "next_schedule_item" => "nextSchedule",
            "previous_schedule_item" => "prevSchedule",
            "next_build" => "nextBuild",
            "previous_build" => "prevBuild",
            "presentation_start" => "gotoStartPresentation",
            "slide_start" => "gotoStartSlide",
            "play" => "Play",
            "pause" => "Pause",
            "toggle_play" => "Toggle",
            "go_to_slide" => return Some(self.action(&format!("gotoSlide {}", int("slide")?))),
            "go_to_schedule_item" => {
                return Some(self.action(&format!("gotoSchedule {}", int("item")?)))
            }
            // Logo and black exclude each other; clear is independent.
            "set_logo" => {
                let on = flag("enabled")?;
                return Some(self.status(on, self.black && !on, self.clear));
            }
            "set_black" => {
                let on = flag("enabled")?;
                return Some(self.status(self.logo && !on, on, self.clear));
            }
            "set_clear" => return Some(self.status(self.logo, self.black, flag("enabled")?)),
            "toggle_logo" => {
                let on = !self.logo;
                return Some(self.status(on, self.black && !on, self.clear));
            }
            "toggle_black" => {
                let on = !self.black;
                return Some(self.status(self.logo && !on, on, self.clear));
            }
            "toggle_clear" => return Some(self.status(self.logo, self.black, !self.clear)),
            _ => return None,
        };
        Some(self.action(simple))
    }
}

impl Module for EasyWorship {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.socket_open {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        if !self.paired {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: "not paired: approve this controller on the EasyWorship computer"
                        .into(),
                }),
            );
            return;
        }
        let Some(message) = self.command_message(name, params) else {
            cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            );
            return;
        };
        self.send(cx, &message);
        // A sent flag change is what EasyWorship now shows, until its next
        // status says otherwise.
        if message["action"] == "status" {
            self.logo = message["logo"] == true;
            self.black = message["black"] == true;
            self.clear = message["clear"] == true;
        }
        cx.complete(id, Ok(Outcome::Unverified));
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.socket_open = true;
                cx.connection(Connection::Connecting);
                self.pair(cx);
            }
            TcpInput::Data(data) => {
                cx.alive();
                self.feed(cx, &data);
            }
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => {
                cx.connection(Connection::Connecting);
                cx.tcp_open(SOCKET, self.device);
            }
            PAIR if self.socket_open && !self.paired => self.pair(cx),
            HEARTBEAT if self.socket_open && self.paired => {
                let heartbeat = self.action("heartbeat");
                self.send(cx, &heartbeat);
                cx.set_timer(HEARTBEAT, KEEPALIVE);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    const PORT: u16 = 52_100;

    fn context(monitor: bool, settings: Value) -> OpenContext {
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 50)),
            port: Some(PORT),
            model: "easyworship".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        }
    }

    fn sent(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => {
                    let text = String::from_utf8(data.clone()).unwrap();
                    assert!(text.ends_with("\r\n"), "{text:?}");
                    Some(serde_json::from_str(text.trim_end()).unwrap())
                }
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

    fn feed(m: &mut EasyWorship, now: Millis, line: &str) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(format!("{line}\r\n").into_bytes()),
        );
        cx.take()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn connected(monitor: bool) -> (EasyWorship, Vec<Action>) {
        let ctx = context(monitor, json!({"device_name": "Booth"}));
        let mut m = EasyWorship::new(&ctx, PORT);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        (m, cx.take())
    }

    fn paired(monitor: bool) -> EasyWorship {
        let (mut m, _) = connected(monitor);
        feed(&mut m, 20, r#"{"action":"paired","requestrev":"5"}"#);
        m
    }

    #[test]
    fn asks_to_pair_on_connecting_and_reports_the_round_trip() {
        let (mut m, a) = connected(true);
        assert!(a.contains(&Action::TcpOpen {
            socket: SOCKET,
            to: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 50)), PORT),
        }));
        assert_eq!(
            sent(&a),
            [
                json!({"action": "connect", "uid": "meros-Booth", "device_name": "Booth",
                    "device_type": 8, "requestrev": "0"})
            ]
        );
        let a = feed(&mut m, 35, r#"{"action":"paired","requestrev":"5"}"#);
        assert!(a.contains(&Action::RoundTrip(35)));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(state(&a), json!({"paired": true}));
        // Each message is answered with a heartbeat carrying its revision.
        assert_eq!(
            sent(&a),
            [json!({"action": "heartbeat", "requestrev": "5"})]
        );
    }

    #[test]
    fn not_paired_waits_for_approval_and_asks_again() {
        let (mut m, _) = connected(true);
        let a = feed(&mut m, 10, r#"{"action":"notPaired"}"#);
        assert!(sent(&a).is_empty(), "no heartbeat for notPaired");
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Unauthorized { .. }))));
        let mut cx = Cx::new(15_010);
        m.command(&mut cx, 1, "next_slide", &Params::new());
        assert!(matches!(
            cx.take().as_slice(),
            [Action::Complete {
                result: Err(CommandError::Auth { .. }),
                ..
            }]
        ));
        let mut cx = Cx::new(15_010);
        m.timer(&mut cx, PAIR);
        assert_eq!(sent(&cx.take())[0]["action"], "connect");
    }

    #[test]
    fn status_becomes_state_and_flag_commands_send_it_back() {
        let mut m = paired(true);
        let a = feed(
            &mut m,
            50,
            r#"{"action":"status","logo":false,"black":true,"clear":false,"rectype":1,"pres_rowid":17,"slide_rowid":230,"pres_no":3,"slide_no":2,"schedulerev":"9","liverev":"12","imagehash":"ab12","permissions":7,"requestrev":"6"}"#,
        );
        assert_eq!(
            state(&a),
            json!({"display": {"logo": false, "black": true, "clear": false},
                   "live": {"presentation": 3, "slide": 2, "record_type": 1,
                            "presentation_id": 17, "slide_id": 230},
                   "schedule_revision": "9", "live_revision": "12"})
        );
        assert_eq!(
            sent(&a),
            [json!({"action": "heartbeat", "requestrev": "6"})]
        );

        // Logo on turns black off; the rest of the status goes back as it was.
        let mut cx = Cx::new(60);
        m.command(&mut cx, 2, "set_logo", &params(json!({"enabled": true})));
        let a = cx.take();
        assert_eq!(
            serde_json::to_string(&sent(&a)[0]).unwrap(),
            r#"{"action":"status","logo":true,"black":false,"clear":false,"rectype":1,"pres_rowid":17,"slide_rowid":230,"pres_no":3,"slide_no":2,"schedulerev":"9","liverev":"12","imagehash":"ab12","permissions":7,"requestrev":"6"}"#
        );
        assert!(a.contains(&Action::Complete {
            id: 2,
            result: Ok(Outcome::Unverified)
        }));
        let mut cx = Cx::new(61);
        m.command(&mut cx, 3, "toggle_clear", &Params::new());
        let s = &sent(&cx.take())[0];
        assert_eq!(
            (s["logo"].clone(), s["black"].clone(), s["clear"].clone()),
            (json!(true), json!(false), json!(true))
        );
    }

    #[test]
    fn navigation_commands_carry_the_revision() {
        let mut m = paired(true);
        for (name, input, action) in [
            ("next_slide", json!({}), "nextSlide"),
            ("previous_schedule_item", json!({}), "prevSchedule"),
            ("go_to_slide", json!({"slide": 4}), "gotoSlide 4"),
            ("go_to_schedule_item", json!({"item": 2}), "gotoSchedule 2"),
            ("toggle_play", json!({}), "Toggle"),
            ("presentation_start", json!({}), "gotoStartPresentation"),
        ] {
            let mut cx = Cx::new(100);
            m.command(&mut cx, 9, name, &params(input));
            assert_eq!(
                sent(&cx.take()),
                [json!({"action": action, "requestrev": "5"})],
                "{name}"
            );
        }
    }

    #[test]
    fn commands_only_opening_still_pairs_and_takes_pushed_status() {
        // Opened for commands only, the module sends the same: the pairing
        // request commands need, and nothing else.
        let (mut m, a) = connected(false);
        assert_eq!(sent(&a).len(), 1);
        assert_eq!(sent(&a)[0]["action"], "connect");
        feed(&mut m, 10, r#"{"action":"paired"}"#);
        let a = feed(
            &mut m,
            20,
            r#"{"action":"status","logo":true,"black":false,"clear":false}"#,
        );
        assert_eq!(state(&a)["display"]["logo"], true);
        let mut cx = Cx::new(30);
        m.command(&mut cx, 1, "play", &Params::new());
        assert_eq!(sent(&cx.take())[0]["action"], "Play");
    }

    #[test]
    fn keeps_alive_while_paired_and_reconnects_with_backoff() {
        let mut m = paired(true);
        let mut cx = Cx::new(30_020);
        m.timer(&mut cx, HEARTBEAT);
        let a = cx.take();
        assert_eq!(
            sent(&a),
            [json!({"action": "heartbeat", "requestrev": "5"})]
        );
        assert!(a.contains(&Action::SetTimer {
            key: HEARTBEAT,
            after: KEEPALIVE
        }));
        let mut cx = Cx::new(40_000);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        let a = cx.take();
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        let mut cx = Cx::new(40_001);
        m.command(&mut cx, 4, "next_slide", &Params::new());
        assert!(cx.take().contains(&Action::Complete {
            id: 4,
            result: Err(CommandError::NotConnected)
        }));
    }

    #[test]
    fn a_message_split_across_reads_is_put_back_together() {
        let (mut m, _) = connected(true);
        let mut cx = Cx::new(5);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(br#"{"action":"pai"#.to_vec()),
        );
        assert!(sent(&cx.take()).is_empty());
        let a = feed(&mut m, 6, r#"red","requestrev":"1"}"#);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
    }
}
