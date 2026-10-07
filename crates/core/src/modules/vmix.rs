//! vMix over its TCP API on port 8099.
//!
//! From vMix's TCP API documentation: every request and reply line ends in
//! CRLF; a reply is `<command> <status> [response]`, where the status is OK,
//! ER, or a byte count of data that follows the line; only one request may be
//! outstanding; and subscribed events (TALLY, ACTS) can arrive at any time,
//! including between a request and its reply. vMix also sends an unrequested
//! `VERSION OK <version>` line on connection.
//!
//! The state comes from two places. Tally and activator events (SUBSCRIBE
//! TALLY and ACTS) carry the fast changes as they happen; the whole XML state
//! (XML), read on connecting and every `state_poll_ms`, carries everything
//! else. The module keeps what it last reported and sends only what an XML
//! read changes, with removals for what the read no longer has.
//!
//! Opened for commands only (`monitor` false), the module subscribes to
//! nothing and never reads the XML state; `XMLTEXT vmix/version` every 10 s
//! is its liveness check, since vMix sends nothing unasked without a
//! subscription.

use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;

use roxmltree::Node;
use serde_json::{json, Map, Value};

use super::vmix_functions::FUNCTIONS;
use crate::catalog::Params;
use crate::engine::template::percent_encode;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};
use crate::session::merge_patch;

const PORT: u16 = 8099;
const SOCKET: Key = "vmix";

const REPLY_TIMEOUT: Millis = 5_000;
/// The XML state is read this often by default (`state_poll_ms`); the read
/// doubles as the liveness check, since events only arrive when something
/// changes.
const STATE_POLL_DEFAULT: Millis = 1_000;
const STATE_POLL_MIN: Millis = 250;
const STATE_POLL_MAX: Millis = 60_000;
/// Commands only: how often the version is asked for, as the liveness check.
const LIVENESS_EVERY: Millis = 10_000;
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
    /// One activator's current value, for what only activators report.
    Acts {
        name: String,
        input: String,
    },
    /// Commands only: the version, by XPath, as the liveness check.
    Version,
}

impl Request {
    fn line(&self) -> String {
        match self {
            Request::Function { line, .. } => line.clone(),
            Request::Subscribe(what) => format!("SUBSCRIBE {what}"),
            Request::Xml => "XML".into(),
            Request::Acts { name, input } => format!("ACTS {name} {input}"),
            Request::Version => "XMLTEXT vmix/version".into(),
        }
    }

    /// The command word the reply starts with.
    fn word(&self) -> &'static str {
        match self {
            Request::Function { .. } => "FUNCTION",
            Request::Subscribe(_) => "SUBSCRIBE",
            Request::Xml => "XML",
            Request::Acts { .. } => "ACTS",
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
    /// How often the XML state is read.
    state_poll: Millis,
    retry_after: Millis,
    /// The state as last reported, from XML reads and events alike: an XML
    /// read reports only what differs from it.
    view: Value,
    /// What the last XML read reported, to find what a read no longer has.
    xml_last: Value,
    /// Input key by input number, from the last XML read: a number that now
    /// holds another input starts again from nothing.
    input_keys: BTreeMap<String, String>,
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

// --- XML values ------------------------------------------------------------

/// vMix writes booleans as True and False.
fn boolean(s: &str) -> Option<Value> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => None,
    }
}

fn float(s: &str) -> Option<Value> {
    let f: f64 = s.trim().parse().ok()?;
    f.is_finite().then(|| json!(f))
}

/// A whole number; vMix writes some (positions) with a fraction.
fn int(s: &str) -> Option<Value> {
    let f: f64 = s.trim().parse().ok()?;
    f.is_finite().then(|| json!(f.round() as i64))
}

fn string(s: &str) -> Option<Value> {
    Some(Value::String(s.to_string()))
}

fn put(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(v) = value {
        map.insert(key.to_string(), v);
    }
}

fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|n| n.has_tag_name(name))
}

/// An element's own text, without its children's.
fn own_text(node: Node) -> String {
    node.children()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect::<String>()
        .trim()
        .to_string()
}

/// The objects under `map[key]`, created if missing.
fn object<'m>(map: &'m mut Map<String, Value>, key: &str) -> &'m mut Map<String, Value> {
    let entry = map
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    entry.as_object_mut().unwrap()
}

/// Booleans of the status elements: element, state key.
const FLAGS: [(&str, &str); 7] = [
    ("fadeToBlack", "fade_to_black"),
    ("recording", "recording"),
    ("streaming", "streaming"),
    ("external", "external"),
    ("multiCorder", "multicorder"),
    ("playList", "playlist"),
    ("fullscreen", "fullscreen"),
];

/// Top-level elements read into their own state; any other element with
/// text only goes under `other`.
const KNOWN: [&str; 19] = [
    "version",
    "edition",
    "preset",
    "inputs",
    "overlays",
    "preview",
    "active",
    "fadeToBlack",
    "transitions",
    "recording",
    "external",
    "streaming",
    "playList",
    "multiCorder",
    "fullscreen",
    "mix",
    "audio",
    "dynamic",
    "outputs",
];

