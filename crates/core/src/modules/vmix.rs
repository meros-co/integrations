//! vMix over its TCP API on port 8099.
//!
//! From vMix's TCP API documentation: every request and reply line ends in
//! CRLF; a reply is `<command> <status> [response]`, where the status is OK,
//! ER, or a byte count of data that follows the line; only one request may be
//! outstanding; and subscribed events (TALLY, ACTS) can arrive at any time,
//! including between a request and its reply. vMix also sends an unrequested
//! `VERSION OK <version>` line on connection.
//!
//! Opened for commands only (`monitor` false), the module subscribes to
//! nothing and never reads the XML state; `XMLTEXT vmix/version` every 10 s
//! is its liveness check, since vMix sends nothing unasked without a
//! subscription.

use std::collections::{BTreeSet, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use super::vmix_functions::FUNCTIONS;
use crate::catalog::Params;
use crate::engine::template::percent_encode;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

const PORT: u16 = 8099;
const SOCKET: Key = "vmix";

const REPLY_TIMEOUT: Millis = 5_000;
/// The full XML state is read this often; the read doubles as the liveness
/// check, since events only arrive when something changes.
const POLL_EVERY: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

const REPLY: Key = "reply";
const POLL: Key = "poll";
const RETRY: Key = "retry";

#[derive(Debug, Clone, PartialEq)]
enum Request {
    Function {
        id: CommandId,
        line: String,
    },
    Subscribe(&'static str),
    Xml,
    /// Commands only: the version, by XPath, as the liveness check.
    Version,
}

impl Request {
    fn line(&self) -> String {
        match self {
            Request::Function { line, .. } => line.clone(),
            Request::Subscribe(what) => format!("SUBSCRIBE {what}"),
            Request::Xml => "XML".into(),
            Request::Version => "XMLTEXT vmix/version".into(),
        }
    }

    /// The command word the reply starts with.
    fn word(&self) -> &'static str {
        match self {
            Request::Function { .. } => "FUNCTION",
            Request::Subscribe(_) => "SUBSCRIBE",
            Request::Xml => "XML",
            Request::Version => "XMLTEXT",
        }
    }
}

/// One reply or event, framed.
#[derive(Debug, PartialEq)]
struct Message {
    command: String,
    status: Status,
    rest: String,
}

#[derive(Debug, PartialEq)]
enum Status {
    Ok,
    Er,
    Data(Vec<u8>),
}

/// Splits the byte stream into messages: a CRLF line, plus the byte count of
/// data its status announces.
#[derive(Default)]
struct Framer {
    buffer: Vec<u8>,
}

impl Framer {
    fn feed(&mut self, data: &[u8]) -> Vec<Message> {
        self.buffer.extend_from_slice(data);
        let mut out = Vec::new();
        while let Some(end) = self.buffer.windows(2).position(|w| w == b"\r\n") {
            let line = String::from_utf8_lossy(&self.buffer[..end]).into_owned();
            let mut parts = line.splitn(3, ' ');
            let command = parts.next().unwrap_or("").to_string();
            let status = parts.next().unwrap_or("").to_string();
            let rest = parts.next().unwrap_or("").to_string();
            let status = match status.as_str() {
                "OK" => Status::Ok,
                "ER" => Status::Er,
                digits if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
                    let length: usize = digits.parse().unwrap_or(0);
                    let start = end + 2;
                    if self.buffer.len() < start + length {
                        // The data has not all arrived.
                        break;
                    }
                    let data = self.buffer[start..start + length].to_vec();
                    self.buffer.drain(..start + length);
                    out.push(Message {
                        command,
                        status: Status::Data(data),
                        rest,
                    });
                    continue;
                }
                _ => Status::Er,
            };
            self.buffer.drain(..end + 2);
            out.push(Message {
                command,
                status,
                rest,
            });
        }
        out
    }
}

pub(crate) struct Vmix {
    device: SocketAddr,
    framer: Framer,
    connected: bool,
    queue: VecDeque<Request>,
    current: Option<Request>,
    /// When `current` was sent.
    sent_at: Millis,
    /// False: commands only, no subscriptions and no state reads.
    monitor: bool,
    retry_after: Millis,
    /// Input numbers in the last XML state, to report removed inputs.
    inputs: BTreeSet<String>,
    tally_inputs: usize,
}

