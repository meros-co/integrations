//! Ember+ providers (Lawo, Riedel, DHD, Wisycom, Stagetec, Evertz and any
//! other device or software that implements the protocol), as a consumer.
//!
//! Written from Lawo's "Ember+ Specification" 2.50 revision 15 (2017-11-09),
//! published in the Lawo/ember-plus repository (Boost Software License). The
//! layers are in their own files: [`s101`] frames the byte stream, [`ber`]
//! is the EmBER subset of BER, and [`glow`] is the Glow DTD 2.50 object
//! schema.
//!
//! - The provider holds a tree of nodes, parameters, matrices and functions.
//!   Each element has a number, unique among its siblings and stable for the
//!   session only, and usually a string identifier, which "must not change"
//!   and is what a consumer should use to find an element again later.
//! - A consumer asks for an element's children with the GetDirectory command
//!   (32), sent as a child of the element; asking also subscribes it to value
//!   changes below that element. A matrix's connections come from a
//!   GetDirectory on the matrix itself.
//! - A value is changed by sending the parameter with the new value; the
//!   provider "must always respond" with the value it now holds, the old one
//!   if it refused.
//! - A parameter with a stream identifier reports changes only after an
//!   explicit Subscribe (30), and then in StreamCollections.
//! - Functions are invoked with Invoke (33); the provider answers with an
//!   InvocationResult carrying the consumer's invocation id.
//! - Either side may send an S101 keep-alive request; the other must answer.
//!
//! State is the tree: every element under `elements`, keyed by its numeric
//! path ("1.2.3"), and `identifiers` mapping identifier paths
//! ("Device/Audio/Gain") to numeric paths. Commands take either form.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

#[path = "emberplus_ber.rs"]
mod ber;
#[path = "emberplus_glow.rs"]
mod glow;
#[path = "emberplus_s101.rs"]
mod s101;

use glow::{Element, Id, Root, Value as Glow};

/// Ember+ has no assigned port; 9000 is common, and the host gives the
/// device's own.
pub(crate) const DEFAULT_PORT: u16 = 9000;
const SOCKET: Key = "s101";

const KEEPALIVE: Key = "keepalive";
const REPLY: Key = "reply";
const RETRY: Key = "retry";
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Node,
    Parameter,
    Matrix,
    Function,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Node => "node",
            Kind::Parameter => "parameter",
            Kind::Matrix => "matrix",
            Kind::Function => "function",
        }
    }
}

/// What the module keeps of an element to address it and type its values.
#[derive(Debug, Clone)]
struct Elem {
    kind: Kind,
    identifier: Option<String>,
    id_path: Option<String>,
    children: BTreeSet<u32>,
    online: bool,
    // Parameters.
    declared_type: Option<i64>,
    value_type: Option<i64>,
    enumeration: Vec<String>,
    enum_map: Vec<(String, i64)>,
    access: Option<i64>,
    minimum: Option<Glow>,
    maximum: Option<Glow>,
    nullable: bool,
    stream: Option<i64>,
    descriptor: Option<glow::StreamDescription>,
    // Functions.
    arguments: Option<Vec<glow::TupleItem>>,
    // Matrices.
    matrix_type: i64,
    addressing: i64,
    target_count: Option<i64>,
    source_count: Option<i64>,
    targets: Option<Vec<u32>>,
    sources: Option<Vec<u32>>,
}

impl Elem {
    fn new(kind: Kind) -> Elem {
        Elem {
            kind,
            identifier: None,
            id_path: None,
            children: BTreeSet::new(),
            online: true,
            declared_type: None,
            value_type: None,
            enumeration: Vec::new(),
            enum_map: Vec::new(),
            access: None,
            minimum: None,
            maximum: None,
            nullable: false,
            stream: None,
            descriptor: None,
            arguments: None,
            matrix_type: 0,
            addressing: 0,
            target_count: None,
            source_count: None,
            targets: None,
            sources: None,
        }
    }

    /// The parameter's type by the DTD's rule (Glow 2.5 change log): trigger
    /// when its type says so, enum when it has an enumeration or enumMap,
    /// else the type of its value, else its type field.
    fn effective_type(&self) -> Option<i64> {
        if self.declared_type == Some(glow::TYPE_TRIGGER) {
            return Some(glow::TYPE_TRIGGER);
        }
        if !self.enumeration.is_empty() || !self.enum_map.is_empty() {
            return Some(glow::TYPE_ENUM);
        }
        self.value_type.or(self.declared_type)
    }
}