/// The whole XML state as the state tree it maps to, with each input
/// number's key.
fn read_xml(root: Node) -> (Map<String, Value>, BTreeMap<String, String>) {
    let mut tree = Map::new();
    let text_of = |name: &str| child(root, name).map(own_text);

    let mut device = Map::new();
    put(
        &mut device,
        "version",
        text_of("version").and_then(|s| string(&s)),
    );
    put(
        &mut device,
        "edition",
        text_of("edition").and_then(|s| string(&s)),
    );
    put(
        &mut device,
        "preset",
        text_of("preset").and_then(|s| string(&s)),
    );
    tree.insert("device".into(), Value::Object(device));
    put(
        &mut tree,
        "program",
        text_of("active").and_then(|s| int(&s)),
    );
    put(
        &mut tree,
        "preview",
        text_of("preview").and_then(|s| int(&s)),
    );

    read_status(root, &mut tree);
    read_transitions(root, &mut tree);
    read_mixes(root, &mut tree);
    read_overlays(root, &mut tree);
    read_outputs(root, &mut tree);
    read_audio(root, &mut tree);
    let keys = read_inputs(root, &mut tree);

    // Anything else vMix reports as plain text, by element name.
    let mut other = Map::new();
    for node in root.children().filter(Node::is_element) {
        let name = node.tag_name().name();
        if KNOWN.contains(&name) || node.children().any(|c| c.is_element()) {
            continue;
        }
        other.insert(name.to_string(), Value::String(own_text(node)));
    }
    if !other.is_empty() {
        tree.insert("other".into(), Value::Object(other));
    }
    (tree, keys)
}

/// Recording, streaming and the other outputs' flags, with the recording's
/// duration and files and each stream's own flag.
fn read_status(root: Node, tree: &mut Map<String, Value>) {
    for (element, key) in FLAGS {
        if let Some(node) = child(root, element) {
            put(tree, key, boolean(&own_text(node)));
        }
    }
    if let Some(node) = child(root, "recording") {
        put(
            tree,
            "recording_duration",
            node.attribute("duration").and_then(int),
        );
        for n in ["1", "2"] {
            if let Some(file) = node
                .attribute(format!("filename{n}").as_str())
                .filter(|f| !f.is_empty())
            {
                object(tree, "recording_files").insert(n.into(), json!(file));
            }
        }
    }
    if let Some(node) = child(root, "streaming") {
        for attribute in node.attributes() {
            if let Some(n) = attribute.name().strip_prefix("channel") {
                if n.parse::<u32>().is_ok() {
                    put(object(tree, "streams"), n, boolean(attribute.value()));
                }
            }
        }
    }
}

/// `<transition number="1" effect="Fade" duration="500"/>`
fn read_transitions(root: Node, tree: &mut Map<String, Value>) {
    let Some(list) = child(root, "transitions") else {
        return;
    };
    let transitions = object(tree, "transitions");
    for node in list.children().filter(|n| n.has_tag_name("transition")) {
        let Some(n) = node.attribute("number") else {
            continue;
        };
        let mut entry = Map::new();
        put(
            &mut entry,
            "effect",
            node.attribute("effect").and_then(string),
        );
        put(
            &mut entry,
            "duration",
            node.attribute("duration").and_then(int),
        );
        transitions.insert(n.into(), Value::Object(entry));
    }
}

/// `<mix number="2"><preview>3</preview><active>4</active></mix>`: the mixes
/// beside the main one, numbered from 2 as the XML numbers them.
fn read_mixes(root: Node, tree: &mut Map<String, Value>) {
    for node in root.children().filter(|n| n.has_tag_name("mix")) {
        let Some(n) = node.attribute("number") else {
            continue;
        };
        let mut entry = Map::new();
        put(
            &mut entry,
            "program",
            child(node, "active").and_then(|a| int(&own_text(a))),
        );
        put(
            &mut entry,
            "preview",
            child(node, "preview").and_then(|p| int(&own_text(p))),
        );
        object(tree, "mixes").insert(n.into(), Value::Object(entry));
    }
}

/// `<overlay number="1" preview="True">2</overlay>`: an overlay channel (or
/// stinger) and the input in it; absent when empty.
fn read_overlays(root: Node, tree: &mut Map<String, Value>) {
    let Some(list) = child(root, "overlays") else {
        return;
    };
    let overlays = object(tree, "overlays");
    for node in list.children().filter(|n| n.has_tag_name("overlay")) {
        let (Some(n), Some(input)) = (node.attribute("number"), int(&own_text(node))) else {
            continue;
        };
        let preview = node.attribute("preview").and_then(boolean);
        overlays.insert(
            n.into(),
            json!({"input": input, "preview": preview.unwrap_or(Value::Bool(false))}),
        );
    }
}

/// `<output type="Output" number="2" source="Input" inputNumber="3" mix="0"
/// ndi="True" omt="False" srt="False"/>`, keyed by type and number
/// (`output2`, `fullscreen1`).
fn read_outputs(root: Node, tree: &mut Map<String, Value>) {
    let Some(list) = child(root, "outputs") else {
        return;
    };
    let outputs = object(tree, "outputs");
    for node in list.children().filter(|n| n.has_tag_name("output")) {
        let (Some(kind), Some(n)) = (node.attribute("type"), node.attribute("number")) else {
            continue;
        };
        let mut entry = Map::new();
        entry.insert("type".into(), json!(kind));
        put(&mut entry, "number", int(n));
        put(
            &mut entry,
            "source",
            node.attribute("source").and_then(string),
        );
        put(
            &mut entry,
            "input",
            node.attribute("inputNumber").and_then(int),
        );
        put(&mut entry, "mix", node.attribute("mix").and_then(int));
        for flag in ["ndi", "omt", "srt"] {
            put(&mut entry, flag, node.attribute(flag).and_then(boolean));
        }
        outputs.insert(
            format!("{}{n}", kind.to_ascii_lowercase()),
            Value::Object(entry),
        );
    }
}