fn text(params: &Params, name: &str) -> String {
    params
        .get(name)
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}

/// `FUNCTION <name> <query>`, with URL-encoded values as the TCP API asks.
fn function_line(name: &str, query: &[(&str, String)]) -> String {
    let query: Vec<String> = query
        .iter()
        .map(|(k, v)| format!("{k}={}", percent_encode(v)))
        .collect();
    if query.is_empty() {
        format!("FUNCTION {name}")
    } else {
        format!("FUNCTION {name} {}", query.join("&"))
    }
}

/// Parameter names as vMix spells them, in the order its examples give them.
const PARAMETERS: [(&str, &str); 7] = [
    ("input", "Input"),
    ("selected_name", "SelectedName"),
    ("selected_index", "SelectedIndex"),
    ("value", "Value"),
    ("duration", "Duration"),
    ("mix", "Mix"),
    ("channel", "Channel"),
];

/// The FUNCTION line for a command: the function from the generated table
/// (`tools/generate_vmix.py`), or the transition named by `run_transition`,
/// with the parameters given.
fn function_for(name: &str, params: &Params) -> Option<String> {
    let function = if name == "run_transition" {
        text(params, "name")
    } else {
        let at = FUNCTIONS.binary_search_by(|(cmd, _)| cmd.cmp(&name)).ok()?;
        FUNCTIONS[at].1.to_string()
    };
    let query: Vec<(&str, String)> = PARAMETERS
        .iter()
        .filter(|(key, _)| params.contains_key(*key))
        .map(|(key, vmix)| (*vmix, text(params, key)))
        .collect();
    Some(function_line(&function, &query))
}

fn is_true(s: &str) -> bool {
    s.trim().eq_ignore_ascii_case("true")
}