fn key(path: &[u32]) -> String {
    path.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn value_type(v: &Glow) -> Option<i64> {
    Some(match v {
        Glow::Integer(_) => glow::TYPE_INTEGER,
        Glow::Real(_) => glow::TYPE_REAL,
        Glow::String(_) => glow::TYPE_STRING,
        Glow::Boolean(_) => glow::TYPE_BOOLEAN,
        Glow::Octets(_) => glow::TYPE_OCTETS,
        Glow::Null => return None,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// A Glow value as state. JSON has no infinities or NaN, so a real that is
/// one is the string "inf", "-inf" or "nan"; octets are lowercase hex; null
/// is JSON null, which removes the value from state.
fn json_of(v: &Glow) -> Value {
    match v {
        Glow::Integer(i) => json!(i),
        Glow::Real(r) if r.is_finite() => json!(r),
        Glow::Real(r) if r.is_nan() => json!("nan"),
        Glow::Real(r) if *r > 0.0 => json!("inf"),
        Glow::Real(_) => json!("-inf"),
        Glow::String(s) => json!(s),
        Glow::Boolean(b) => json!(b),
        Glow::Octets(o) => json!(hex(o)),
        Glow::Null => Value::Null,
    }
}

/// A JSON value as the Glow type `kind` (a ParameterType); with no type
/// known, by the JSON's own type.
fn typed(kind: Option<i64>, v: &Value) -> Result<Glow, String> {
    let wrong = |what: &str| format!("expected {what}, got {v}");
    match kind {
        Some(glow::TYPE_INTEGER) | Some(glow::TYPE_ENUM) => v
            .as_i64()
            .map(Glow::Integer)
            .ok_or_else(|| wrong("an integer")),
        Some(glow::TYPE_REAL) => match v {
            Value::String(s) if s == "inf" => Ok(Glow::Real(f64::INFINITY)),
            Value::String(s) if s == "-inf" => Ok(Glow::Real(f64::NEG_INFINITY)),
            Value::String(s) if s == "nan" => Ok(Glow::Real(f64::NAN)),
            _ => v.as_f64().map(Glow::Real).ok_or_else(|| wrong("a number")),
        },
        Some(glow::TYPE_STRING) => v
            .as_str()
            .map(|s| Glow::String(s.into()))
            .ok_or_else(|| wrong("a string")),
        Some(glow::TYPE_BOOLEAN) => v
            .as_bool()
            .map(Glow::Boolean)
            .ok_or_else(|| wrong("true or false")),
        Some(glow::TYPE_OCTETS) => v
            .as_str()
            .and_then(unhex)
            .map(Glow::Octets)
            .ok_or_else(|| wrong("octets as a hex string")),
        _ => match v {
            Value::Bool(b) => Ok(Glow::Boolean(*b)),
            Value::Number(n) => Ok(n
                .as_i64()
                .map(Glow::Integer)
                .unwrap_or_else(|| Glow::Real(n.as_f64().unwrap_or_default()))),
            Value::String(s) => Ok(Glow::String(s.clone())),
            _ => Err(wrong("a number, string or boolean")),
        },
    }
}

fn numeric(v: &Glow) -> Option<f64> {
    match v {
        Glow::Integer(i) => Some(*i as f64),
        Glow::Real(r) => Some(*r),
        _ => None,
    }
}

/// A value from a stream's octets by its StreamDescription.
fn stream_slice(octets: &[u8], d: &glow::StreamDescription) -> Option<Glow> {
    glow::name_of(glow::STREAM_FORMATS, d.format)?;
    let kind = d.format >> 3;
    let size = 1usize << ((d.format >> 1) & 0x03);
    let little = d.format & 1 == 1;
    let at = usize::try_from(d.offset).ok()?;
    let bytes = octets.get(at..at.checked_add(size)?)?;
    let mut be: Vec<u8> = bytes.to_vec();
    if little {
        be.reverse();
    }
    let raw = be.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b));
    let bits = 8 * size as u32;
    Some(match kind {
        0 => i64::try_from(raw)
            .map(Glow::Integer)
            .unwrap_or(Glow::Real(raw as f64)),
        1 => {
            let shift = 64 - bits;
            Glow::Integer(((raw << shift) as i64) >> shift)
        }
        2 if size == 4 => Glow::Real(f64::from(f32::from_bits(raw as u32))),
        2 if size == 8 => Glow::Real(f64::from_bits(raw)),
        _ => return None,
    })
}

/// An element with nothing but its address and, optionally, children.
fn bare(kind: Kind, id: Id, children: Option<Vec<Element>>) -> Element {
    match kind {
        Kind::Node => Element::Node(glow::Node {
            id,
            contents: None,
            children,
        }),
        Kind::Parameter => Element::Parameter(glow::Parameter {
            id,
            contents: None,
            children,
        }),
        Kind::Function => Element::Function(glow::Function {
            id,
            contents: None,
            children,
        }),
        Kind::Matrix => Element::Matrix(glow::Matrix {
            id,
            contents: None,
            children,
            targets: None,
            sources: None,
            connections: None,
        }),
    }
}

/// A state patch under construction.
#[derive(Debug, Default)]
struct Patch(Map<String, Value>);

impl Patch {
    fn object<'a>(map: &'a mut Map<String, Value>, k: &str) -> &'a mut Map<String, Value> {
        let v = map
            .entry(k.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !v.is_object() {
            *v = Value::Object(Map::new());
        }
        v.as_object_mut().expect("just made an object")
    }

    fn elem(&mut self, path: &[u32]) -> &mut Map<String, Value> {
        let elements = Patch::object(&mut self.0, "elements");
        Patch::object(elements, &key(path))
    }

    fn put(&mut self, path: &[u32], field: &str, v: Value) {
        self.elem(path).insert(field.into(), v);
    }

    fn identifier(&mut self, id_path: &str, numeric: Value) {
        Patch::object(&mut self.0, "identifiers").insert(id_path.into(), numeric);
    }

    fn connection(&mut self, path: &[u32], target: u32, v: Value) {
        let connections = Patch::object(self.elem(path), "connections");
        connections.insert(target.to_string(), v);
    }

    fn flush(&mut self, cx: &mut Cx) {
        if !self.0.is_empty() {
            cx.state(Value::Object(std::mem::take(&mut self.0)));
        }
    }
}

#[derive(Debug, Clone)]
struct Settings {
    walk_depth: usize,
    walk_matrices: bool,
    qualified: bool,
    concurrency: usize,
    keepalive_interval: Millis,
    keepalive_timeout: Millis,
    request_timeout: Millis,
}

fn int_setting(p: &Params, k: &str, default: i64) -> i64 {
    p.get(k).and_then(Value::as_i64).unwrap_or(default)
}

impl Settings {
    fn from(p: &Params) -> Settings {
        Settings {
            walk_depth: int_setting(p, "walk_depth", 255).clamp(1, 255) as usize,
            walk_matrices: p
                .get("walk_matrices")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            qualified: p
                .get("qualified_requests")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            concurrency: int_setting(p, "directory_concurrency", 8).clamp(1, 256) as usize,
            keepalive_interval: int_setting(p, "keepalive_interval_ms", 5_000).max(100) as Millis,
            keepalive_timeout: int_setting(p, "keepalive_timeout_ms", 5_000).max(100) as Millis,
            request_timeout: int_setting(p, "request_timeout_ms", 10_000).max(100) as Millis,
        }
    }
}

#[derive(Debug)]
struct Waiting {
    id: CommandId,
    path: Vec<u32>,
    deadline: Millis,
}

#[derive(Debug)]
struct ConnectWait {
    id: CommandId,
    path: Vec<u32>,
    target: u32,
    deadline: Millis,
}

pub(crate) struct EmberPlus {
    device: SocketAddr,
    settings: Settings,
    decoder: s101::Decoder,
    assembler: s101::Assembler,
    socket_open: bool,
    connected: bool,
    tree: BTreeMap<Vec<u32>, Elem>,
    ids: HashMap<String, Vec<u32>>,
    streams: HashMap<i64, BTreeSet<Vec<u32>>>,
    dir_queue: VecDeque<Vec<u32>>,
    dir_requested: HashSet<Vec<u32>>,
    dir_outstanding: BTreeMap<Vec<u32>, Millis>,
    dir_waiters: Vec<(Vec<u32>, CommandId)>,
    walk_complete: bool,
    sets: Vec<Waiting>,
    connects: Vec<ConnectWait>,
    invocations: BTreeMap<i64, (CommandId, Millis)>,
    next_invocation: i64,
    /// Elements subscribed to, by identifier path where known (numbers may
    /// change between sessions), else by numeric path.
    subscriptions: BTreeSet<String>,
    subscribed_now: HashSet<Vec<u32>>,
    touched: Vec<Vec<u32>>,
    last_heard: Millis,
    keepalive_sent: Option<Millis>,
    retry_after: Millis,
}

impl EmberPlus {
    pub(crate) fn new(ctx: OpenContext) -> EmberPlus {
        EmberPlus {
            device: SocketAddr::new(ctx.host, ctx.port.unwrap_or(DEFAULT_PORT)),
            settings: Settings::from(&ctx.settings),
            decoder: s101::Decoder::default(),
            assembler: s101::Assembler::default(),
            socket_open: false,
            connected: false,
            tree: BTreeMap::new(),
            ids: HashMap::new(),
            streams: HashMap::new(),
            dir_queue: VecDeque::new(),
            dir_requested: HashSet::new(),
            dir_outstanding: BTreeMap::new(),
            dir_waiters: Vec::new(),
            walk_complete: false,
            sets: Vec::new(),
            connects: Vec::new(),
            invocations: BTreeMap::new(),
            next_invocation: 1,
            subscriptions: BTreeSet::new(),
            subscribed_now: HashSet::new(),
            touched: Vec::new(),
            last_heard: 0,
            keepalive_sent: None,
            retry_after: RETRY_MIN,
        }
    }

    // --- Connection ---------------------------------------------------------

    fn open(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        let error = CommandError::Transport {
            message: reason.clone(),
        };
        self.fail_all(cx, &error);
        for k in [KEEPALIVE, REPLY] {
            cx.cancel_timer(k);
        }
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.connected = false;
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn fail_all(&mut self, cx: &mut Cx, error: &CommandError) {
        for (_, id) in self.dir_waiters.drain(..) {
            cx.complete(id, Err(error.clone()));
        }
        for w in self.sets.drain(..) {
            cx.complete(w.id, Err(error.clone()));
        }
        for w in self.connects.drain(..) {
            cx.complete(w.id, Err(error.clone()));
        }
        for (_, (id, _)) in std::mem::take(&mut self.invocations) {
            cx.complete(id, Err(error.clone()));
        }
        self.dir_outstanding.clear();
        self.dir_queue.clear();
    }

    fn send(&mut self, cx: &mut Cx, root: &Root) {
        cx.tcp_send(SOCKET, s101::ember_frames(&glow::encode(root)));
    }

    fn arm_reply_timer(&self, cx: &mut Cx) {
        let next = self
            .dir_outstanding
            .values()
            .copied()
            .chain(self.sets.iter().map(|w| w.deadline))
            .chain(self.connects.iter().map(|w| w.deadline))
            .chain(self.invocations.values().map(|(_, d)| *d))
            .min();
        match next {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    // --- Addressing -----------------------------------------------------------

    /// A message addressing the element at `path`: a Qualified element at the
    /// root, or the path spelled out as nested elements.
    fn addressed(&self, path: &[u32], leaf: impl FnOnce(Id) -> Element) -> Root {
        let Some((&last, parents)) = path.split_last() else {
            unreachable!("the root is addressed by a bare command");
        };
        if self.settings.qualified {
            return Root::Elements(vec![leaf(Id::Path(path.to_vec()))]);
        }
        let mut e = leaf(Id::Number(last));
        for depth in (0..parents.len()).rev() {
            let kind = self
                .tree
                .get(&path[..=depth])
                .map_or(Kind::Node, |x| x.kind);
            e = bare(kind, Id::Number(path[depth]), Some(vec![e]));
        }
        Root::Elements(vec![e])
    }

    fn command_on(&self, path: &[u32], command: glow::Command) -> Root {
        if path.is_empty() {
            return Root::Elements(vec![Element::Command(command)]);
        }
        let kind = self.tree.get(path).map_or(Kind::Node, |e| e.kind);
        self.addressed(path, |id| {
            bare(kind, id, Some(vec![Element::Command(command)]))
        })
    }

    fn resolve(&self, p: &Params) -> Result<Vec<u32>, CommandError> {
        let raw = p.get("path").and_then(Value::as_str).unwrap_or("").trim();
        let raw = raw.trim_start_matches('/');
        if raw.is_empty() {
            return Ok(Vec::new());
        }
        if raw.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return raw
                .split('.')
                .map(|s| s.parse::<u32>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| invalid(format!("'{raw}' is not a numeric path like 1.2.3")));
        }
        self.ids.get(raw).cloned().ok_or_else(|| {
            invalid(format!(
                "no element with the identifier path '{raw}' is known; the tree may not have been walked that far (get_directory its parent)"
            ))
        })
    }

    fn element_of(&self, path: &[u32], kind: Kind) -> Result<&Elem, CommandError> {
        match self.tree.get(path) {
            Some(e) if e.kind == kind => Ok(e),
            Some(e) => Err(invalid(format!(
                "{} is a {}, not a {}",
                key(path),
                e.kind.name(),
                kind.name()
            ))),
            None => Err(invalid(format!(
                "no {} at {} is known; get_directory its parent first",
                kind.name(),
                key(path)
            ))),
        }
    }

    // --- Directory walk -------------------------------------------------------

    fn send_directory(&mut self, cx: &mut Cx, path: &[u32], mask: i64) {
        let root = self.command_on(
            path,
            glow::Command {
                number: glow::GET_DIRECTORY,
                dir_field_mask: Some(mask),
                invocation: None,
            },
        );
        self.send(cx, &root);
        self.dir_requested.insert(path.to_vec());
        self.dir_outstanding
            .insert(path.to_vec(), cx.now() + self.settings.request_timeout);
        self.arm_reply_timer(cx);
    }

    fn enqueue(&mut self, path: Vec<u32>) {
        if self.dir_requested.insert(path.clone()) {
            self.dir_queue.push_back(path);
        }
    }

    /// Send queued directory requests up to the concurrency limit, and note
    /// when the walk has finished.
    fn pump(&mut self, cx: &mut Cx, p: &mut Patch) {
        while self.dir_outstanding.len() < self.settings.concurrency {
            let Some(path) = self.dir_queue.pop_front() else {
                break;
            };
            self.send_directory(cx, &path, -1);
        }
        if !self.walk_complete && self.dir_queue.is_empty() && self.dir_outstanding.is_empty() {
            self.walk_complete = true;
            p.0.insert("walk".into(), json!({"complete": true}));
        }
    }

    fn answered(&mut self, cx: &mut Cx, path: &[u32]) {
        if self.dir_outstanding.remove(path).is_none() {
            return;
        }
        let mut i = 0;
        while i < self.dir_waiters.len() {
            if self.dir_waiters[i].0 == path {
                let (_, id) = self.dir_waiters.remove(i);
                cx.complete(id, Ok(Outcome::Ack));
            } else {
                i += 1;
            }
        }
        self.arm_reply_timer(cx);
    }

    /// Forget the tree and walk it again from the root.
    fn start_walk(&mut self, cx: &mut Cx) {
        self.tree.clear();
        self.ids.clear();
        self.streams.clear();
        self.dir_queue.clear();
        self.dir_requested.clear();
        self.subscribed_now.clear();
        self.walk_complete = false;
        cx.state(json!({"elements": null, "identifiers": null, "walk": {"complete": false}}));
        self.enqueue(Vec::new());
        let mut p = Patch::default();
        self.pump(cx, &mut p);
        p.flush(cx);
    }

    // --- Applying what the provider reports -------------------------------------

    /// The element at `path`, made if new. Returns whether it is new.
    fn touch(&mut self, p: &mut Patch, path: &[u32], kind: Kind) -> bool {
        self.touched.push(path.to_vec());
        if let Some(e) = self.tree.get_mut(path) {
            if e.kind != kind {
                *e = Elem::new(kind);
                p.put(path, "type", json!(kind.name()));
            }
            return false;
        }
        self.tree.insert(path.to_vec(), Elem::new(kind));
        if let Some((last, parent)) = path.split_last() {
            if let Some(parent) = self.tree.get_mut(parent) {
                parent.children.insert(*last);
            }
            let e = p.elem(path);
            e.insert("type".into(), json!(kind.name()));
            e.insert("path".into(), json!(key(path)));
            e.insert("number".into(), json!(last));
        }
        true
    }

    fn set_identifier(&mut self, p: &mut Patch, path: &[u32], identifier: &str) {
        p.put(path, "identifier", json!(identifier));
        if let Some(e) = self.tree.get_mut(path) {
            e.identifier = Some(identifier.to_string());
        }
        self.update_id_path(p, path);
    }

    /// The identifier path of `path` and, if it changed, of its descendants.
    fn update_id_path(&mut self, p: &mut Patch, path: &[u32]) {
        let Some(e) = self.tree.get(path) else { return };
        let Some(identifier) = e.identifier.clone() else {
            return;
        };
        let prefix = match path.split_last() {
            Some((_, [])) => String::new(),
            Some((_, parent)) => match self.tree.get(parent).and_then(|e| e.id_path.clone()) {
                Some(prefix) => format!("{prefix}/"),
                None => return,
            },
            None => return,
        };
        let full = format!("{prefix}{identifier}");
        if e.id_path.as_deref() == Some(full.as_str()) {
            return;
        }
        let children: Vec<u32> = e.children.iter().copied().collect();
        if let Some(old) = self
            .tree
            .get_mut(path)
            .and_then(|e| e.id_path.replace(full.clone()))
        {
            if self.ids.get(&old).map(Vec::as_slice) == Some(path) {
                self.ids.remove(&old);
                p.identifier(&old, Value::Null);
            }
        }
        self.ids.insert(full.clone(), path.to_vec());
        p.identifier(&full, json!(key(path)));
        p.put(path, "identifier_path", json!(full));
        for c in children {
            let mut child = path.to_vec();
            child.push(c);
            self.update_id_path(p, &child);
        }
    }

    fn walk_child(&mut self, path: &[u32], kind: Kind) {
        let wanted = match kind {
            Kind::Node => path.len() < self.settings.walk_depth,
            Kind::Matrix => self.settings.walk_matrices && path.len() <= self.settings.walk_depth,
            _ => false,
        };
        if wanted {
            self.enqueue(path.to_vec());
        }
    }

    fn element(&mut self, cx: &mut Cx, p: &mut Patch, parent: &[u32], e: &Element) {
        let path = |id: &Id| match id {
            Id::Number(n) => {
                let mut v = parent.to_vec();
                v.push(*n);
                v
            }
            Id::Path(v) => v.clone(),
        };
        match e {
            Element::Node(n) => {
                let path = path(&n.id);
                self.node(cx, p, &path, n);
            }
            Element::Parameter(x) => {
                let path = path(&x.id);
                self.parameter(cx, p, &path, x);
            }
            Element::Matrix(m) => {
                let path = path(&m.id);
                self.matrix(cx, p, &path, m);
            }
            Element::Function(f) => {
                let path = path(&f.id);
                self.function(cx, p, &path, f);
            }
            // Commands are a consumer's; templates are discarded.
            Element::Command(_) | Element::Template => {}
        }
    }

    fn children(&mut self, cx: &mut Cx, p: &mut Patch, path: &[u32], c: &Option<Vec<Element>>) {
        for child in c.iter().flatten() {
            self.element(cx, p, path, child);
        }
    }

    fn node(&mut self, cx: &mut Cx, p: &mut Patch, path: &[u32], n: &glow::Node) {
        let new = self.touch(p, path, Kind::Node);
        if let Some(c) = &n.contents {
            if let Some(v) = &c.identifier {
                self.set_identifier(p, path, v);
            }
            if let Some(v) = &c.description {
                p.put(path, "description", json!(v));
            }
            if let Some(v) = c.is_root {
                p.put(path, "is_root", json!(v));
            }
            if let Some(v) = &c.schema_identifiers {
                p.put(
                    path,
                    "schema_identifiers",
                    json!(v.split('\n').collect::<Vec<_>>()),
                );
            }
            if let Some(v) = &c.template_reference {
                p.put(path, "template_reference", json!(key(v)));
            }
            if let Some(online) = c.is_online {
                p.put(path, "is_online", json!(online));
                let e = self.tree.get_mut(path).expect("touched");
                let back = online && !e.online;
                e.online = online;
                // A node back online may hold a different sub-tree: ask again.
                if back && self.dir_requested.remove(path) {
                    self.walk_child(path, Kind::Node);
                }
            }
        }
        if n.children.is_some() {
            // Its children are known; there is no need to ask.
            self.dir_requested.insert(path.to_vec());
        }
        self.children(cx, p, path, &n.children);
        // "A node without any children must report itself without any
        // properties set": the answer to a GetDirectory on an empty node.
        if n.children.is_some() || n.contents.is_none() {
            self.answered(cx, path);
        }
        if new {
            self.walk_child(path, Kind::Node);
        }
    }

    fn parameter(&mut self, cx: &mut Cx, p: &mut Patch, path: &[u32], x: &glow::Parameter) {
        self.touch(p, path, Kind::Parameter);
        let Some(c) = &x.contents else {
            self.answered(cx, path);
            return;
        };
        if let Some(v) = &c.identifier {
            self.set_identifier(p, path, v);
        }
        let mut stream_change = None;
        {
            let e = self.tree.get_mut(path).expect("touched");
            if let Some(v) = &c.description {
                p.put(path, "description", json!(v));
            }
            if let Some(v) = &c.minimum {
                p.put(path, "minimum", json_of(v));
                e.minimum = Some(v.clone());
            }
            if let Some(v) = &c.maximum {
                p.put(path, "maximum", json_of(v));
                e.maximum = Some(v.clone());
            }
            if let Some(v) = c.access {
                p.put(
                    path,
                    "access",
                    glow::name_of(glow::ACCESS, v).map_or(json!(v), |s| json!(s)),
                );
                e.access = Some(v);
            }
            if let Some(v) = &c.format {
                p.put(path, "format", json!(v));
            }
            if let Some(v) = &c.enumeration {
                e.enumeration = v.split('\n').map(str::to_string).collect();
                p.put(path, "enumeration", json!(e.enumeration));
            }
            if let Some(v) = c.factor {
                p.put(path, "factor", json!(v));
            }
            if let Some(v) = c.is_online {
                p.put(path, "is_online", json!(v));
            }
            if let Some(v) = &c.formula {
                p.put(path, "formula", json!(v.split('\n').collect::<Vec<_>>()));
            }
            if let Some(v) = c.step {
                p.put(path, "step", json!(v));
            }
            if let Some(v) = &c.default {
                e.nullable = *v == Glow::Null;
                p.put(path, "default", json_of(v));
                p.put(path, "nullable", json!(e.nullable));
            }
            if let Some(v) = c.kind {
                e.declared_type = Some(v);
                p.put(
                    path,
                    "parameter_type",
                    glow::name_of(glow::PARAMETER_TYPES, v).map_or(json!(v), |s| json!(s)),
                );
            }
            if let Some(v) = c.stream_identifier {
                p.put(path, "stream_identifier", json!(v));
                if e.stream != Some(v) {
                    stream_change = Some((e.stream.replace(v), v));
                }
            }
            if let Some(m) = &c.enum_map {
                e.enum_map = m.clone();
                let entries: Vec<Value> = m
                    .iter()
                    .map(|(name, value)| json!({"name": name, "value": value}))
                    .collect();
                p.put(path, "enum_map", Value::Array(entries));
            }
            if let Some(d) = &c.stream_descriptor {
                e.descriptor = Some(d.clone());
                p.put(
                    path,
                    "stream_descriptor",
                    json!({
                        "format": glow::name_of(glow::STREAM_FORMATS, d.format).map_or(json!(d.format), |s| json!(s)),
                        "offset": d.offset,
                    }),
                );
            }
            if let Some(v) = &c.schema_identifiers {
                p.put(
                    path,
                    "schema_identifiers",
                    json!(v.split('\n').collect::<Vec<_>>()),
                );
            }
            if let Some(v) = &c.template_reference {
                p.put(path, "template_reference", json!(key(v)));
            }
            if let Some(v) = &c.value {
                if let Some(t) = value_type(v) {
                    e.value_type = Some(t);
                }
            }
        }
        if let Some((old, new)) = stream_change {
            if let Some(old) = old.and_then(|o| self.streams.get_mut(&o)) {
                old.remove(path);
            }
            self.streams.entry(new).or_default().insert(path.to_vec());
        }
        if let Some(v) = &c.value {
            let value = json_of(v);
            p.put(path, "value", value.clone());
            if let Some(i) = self.sets.iter().position(|w| w.path == path) {
                let w = self.sets.remove(i);
                cx.complete(w.id, Ok(Outcome::Value { value }));
                self.arm_reply_timer(cx);
            }
        }
        self.answered(cx, path);
    }

    fn matrix(&mut self, cx: &mut Cx, p: &mut Patch, path: &[u32], m: &glow::Matrix) {
        let new = self.touch(p, path, Kind::Matrix);
        if let Some(c) = &m.contents {
            if let Some(v) = &c.identifier {
                self.set_identifier(p, path, v);
            }
            let e = self.tree.get_mut(path).expect("touched");
            if let Some(v) = &c.description {
                p.put(path, "description", json!(v));
            }
            if let Some(v) = c.kind {
                e.matrix_type = v;
                p.put(
                    path,
                    "matrix_type",
                    glow::name_of(glow::MATRIX_TYPES, v).map_or(json!(v), |s| json!(s)),
                );
            }
            if let Some(v) = c.addressing_mode {
                e.addressing = v;
                p.put(
                    path,
                    "addressing_mode",
                    glow::name_of(glow::ADDRESSING_MODES, v).map_or(json!(v), |s| json!(s)),
                );
            }
            if let Some(v) = c.target_count {
                e.target_count = Some(v);
                p.put(path, "target_count", json!(v));
            }
            if let Some(v) = c.source_count {
                e.source_count = Some(v);
                p.put(path, "source_count", json!(v));
            }
            if let Some(v) = c.maximum_total_connects {
                p.put(path, "maximum_total_connects", json!(v));
            }
            if let Some(v) = c.maximum_connects_per_target {
                p.put(path, "maximum_connects_per_target", json!(v));
            }
            if let Some(l) = &c.parameters_location {
                // An inline location is "a single sub-identifier to be
                // appended to the path of the matrix".
                let at = match l {
                    glow::ParametersLocation::BasePath(b) => Some(b.clone()),
                    glow::ParametersLocation::Inline(n) => u32::try_from(*n).ok().map(|n| {
                        let mut v = path.to_vec();
                        v.push(n);
                        v
                    }),
                };
                if let Some(at) = at {
                    p.put(path, "parameters_location", json!(key(&at)));
                }
            }
            if let Some(v) = c.gain_parameter_number {
                p.put(path, "gain_parameter_number", json!(v));
            }
            if let Some(labels) = &c.labels {
                let labels: Vec<Value> = labels
                    .iter()
                    .map(|l| json!({"base_path": key(&l.base_path), "description": l.description}))
                    .collect();
                p.put(path, "labels", Value::Array(labels));
            }
            if let Some(v) = &c.schema_identifiers {
                p.put(
                    path,
                    "schema_identifiers",
                    json!(v.split('\n').collect::<Vec<_>>()),
                );
            }
            if let Some(v) = &c.template_reference {
                p.put(path, "template_reference", json!(key(v)));
            }
        }
        {
            let e = self.tree.get_mut(path).expect("touched");
            if let Some(t) = &m.targets {
                e.targets = Some(t.clone());
                p.put(path, "targets", json!(t));
            }
            if let Some(s) = &m.sources {
                e.sources = Some(s.clone());
                p.put(path, "sources", json!(s));
            }
        }
        for c in m.connections.iter().flatten() {
            let sources = c.sources.clone().unwrap_or_default();
            let disposition = c.disposition.unwrap_or(0);
            let disposition_name = glow::name_of(glow::DISPOSITIONS, disposition)
                .map_or(json!(disposition), |s| json!(s));
            p.connection(
                path,
                c.target,
                json!({"sources": sources, "disposition": disposition_name}),
            );
            if let Some(i) = self
                .connects
                .iter()
                .position(|w| w.path == path && w.target == c.target)
            {
                let w = self.connects.remove(i);
                let result = if disposition == glow::DISPOSITION_LOCKED {
                    Err(CommandError::DeviceError {
                        code: Some("locked".into()),
                        message: format!(
                            "target {} is locked; its sources are {sources:?}",
                            c.target
                        ),
                    })
                } else {
                    Ok(Outcome::Value {
                        value: json!({"target": c.target, "sources": sources, "disposition": disposition_name}),
                    })
                };
                cx.complete(w.id, result);
                self.arm_reply_timer(cx);
            }
        }
        self.children(cx, p, path, &m.children);
        let bare = m.contents.is_none() && m.children.is_none();
        if m.connections.is_some() || m.targets.is_some() || m.sources.is_some() || bare {
            self.answered(cx, path);
        }
        if new {
            self.walk_child(path, Kind::Matrix);
        }
    }

    fn function(&mut self, cx: &mut Cx, p: &mut Patch, path: &[u32], f: &glow::Function) {
        self.touch(p, path, Kind::Function);
        if let Some(c) = &f.contents {
            if let Some(v) = &c.identifier {
                self.set_identifier(p, path, v);
            }
            if let Some(v) = &c.description {
                p.put(path, "description", json!(v));
            }
            let describe = |items: &[glow::TupleItem]| -> Value {
                Value::Array(
                    items
                        .iter()
                        .map(|i| {
                            json!({
                                "type": glow::name_of(glow::PARAMETER_TYPES, i.kind).map_or(json!(i.kind), |s| json!(s)),
                                "name": i.name,
                            })
                        })
                        .collect(),
                )
            };
            if let Some(a) = &c.arguments {
                p.put(path, "arguments", describe(a));
                self.tree.get_mut(path).expect("touched").arguments = Some(a.clone());
            }
            if let Some(r) = &c.result {
                p.put(path, "result", describe(r));
            }
            if let Some(v) = &c.template_reference {
                p.put(path, "template_reference", json!(key(v)));
            }
        }
        self.answered(cx, path);
    }

    fn streams(&mut self, p: &mut Patch, entries: &[glow::StreamEntry]) {
        for entry in entries {
            let Some(paths) = self.streams.get(&entry.identifier) else {
                continue;
            };
            for path in paths {
                let Some(e) = self.tree.get(path) else {
                    continue;
                };
                let value = match (&entry.value, &e.descriptor) {
                    (Glow::Octets(o), Some(d)) => match stream_slice(o, d) {
                        Some(v) => v,
                        None => continue,
                    },
                    (v, _) => v.clone(),
                };
                p.put(path, "value", json_of(&value));
            }
        }
    }

    fn invocation_result(&mut self, cx: &mut Cx, r: &glow::InvocationResult) {
        let Some((id, _)) = self.invocations.remove(&r.invocation_id) else {
            return;
        };
        let values: Vec<Value> = r.result.iter().flatten().map(json_of).collect();
        let result = if r.success == Some(false) {
            Err(CommandError::DeviceError {
                code: None,
                message: format!("the function reported failure (result {values:?})"),
            })
        } else {
            Ok(Outcome::Value {
                value: Value::Array(values),
            })
        };
        cx.complete(id, result);
        self.arm_reply_timer(cx);
    }

    /// Subscriptions from an earlier session, made again once their element
    /// is found.
    fn resubscribe(&mut self, cx: &mut Cx) {
        let touched = std::mem::take(&mut self.touched);
        if self.subscriptions.is_empty() {
            return;
        }
        for path in touched {
            if self.subscribed_now.contains(&path) {
                continue;
            }
            let Some(e) = self.tree.get(&path) else {
                continue;
            };
            let wanted = e
                .id_path
                .as_ref()
                .is_some_and(|i| self.subscriptions.contains(i))
                || self.subscriptions.contains(&key(&path));
            if wanted {
                self.subscribed_now.insert(path.clone());
                let root = self.command_on(&path, glow::Command::new(glow::SUBSCRIBE));
                self.send(cx, &root);
            }
        }
    }

    fn ember(&mut self, cx: &mut Cx, payload: &[u8]) {
        let roots = match glow::decode(payload) {
            Ok(r) => r,
            Err(e) => {
                cx.log(Level::Warning, format!("discarded a Glow message: {e}"));
                return;
            }
        };
        let mut p = Patch::default();
        for root in roots {
            match root {
                Root::Elements(elements) => {
                    for e in &elements {
                        self.element(cx, &mut p, &[], e);
                    }
                    // The answer to the root's GetDirectory.
                    self.answered(cx, &[]);
                }
                Root::Streams(entries) => self.streams(&mut p, &entries),
                Root::InvocationResult(r) => self.invocation_result(cx, &r),
            }
        }
        self.resubscribe(cx);
        self.pump(cx, &mut p);
        p.flush(cx);
    }

    fn data(&mut self, cx: &mut Cx, bytes: &[u8]) {
        let (frames, errors) = self.decoder.feed(bytes);
        for e in errors {
            cx.log(Level::Warning, format!("S101: {e}"));
        }
        for frame in frames {
            if !self.socket_open {
                return;
            }
            let message = match s101::parse_message(&frame) {
                Ok(m) => m,
                Err(e) => {
                    cx.log(Level::Debug, format!("S101: ignored {e}"));
                    continue;
                }
            };
            self.last_heard = cx.now();
            self.keepalive_sent = None;
            cx.alive();
            if !self.connected {
                self.connected = true;
                self.retry_after = RETRY_MIN;
                cx.connection(Connection::Connected);
            }
            match message {
                s101::Message::KeepAliveRequest => cx.tcp_send(SOCKET, s101::keepalive_response()),
                s101::Message::KeepAliveResponse => {}
                s101::Message::Ember { flags, payload } => {
                    match self.assembler.push(flags, payload) {
                        Ok(Some(message)) => self.ember(cx, &message),
                        Ok(None) => {}
                        Err(e) => cx.log(Level::Warning, format!("S101: {e}")),
                    }
                }
            }
        }
    }

    // --- Commands -----------------------------------------------------------

    fn run(
        &mut self,
        cx: &mut Cx,
        id: CommandId,
        name: &str,
        p: &Params,
    ) -> Result<(), CommandError> {
        let deadline = cx.now() + self.settings.request_timeout;
        match name {
            "get_directory" => {
                let path = self.resolve(p)?;
                let mask = p
                    .get("field_mask")
                    .and_then(Value::as_str)
                    .and_then(|m| glow::FIELD_FLAGS.iter().find(|(n, _)| *n == m))
                    .map_or(-1, |(_, v)| *v);
                self.dir_waiters.push((path.clone(), id));
                self.send_directory(cx, &path, mask);
            }
            "refresh" => {
                self.dir_waiters.push((Vec::new(), id));
                self.dir_outstanding.remove(&Vec::new());
                self.start_walk(cx);
            }
            "set_parameter" | "set_parameter_null" => {
                let path = self.resolve(p)?;
                let e = self.element_of(&path, Kind::Parameter)?;
                if let Some(a @ (0 | 1)) = e.access {
                    return Err(invalid(format!(
                        "{} has access '{}'; it cannot be written",
                        key(&path),
                        glow::name_of(glow::ACCESS, a).unwrap_or("?")
                    )));
                }
                let value = if name == "set_parameter_null" {
                    if !e.nullable {
                        return Err(invalid(format!(
                            "{} is not nullable: its default is not null",
                            key(&path)
                        )));
                    }
                    Glow::Null
                } else {
                    parameter_value(e, p.get("value").unwrap_or(&Value::Null))
                        .map_err(|m| invalid(format!("{}: {m}", key(&path))))?
                };
                let root = self.addressed(&path, |id| {
                    Element::Parameter(glow::Parameter {
                        id,
                        contents: Some(glow::ParameterContents {
                            value: Some(value),
                            ..Default::default()
                        }),
                        children: None,
                    })
                });
                self.send(cx, &root);
                self.sets.push(Waiting { id, path, deadline });
                self.arm_reply_timer(cx);
            }
            "subscribe" | "unsubscribe" => {
                let path = self.resolve(p)?;
                if path.is_empty() {
                    return Err(invalid("give the path of an element"));
                }
                let e = self
                    .tree
                    .get(&path)
                    .ok_or_else(|| invalid(format!("no element at {} is known", key(&path))))?;
                let name_key = e.id_path.clone().unwrap_or_else(|| key(&path));
                let number = if name == "subscribe" {
                    self.subscriptions.insert(name_key);
                    self.subscribed_now.insert(path.clone());
                    glow::SUBSCRIBE
                } else {
                    self.subscriptions.remove(&name_key);
                    self.subscriptions.remove(&key(&path));
                    self.subscribed_now.remove(&path);
                    glow::UNSUBSCRIBE
                };
                let root = self.command_on(&path, glow::Command::new(number));
                self.send(cx, &root);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "matrix_connect" | "matrix_disconnect" | "matrix_absolute" => {
                let path = self.resolve(p)?;
                let e = self.element_of(&path, Kind::Matrix)?;
                let target = p
                    .get("target")
                    .and_then(Value::as_u64)
                    .and_then(|t| u32::try_from(t).ok())
                    .ok_or_else(|| invalid("'target' must be a signal number"))?;
                let sources = signal_list(p.get("sources"))?;
                let operation = match name {
                    "matrix_connect" => glow::OPERATION_CONNECT,
                    "matrix_disconnect" => glow::OPERATION_DISCONNECT,
                    _ => glow::OPERATION_ABSOLUTE,
                };
                check_signals(e, target, &sources, operation)
                    .map_err(|m| invalid(format!("{}: {m}", key(&path))))?;
                let root = self.addressed(&path, |id| {
                    Element::Matrix(glow::Matrix {
                        id,
                        contents: None,
                        children: None,
                        targets: None,
                        sources: None,
                        connections: Some(vec![glow::Connection {
                            target,
                            sources: Some(sources),
                            operation: Some(operation),
                            disposition: None,
                        }]),
                    })
                });
                self.send(cx, &root);
                self.connects.push(ConnectWait {
                    id,
                    path,
                    target,
                    deadline,
                });
                self.arm_reply_timer(cx);
            }
            "invoke_function" => {
                let path = self.resolve(p)?;
                let e = self.element_of(&path, Kind::Function)?;
                let given = match p.get("arguments") {
                    None => Vec::new(),
                    Some(Value::Array(a)) => a.clone(),
                    Some(_) => return Err(invalid("'arguments' must be an array")),
                };
                let arguments = function_arguments(e.arguments.as_deref(), &given)
                    .map_err(|m| invalid(format!("{}: {m}", key(&path))))?;
                let invocation = self.next_invocation;
                self.next_invocation = if invocation >= i64::from(i32::MAX) {
                    1
                } else {
                    invocation + 1
                };
                let root = self.addressed(&path, |id| {
                    bare(
                        Kind::Function,
                        id,
                        Some(vec![Element::Command(glow::Command {
                            number: glow::INVOKE,
                            dir_field_mask: None,
                            invocation: Some(glow::Invocation {
                                id: Some(invocation),
                                arguments: Some(arguments),
                            }),
                        })]),
                    )
                });
                self.send(cx, &root);
                self.invocations.insert(invocation, (id, deadline));
                self.arm_reply_timer(cx);
            }
            other => {
                return Err(CommandError::UnknownCommand {
                    command: other.into(),
                })
            }
        }
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

/// A requested value, typed by the parameter, an enum's entry given by
/// number or by name, and checked against its range.
fn parameter_value(e: &Elem, v: &Value) -> Result<Glow, String> {
    let kind = e.effective_type();
    let value = if kind == Some(glow::TYPE_ENUM) {
        match v {
            Value::String(name) => {
                if let Some((_, n)) = e.enum_map.iter().find(|(s, _)| s == name) {
                    Glow::Integer(*n)
                } else if let Some(i) = e.enumeration.iter().position(|s| s == name) {
                    Glow::Integer(i as i64)
                } else {
                    return Err(format!("'{name}' is not one of its enumeration entries"));
                }
            }
            _ => typed(kind, v)?,
        }
    } else {
        typed(kind, v)?
    };
    if kind != Some(glow::TYPE_ENUM) {
        if let Some(n) = numeric(&value).filter(|n| n.is_finite()) {
            if let Some(min) = e.minimum.as_ref().and_then(numeric) {
                if n < min {
                    return Err(format!("{n} is below its minimum {min}"));
                }
            }
            if let Some(max) = e.maximum.as_ref().and_then(numeric) {
                if n > max {
                    return Err(format!("{n} is above its maximum {max}"));
                }
            }
        }
    }
    Ok(value)
}

fn function_arguments(
    described: Option<&[glow::TupleItem]>,
    given: &[Value],
) -> Result<Vec<Glow>, String> {
    match described {
        Some(items) => {
            if items.len() != given.len() {
                return Err(format!(
                    "the function takes {} argument(s), {} given",
                    items.len(),
                    given.len()
                ));
            }
            items
                .iter()
                .zip(given)
                .map(|(item, v)| {
                    typed(Some(item.kind), v).map_err(|m| {
                        format!("argument '{}': {m}", item.name.as_deref().unwrap_or("?"))
                    })
                })
                .collect()
        }
        None => given.iter().map(|v| typed(None, v)).collect(),
    }
}

fn signal_list(v: Option<&Value>) -> Result<Vec<u32>, CommandError> {
    let list = match v {
        None => return Ok(Vec::new()),
        Some(Value::Array(a)) => a,
        Some(_) => return Err(invalid("'sources' must be an array of signal numbers")),
    };
    list.iter()
        .map(|s| {
            s.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| invalid("'sources' must be an array of signal numbers"))
        })
        .collect()
}

/// Signals within the matrix as it describes itself: below the counts of a
/// linear matrix, among the listed signals of a non-linear one, and one
/// source per target unless the matrix is N:N.
fn check_signals(e: &Elem, target: u32, sources: &[u32], operation: i64) -> Result<(), String> {
    let known = |listed: &Option<Vec<u32>>, count: Option<i64>, n: u32| -> bool {
        if e.addressing == 1 {
            listed.as_ref().is_none_or(|l| l.contains(&n))
        } else {
            count.is_none_or(|c| i64::from(n) < c)
        }
    };
    if !known(&e.targets, e.target_count, target) {
        return Err(format!(
            "target {target} is not one of the matrix's targets"
        ));
    }
    if let Some(s) = sources
        .iter()
        .find(|&&s| !known(&e.sources, e.source_count, s))
    {
        return Err(format!("source {s} is not one of the matrix's sources"));
    }
    if e.matrix_type != 2 && operation != glow::OPERATION_DISCONNECT && sources.len() > 1 {
        return Err(format!(
            "a {} matrix connects one source to a target",
            glow::name_of(glow::MATRIX_TYPES, e.matrix_type).unwrap_or("1:N")
        ));
    }
    Ok(())
}

impl Module for EmberPlus {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        if let Err(e) = self.run(cx, id, name, params) {
            cx.complete(id, Err(e));
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.socket_open = true;
                self.decoder = s101::Decoder::default();
                self.assembler = s101::Assembler::default();
                self.last_heard = cx.now();
                self.keepalive_sent = None;
                // Numbers are only valid for a session: walk afresh.
                self.start_walk(cx);
                cx.set_timer(KEEPALIVE, self.settings.keepalive_interval);
            }
            TcpInput::Data(data) => self.data(cx, &data),
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, timer: Key) {
        let now = cx.now();
        match timer {
            RETRY => {
                if !self.socket_open {
                    self.open(cx);
                }
            }
            KEEPALIVE => {
                if !self.socket_open {
                    return;
                }
                if let Some(sent) = self.keepalive_sent {
                    if now.saturating_sub(sent) >= self.settings.keepalive_timeout {
                        self.lost(cx, "no answer to an S101 keep-alive request".into());
                        return;
                    }
                    cx.set_timer(
                        KEEPALIVE,
                        self.settings.keepalive_timeout - now.saturating_sub(sent),
                    );
                    return;
                }
                let quiet = now.saturating_sub(self.last_heard);
                if quiet >= self.settings.keepalive_interval {
                    cx.tcp_send(SOCKET, s101::keepalive_request());
                    self.keepalive_sent = Some(now);
                    cx.set_timer(KEEPALIVE, self.settings.keepalive_timeout);
                } else {
                    cx.set_timer(KEEPALIVE, self.settings.keepalive_interval - quiet);
                }
            }
            REPLY => {
                let expired: Vec<Vec<u32>> = self
                    .dir_outstanding
                    .iter()
                    .filter(|(_, &d)| d <= now)
                    .map(|(p, _)| p.clone())
                    .collect();
                for path in expired {
                    self.dir_outstanding.remove(&path);
                    if path.is_empty() && !self.connected {
                        self.lost(
                            cx,
                            "the provider accepted the connection but sent nothing; check the port is its Ember+ port".into(),
                        );
                        return;
                    }
                    let mut i = 0;
                    while i < self.dir_waiters.len() {
                        if self.dir_waiters[i].0 == path {
                            let (_, id) = self.dir_waiters.remove(i);
                            cx.complete(id, Err(CommandError::Timeout));
                        } else {
                            i += 1;
                        }
                    }
                    cx.log(
                        Level::Debug,
                        format!("no answer to GetDirectory on '{}'", key(&path)),
                    );
                }
                let (late, sets): (Vec<_>, Vec<_>) = std::mem::take(&mut self.sets)
                    .into_iter()
                    .partition(|w| w.deadline <= now);
                self.sets = sets;
                for w in late {
                    cx.complete(w.id, Err(CommandError::Timeout));
                }
                let (late, connects): (Vec<_>, Vec<_>) = std::mem::take(&mut self.connects)
                    .into_iter()
                    .partition(|w| w.deadline <= now);
                self.connects = connects;
                for w in late {
                    cx.complete(w.id, Err(CommandError::Timeout));
                }
                let late: Vec<i64> = self
                    .invocations
                    .iter()
                    .filter(|(_, (_, d))| *d <= now)
                    .map(|(k, _)| *k)
                    .collect();
                for k in late {
                    if let Some((id, _)) = self.invocations.remove(&k) {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                }
                let mut p = Patch::default();
                self.pump(cx, &mut p);
                p.flush(cx);
                self.arm_reply_timer(cx);
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
    use super::glow::{
        decode, encode, Command, Function, FunctionContents, InvocationResult, Matrix,
        MatrixContents, Node, NodeContents, Parameter, ParameterContents, StreamDescription,
        StreamEntry, TupleItem, GET_DIRECTORY, INVOKE, OPERATION_CONNECT, SUBSCRIBE, TYPE_INTEGER,
        TYPE_REAL,
    };
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn module(settings: Value) -> EmberPlus {
        EmberPlus::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 40)),
            port: None,
            model: "provider".into(),
            channels: None,
            settings: params(settings),
        })
    }

    /// The Glow roots the module sent.
    fn sent(actions: &[Action]) -> Vec<Root> {
        let mut d = s101::Decoder::default();
        let mut asm = s101::Assembler::default();
        let mut out = Vec::new();
        for a in actions {
            if let Action::TcpSend { data, .. } = a {
                for f in d.feed(data).0 {
                    if let s101::Message::Ember { flags, payload } =
                        s101::parse_message(&f).unwrap()
                    {
                        if let Some(m) = asm.push(flags, payload).unwrap() {
                            out.extend(decode(&m).unwrap());
                        }
                    }
                }
            }
        }
        out
    }

    fn state(into: &mut Value, actions: &[Action]) {
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(into, p);
            }
        }
    }

    fn feed(m: &mut EmberPlus, now: Millis, root: Root) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(s101::ember_frames(&encode(&root))),
        );
        cx.take()
    }

    fn run(m: &mut EmberPlus, now: Millis, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, &params(p));
        cx.take()
    }

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    use crate::module::CommandResult;

    fn get_dir(mask: i64) -> Element {
        Element::Command(Command {
            number: GET_DIRECTORY,
            dir_field_mask: Some(mask),
            invocation: None,
        })
    }

    fn node(id: Id, identifier: Option<&str>, children: Option<Vec<Element>>) -> Element {
        Element::Node(Node {
            id,
            contents: identifier.map(|i| NodeContents {
                identifier: Some(i.into()),
                ..Default::default()
            }),
            children,
        })
    }

    fn param(number: u32, contents: ParameterContents) -> Element {
        Element::Parameter(Parameter {
            id: Id::Number(number),
            contents: Some(contents),
            children: None,
        })
    }

    /// Connected, the root answered with node 1 "Device" and node 1 answered
    /// with a gain, an enum, a stream meter, a router and a function.
    fn walked(settings: Value) -> (EmberPlus, Value) {
        let mut m = module(settings.clone());
        let mut st = json!({});
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = cx.take();
        assert!(a.contains(&Action::TcpOpen {
            socket: SOCKET,
            to: "10.0.0.40:9000".parse().unwrap()
        }));
        assert_eq!(sent(&a), vec![Root::Elements(vec![get_dir(-1)])]);

        let a = feed(
            &mut m,
            10,
            Root::Elements(vec![node(Id::Number(1), Some("Device"), None)]),
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        state(&mut st, &a);
        // Node 1 is asked for next.
        let qualified = settings.get("qualified_requests") != Some(&json!(false));
        let out = sent(&a);
        assert_eq!(out.len(), 1);
        assert!(
            !qualified
                || out
                    == vec![Root::Elements(vec![Element::Node(Node {
                        id: Id::Path(vec![1]),
                        contents: None,
                        children: Some(vec![get_dir(-1)])
                    })])]
        );
        let children = vec![
            param(
                1,
                ParameterContents {
                    identifier: Some("gain".into()),
                    value: Some(Glow::Real(-10.0)),
                    minimum: Some(Glow::Real(-128.0)),
                    maximum: Some(Glow::Real(15.0)),
                    access: Some(3),
                    ..Default::default()
                },
            ),
            param(
                2,
                ParameterContents {
                    identifier: Some("mode".into()),
                    value: Some(Glow::Integer(0)),
                    enumeration: Some("Off\nOn\n~Hidden".into()),
                    access: Some(3),
                    ..Default::default()
                },
            ),
            param(
                3,
                ParameterContents {
                    identifier: Some("level".into()),
                    kind: Some(TYPE_REAL),
                    stream_identifier: Some(7),
                    stream_descriptor: Some(StreamDescription {
                        format: 21,
                        offset: 4,
                    }),
                    ..Default::default()
                },
            ),
            Element::Matrix(Matrix {
                id: Id::Number(4),
                contents: Some(MatrixContents {
                    identifier: Some("router".into()),
                    target_count: Some(4),
                    source_count: Some(4),
                    ..Default::default()
                }),
                children: None,
                targets: None,
                sources: None,
                connections: None,
            }),
            Element::Function(Function {
                id: Id::Number(5),
                contents: Some(FunctionContents {
                    identifier: Some("add".into()),
                    description: None,
                    arguments: Some(vec![
                        TupleItem {
                            kind: TYPE_INTEGER,
                            name: Some("a".into()),
                        },
                        TupleItem {
                            kind: TYPE_INTEGER,
                            name: Some("b".into()),
                        },
                    ]),
                    result: Some(vec![TupleItem {
                        kind: TYPE_INTEGER,
                        name: Some("sum".into()),
                    }]),
                    template_reference: None,
                }),
                children: None,
            }),
        ];
        let a = feed(
            &mut m,
            20,
            Root::Elements(vec![node(Id::Number(1), None, Some(children))]),
        );
        state(&mut st, &a);
        // The router's connections are asked for.
        assert!(
            !qualified
                || sent(&a)
                    == vec![Root::Elements(vec![Element::Matrix(Matrix {
                        id: Id::Path(vec![1, 4]),
                        contents: None,
                        children: Some(vec![get_dir(-1)]),
                        targets: None,
                        sources: None,
                        connections: None
                    })])]
        );
        let a = feed(
            &mut m,
            30,
            Root::Elements(vec![Element::Matrix(Matrix {
                id: Id::Path(vec![1, 4]),
                contents: None,
                children: None,
                targets: None,
                sources: None,
                connections: Some(vec![glow::Connection {
                    target: 0,
                    sources: Some(vec![1]),
                    operation: None,
                    disposition: None,
                }]),
            })]),
        );
        state(&mut st, &a);
        assert_eq!(st["walk"]["complete"], true);
        (m, st)
    }

    #[test]
    fn walks_the_tree_into_state() {
        let (_, st) = walked(json!({}));
        assert_eq!(st["elements"]["1"]["identifier"], "Device");
        assert_eq!(st["elements"]["1.1"]["value"], -10.0);
        assert_eq!(st["elements"]["1.1"]["access"], "read_write");
        assert_eq!(st["elements"]["1.1"]["identifier_path"], "Device/gain");
        assert_eq!(
            st["elements"]["1.2"]["enumeration"],
            json!(["Off", "On", "~Hidden"])
        );
        assert_eq!(st["elements"]["1.3"]["parameter_type"], "real");
        assert_eq!(
            st["elements"]["1.3"]["stream_descriptor"],
            json!({"format": "ieee_float32_little_endian", "offset": 4})
        );
        assert_eq!(st["elements"]["1.4"]["type"], "matrix");
        assert_eq!(
            st["elements"]["1.4"]["connections"]["0"],
            json!({"sources": [1], "disposition": "tally"})
        );
        assert_eq!(
            st["elements"]["1.5"]["arguments"],
            json!([{"type": "integer", "name": "a"}, {"type": "integer", "name": "b"}])
        );
        assert_eq!(st["identifiers"]["Device/router"], "1.4");
    }

    #[test]
    fn set_parameter_completes_with_the_providers_value() {
        let (mut m, _) = walked(json!({}));
        let a = run(
            &mut m,
            100,
            1,
            "set_parameter",
            json!({"path": "Device/gain", "value": -6.5}),
        );
        assert_eq!(
            sent(&a),
            vec![Root::Elements(vec![Element::Parameter(Parameter {
                id: Id::Path(vec![1, 1]),
                contents: Some(ParameterContents {
                    value: Some(Glow::Real(-6.5)),
                    ..Default::default()
                }),
                children: None
            })])]
        );
        let a = feed(
            &mut m,
            110,
            Root::Elements(vec![Element::Parameter(Parameter {
                id: Id::Path(vec![1, 1]),
                contents: Some(ParameterContents {
                    value: Some(Glow::Real(-6.5)),
                    ..Default::default()
                }),
                children: None,
            })]),
        );
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(-6.5) }))
        );
        // Out of range, of the wrong type, an enum by name, unknown.
        let a = run(
            &mut m,
            120,
            2,
            "set_parameter",
            json!({"path": "1.1", "value": 20.0}),
        );
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(
            &mut m,
            120,
            3,
            "set_parameter",
            json!({"path": "1.1", "value": "loud"}),
        );
        assert!(matches!(
            completed(&a, 3),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(
            &mut m,
            120,
            4,
            "set_parameter",
            json!({"path": "Device/mode", "value": "On"}),
        );
        assert_eq!(completed(&a, 4), None);
        let Root::Elements(e) = &sent(&a)[0] else {
            panic!()
        };
        let Element::Parameter(p) = &e[0] else {
            panic!()
        };
        assert_eq!(p.contents.as_ref().unwrap().value, Some(Glow::Integer(1)));
        let a = run(
            &mut m,
            120,
            5,
            "set_parameter",
            json!({"path": "Device/nope", "value": 1}),
        );
        assert!(matches!(
            completed(&a, 5),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        // No answer: a timeout.
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, REPLY);
        assert_eq!(completed(&cx.take(), 4), Some(Err(CommandError::Timeout)));
    }

    #[test]
    fn nested_requests_when_qualified_ones_are_off() {
        let (mut m, _) = walked(json!({"qualified_requests": false}));
        let a = run(
            &mut m,
            100,
            1,
            "set_parameter",
            json!({"path": "1.1", "value": 1.0}),
        );
        assert_eq!(
            sent(&a),
            vec![Root::Elements(vec![Element::Node(Node {
                id: Id::Number(1),
                contents: None,
                children: Some(vec![Element::Parameter(Parameter {
                    id: Id::Number(1),
                    contents: Some(ParameterContents {
                        value: Some(Glow::Real(1.0)),
                        ..Default::default()
                    }),
                    children: None
                })])
            })])]
        );
    }

    #[test]
    fn matrix_operations_complete_from_the_reported_connection() {
        let (mut m, _) = walked(json!({}));
        let a = run(
            &mut m,
            100,
            1,
            "matrix_connect",
            json!({"path": "Device/router", "target": 2, "sources": [3]}),
        );
        let Root::Elements(e) = &sent(&a)[0] else {
            panic!()
        };
        let Element::Matrix(x) = &e[0] else { panic!() };
        assert_eq!(x.id, Id::Path(vec![1, 4]));
        assert_eq!(
            x.connections,
            Some(vec![glow::Connection {
                target: 2,
                sources: Some(vec![3]),
                operation: Some(OPERATION_CONNECT),
                disposition: None
            }])
        );
        let reply = |disposition| {
            Root::Elements(vec![Element::Matrix(Matrix {
                id: Id::Path(vec![1, 4]),
                contents: None,
                children: None,
                targets: None,
                sources: None,
                connections: Some(vec![glow::Connection {
                    target: 2,
                    sources: Some(vec![3]),
                    operation: None,
                    disposition: Some(disposition),
                }]),
            })])
        };
        let a = feed(&mut m, 110, reply(1));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!({"target": 2, "sources": [3], "disposition": "modified"})
            }))
        );
        run(
            &mut m,
            120,
            2,
            "matrix_absolute",
            json!({"path": "1.4", "target": 2, "sources": [0]}),
        );
        let a = feed(&mut m, 130, reply(3));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
        // Outside a linear 4x4 matrix, or two sources on a 1:N one.
        let a = run(
            &mut m,
            140,
            3,
            "matrix_connect",
            json!({"path": "1.4", "target": 4, "sources": [0]}),
        );
        assert!(matches!(
            completed(&a, 3),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(
            &mut m,
            140,
            4,
            "matrix_absolute",
            json!({"path": "1.4", "target": 0, "sources": [0, 1]}),
        );
        assert!(matches!(
            completed(&a, 4),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
    }

    #[test]
    fn subscribed_streams_update_values() {
        let (mut m, mut st) = walked(json!({}));
        let a = run(&mut m, 100, 1, "subscribe", json!({"path": "Device/level"}));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Unverified)));
        assert_eq!(
            sent(&a),
            vec![Root::Elements(vec![Element::Parameter(Parameter {
                id: Id::Path(vec![1, 3]),
                contents: None,
                children: Some(vec![Element::Command(Command::new(SUBSCRIBE))])
            })])]
        );
        // A little-endian float at offset 4 of the stream's octets.
        let mut octets = vec![0xAA; 4];
        octets.extend((-20.5f32).to_le_bytes());
        let a = feed(
            &mut m,
            110,
            Root::Streams(vec![StreamEntry {
                identifier: 7,
                value: Glow::Octets(octets),
            }]),
        );
        state(&mut st, &a);
        assert_eq!(st["elements"]["1.3"]["value"], -20.5);
    }

    #[test]
    fn functions_are_invoked() {
        let (mut m, _) = walked(json!({}));
        let a = run(
            &mut m,
            100,
            1,
            "invoke_function",
            json!({"path": "Device/add", "arguments": [2, 3]}),
        );
        let Root::Elements(e) = &sent(&a)[0] else {
            panic!()
        };
        let Element::Function(f) = &e[0] else {
            panic!()
        };
        let Some(Element::Command(c)) = f.children.as_ref().and_then(|c| c.first()) else {
            panic!()
        };
        assert_eq!(c.number, INVOKE);
        let invocation = c.invocation.clone().unwrap();
        assert_eq!(
            invocation.arguments,
            Some(vec![Glow::Integer(2), Glow::Integer(3)])
        );
        let a = feed(
            &mut m,
            110,
            Root::InvocationResult(InvocationResult {
                invocation_id: invocation.id.unwrap(),
                success: Some(true),
                result: Some(vec![Glow::Integer(5)]),
            }),
        );
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!([5]) }))
        );
        let a = run(
            &mut m,
            120,
            2,
            "invoke_function",
            json!({"path": "Device/add", "arguments": [2]}),
        );
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
    }

    #[test]
    fn keepalives_are_answered_and_sent_when_quiet() {
        let (mut m, _) = walked(json!({}));
        let mut cx = Cx::new(100);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(s101::keepalive_request()));
        let a = cx.take();
        assert!(a.contains(&Action::TcpSend {
            socket: SOCKET,
            data: s101::keepalive_response()
        }));
        let mut cx = Cx::new(5_200);
        m.timer(&mut cx, KEEPALIVE);
        assert!(cx.take().contains(&Action::TcpSend {
            socket: SOCKET,
            data: s101::keepalive_request()
        }));
        let mut cx = Cx::new(10_300);
        m.timer(&mut cx, KEEPALIVE);
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::TcpClose { socket: SOCKET }));
    }

    #[test]
    fn a_reconnection_walks_again_and_restores_subscriptions() {
        let (mut m, _) = walked(json!({}));
        run(&mut m, 100, 1, "subscribe", json!({"path": "1.3"}));
        let mut cx = Cx::new(200);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        m.timer(&mut cx, RETRY);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = cx.take();
        let mut st = json!({"elements": {"1": {}}});
        state(&mut st, &a);
        assert_eq!(st["elements"], Value::Null);
        assert_eq!(sent(&a), vec![Root::Elements(vec![get_dir(-1)])]);
        // The level parameter is now 2.9: found by its identifier path.
        let a = feed(
            &mut m,
            300,
            Root::Elements(vec![node(
                Id::Number(2),
                Some("Device"),
                Some(vec![param(
                    9,
                    ParameterContents {
                        identifier: Some("level".into()),
                        stream_identifier: Some(8),
                        ..Default::default()
                    },
                )]),
            )]),
        );
        assert_eq!(
            sent(&a),
            vec![Root::Elements(vec![Element::Parameter(Parameter {
                id: Id::Path(vec![2, 9]),
                contents: None,
                children: Some(vec![Element::Command(Command::new(SUBSCRIBE))])
            })])]
        );
    }

    #[test]
    fn an_empty_node_answers_its_directory_and_offline_nodes_are_asked_again() {
        let (mut m, mut st) = walked(json!({}));
        let a = run(
            &mut m,
            100,
            1,
            "get_directory",
            json!({"path": "Device/router"}),
        );
        assert_eq!(completed(&a, 1), None);
        let a = feed(
            &mut m,
            110,
            Root::Elements(vec![bare(Kind::Matrix, Id::Path(vec![1, 4]), None)]),
        );
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        let off = |online| {
            Root::Elements(vec![Element::Node(Node {
                id: Id::Path(vec![1]),
                contents: Some(NodeContents {
                    is_online: Some(online),
                    ..Default::default()
                }),
                children: None,
            })])
        };
        let a = feed(&mut m, 120, off(false));
        state(&mut st, &a);
        assert_eq!(st["elements"]["1"]["is_online"], false);
        let a = feed(&mut m, 130, off(true));
        assert_eq!(sent(&a).len(), 1);
    }

    #[test]
    fn stream_formats() {
        let d = |format, offset| StreamDescription { format, offset };
        let b = [0x01, 0xFF, 0xFE, 0x00, 0x00, 0x00, 0x00, 0x80];
        assert_eq!(stream_slice(&b, &d(0, 1)), Some(Glow::Integer(255)));
        assert_eq!(stream_slice(&b, &d(8, 1)), Some(Glow::Integer(-1)));
        assert_eq!(stream_slice(&b, &d(10, 1)), Some(Glow::Integer(-2)));
        assert_eq!(stream_slice(&b, &d(3, 0)), Some(Glow::Integer(0xFF01)));
        assert_eq!(
            stream_slice(&b, &d(13, 4)),
            Some(Glow::Integer(i64::from(i32::MIN)))
        );
        assert_eq!(stream_slice(&b, &d(22, 1)), None);
        assert_eq!(stream_slice(&b, &d(1, 0)), None);
        assert_eq!(
            stream_slice(&1.5f64.to_be_bytes(), &d(22, 0)),
            Some(Glow::Real(1.5))
        );
    }

    #[test]
    fn commands_wait_for_the_provider() {
        let mut m = module(json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = run(&mut m, 1, 1, "refresh", json!({}));
        assert_eq!(completed(&a, 1), Some(Err(CommandError::NotConnected)));
        // Silence: the root directory times out and the connection is dropped.
        let mut cx = Cx::new(10_000);
        m.timer(&mut cx, REPLY);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
    }
}