/// The audio buses beside the master.
const BUSES: [&str; 7] = ["A", "B", "C", "D", "E", "F", "G"];

/// `<audio><master volume="100" muted="False" meterF1="0.1" meterF2="0.1"
/// headphonesVolume="74"/><busA volume="100" muted="False" meterF1="0"
/// meterF2="0" solo="False" sendToMaster="False"/></audio>`
fn read_audio(root: Node, tree: &mut Map<String, Value>) {
    let Some(audio) = child(root, "audio") else {
        return;
    };
    for node in audio.children().filter(Node::is_element) {
        let name = node.tag_name().name();
        let target = if name == "master" {
            object(tree, "master")
        } else if let Some(bus) = name.strip_prefix("bus").filter(|b| BUSES.contains(b)) {
            object(object(tree, "buses"), bus)
        } else {
            continue;
        };
        for attribute in node.attributes() {
            let value = attribute.value();
            match attribute.name() {
                "volume" => put(target, "volume", float(value)),
                "muted" => put(target, "muted", boolean(value)),
                "solo" => put(target, "solo", boolean(value)),
                "sendToMaster" => put(target, "send_to_master", boolean(value)),
                "headphonesVolume" => put(target, "headphones_volume", float(value)),
                "meterF1" => put(target, "meter_left", float(value)),
                "meterF2" => put(target, "meter_right", float(value)),
                other => put(object(target, "attributes"), other, string(value)),
            }
        }
    }
}

/// Every input, keyed by number; returns each number's key.
fn read_inputs(root: Node, tree: &mut Map<String, Value>) -> BTreeMap<String, String> {
    let mut keys = BTreeMap::new();
    let mut inputs = Map::new();
    if let Some(list) = child(root, "inputs") {
        for node in list.children().filter(|n| n.has_tag_name("input")) {
            let Some(n) = node.attribute("number") else {
                continue;
            };
            keys.insert(
                n.to_string(),
                node.attribute("key").unwrap_or("").to_string(),
            );
            inputs.insert(n.to_string(), Value::Object(read_input(node)));
        }
    }
    tree.insert("inputs".into(), Value::Object(inputs));
    keys
}

fn read_input(node: Node) -> Map<String, Value> {
    let mut entry = Map::new();
    let attr = |name: &str| node.attribute(name);
    entry.insert("title".into(), json!(attr("title").unwrap_or("")));
    entry.insert("type".into(), json!(attr("type").unwrap_or("")));
    entry.insert("key".into(), json!(attr("key").unwrap_or("")));
    entry.insert("playing".into(), json!(attr("state") == Some("Running")));
    put(&mut entry, "muted", attr("muted").and_then(boolean));
    put(&mut entry, "volume", attr("volume").and_then(float));
    put(&mut entry, "balance", attr("balance").and_then(float));
    put(&mut entry, "solo", attr("solo").and_then(boolean));
    put(&mut entry, "gain_db", attr("gainDb").and_then(float));
    put(&mut entry, "meter_left", attr("meterF1").and_then(float));
    put(&mut entry, "meter_right", attr("meterF2").and_then(float));
    // `audiobusses="M,A,C"`: every bus, on or off.
    if let Some(buses) = attr("audiobusses") {
        let on: Vec<&str> = buses.split(',').map(str::trim).collect();
        let routing = object(&mut entry, "audio_buses");
        for bus in std::iter::once("M").chain(BUSES) {
            routing.insert(bus.into(), json!(on.contains(&bus)));
        }
    }
    entry
}

/// What `new` changes in `view`, and removals of what `old` had and `new`
/// has not, as a merge patch; None when nothing changes.
fn diff(old: Option<&Value>, view: Option<&Value>, new: &Value) -> Option<Value> {
    match new {
        Value::Object(fields) => {
            let mut out = Map::new();
            for (k, v) in fields {
                if let Some(d) = diff(old.and_then(|o| o.get(k)), view.and_then(|w| w.get(k)), v) {
                    out.insert(k.clone(), d);
                }
            }
            if let Some(Value::Object(before)) = old {
                for k in before.keys() {
                    if !fields.contains_key(k) {
                        out.insert(k.clone(), Value::Null);
                    }
                }
            }
            (!out.is_empty()).then_some(Value::Object(out))
        }
        leaf => (view != Some(leaf)).then(|| leaf.clone()),
    }
}