impl Vmix {
    pub(crate) fn new(ctx: OpenContext) -> Vmix {
        let mut m = Vmix::for_device(SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)));
        m.monitor = ctx.monitor;
        m
    }

    fn for_device(device: SocketAddr) -> Vmix {
        Vmix {
            device,
            framer: Framer::default(),
            connected: false,
            queue: VecDeque::new(),
            current: None,
            sent_at: 0,
            monitor: true,
            retry_after: RETRY_MIN,
            inputs: BTreeSet::new(),
            tally_inputs: 0,
        }
    }

    fn enqueue(&mut self, cx: &mut Cx, request: Request) {
        self.queue.push_back(request);
        self.pump(cx);
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.current.is_some() {
            return;
        }
        let Some(request) = self.queue.pop_front() else {
            return;
        };
        cx.tcp_send(SOCKET, format!("{}\r\n", request.line()));
        cx.set_timer(REPLY, REPLY_TIMEOUT);
        self.sent_at = cx.now();
        self.current = Some(request);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        let error = CommandError::Transport {
            message: reason.clone(),
        };
        for request in self.current.take().into_iter().chain(self.queue.drain(..)) {
            if let Request::Function { id, .. } = request {
                cx.complete(id, Err(error.clone()));
            }
        }
        cx.tcp_close(SOCKET);
        cx.cancel_timer(REPLY);
        cx.cancel_timer(POLL);
        self.framer = Framer::default();
        self.connected = false;
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn message(&mut self, cx: &mut Cx, message: Message) {
        if !self.connected {
            self.connected = true;
            self.retry_after = RETRY_MIN;
            cx.connection(Connection::Connected);
        }
        match message.command.as_str() {
            // Unrequested, on connection.
            "VERSION" => {
                cx.state(json!({"device": {"version": message.rest}}));
                return;
            }
            // Only ever events: the core never sends TALLY or ACTS requests.
            "TALLY" | "ACTS" => {
                if message.status == Status::Ok {
                    if message.command == "TALLY" {
                        self.tally(cx, &message.rest);
                    } else {
                        self.activator(cx, &message.rest);
                    }
                }
                return;
            }
            _ => {}
        }
        let answers_current = self
            .current
            .as_ref()
            .is_some_and(|r| r.word() == message.command);
        if !answers_current {
            cx.log(
                Level::Warning,
                format!(
                    "unexpected message from vMix: {} {}",
                    message.command, message.rest
                ),
            );
            return;
        }
        cx.cancel_timer(REPLY);
        cx.round_trip(cx.now().saturating_sub(self.sent_at));
        match self.current.take().unwrap() {
            Request::Function { id, .. } => {
                let result = match message.status {
                    Status::Er => Err(CommandError::DeviceError {
                        code: None,
                        message: message.rest,
                    }),
                    _ => Ok(Outcome::Ack),
                };
                cx.complete(id, result);
            }
            Request::Subscribe(what) => {
                if message.status == Status::Er {
                    cx.log(
                        Level::Warning,
                        format!("vMix refused SUBSCRIBE {what}: {}", message.rest),
                    );
                }
            }
            Request::Xml => match message.status {
                Status::Data(data) => self.xml(cx, &String::from_utf8_lossy(&data)),
                _ => cx.log(
                    Level::Warning,
                    format!("vMix refused XML: {}", message.rest),
                ),
            },
            Request::Version => {
                if message.status == Status::Ok {
                    cx.state(json!({"device": {"version": message.rest}}));
                }
            }
        }
        self.pump(cx);
    }

    /// `TALLY OK 0121...`: one digit per input from input 1; 0 off,
    /// 1 program, 2 preview.
    fn tally(&mut self, cx: &mut Cx, digits: &str) {
        let mut tally = Map::new();
        for (i, d) in digits.trim().bytes().enumerate() {
            tally.insert(
                (i + 1).to_string(),
                json!({"program": d == b'1', "preview": d == b'2'}),
            );
        }
        // Inputs that have gone since the last tally.
        for i in digits.trim().len()..self.tally_inputs {
            tally.insert((i + 1).to_string(), Value::Null);
        }
        self.tally_inputs = digits.trim().len();
        cx.state(json!({"tally": tally}));
    }

    /// `ACTS OK <Name> [<input>] <value>`, value 0 to 1.
    fn activator(&mut self, cx: &mut Cx, body: &str) {
        let parts: Vec<&str> = body.split_whitespace().collect();
        let on = |v: &str| v == "1";
        let patch = match parts.as_slice() {
            ["Input", input, v] if on(v) => json!({"program": input.parse::<u32>().ok()}),
            ["InputPreview", input, v] if on(v) => {
                json!({"preview": input.parse::<u32>().ok()})
            }
            ["InputPlaying", input, v] => json!({"inputs": {*input: {"playing": on(v)}}}),
            ["InputAudio", input, v] => json!({"inputs": {*input: {"muted": !on(v)}}}),
            ["Recording", v] => json!({"recording": on(v)}),
            ["Streaming", v] => json!({"streaming": on(v)}),
            ["External", v] => json!({"external": on(v)}),
            ["MultiCorder", v] => json!({"multicorder": on(v)}),
            ["FadeToBlack", v] => json!({"fade_to_black": on(v)}),
            _ => return,
        };
        cx.state(patch);
    }

    fn xml(&mut self, cx: &mut Cx, document: &str) {
        let doc = match roxmltree::Document::parse(document.trim_start_matches('\u{feff}')) {
            Ok(doc) => doc,
            Err(e) => {
                cx.log(
                    Level::Warning,
                    format!("unreadable XML state from vMix: {e}"),
                );
                return;
            }
        };
        let root = doc.root_element();
        let child = |name: &str| {
            root.children()
                .find(|n| n.has_tag_name(name))
                .and_then(|n| n.text())
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let number = |s: String| s.parse::<u32>().ok();

        let mut patch = json!({
            "device": {"version": child("version"), "edition": child("edition")},
            "program": number(child("active")),
            "preview": number(child("preview")),
            "recording": is_true(&child("recording")),
            "streaming": is_true(&child("streaming")),
            "external": is_true(&child("external")),
            "multicorder": is_true(&child("multiCorder")),
            "fade_to_black": is_true(&child("fadeToBlack")),
        });

        let mut inputs = Map::new();
        let mut seen = BTreeSet::new();
        if let Some(list) = root.children().find(|n| n.has_tag_name("inputs")) {
            for input in list.children().filter(|n| n.has_tag_name("input")) {
                let Some(n) = input.attribute("number") else {
                    continue;
                };
                seen.insert(n.to_string());
                let mut entry = json!({
                    "title": input.attribute("title").unwrap_or(""),
                    "type": input.attribute("type").unwrap_or(""),
                    "key": input.attribute("key").unwrap_or(""),
                    "playing": input.attribute("state") == Some("Running"),
                });
                if let Some(muted) = input.attribute("muted") {
                    entry["muted"] = json!(is_true(muted));
                }
                if let Some(volume) = input
                    .attribute("volume")
                    .and_then(|v| v.parse::<f64>().ok())
                {
                    entry["volume"] = json!(volume);
                }
                inputs.insert(n.to_string(), entry);
            }
        }
        for gone in self.inputs.difference(&seen) {
            inputs.insert(gone.clone(), Value::Null);
        }
        self.inputs = seen;
        patch["inputs"] = Value::Object(inputs);

        if let Some(list) = root.children().find(|n| n.has_tag_name("overlays")) {
            let mut overlays = Map::new();
            for overlay in list.children().filter(|n| n.has_tag_name("overlay")) {
                let Some(n) = overlay.attribute("number") else {
                    continue;
                };
                let input = overlay.text().and_then(|t| t.trim().parse::<u32>().ok());
                overlays.insert(
                    n.to_string(),
                    match input {
                        Some(i) => json!({"input": i}),
                        None => Value::Null,
                    },
                );
            }
            patch["overlays"] = Value::Object(overlays);
        }
        // An unparseable program or preview is null, which removes it.
        cx.state(patch);
    }
}

impl Module for Vmix {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match function_for(name, params) {
            Some(line) => self.enqueue(cx, Request::Function { id, line }),
            None => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            ),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                if self.monitor {
                    self.enqueue(cx, Request::Subscribe("TALLY"));
                    self.enqueue(cx, Request::Subscribe("ACTS"));
                    self.enqueue(cx, Request::Xml);
                }
                // Commands only: the first liveness request goes at the
                // first poll; the unrequested VERSION line shows vMix is there.
                cx.set_timer(POLL, POLL_EVERY);
            }
            TcpInput::Data(data) => {
                cx.alive();
                for message in self.framer.feed(&data) {
                    self.message(cx, message);
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => {
                cx.connection(Connection::Connecting);
                cx.tcp_open(SOCKET, self.device);
            }
            // A missing reply leaves every later reply unpaired: start again.
            REPLY => self.lost(cx, "no reply from vMix within 5 s".into()),
            POLL => {
                let request = if self.monitor {
                    Request::Xml
                } else {
                    Request::Version
                };
                if !self.queue.contains(&request) && self.current.as_ref() != Some(&request) {
                    self.enqueue(cx, request);
                }
                cx.set_timer(POLL, POLL_EVERY);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_send(SOCKET, "QUIT\r\n");
        cx.tcp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn vmix() -> Vmix {
        Vmix::for_device(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 4)),
            PORT,
        ))
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

    fn feed(m: &mut Vmix, now: Millis, data: &str) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(data.as_bytes().to_vec()));
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

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    const XML: &str = r#"<vmix><version>27.0.0.49</version><edition>4K</edition><inputs><input key="26cae087-b7b7-43e6-a7b4-3c3a8a3ee7a5" number="1" type="Capture" title="Camera 1" state="Running" muted="False" volume="100"/><input key="5a5b0b2b-2f7a-4d1e-8d44-6a3e8b3b1f00" number="2" type="GT" title="Lower third" state="Paused">Lower third</input></inputs><overlays><overlay number="1">2</overlay><overlay number="2"/></overlays><preview>2</preview><active>1</active><fadeToBlack>False</fadeToBlack><recording>True</recording><external>False</external><streaming>False</streaming><multiCorder>False</multiCorder></vmix>"#;

    /// Connected, subscribed, and the first XML state read.
    fn connected() -> (Vmix, Vec<Action>) {
        let mut m = vmix();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let mut a = cx.take();
        a.extend(feed(&mut m, 10, "VERSION OK 27.0.0.49\r\n"));
        a.extend(feed(&mut m, 11, "SUBSCRIBE OK TALLY\r\n"));
        a.extend(feed(&mut m, 12, "SUBSCRIBE OK ACTS\r\n"));
        a.extend(feed(
            &mut m,
            13,
            &format!("XML {}\r\n{XML}\r\n", XML.len() + 2),
        ));
        (m, a)
    }

    #[test]
    fn framing_handles_lines_and_counted_data_split_anywhere() {
        let mut f = Framer::default();
        // The count includes the data's own trailing CRLF.
        assert!(f.feed(b"XML 8\r\n<vm").is_empty());
        let out = f.feed(b"ix>\r\nFUNCTION OK Completed\r\nTALLY OK 012");
        assert_eq!(
            out,
            [
                Message {
                    command: "XML".into(),
                    status: Status::Data(b"<vmix>\r\n".to_vec()),
                    rest: "".into()
                },
                Message {
                    command: "FUNCTION".into(),
                    status: Status::Ok,
                    rest: "Completed".into()
                },
            ]
        );
        assert_eq!(
            f.feed(b"\r\n"),
            [Message {
                command: "TALLY".into(),
                status: Status::Ok,
                rest: "012".into()
            }]
        );
    }

    #[test]
    fn requests_go_one_at_a_time() {
        let mut m = vmix();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(sent(&cx.take()), ["SUBSCRIBE TALLY\r\n"]);
        // An event in between is not the reply.
        assert!(sent(&feed(&mut m, 5, "TALLY OK 12\r\n")).is_empty());
        assert_eq!(
            sent(&feed(&mut m, 10, "SUBSCRIBE OK TALLY\r\n")),
            ["SUBSCRIBE ACTS\r\n"]
        );
        assert_eq!(
            sent(&feed(&mut m, 11, "SUBSCRIBE OK ACTS\r\n")),
            ["XML\r\n"]
        );
    }

    #[test]
    fn the_xml_state_maps_onto_inputs_and_outputs() {
        let (_, a) = connected();
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let s = state(&a);
        assert_eq!(
            s["device"],
            json!({"version": "27.0.0.49", "edition": "4K"})
        );
        assert_eq!(s["program"], 1);
        assert_eq!(s["preview"], 2);
        assert_eq!(s["recording"], true);
        assert_eq!(
            s["inputs"]["1"],
            json!({"title": "Camera 1", "type": "Capture", "key": "26cae087-b7b7-43e6-a7b4-3c3a8a3ee7a5",
                   "playing": true, "muted": false, "volume": 100.0})
        );
        assert_eq!(s["inputs"]["2"]["playing"], false);
        assert_eq!(s["inputs"]["2"].get("muted"), None);
        assert_eq!(s["overlays"], json!({"1": {"input": 2}}));
    }

    #[test]
    fn removed_inputs_are_removed_from_the_state() {
        let (mut m, _) = connected();
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, POLL);
        assert_eq!(sent(&cx.take()), ["XML\r\n"]);
        let one = r#"<vmix><version>27</version><inputs><input key="k" number="1" type="Capture" title="Camera 1" state="Running"/></inputs></vmix>"#;
        let a = feed(
            &mut m,
            20_010,
            &format!("XML {}\r\n{one}\r\n", one.len() + 2),
        );
        let patch = a
            .iter()
            .find_map(|x| match x {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(patch["inputs"]["2"], Value::Null);
    }

    #[test]
    fn tally_and_activator_events_update_the_state() {
        let (mut m, _) = connected();
        let mut a = feed(
            &mut m,
            100,
            "TALLY OK 0210\r\nACTS OK Input 2 1\r\nACTS OK InputAudio 1 0\r\n",
        );
        a.extend(feed(
            &mut m,
            101,
            "ACTS OK Streaming 1\r\nACTS OK InputPlaying 2 1\r\n",
        ));
        let s = state(&a);
        assert_eq!(s["tally"]["1"], json!({"program": false, "preview": false}));
        assert_eq!(s["tally"]["2"], json!({"program": false, "preview": true}));
        assert_eq!(s["tally"]["3"], json!({"program": true, "preview": false}));
        assert_eq!(s["program"], 2);
        assert_eq!(s["inputs"]["1"]["muted"], true);
        assert_eq!(s["inputs"]["2"]["playing"], true);
        assert_eq!(s["streaming"], true);

        // One input fewer: its tally goes.
        let a = feed(&mut m, 102, "TALLY OK 01\r\n");
        let patch = a
            .iter()
            .find_map(|x| match x {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(patch["tally"]["3"], Value::Null);
    }

    #[test]
    fn functions_are_url_encoded_and_their_replies_complete_commands() {
        let (mut m, _) = connected();
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            4,
            "set_text",
            &params(json!({"input": "Lower third", "selected_name": "Headline.Text", "value": "Hello & welcome"})),
        );
        assert_eq!(
            sent(&cx.take()),
            ["FUNCTION SetText Input=Lower%20third&SelectedName=Headline.Text&Value=Hello%20%26%20welcome\r\n"]
        );
        let a = feed(&mut m, 110, "FUNCTION OK Completed\r\n");
        assert!(a.contains(&Action::Complete {
            id: 4,
            result: Ok(Outcome::Ack)
        }));

        let mut cx = Cx::new(120);
        m.command(&mut cx, 5, "cut_direct", &params(json!({"input": "99"})));
        cx.take();
        let a = feed(&mut m, 130, "FUNCTION ER Input not found\r\n");
        assert!(a.contains(&Action::Complete {
            id: 5,
            result: Err(CommandError::DeviceError {
                code: None,
                message: "Input not found".into()
            })
        }));
    }

    #[test]
    fn function_lines() {
        let line = |name: &str, p: Value| function_for(name, &params(p)).unwrap();
        assert_eq!(line("cut", json!({})), "FUNCTION Cut");
        assert_eq!(line("cut", json!({"input": "3"})), "FUNCTION Cut Input=3");
        assert_eq!(
            line("fade", json!({"duration": 1000})),
            "FUNCTION Fade Duration=1000"
        );
        assert_eq!(
            line("overlay_input1_in", json!({"input": "5"})),
            "FUNCTION OverlayInput1In Input=5"
        );
        // Names that do not convert back from snake_case come from the table.
        assert_eq!(
            line("ptz_move_up", json!({"input": "Cam 1", "value": "0.5"})),
            "FUNCTION PTZMoveUp Input=Cam%201&Value=0.5"
        );
        assert_eq!(
            line("run_transition", json!({"name": "Merge", "duration": 500})),
            "FUNCTION Merge Duration=500"
        );
        assert!(FUNCTIONS.windows(2).all(|w| w[0].0 < w[1].0), "sorted");
        assert_eq!(function_for("no_such_function", &params(json!({}))), None);
    }

    #[test]
    fn a_missing_reply_drops_the_connection_and_fails_the_command() {
        let (mut m, _) = connected();
        let mut cx = Cx::new(100);
        m.command(&mut cx, 6, "cut", &params(json!({})));
        m.command(&mut cx, 7, "fade_to_black", &params(json!({})));
        cx.take();
        let mut cx = Cx::new(5_200);
        m.timer(&mut cx, REPLY);
        let a = cx.take();
        for id in [6, 7] {
            assert!(a.iter().any(|x| matches!(
                x,
                Action::Complete { id: i, result: Err(CommandError::Transport { .. }) } if *i == id
            )));
        }
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
    }

    #[test]
    fn opened_for_commands_only_it_subscribes_to_nothing_and_reads_nothing() {
        let mut m = vmix();
        m.monitor = false;
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert!(sent(&cx.take()).is_empty());
        let a = feed(&mut m, 10, "VERSION OK 27.0.0.49\r\n");
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(sent(&a).is_empty());

        // The poll asks only for the version.
        let mut cx = Cx::new(POLL_EVERY);
        m.timer(&mut cx, POLL);
        assert_eq!(sent(&cx.take()), ["XMLTEXT vmix/version\r\n"]);
        let a = feed(&mut m, POLL_EVERY + 5, "XMLTEXT OK 27.0.0.49\r\n");
        assert_eq!(state(&a)["device"]["version"], "27.0.0.49");

        // Commands work as before.
        let mut cx = Cx::new(POLL_EVERY + 10);
        m.command(&mut cx, 3, "cut", &params(json!({})));
        assert_eq!(sent(&cx.take()), ["FUNCTION Cut\r\n"]);
        let a = feed(&mut m, POLL_EVERY + 20, "FUNCTION OK Completed\r\n");
        assert!(a.contains(&Action::Complete {
            id: 3,
            result: Ok(Outcome::Ack)
        }));
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let (mut m, _) = connected();
        let mut cx = Cx::new(100);
        m.command(&mut cx, 4, "cut", &params(json!({})));
        cx.take();
        // An event before the reply is not the reply.
        let a = feed(&mut m, 120, "TALLY OK 12\r\n");
        assert!(!a.iter().any(|x| matches!(x, Action::RoundTrip(_))));
        let a = feed(&mut m, 137, "FUNCTION OK Completed\r\n");
        assert!(a.contains(&Action::RoundTrip(37)));
    }
}