impl Vmix {
    pub(crate) fn new(ctx: OpenContext) -> Vmix {
        let mut m = Vmix::for_device(SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)));
        m.monitor = ctx.monitor;
        if let Some(ms) = ctx.settings.get("state_poll_ms").and_then(Value::as_u64) {
            m.state_poll = (ms as Millis).clamp(STATE_POLL_MIN, STATE_POLL_MAX);
        }
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
            state_poll: STATE_POLL_DEFAULT,
            retry_after: RETRY_MIN,
            view: json!({}),
            xml_last: json!({}),
            input_keys: BTreeMap::new(),
            tally_inputs: 0,
        }
    }

    fn enqueue(&mut self, cx: &mut Cx, request: Request) {
        self.queue.push_back(request);
        self.pump(cx);
    }

    /// Commands go ahead of queued reads, so an operator never waits for them.
    fn enqueue_command(&mut self, cx: &mut Cx, request: Request) {
        let at = self
            .queue
            .iter()
            .position(|r| !matches!(r, Request::Function { .. }))
            .unwrap_or(self.queue.len());
        self.queue.insert(at, request);
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

    /// Reports a patch and keeps it in the view.
    fn report(&mut self, cx: &mut Cx, patch: Value) {
        merge_patch(&mut self.view, &patch);
        cx.state(patch);
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
                self.report(cx, json!({"device": {"version": message.rest}}));
                return;
            }
            // Only ever an event: the core never sends TALLY requests.
            "TALLY" => {
                if message.status == Status::Ok {
                    self.tally(cx, &message.rest);
                }
                return;
            }
            // An event, or the answer to an ACTS query, which has the same
            // shape: either way it is applied.
            "ACTS" => {
                if message.status == Status::Ok {
                    self.activator(cx, &message.rest);
                }
                let answers = match &self.current {
                    Some(Request::Acts { name, input }) => {
                        let mut parts = message.rest.split_whitespace();
                        message.status != Status::Ok
                            || (parts.next() == Some(name.as_str())
                                && parts.next() == Some(input.as_str()))
                    }
                    _ => false,
                };
                if !answers {
                    return;
                }
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
            // Refused for an input that has no such activator: nothing to keep.
            Request::Acts { .. } => {}
            Request::Version => {
                if message.status == Status::Ok {
                    self.report(cx, json!({"device": {"version": message.rest}}));
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
        self.report(cx, json!({"tally": tally}));
    }

    /// `ACTS OK <Name> [<input>] <value>`, value 0 to 1.
    fn activator(&mut self, cx: &mut Cx, body: &str) {
        let parts: Vec<&str> = body.split_whitespace().collect();
        let on = |v: &str| v == "1";
        // Volumes come as 0 to 1; the XML and the functions use 0 to 100.
        let percent = |v: &str| {
            v.parse::<f64>()
                .ok()
                .filter(|f| f.is_finite())
                .map(|f| (f * 10_000.0).round() / 100.0)
        };
        let bus = |name: &str, suffix: &str| {
            name.strip_prefix("Bus")
                .and_then(|n| n.strip_suffix(suffix))
                .filter(|b| BUSES.contains(b))
                .map(str::to_string)
        };
        let input_bus = |name: &str| {
            if name == "InputMasterAudio" {
                return Some("M".to_string());
            }
            name.strip_prefix("InputBus")
                .and_then(|n| n.strip_suffix("Audio"))
                .filter(|b| BUSES.contains(b))
                .map(str::to_string)
        };
        let channel = |name: &str| {
            name.strip_prefix("InputVolumeChannelMixer")
                .filter(|n| n.parse::<u32>().is_ok_and(|n| (1..=16).contains(&n)))
                .map(str::to_string)
        };
        let mix = |name: &str, prefix: &str| {
            name.strip_prefix(prefix)
                .filter(|n| n.parse::<u32>().is_ok_and(|n| (2..=16).contains(&n)))
                .map(str::to_string)
        };
        let patch = match parts.as_slice() {
            ["Input", input, v] if on(v) => json!({"program": input.parse::<u32>().ok()}),
            ["InputPreview", input, v] if on(v) => {
                json!({"preview": input.parse::<u32>().ok()})
            }
            [name, input, v] if on(v) && mix(name, "InputMix").is_some() => {
                json!({"mixes": {mix(name, "InputMix").unwrap(): {"program": input.parse::<u32>().ok()}}})
            }
            [name, input, v] if on(v) && mix(name, "InputPreviewMix").is_some() => {
                json!({"mixes": {mix(name, "InputPreviewMix").unwrap(): {"preview": input.parse::<u32>().ok()}}})
            }
            ["InputPlaying", input, v] => json!({"inputs": {*input: {"playing": on(v)}}}),
            ["InputAudio", input, v] => json!({"inputs": {*input: {"muted": !on(v)}}}),
            ["InputSolo", input, v] => json!({"inputs": {*input: {"solo": on(v)}}}),
            ["InputAudioAuto", input, v] => json!({"inputs": {*input: {"audio_auto": on(v)}}}),
            ["InputVolume", input, v] => json!({"inputs": {*input: {"volume": percent(v)}}}),
            [name, input, v] if input_bus(name).is_some() => {
                json!({"inputs": {*input: {"audio_buses": {input_bus(name).unwrap(): on(v)}}}})
            }
            [name, input, v] if channel(name).is_some() => {
                json!({"inputs": {*input: {"channel_mixer": {channel(name).unwrap(): percent(v)}}}})
            }
            ["MasterVolume", v] => json!({"master": {"volume": percent(v)}}),
            ["MasterHeadphones", v] => json!({"master": {"headphones_volume": percent(v)}}),
            ["MasterAudio", v] => json!({"master": {"muted": !on(v)}}),
            [name, v] if bus(name, "Volume").is_some() => {
                json!({"buses": {bus(name, "Volume").unwrap(): {"volume": percent(v)}}})
            }
            [name, v] if bus(name, "Audio").is_some() => {
                json!({"buses": {bus(name, "Audio").unwrap(): {"muted": !on(v)}}})
            }
            [name, v] if bus(name, "Solo").is_some() => {
                json!({"buses": {bus(name, "Solo").unwrap(): {"solo": on(v)}}})
            }
            ["Recording", v] => json!({"recording": on(v)}),
            ["Streaming", v] => json!({"streaming": on(v)}),
            ["External", v] => json!({"external": on(v)}),
            ["MultiCorder", v] => json!({"multicorder": on(v)}),
            ["Fullscreen", v] => json!({"fullscreen": on(v)}),
            ["FadeToBlack", v] => json!({"fade_to_black": on(v)}),
            _ => return,
        };
        self.report(cx, patch);
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
        let (tree, keys) = read_xml(doc.root_element());

        // A number that now holds another input (inputs were added, removed
        // or moved) loses everything it had, including what only events set.
        let fresh: Vec<String> = keys
            .iter()
            .filter(|(n, key)| self.input_keys.get(*n) != Some(*key))
            .map(|(n, _)| n.clone())
            .collect();
        let moved: Map<String, Value> = keys
            .iter()
            .filter(|(n, key)| self.input_keys.get(*n).is_some_and(|k| k != *key))
            .map(|(n, _)| (n.clone(), Value::Null))
            .collect();
        if !moved.is_empty() {
            for n in moved.keys() {
                if let Some(inputs) = self
                    .xml_last
                    .get_mut("inputs")
                    .and_then(Value::as_object_mut)
                {
                    inputs.remove(n);
                }
            }
            self.report(cx, json!({"inputs": moved}));
        }
        self.input_keys = keys;

        let tree = Value::Object(tree);
        if let Some(patch) = diff(Some(&self.xml_last), Some(&self.view), &tree) {
            self.report(cx, patch);
        }
        // Automixing and the channel mixer are reported only by activators,
        // which say nothing until they change: ask once for each new input
        // with audio.
        for n in fresh {
            if tree["inputs"][&n].get("muted").is_none() {
                continue;
            }
            let names = std::iter::once("InputAudioAuto".to_string())
                .chain((1..=16).map(|c| format!("InputVolumeChannelMixer{c}")));
            for name in names {
                self.enqueue(
                    cx,
                    Request::Acts {
                        name,
                        input: n.clone(),
                    },
                );
            }
        }
        self.xml_last = tree;
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
            Some(line) => self.enqueue_command(cx, Request::Function { id, line }),
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
                    cx.set_timer(POLL, self.state_poll);
                } else {
                    // Commands only: the first liveness request goes at the
                    // first poll; the unrequested VERSION line shows vMix is
                    // there.
                    cx.set_timer(POLL, LIVENESS_EVERY);
                }
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
                let (request, every) = if self.monitor {
                    (Request::Xml, self.state_poll)
                } else {
                    (Request::Version, LIVENESS_EVERY)
                };
                if !self.queue.contains(&request) && self.current.as_ref() != Some(&request) {
                    self.enqueue(cx, request);
                }
                cx.set_timer(POLL, every);
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

    fn patches(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    fn xml_reply(xml: &str) -> String {
        format!("XML {}\r\n{xml}\r\n", xml.len() + 2)
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
        a.extend(settle(&mut m, 14));
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
        assert_eq!(s["overlays"], json!({"1": {"input": 2, "preview": false}}));
    }

    #[test]
    fn removed_inputs_are_removed_from_the_state() {
        let (mut m, a) = connected();
        let mut s = state(&a);
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, POLL);
        assert_eq!(sent(&cx.take()), ["XML\r\n"]);
        let one = XML.replace(
            r#"<input key="5a5b0b2b-2f7a-4d1e-8d44-6a3e8b3b1f00" number="2" type="GT" title="Lower third" state="Paused">Lower third</input>"#,
            "",
        );
        let a = feed(&mut m, 20_010, &xml_reply(&one));
        // Only what changed: input 2 has gone.
        assert_eq!(patches(&a), [json!({"inputs": {"2": null}})]);
        for p in patches(&a) {
            merge_patch(&mut s, &p);
        }
        assert_eq!(s["inputs"].as_object().unwrap().len(), 1);
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
        let mut cx = Cx::new(LIVENESS_EVERY);
        m.timer(&mut cx, POLL);
        let a = cx.take();
        assert_eq!(sent(&a), ["XMLTEXT vmix/version\r\n"]);
        assert!(a.contains(&Action::SetTimer {
            key: POLL,
            after: LIVENESS_EVERY
        }));
        let a = feed(&mut m, LIVENESS_EVERY + 5, "XMLTEXT OK 27.0.0.49\r\n");
        assert_eq!(state(&a)["device"]["version"], "27.0.0.49");

        // Commands work as before.
        let mut cx = Cx::new(LIVENESS_EVERY + 10);
        m.command(&mut cx, 3, "cut", &params(json!({})));
        assert_eq!(sent(&cx.take()), ["FUNCTION Cut\r\n"]);
        let a = feed(&mut m, LIVENESS_EVERY + 20, "FUNCTION OK Completed\r\n");
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

    /// The status, transitions, mixes and outputs, in the shape vMix 27
    /// reports them.
    const STATUS_XML: &str = r#"<vmix><version>27.0.0.49</version><edition>4K</edition><preset>C:\Shows\Sunday.vmix</preset><inputs><input key="a" number="1" type="Capture" title="Camera 1" state="Running"/><input key="b" number="2" type="Capture" title="Camera 2" state="Running"/><input key="c" number="3" type="Video" title="Clip" state="Paused"/></inputs><overlays><overlay number="1"/><overlay number="2" preview="True">3</overlay><overlay number="3"/><overlay number="4"/><overlay number="5"/><overlay number="6"/><overlay number="7"/><overlay number="8"/></overlays><preview>2</preview><active>1</active><fadeToBlack>False</fadeToBlack><transitions><transition number="1" effect="Fade" duration="500"/><transition number="2" effect="Merge" duration="1000"/><transition number="3" effect="Wipe" duration="500"/><transition number="4" effect="CubeZoom" duration="3000"/></transitions><recording duration="125" filename1="D:\Rec\capture.mp4">True</recording><external>False</external><streaming channel1="True" channel2="False" channel3="False">True</streaming><playList>False</playList><multiCorder>False</multiCorder><fullscreen>True</fullscreen><mix number="2"><preview>1</preview><active>3</active></mix><mix number="3"><preview>2</preview><active>2</active></mix><outputs><output type="Output" number="1" source="Output" ndi="True" omt="False" srt="False"/><output type="Output" number="2" source="Input" inputNumber="3" ndi="False" omt="False" srt="True"/><output type="Output" number="3" source="Mix" mix="1" ndi="False" omt="False" srt="False"/><output type="Fullscreen" number="1" source="Output"/></outputs><audioOutput>Speakers</audioOutput></vmix>"#;

    fn connected_to(xml: &str) -> (Vmix, Value) {
        let mut m = vmix();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let mut a = cx.take();
        a.extend(feed(&mut m, 11, "SUBSCRIBE OK TALLY\r\n"));
        a.extend(feed(&mut m, 12, "SUBSCRIBE OK ACTS\r\n"));
        a.extend(feed(&mut m, 13, &xml_reply(xml)));
        a.extend(settle(&mut m, 14));
        (m, state(&a))
    }

    /// Polls and answers with `xml`, returning the patches.
    fn read(m: &mut Vmix, now: Millis, xml: &str) -> Vec<Value> {
        let mut cx = Cx::new(now);
        m.timer(&mut cx, POLL);
        assert_eq!(sent(&cx.take()), ["XML\r\n"]);
        patches(&feed(m, now + 10, &xml_reply(xml)))
    }

    #[test]
    fn status_transitions_mixes_and_outputs_are_read() {
        let (_, s) = connected_to(STATUS_XML);
        assert_eq!(s["device"]["preset"], r"C:\Shows\Sunday.vmix");
        assert_eq!(s["recording"], true);
        assert_eq!(s["recording_duration"], 125);
        assert_eq!(s["recording_files"], json!({"1": r"D:\Rec\capture.mp4"}));
        assert_eq!(s["streaming"], true);
        assert_eq!(s["streams"], json!({"1": true, "2": false, "3": false}));
        assert_eq!(s["playlist"], false);
        assert_eq!(s["fullscreen"], true);
        assert_eq!(
            s["transitions"]["2"],
            json!({"effect": "Merge", "duration": 1000})
        );
        assert_eq!(s["transitions"].as_object().unwrap().len(), 4);
        assert_eq!(
            s["mixes"],
            json!({"2": {"program": 3, "preview": 1}, "3": {"program": 2, "preview": 2}})
        );
        assert_eq!(s["overlays"], json!({"2": {"input": 3, "preview": true}}));
        assert_eq!(
            s["outputs"]["output2"],
            json!({"type": "Output", "number": 2, "source": "Input", "input": 3,
                   "ndi": false, "omt": false, "srt": true})
        );
        assert_eq!(s["outputs"]["output3"]["mix"], 1);
        assert_eq!(
            s["outputs"]["fullscreen1"],
            json!({"type": "Fullscreen", "number": 1, "source": "Output"})
        );
        // An element the module does not know, kept by name.
        assert_eq!(s["other"]["audioOutput"], "Speakers");
    }

    #[test]
    fn a_read_reports_only_what_changed() {
        let (mut m, _) = connected_to(STATUS_XML);
        assert!(read(&mut m, 1_000, STATUS_XML).is_empty());

        let changed = STATUS_XML
            .replace(r#"duration="125""#, r#"duration="126""#)
            .replace(
                r#"<overlay number="2" preview="True">3</overlay>"#,
                r#"<overlay number="2"/>"#,
            );
        assert_eq!(
            read(&mut m, 2_000, &changed),
            [json!({"recording_duration": 126, "overlays": {"2": null}})]
        );
    }

    #[test]
    fn a_number_that_holds_another_input_starts_again() {
        let (mut m, _) = connected_to(STATUS_XML);
        // Input 2 is removed, so the clip becomes input 2.
        let moved = STATUS_XML.replace(
            r#"<input key="b" number="2" type="Capture" title="Camera 2" state="Running"/><input key="c" number="3" type="Video" title="Clip" state="Paused"/>"#,
            r#"<input key="c" number="2" type="Video" title="Clip" state="Paused"/>"#,
        );
        let p = read(&mut m, 1_000, &moved);
        assert_eq!(p[0], json!({"inputs": {"2": null}}));
        assert_eq!(p[1]["inputs"]["2"]["key"], "c");
        assert_eq!(p[1]["inputs"]["2"]["title"], "Clip");
        assert_eq!(p[1]["inputs"]["3"], Value::Null);
    }

    #[test]
    fn commands_go_ahead_of_queued_reads() {
        let mut m = vmix();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        feed(&mut m, 5, "VERSION OK 27.0.0.49\r\n");
        let mut cx = Cx::new(6);
        m.command(&mut cx, 9, "cut", &params(json!({})));
        assert!(sent(&cx.take()).is_empty());
        // SUBSCRIBE TALLY is answered; the command goes before ACTS and XML.
        assert_eq!(
            sent(&feed(&mut m, 10, "SUBSCRIBE OK TALLY\r\n")),
            ["FUNCTION Cut\r\n"]
        );
        assert_eq!(
            sent(&feed(&mut m, 11, "FUNCTION OK Completed\r\n")),
            ["SUBSCRIBE ACTS\r\n"]
        );
    }

    #[test]
    fn the_state_is_read_as_often_as_set() {
        let ctx = |settings: Value| OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 4)),
            host_name: None,
            port: None,
            model: "vmix".into(),
            channels: None,
            settings: params(settings),
            monitor: true,
        };
        assert_eq!(Vmix::new(ctx(json!({}))).state_poll, STATE_POLL_DEFAULT);
        assert_eq!(
            Vmix::new(ctx(json!({"state_poll_ms": 5000}))).state_poll,
            5000
        );
        assert_eq!(
            Vmix::new(ctx(json!({"state_poll_ms": 10}))).state_poll,
            STATE_POLL_MIN
        );
        let mut m = Vmix::new(ctx(json!({"state_poll_ms": 5000})));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert!(cx.take().contains(&Action::SetTimer {
            key: POLL,
            after: 5000
        }));
    }

    #[test]
    fn other_mixes_follow_their_activators() {
        let (mut m, _) = connected_to(STATUS_XML);
        let s = state(&feed(
            &mut m,
            100,
            "ACTS OK InputMix2 1 1\r\nACTS OK InputPreviewMix3 3 1\r\nACTS OK InputMix2 3 0\r\nACTS OK Fullscreen 0\r\n",
        ));
        assert_eq!(s["mixes"]["2"]["program"], 1);
        assert_eq!(s["mixes"]["3"]["preview"], 3);
        assert_eq!(s["fullscreen"], false);
        // The read that follows disagrees, so reports what differs.
        assert_eq!(
            read(&mut m, 1_000, STATUS_XML),
            [json!({"fullscreen": true, "mixes": {"2": {"program": 3}, "3": {"preview": 2}}})]
        );
    }

    /// Every leaf path of a state value.
    fn leaves(v: &Value, at: Vec<String>, out: &mut Vec<Vec<String>>) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    let mut next = at.clone();
                    next.push(k.clone());
                    leaves(v, next, out);
                }
            }
            _ => out.push(at),
        }
    }

    #[test]
    fn every_state_path_written_is_declared_in_the_spec() {
        let catalog = crate::catalog::Catalog::embedded();
        let spec = catalog.device("vmix").unwrap();
        let declared: Vec<Vec<String>> = spec
            .state
            .keys()
            .map(|k| k.split('.').map(str::to_string).collect())
            .collect();
        let mut paths = Vec::new();
        leaves(&connected_to(STATUS_XML).1, Vec::new(), &mut paths);
        leaves(&state(&connected().1), Vec::new(), &mut paths);
        let (mut m, s) = connected_to(AUDIO_XML);
        leaves(&s, Vec::new(), &mut paths);
        let events = "ACTS OK InputAudioAuto 1 1\r\nACTS OK InputVolumeChannelMixer2 1 1\r\nACTS OK InputBusAAudio 1 1\r\nACTS OK BusCSolo 1\r\nACTS OK MasterHeadphones 1\r\nACTS OK InputMix2 1 1\r\nACTS OK InputPreviewMix2 1 1\r\n";
        leaves(&state(&feed(&mut m, 100, events)), Vec::new(), &mut paths);
        for path in paths {
            assert!(
                declared.iter().any(|d| d.len() == path.len()
                    && d.iter().zip(&path).all(|(a, b)| a == "*" || a == b)),
                "{} is not declared",
                path.join(".")
            );
        }
    }

    /// Audio, in the shape vMix 27 reports it: an input with audio, one
    /// without, the master and two buses.
    const AUDIO_XML: &str = r#"<vmix><version>27.0.0.49</version><edition>Pro</edition><inputs><input key="a" number="1" type="Capture" title="Camera 1" state="Running" position="0" duration="0" loop="False" muted="False" volume="80.5" balance="-0.25" solo="True" soloPFL="False" audiobusses="M,B" meterF1="0.0912" meterF2="0.0837" gainDb="6">Camera 1</input><input key="b" number="2" type="Colour" title="Black" state="Paused" position="0" duration="0" loop="False">Black</input></inputs><preview>2</preview><active>1</active><audio><master volume="100" muted="False" meterF1="0.0844" meterF2="0.0799" headphonesVolume="74.5"/><busA volume="90" muted="True" meterF1="0" meterF2="0" solo="False" sendToMaster="True"/><busB volume="100" muted="False" meterF1="0.0912" meterF2="0.0837" solo="True" sendToMaster="False"/></audio></vmix>"#;

    #[test]
    fn audio_is_read_for_inputs_the_master_and_the_buses() {
        let (_, s) = connected_to(AUDIO_XML);
        let one = &s["inputs"]["1"];
        assert_eq!(one["volume"], 80.5);
        assert_eq!(one["balance"], -0.25);
        assert_eq!(one["solo"], true);
        assert_eq!(one["gain_db"], 6.0);
        assert_eq!(one["meter_left"], 0.0912);
        assert_eq!(one["meter_right"], 0.0837);
        assert_eq!(
            one["audio_buses"],
            json!({"M": true, "A": false, "B": true, "C": false, "D": false,
                   "E": false, "F": false, "G": false})
        );
        // No audio, no audio state.
        assert_eq!(s["inputs"]["2"].get("audio_buses"), None);
        assert_eq!(s["inputs"]["2"].get("volume"), None);
        assert_eq!(
            s["master"],
            json!({"volume": 100.0, "muted": false, "meter_left": 0.0844,
                   "meter_right": 0.0799, "headphones_volume": 74.5})
        );
        assert_eq!(
            s["buses"]["A"],
            json!({"volume": 90.0, "muted": true, "meter_left": 0.0, "meter_right": 0.0,
                   "solo": false, "send_to_master": true})
        );
        assert_eq!(s["buses"]["B"]["solo"], true);
    }

    #[test]
    fn what_only_activators_report_is_asked_for_each_new_input_with_audio() {
        let mut m = vmix();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        feed(&mut m, 11, "SUBSCRIBE OK TALLY\r\n");
        feed(&mut m, 12, "SUBSCRIBE OK ACTS\r\n");
        // Input 1 has audio, input 2 has not.
        let a = feed(&mut m, 13, &xml_reply(AUDIO_XML));
        assert_eq!(sent(&a), ["ACTS InputAudioAuto 1\r\n"]);
        // An event in between is applied and is not the answer.
        let a = feed(&mut m, 14, "ACTS OK InputAudio 1 0\r\n");
        assert!(sent(&a).is_empty());
        assert_eq!(state(&a)["inputs"]["1"]["muted"], true);
        let a = feed(&mut m, 15, "ACTS OK InputAudioAuto 1 1\r\n");
        assert_eq!(state(&a)["inputs"]["1"]["audio_auto"], true);
        assert_eq!(sent(&a), ["ACTS InputVolumeChannelMixer1 1\r\n"]);
        let a = feed(&mut m, 16, "ACTS OK InputVolumeChannelMixer1 1 0.5\r\n");
        assert_eq!(state(&a)["inputs"]["1"]["channel_mixer"]["1"], 50.0);
        assert_eq!(sent(&a), ["ACTS InputVolumeChannelMixer2 1\r\n"]);
        // A refusal answers too.
        let a = feed(&mut m, 17, "ACTS ER No Input\r\n");
        assert_eq!(sent(&a), ["ACTS InputVolumeChannelMixer3 1\r\n"]);
        let rest = settle(&mut m, 18);
        assert_eq!(sent(&rest).len(), 13);
        assert!(m.current.is_none());

        // The same inputs again: nothing more is asked.
        let mut cx = Cx::new(1_000);
        m.timer(&mut cx, POLL);
        cx.take();
        let a = feed(&mut m, 1_010, &xml_reply(AUDIO_XML));
        assert!(sent(&a).is_empty());
        // What only activators set survives the read.
        assert_eq!(patches(&a), [json!({"inputs": {"1": {"muted": false}}})]);
    }

    #[test]
    fn audio_activators_update_inputs_and_buses() {
        let (mut m, _) = connected_to(AUDIO_XML);
        let s = state(&feed(
            &mut m,
            100,
            "ACTS OK InputVolume 1 0.25\r\nACTS OK InputSolo 1 0\r\nACTS OK InputBusAAudio 1 1\r\nACTS OK InputMasterAudio 1 0\r\nACTS OK InputVolumeChannelMixer16 1 1\r\nACTS OK MasterVolume 0.9\r\nACTS OK MasterAudio 0\r\nACTS OK MasterHeadphones 0.5\r\nACTS OK BusAVolume 0.123456\r\nACTS OK BusAAudio 1\r\nACTS OK BusGSolo 1\r\n",
        ));
        let one = &s["inputs"]["1"];
        assert_eq!(one["volume"], 25.0);
        assert_eq!(one["solo"], false);
        assert_eq!(one["audio_buses"], json!({"A": true, "M": false}));
        assert_eq!(one["channel_mixer"]["16"], 100.0);
        assert_eq!(
            s["master"],
            json!({"volume": 90.0, "muted": true, "headphones_volume": 50.0})
        );
        assert_eq!(s["buses"]["A"], json!({"volume": 12.35, "muted": false}));
        assert_eq!(s["buses"]["G"], json!({"solo": true}));
    }

    /// Answers every outstanding ACTS query with a refusal.
    fn settle(m: &mut Vmix, now: Millis) -> Vec<Action> {
        let mut a = Vec::new();
        while let Some(Request::Acts { .. }) = &m.current {
            a.extend(feed(m, now, "ACTS ER No Input\r\n"));
        }
        a
    }
}
