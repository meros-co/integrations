//! A vendor's device over SNMP, from a profile of its MIB: the subtrees to
//! walk, how each object becomes state, the objects commands write, and the
//! names of its notifications. The protocol is modules/snmp.rs's (v1, v2c
//! and v3 with the USM) and the settings are generic-snmp's.
//!
//! - Every poll walks the profile's subtrees for the device's model (GETBULK
//!   on v2c and v3, GETNEXT on v1), so state holds whatever of them the
//!   agent implements and nothing it does not. Each object found is named
//!   by the profile: a scalar at its path, a table column at its path with
//!   the row's index in place of each `*`. Enumerations become their MIB
//!   labels (in snake case), octet strings text (a MAC address in hex),
//!   TimeTicks hundredths of a second. Only values that changed are
//!   reported; a table row the agent no longer lists leaves state.
//! - The system group (RFC 3418) is read on opening; sysUpTime.0 is read
//!   with every poll and is the liveness check, as for generic-snmp.
//! - A command named by the profile SETs one object, with the row given by
//!   its index parameters, then reads its table or group again, so state
//!   shows what the agent did with it (a control object usually reads back
//!   as the state it caused, not the action written). `get`, `walk` and
//!   `set` reach any object by OID, and `refresh` polls at once.
//! - With `trap_port`, traps and informs from the agent become message
//!   events named by the profile where it knows them, are kept in
//!   `traps.last`, and start a poll, since most say a value changed.
//! - Opened for commands only, nothing is walked: sysUpTime.0 is still read
//!   each interval as the liveness check, and notifications are received.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use serde_json::{json, Map, Value as Json};

use super::snmp::{
    self, command_error, parse_oids, varbind_json, Client, Notification, Reply, Request, Security,
    SnmpError, User, VarBind,
};
use super::snmp_ber::{hex, Oid, Value};
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    PollStamps,
};

const SOCKET: Key = "snmp";
const TIMER: Key = "snmp_retry";
const TRAPS: Key = "traps";
const TRAP_RETRY: Key = "trap_retry";
const POLL: Key = "poll";
/// A poll soon after a notification, which usually says a value changed.
const AFTER_TRAP: Millis = 500;
const RETRY_MIN: Millis = 1000;
const RETRY_MAX: Millis = 30_000;

/// How an object's value becomes state. A build with one profile may use
/// only some of the kinds.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum Kind {
    /// A number (INTEGER, Gauge32, Unsigned32, Counter32, Counter64).
    Int,
    /// TimeTicks: hundredths of a second.
    Ticks,
    /// An OCTET STRING as text.
    Text,
    /// An OCTET STRING of address bytes, in lowercase hex separated by colons.
    Mac,
    /// An IpAddress in dotted decimal.
    Ip,
    /// An OBJECT IDENTIFIER in dotted decimal.
    Oid,
    /// An enumerated INTEGER: each number's label (the MIB's, in snake case).
    Enum(&'static [(i64, &'static str)]),
    /// An OBJECT IDENTIFIER naming one of a set (RFC 1628's tests): each
    /// OID's label.
    Choice(&'static [(&'static [u32], &'static str)]),
}

/// The SNMP type a SET of an object carries, as its MIB's SYNTAX says (a
/// build with one profile may use only some of them).
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub(crate) enum Wire {
    Integer,
    Gauge32,
    TimeTicks,
    OctetString,
    IpAddress,
    ObjectIdentifier,
}

/// A readable object: a scalar (its value at `oid`.0) or a table column
/// (its values at `oid`.index), named in state by `path`, which holds one
/// `*` for each sub-identifier of a row's index.
pub(crate) struct Object {
    pub(crate) oid: &'static [u32],
    pub(crate) path: &'static str,
    pub(crate) kind: Kind,
}

/// A command writing one object: `indexes` names its index parameters in
/// order (none for a scalar), the value is the `value` parameter, and
/// `reread` is the subtree read again after the SET. A `lock` is an
/// advisory lock (TestAndIncr, RFC 2579) read first and written with the
/// object in one SET, as RFC 1628 asks of a test.
pub(crate) struct Setter {
    pub(crate) command: &'static str,
    pub(crate) oid: &'static [u32],
    pub(crate) kind: Kind,
    pub(crate) wire: Wire,
    pub(crate) indexes: &'static [&'static str],
    pub(crate) reread: &'static [u32],
    pub(crate) lock: &'static [u32],
}

/// A subtree walked each poll, for the models named (every model when none).
pub(crate) struct Root {
    pub(crate) oid: &'static [u32],
    pub(crate) models: &'static [&'static str],
}

pub(crate) struct Profile {
    pub(crate) roots: &'static [Root],
    pub(crate) objects: &'static [Object],
    pub(crate) setters: &'static [Setter],
    /// Notification OIDs (snmpTrapOID, a v1 trap's as RFC 3584 derives
    /// it) and their MIB names.
    pub(crate) traps: &'static [(&'static [u32], &'static str)],
}

fn system_group() -> Vec<(&'static str, Oid)> {
    [
        ("description", 1),
        ("object_id", 2),
        ("uptime", 3),
        ("contact", 4),
        ("name", 5),
        ("location", 6),
    ]
    .into_iter()
    .map(|(n, i)| (n, Oid(vec![1, 3, 6, 1, 2, 1, 1, i, 0])))
    .collect()
}

fn sys_up_time() -> Oid {
    Oid(vec![1, 3, 6, 1, 2, 1, 1, 3, 0])
}

/// An object's value as state, or None when it has none (an exception).
fn decode(kind: Kind, value: &Value) -> Option<Json> {
    let number = match value {
        Value::Integer(n) => Some(*n),
        Value::Gauge32(n) | Value::Counter32(n) | Value::TimeTicks(n) => Some(i64::from(*n)),
        Value::Counter64(n) => return matches!(kind, Kind::Int | Kind::Ticks).then(|| json!(n)),
        _ => None,
    };
    match (kind, value) {
        (Kind::Enum(labels), _) => {
            let n = number?;
            Some(match labels.iter().find(|(v, _)| *v == n) {
                Some((_, label)) => json!(label),
                // A value the MIB does not list: its number, as text.
                None => json!(n.to_string()),
            })
        }
        (Kind::Choice(choices), Value::Oid(o)) => {
            Some(match choices.iter().find(|(c, _)| *c == o.0.as_slice()) {
                Some((_, label)) => json!(label),
                None => json!(o.to_string()),
            })
        }
        (Kind::Int | Kind::Ticks, _) => number.map(|n| json!(n)),
        (Kind::Mac, Value::OctetString(b)) => Some(json!(b
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<Vec<_>>()
            .join(":"))),
        (Kind::Text, Value::OctetString(b)) => {
            let text = String::from_utf8_lossy(b);
            // Agents pad some strings with NULs.
            Some(json!(text.trim_end_matches('\0')))
        }
        (Kind::Ip, Value::IpAddress(a)) => Some(json!(std::net::Ipv4Addr::from(*a).to_string())),
        (Kind::Oid, Value::Oid(o)) => Some(json!(o.to_string())),
        (Kind::Text | Kind::Mac | Kind::Ip | Kind::Oid, v) if !v.is_exception() => {
            match v.to_json() {
                Json::Null => None,
                Json::String(s) => Some(json!(s)),
                other => Some(json!(other.to_string())),
            }
        }
        _ => None,
    }
}

/// Set `path` (dotted) to `value` in a nested patch.
fn insert(patch: &mut Map<String, Json>, path: &str, value: Json) {
    let mut parts = path.split('.').peekable();
    let mut map = patch;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            map.insert(part.to_string(), value);
            return;
        }
        let entry = map
            .entry(part.to_string())
            .or_insert_with(|| Json::Object(Map::new()));
        if !entry.is_object() {
            *entry = Json::Object(Map::new());
        }
        map = entry.as_object_mut().expect("an object");
    }
}

/// What a request was for.
enum Purpose {
    /// A poll's walk of one root.
    Walk(usize),
    /// A walk after a SET, of the subtree it changed.
    Reread,
    System,
    Liveness,
    /// A raw command, answered with its variable bindings.
    Command(CommandId),
    /// A profile command's SET, then a read of `reread`.
    Set(CommandId, Oid),
    /// A read of a setter's lock, before the SET it goes with.
    Lock(CommandId, Oid, VarBind, Oid),
}

pub(crate) struct SnmpDevice {
    profile: &'static Profile,
    /// Indexes of the roots walked for the model.
    roots: Vec<usize>,
    /// An object's OID to its index in the profile.
    lookup: HashMap<&'static [u32], usize>,
    agent: Option<SocketAddr>,
    security: Option<Security>,
    timeout: Millis,
    retries: u32,
    max_repetitions: u32,
    interval: Millis,
    monitor: bool,
    /// When each poll was last answered (`polled.status`, SPEC.md §8).
    stamps: PollStamps,
    trap: Option<(Option<std::net::IpAddr>, u16)>,
    trap_community: Vec<u8>,
    trap_user: Option<User>,
    local_engine_id: Vec<u8>,
    seed: u64,
    client: Option<Client>,
    receiver: Option<snmp::Receiver>,
    next_tag: u64,
    purposes: HashMap<u64, Purpose>,
    connected: bool,
    system_read: bool,
    trap_retry: Millis,
    trap_count: u64,
    /// Every value reported, by dotted path.
    values: HashMap<String, Json>,
    /// The table rows each root's last walk found, as their paths.
    rows: HashMap<usize, HashSet<String>>,
    /// `refresh` commands waiting for the reads they started.
    refreshing: Vec<(CommandId, Vec<u64>)>,
}

impl SnmpDevice {
    pub(crate) fn new(profile: &'static Profile, ctx: OpenContext) -> Result<SnmpDevice, String> {
        let s = snmp::Settings::parse(&ctx, 10_000)?;
        let roots = profile
            .roots
            .iter()
            .enumerate()
            .filter(|(_, r)| r.models.is_empty() || r.models.contains(&ctx.model.as_str()))
            .map(|(i, _)| i)
            .collect();
        let lookup = profile
            .objects
            .iter()
            .enumerate()
            .map(|(i, o)| (o.oid, i))
            .collect();
        Ok(SnmpDevice {
            profile,
            roots,
            lookup,
            agent: s.agent,
            security: Some(s.security),
            timeout: s.timeout,
            retries: s.retries,
            max_repetitions: s.max_repetitions,
            interval: s.interval,
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            trap: s.trap,
            trap_community: s.trap_community,
            trap_user: s.user,
            local_engine_id: s.local_engine_id,
            seed: s.seed,
            client: None,
            receiver: None,
            next_tag: 1,
            purposes: HashMap::new(),
            connected: false,
            system_read: false,
            trap_retry: RETRY_MIN,
            trap_count: 0,
            values: HashMap::new(),
            rows: HashMap::new(),
            refreshing: Vec::new(),
        })
    }

    fn send(&mut self, cx: &mut Cx, purpose: Purpose, request: Request) -> u64 {
        let tag = self.next_tag;
        self.next_tag += 1;
        self.purposes.insert(tag, purpose);
        let replies = match &mut self.client {
            Some(c) => c.send(cx, tag, request),
            None => Vec::new(),
        };
        self.replies(cx, replies);
        tag
    }

    fn walk(&self, root: &[u32]) -> Request {
        Request::Walk {
            root: Oid(root.to_vec()),
            bulk: Some(self.max_repetitions),
            limit: None,
        }
    }

    /// Read what state keeps; returns the request tags.
    fn poll(&mut self, cx: &mut Cx) -> Vec<u64> {
        let mut tags = Vec::new();
        if self.monitor && !self.system_read {
            let oids = system_group().into_iter().map(|(_, o)| o).collect();
            tags.push(self.send(cx, Purpose::System, Request::Get(oids)));
        } else {
            tags.push(self.send(cx, Purpose::Liveness, Request::Get(vec![sys_up_time()])));
        }
        if self.monitor {
            for i in self.roots.clone() {
                let request = self.walk(self.profile.roots[i].oid);
                tags.push(self.send(cx, Purpose::Walk(i), request));
            }
        }
        tags
    }

    fn polling(&self) -> bool {
        self.purposes
            .values()
            .any(|p| matches!(p, Purpose::Walk(_) | Purpose::System | Purpose::Liveness))
    }

    /// The object a variable binding is an instance of, and its path with
    /// the row's index filled in.
    fn name(&self, oid: &Oid) -> Option<(usize, String)> {
        let sub = &oid.0;
        for k in 1..=sub.len().min(8) {
            let Some(&i) = self.lookup.get(&sub[..sub.len() - k]) else {
                continue;
            };
            let object = &self.profile.objects[i];
            let index = &sub[sub.len() - k..];
            let stars = object.path.matches('*').count();
            if stars == 0 {
                return (index == [0]).then(|| (i, object.path.to_string()));
            }
            if stars != k {
                return None;
            }
            let mut path = String::with_capacity(object.path.len() + 8);
            let mut it = index.iter();
            for (j, part) in object.path.split('*').enumerate() {
                if j > 0 {
                    path.push_str(&it.next()?.to_string());
                }
                path.push_str(part);
            }
            return Some((i, path));
        }
        None
    }

    /// Report what changed among these variable bindings; returns the
    /// table rows they name.
    fn apply(&mut self, cx: &mut Cx, varbinds: &[VarBind]) -> HashSet<String> {
        let mut patch = Map::new();
        let mut rows = HashSet::new();
        for vb in varbinds {
            let Some((i, path)) = self.name(&vb.oid) else {
                continue;
            };
            let object = &self.profile.objects[i];
            if object.path.contains('*') {
                if let Some(end) = path.rfind('.') {
                    rows.insert(path[..end].to_string());
                }
            }
            let Some(value) = decode(object.kind, &vb.value) else {
                continue;
            };
            if self.values.get(&path) != Some(&value) {
                self.values.insert(path.clone(), value.clone());
                insert(&mut patch, &path, value);
            }
        }
        if !patch.is_empty() {
            cx.state(Json::Object(patch));
        }
        rows
    }

    /// A root's walk ended: rows it no longer lists leave state.
    fn walked(&mut self, cx: &mut Cx, root: usize, rows: HashSet<String>) {
        let before = self.rows.insert(root, rows).unwrap_or_default();
        let now = &self.rows[&root];
        let mut patch = Map::new();
        for gone in before.iter().filter(|r| !now.contains(*r)) {
            let prefix = format!("{gone}.");
            self.values.retain(|k, _| !k.starts_with(&prefix));
            insert(&mut patch, gone, Json::Null);
        }
        if !patch.is_empty() {
            cx.state(Json::Object(patch));
        }
    }

    fn up(&mut self, cx: &mut Cx, round_trip: Option<Millis>) {
        cx.alive();
        if let Some(rt) = round_trip {
            cx.round_trip(rt);
        }
        if !self.connected {
            self.connected = true;
            cx.connection(Connection::Connected);
        }
        if let Some((id, boots, time)) = self.client.as_ref().and_then(|c| c.engine(cx.now())) {
            let engine = json!({ "id": hex(&id), "boots": boots, "time": time });
            if self.values.get("engine") != Some(&engine) {
                self.values.insert("engine".into(), engine.clone());
                cx.state(json!({ "engine": engine }));
            }
        }
    }

    fn down(&mut self, cx: &mut Cx, e: &SnmpError) {
        self.connected = false;
        self.system_read = false;
        match e {
            SnmpError::Auth(m) => cx.connection(Connection::Unauthorized { reason: m.clone() }),
            other => cx.connection(Connection::Disconnected {
                reason: other.to_string(),
            }),
        }
    }

    fn replies(&mut self, cx: &mut Cx, replies: Vec<Reply>) {
        for reply in replies {
            let Some(purpose) = self.purposes.remove(&reply.tag) else {
                continue;
            };
            let tag = reply.tag;
            match (purpose, reply.result) {
                (Purpose::Walk(root), Ok(varbinds)) => {
                    self.up(cx, reply.round_trip);
                    let rows = self.apply(cx, &varbinds);
                    self.walked(cx, root, rows);
                    self.stamps.answered(cx, "status");
                }
                (Purpose::Reread, Ok(varbinds)) => {
                    cx.alive();
                    self.apply(cx, &varbinds);
                }
                (Purpose::System | Purpose::Liveness, Ok(varbinds)) => {
                    self.up(cx, reply.round_trip);
                    let mut system = Map::new();
                    for (name, oid) in system_group() {
                        if let Some(vb) = varbinds.iter().find(|v| v.oid == oid) {
                            if !vb.value.is_exception() {
                                system.insert(name.into(), vb.value.to_json());
                            }
                        }
                    }
                    if varbinds.len() > 1 {
                        self.system_read = true;
                    }
                    cx.state(json!({ "system": system }));
                }
                (Purpose::Command(id), Ok(varbinds)) => {
                    self.up(cx, reply.round_trip);
                    self.apply(cx, &varbinds);
                    let list: Vec<Json> = varbinds.iter().map(varbind_json).collect();
                    cx.complete(
                        id,
                        Ok(Outcome::Value {
                            value: json!({ "varbinds": list }),
                        }),
                    );
                }
                (Purpose::Set(id, reread), Ok(varbinds)) => {
                    self.up(cx, reply.round_trip);
                    self.apply(cx, &varbinds);
                    cx.complete(id, Ok(Outcome::Ack));
                    if self.monitor {
                        let request = self.walk(&reread.0);
                        self.send(cx, Purpose::Reread, request);
                    }
                }
                (Purpose::Lock(id, lock, vb, reread), Ok(varbinds)) => {
                    self.up(cx, reply.round_trip);
                    match varbinds.iter().find(|v| v.oid == lock).map(|v| &v.value) {
                        // TestAndIncr: the value read, written back with the object.
                        Some(Value::Integer(n)) => {
                            let held = VarBind {
                                oid: lock,
                                value: Value::Integer(*n),
                            };
                            self.send(cx, Purpose::Set(id, reread), Request::Set(vec![held, vb]));
                        }
                        _ => cx.complete(
                            id,
                            Err(CommandError::DeviceError {
                                code: None,
                                message: "the agent gave no lock to write with the object".into(),
                            }),
                        ),
                    }
                }
                (Purpose::Command(id) | Purpose::Set(id, _) | Purpose::Lock(id, ..), Err(e)) => {
                    if !matches!(e, SnmpError::Timeout) {
                        cx.alive();
                    }
                    cx.complete(id, Err(command_error(&e)));
                }
                // A v1 agent lacking one of the system group's objects
                // refuses the GET; it answered, and the next poll asks for
                // sysUpTime.0 alone.
                (Purpose::System | Purpose::Liveness, Err(SnmpError::Status { .. })) => {
                    self.system_read = true;
                    self.up(cx, reply.round_trip);
                }
                (Purpose::System | Purpose::Liveness, Err(e)) => self.down(cx, &e),
                // A subtree the agent refuses or does not have is not this
                // device's: it is left out of state, and the agent is up.
                (Purpose::Walk(_) | Purpose::Reread, Err(SnmpError::Status { status, .. })) => {
                    cx.alive();
                    cx.log(
                        Level::Debug,
                        format!("walk refused: {}", snmp::status_name(status)),
                    );
                }
                (Purpose::Walk(_) | Purpose::Reread, Err(e)) => self.down(cx, &e),
            }
            self.refreshed(cx, tag);
        }
    }

    fn refreshed(&mut self, cx: &mut Cx, tag: u64) {
        let mut done = Vec::new();
        for (id, waiting) in self.refreshing.iter_mut() {
            waiting.retain(|t| *t != tag);
            if waiting.is_empty() {
                done.push(*id);
            }
        }
        self.refreshing.retain(|(_, w)| !w.is_empty());
        for id in done {
            cx.complete(id, Ok(Outcome::Ack));
        }
    }

    fn listen(&mut self, cx: &mut Cx) {
        let Some((address, port)) = self.trap else {
            return;
        };
        cx.udp_open(
            TRAPS,
            snmp::listen_bind(address, port, self.agent.is_none()),
        );
        if self.agent.is_none() {
            cx.connection(Connection::Connected);
        }
    }

    fn notification(&mut self, cx: &mut Cx, n: Notification) {
        cx.alive();
        self.trap_count += 1;
        let name = n.trap_oid.as_ref().and_then(|t| {
            self.profile
                .traps
                .iter()
                .find(|(oid, _)| *oid == t.0.as_slice())
                .map(|(_, name)| *name)
        });
        let varbinds: Vec<Json> = n.varbinds.iter().map(varbind_json).collect();
        let mut body = json!({
            "kind": n.kind,
            "version": n.version.name(),
            "source": n.source.to_string(),
            "trap_oid": n.trap_oid.as_ref().map(Oid::to_string),
            "name": name,
            "uptime": n.uptime,
            "varbinds": varbinds,
        });
        if let Some((enterprise, agent, generic, specific)) = &n.v1 {
            body["enterprise"] = json!(enterprise.to_string());
            body["agent_address"] = json!(std::net::Ipv4Addr::from(*agent).to_string());
            body["generic_trap"] = json!(generic);
            body["specific_trap"] = json!(specific);
        }
        if let Some(user) = &n.user {
            body["user"] = json!(user);
        }
        cx.message(
            n.trap_oid
                .as_ref()
                .map_or_else(|| "trap".into(), Oid::to_string),
            None,
            body.clone(),
            n.source,
        );
        self.apply(cx, &n.varbinds);
        cx.state(json!({ "traps": { "count": self.trap_count, "last": body } }));
        if self.monitor && self.client.is_some() && !self.polling() {
            cx.set_timer(POLL, AFTER_TRAP);
        }
    }

    /// A profile command's SET: the object's instance and its value.
    fn setter(&self, setter: &Setter, params: &Params) -> Result<VarBind, CommandError> {
        let invalid = |message: String| CommandError::InvalidParams { message };
        let mut oid = setter.oid.to_vec();
        if setter.indexes.is_empty() {
            oid.push(0);
        }
        for name in setter.indexes {
            let n = params
                .get(*name)
                .and_then(Json::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| invalid(format!("'{name}' is required")))?;
            oid.push(n);
        }
        let given = params
            .get("value")
            .ok_or_else(|| invalid("'value' is required".into()))?;
        let number = || {
            given
                .as_i64()
                .ok_or_else(|| invalid("'value' must be a whole number".into()))
        };
        let value = match (setter.kind, setter.wire) {
            (Kind::Enum(labels), _) => {
                let label = given.as_str().unwrap_or("");
                let n = labels
                    .iter()
                    .find(|(_, l)| *l == label)
                    .map(|(n, _)| *n)
                    .ok_or_else(|| invalid(format!("'{label}' is not a value of this object")))?;
                Value::Integer(n)
            }
            (_, Wire::Integer) => Value::Integer(number()?),
            (_, Wire::Gauge32 | Wire::TimeTicks) => {
                let n = u32::try_from(number()?)
                    .map_err(|_| invalid("'value' is out of range".into()))?;
                if setter.wire == Wire::Gauge32 {
                    Value::Gauge32(n)
                } else {
                    Value::TimeTicks(n)
                }
            }
            (_, Wire::OctetString) => {
                Value::OctetString(given.as_str().unwrap_or("").as_bytes().to_vec())
            }
            (Kind::Choice(choices), _) => {
                let label = given.as_str().unwrap_or("");
                let oid = choices
                    .iter()
                    .find(|(_, l)| *l == label)
                    .map(|(o, _)| *o)
                    .ok_or_else(|| invalid(format!("'{label}' is not a value of this object")))?;
                Value::Oid(Oid(oid.to_vec()))
            }
            (_, Wire::ObjectIdentifier) => Value::Oid(
                Oid::parse(given.as_str().unwrap_or(""))
                    .map_err(|e| invalid(format!("'value': {e}")))?,
            ),
            (_, Wire::IpAddress) => {
                let a: std::net::Ipv4Addr = given
                    .as_str()
                    .unwrap_or("")
                    .parse()
                    .map_err(|_| invalid("'value' is not an IPv4 address".into()))?;
                Value::IpAddress(a.octets())
            }
        };
        Ok(VarBind {
            oid: Oid(oid),
            value,
        })
    }
}

impl Module for SnmpDevice {
    fn start(&mut self, cx: &mut Cx) {
        if let (Some(agent), Some(security)) = (self.agent, self.security.take()) {
            cx.connection(Connection::Connecting);
            let version = security.version().name();
            self.client = Some(Client::new(
                cx,
                agent,
                SOCKET,
                TIMER,
                security,
                self.timeout,
                self.retries,
                self.seed,
            ));
            cx.state(json!({ "version": version }));
            self.poll(cx);
            cx.set_timer(POLL, self.interval);
        }
        if self.trap.is_some() {
            self.receiver = Some(snmp::Receiver::new(
                cx,
                TRAPS,
                self.trap_community.clone(),
                self.trap_user.clone(),
                self.local_engine_id.clone(),
                self.seed.rotate_left(17),
            ));
            cx.state(
                json!({ "traps": { "count": 0 }, "local_engine_id": hex(&self.local_engine_id) }),
            );
            self.listen(cx);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.client.is_none() {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let invalid = |message: String| CommandError::InvalidParams { message };
        let text = |k: &str| params.get(k).and_then(Json::as_str).unwrap_or("");
        let request = match name {
            "get" => match parse_oids(text("oids")) {
                Ok(oids) => Request::Get(oids),
                Err(e) => return cx.complete(id, Err(invalid(e))),
            },
            "walk" => match Oid::parse(text("oid")) {
                Ok(root) => Request::Walk {
                    root,
                    bulk: Some(self.max_repetitions),
                    limit: params
                        .get("limit")
                        .and_then(Json::as_u64)
                        .map(|l| l as usize),
                },
                Err(e) => return cx.complete(id, Err(invalid(e))),
            },
            "set" => match Oid::parse(text("oid")).and_then(|oid| {
                Ok(VarBind {
                    oid,
                    value: Value::parse(text("type"), text("value"))?,
                })
            }) {
                Ok(vb) => Request::Set(vec![vb]),
                Err(e) => return cx.complete(id, Err(invalid(e))),
            },
            "refresh" => {
                self.system_read = false;
                let tags = self.poll(cx);
                self.refreshing.push((id, tags));
                return;
            }
            other => {
                let Some(setter) = self.profile.setters.iter().find(|s| s.command == other) else {
                    return cx.complete(
                        id,
                        Err(CommandError::UnknownCommand {
                            command: other.into(),
                        }),
                    );
                };
                match self.setter(setter, params) {
                    Ok(vb) if !setter.lock.is_empty() => {
                        let mut lock = setter.lock.to_vec();
                        lock.push(0);
                        let lock = Oid(lock);
                        let reread = Oid(setter.reread.to_vec());
                        let request = Request::Get(vec![lock.clone()]);
                        self.send(cx, Purpose::Lock(id, lock, vb, reread), request);
                    }
                    Ok(vb) => {
                        let reread = Oid(setter.reread.to_vec());
                        self.send(cx, Purpose::Set(id, reread), Request::Set(vec![vb]));
                    }
                    Err(e) => cx.complete(id, Err(e)),
                }
                return;
            }
        };
        self.send(cx, Purpose::Command(id), request);
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        match socket {
            SOCKET => {
                let replies = match &mut self.client {
                    Some(c) => c.datagram(cx, from, data),
                    None => return,
                };
                self.replies(cx, replies);
            }
            TRAPS => {
                if let Some(agent) = self.agent {
                    if from.ip() != agent.ip() {
                        return;
                    }
                }
                let Some(receiver) = &mut self.receiver else {
                    return;
                };
                match receiver.datagram(cx, from, data) {
                    Ok(Some(n)) => self.notification(cx, n),
                    Ok(None) => {}
                    Err(e) => cx.log(Level::Debug, e),
                }
            }
            _ => {}
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        match socket {
            TRAPS => {
                cx.log(
                    Level::Warning,
                    format!("cannot receive notifications: {message}"),
                );
                if self.agent.is_none() {
                    cx.connection(Connection::Disconnected {
                        reason: message.into(),
                    });
                }
                cx.set_timer(TRAP_RETRY, self.trap_retry);
                self.trap_retry = (self.trap_retry * 2).min(RETRY_MAX);
            }
            _ => {
                self.connected = false;
                cx.connection(Connection::Disconnected {
                    reason: message.into(),
                });
                cx.udp_open(SOCKET, crate::module::Bind::Ephemeral);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            TIMER => {
                let replies = match &mut self.client {
                    Some(c) => c.timer(cx),
                    None => return,
                };
                self.replies(cx, replies);
            }
            POLL => {
                cx.set_timer(POLL, self.interval);
                // A poll still unanswered is not doubled.
                if !self.polling() {
                    self.poll(cx);
                }
            }
            TRAP_RETRY => self.listen(cx),
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if let Some(c) = &mut self.client {
            c.clear(cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::modules::snmp::agent::Agent;
    use std::net::{IpAddr, Ipv4Addr};

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 40));

    static STATES: [(i64, &str); 3] = [
        (1, "immediate_on"),
        (2, "immediate_off"),
        (3, "outlet_reboot"),
    ];
    static OBJECTS: [Object; 4] = [
        Object {
            oid: &[1, 3, 6, 1, 4, 1, 99, 1, 1],
            path: "device.name",
            kind: Kind::Text,
        },
        Object {
            oid: &[1, 3, 6, 1, 4, 1, 99, 1, 2],
            path: "device.load",
            kind: Kind::Int,
        },
        Object {
            oid: &[1, 3, 6, 1, 4, 1, 99, 2, 1, 1, 2],
            path: "outlets.*.name",
            kind: Kind::Text,
        },
        Object {
            oid: &[1, 3, 6, 1, 4, 1, 99, 2, 1, 1, 3],
            path: "outlets.*.state",
            kind: Kind::Enum(&STATES),
        },
    ];
    static SETTERS: [Setter; 2] = [
        Setter {
            command: "set_outlets_state",
            oid: &[1, 3, 6, 1, 4, 1, 99, 2, 1, 1, 3],
            kind: Kind::Enum(&STATES),
            wire: Wire::Integer,
            indexes: &["index"],
            reread: &[1, 3, 6, 1, 4, 1, 99, 2],
            lock: &[],
        },
        Setter {
            command: "set_device_name",
            oid: &[1, 3, 6, 1, 4, 1, 99, 1, 1],
            kind: Kind::Text,
            wire: Wire::OctetString,
            indexes: &[],
            reread: &[1, 3, 6, 1, 4, 1, 99, 1],
            lock: &[],
        },
    ];
    static PROFILE: Profile = Profile {
        roots: &[Root {
            oid: &[1, 3, 6, 1, 4, 1, 99],
            models: &[],
        }],
        objects: &OBJECTS,
        setters: &SETTERS,
        traps: &[(&[1, 3, 6, 1, 4, 1, 99, 0, 7], "outletOff")],
    };

    fn agent() -> Agent {
        Agent::new(&[
            (
                "1.3.6.1.2.1.1.1.0",
                Value::OctetString(b"Rack PDU".to_vec()),
            ),
            ("1.3.6.1.2.1.1.3.0", Value::TimeTicks(500)),
            (
                "1.3.6.1.4.1.99.1.1.0",
                Value::OctetString(b"pdu-a\0".to_vec()),
            ),
            ("1.3.6.1.4.1.99.1.2.0", Value::Gauge32(42)),
            (
                "1.3.6.1.4.1.99.2.1.1.2.1",
                Value::OctetString(b"Encoder".to_vec()),
            ),
            (
                "1.3.6.1.4.1.99.2.1.1.2.2",
                Value::OctetString(b"Switch".to_vec()),
            ),
            ("1.3.6.1.4.1.99.2.1.1.3.1", Value::Integer(1)),
            ("1.3.6.1.4.1.99.2.1.1.3.2", Value::Integer(9)),
        ])
    }

    fn open(monitor: bool) -> SnmpDevice {
        SnmpDevice::new(
            &PROFILE,
            OpenContext {
                host: HOST,
                host_name: None,
                port: None,
                model: "pdu".into(),
                channels: None,
                settings: json!({ "write_community": "private" })
                    .as_object()
                    .unwrap()
                    .clone(),
                monitor,
            },
        )
        .unwrap()
    }

    /// Answer every request the module sends until it sends no more.
    fn run(m: &mut SnmpDevice, agent: &mut Agent, actions: Vec<Action>) -> Vec<Action> {
        let mut all = actions.clone();
        let mut pending = actions;
        for step in 0..40 {
            let sends: Vec<Vec<u8>> = pending
                .iter()
                .filter_map(|a| match a {
                    Action::UdpSend {
                        socket: SOCKET,
                        data,
                        ..
                    } => Some(data.clone()),
                    _ => None,
                })
                .collect();
            if sends.is_empty() {
                break;
            }
            pending = Vec::new();
            for d in sends {
                if let Some(answer) = agent.handle(&d) {
                    let mut cx = Cx::new(100 + step);
                    m.datagram(&mut cx, SOCKET, SocketAddr::new(HOST, 161), &answer);
                    pending.extend(cx.take());
                }
            }
            all.extend(pending.clone());
        }
        all
    }

    fn state(actions: &[Action]) -> Json {
        let mut out = json!({});
        for a in actions {
            if let Action::State(p) = a {
                crate::session::merge_patch(&mut out, p);
            }
        }
        out
    }

    #[test]
    fn a_poll_walks_the_subtree_into_named_state() {
        let mut m = open(true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let mut a = agent();
        let actions = run(&mut m, &mut a, cx.take());
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        let s = state(&actions);
        assert_eq!(s["system"]["description"], "Rack PDU");
        assert_eq!(s["device"]["name"], "pdu-a");
        assert_eq!(s["device"]["load"], 42);
        assert_eq!(s["outlets"]["1"]["name"], "Encoder");
        assert_eq!(s["outlets"]["1"]["state"], "immediate_on");
        // A value the MIB does not list is its number.
        assert_eq!(s["outlets"]["2"]["state"], "9");
        // A second poll reports nothing that did not change.
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, POLL);
        let again = run(&mut m, &mut a, cx.take());
        let s = state(&again);
        assert!(s.get("outlets").is_none(), "{s}");
    }

    #[test]
    fn a_profile_command_sets_its_object_and_reads_its_table_again() {
        let mut m = open(true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let mut a = agent();
        run(&mut m, &mut a, cx.take());
        let mut cx = Cx::new(1000);
        let p = json!({ "index": 2, "value": "immediate_off" });
        m.command(&mut cx, 7, "set_outlets_state", p.as_object().unwrap());
        let actions = run(&mut m, &mut a, cx.take());
        assert!(actions.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Ack)
        }));
        assert_eq!(state(&actions)["outlets"]["2"]["state"], "immediate_off");
        // A label the object does not have is refused before sending.
        let mut cx = Cx::new(2000);
        let p = json!({ "index": 2, "value": "sideways" });
        m.command(&mut cx, 8, "set_outlets_state", p.as_object().unwrap());
        assert!(matches!(
            &cx.take()[..],
            [Action::Complete {
                result: Err(CommandError::InvalidParams { .. }),
                ..
            }]
        ));
        // A scalar gets .0.
        let vb = m
            .setter(
                &SETTERS[1],
                json!({ "value": "pdu-b" }).as_object().unwrap(),
            )
            .unwrap();
        assert_eq!(vb.oid.to_string(), "1.3.6.1.4.1.99.1.1.0");
    }

    #[test]
    fn a_row_no_longer_listed_leaves_state() {
        let mut m = open(true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let mut a = agent();
        run(&mut m, &mut a, cx.take());
        let mut fewer = Agent::new(&[
            ("1.3.6.1.2.1.1.3.0", Value::TimeTicks(900)),
            (
                "1.3.6.1.4.1.99.2.1.1.2.1",
                Value::OctetString(b"Encoder".to_vec()),
            ),
            ("1.3.6.1.4.1.99.2.1.1.3.1", Value::Integer(1)),
        ]);
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, POLL);
        let actions = run(&mut m, &mut fewer, cx.take());
        let removed = actions.iter().any(|x| match x {
            Action::State(p) => p["outlets"]["2"].is_null() && p["outlets"].get("2").is_some(),
            _ => false,
        });
        assert!(removed);
    }

    #[test]
    fn commands_only_walks_nothing() {
        let mut m = open(false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let sends = cx
            .take()
            .into_iter()
            .filter(|a| matches!(a, Action::UdpSend { .. }))
            .count();
        assert_eq!(sends, 1);
    }

    static TESTS: [(&[u32], &str); 2] = [
        (&[1, 3, 6, 1, 4, 1, 99, 9, 1], "no_tests_initiated"),
        (&[1, 3, 6, 1, 4, 1, 99, 9, 2], "quick_battery_test"),
    ];
    static TEST_OBJECTS: [Object; 1] = [Object {
        oid: &[1, 3, 6, 1, 4, 1, 99, 3, 2],
        path: "test.id",
        kind: Kind::Choice(&TESTS),
    }];
    static TEST_SETTERS: [Setter; 1] = [Setter {
        command: "set_test_id",
        oid: &[1, 3, 6, 1, 4, 1, 99, 3, 2],
        kind: Kind::Choice(&TESTS),
        wire: Wire::ObjectIdentifier,
        indexes: &[],
        reread: &[1, 3, 6, 1, 4, 1, 99, 3],
        lock: &[1, 3, 6, 1, 4, 1, 99, 3, 1],
    }];
    static TEST_PROFILE: Profile = Profile {
        roots: &[],
        objects: &TEST_OBJECTS,
        setters: &TEST_SETTERS,
        traps: &[],
    };

    #[test]
    fn a_locked_object_is_written_with_the_lock_it_read() {
        let mut m = SnmpDevice::new(
            &TEST_PROFILE,
            OpenContext {
                host: HOST,
                host_name: None,
                port: None,
                model: "ups".into(),
                channels: None,
                settings: Params::new(),
                monitor: true,
            },
        )
        .unwrap();
        let mut a = Agent::new(&[
            ("1.3.6.1.2.1.1.3.0", Value::TimeTicks(5)),
            ("1.3.6.1.4.1.99.3.1.0", Value::Integer(41)),
            (
                "1.3.6.1.4.1.99.3.2.0",
                Value::Oid(Oid::parse("1.3.6.1.4.1.99.9.1").unwrap()),
            ),
        ]);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        run(&mut m, &mut a, cx.take());
        let mut cx = Cx::new(10);
        let p = json!({ "value": "quick_battery_test" });
        m.command(&mut cx, 3, "set_test_id", p.as_object().unwrap());
        let actions = run(&mut m, &mut a, cx.take());
        assert!(actions.contains(&Action::Complete {
            id: 3,
            result: Ok(Outcome::Ack)
        }));
        // The SET carried the lock as read, then the test's OID; the
        // reread names it.
        assert_eq!(state(&actions)["test"]["id"], "quick_battery_test");
        let sets: Vec<_> = actions
            .iter()
            .filter_map(|x| match x {
                Action::UdpSend { data, .. } => match snmp::decode(data).unwrap() {
                    snmp::Message::Community {
                        pdu: snmp::AnyPdu::Pdu(p),
                        ..
                    } if p.varbinds.len() == 2 => Some(p.varbinds),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(sets[0][0].value, Value::Integer(41));
        assert_eq!(sets[0][1].oid.to_string(), "1.3.6.1.4.1.99.3.2.0");
    }

    /// Every profile built in, and the spec it serves.
    fn profiles() -> Vec<(&'static str, &'static Profile)> {
        vec![
            #[cfg(feature = "ups-mib")]
            ("ups-mib", &crate::modules::ups_mib_profile::PROFILE),
            #[cfg(feature = "cyberpower-ups")]
            (
                "cyberpower-ups",
                &crate::modules::cyberpower_ups_profile::PROFILE,
            ),
            #[cfg(feature = "cyberpower-pdu")]
            (
                "cyberpower-pdu",
                &crate::modules::cyberpower_pdu_profile::PROFILE,
            ),
            #[cfg(feature = "apc-ups")]
            ("apc-ups", &crate::modules::apc_ups_profile::PROFILE),
            #[cfg(feature = "apc-rack-pdu")]
            (
                "apc-rack-pdu",
                &crate::modules::apc_rack_pdu_profile::PROFILE,
            ),
        ]
    }

    /// The state key a path declares: each number a `*`.
    fn declared(path: &str) -> String {
        path.split('.')
            .map(|s| {
                if s.chars().all(|c| c.is_ascii_digit()) {
                    "*"
                } else {
                    s
                }
            })
            .collect::<Vec<_>>()
            .join(".")
    }

    #[test]
    fn each_profile_matches_its_spec() {
        let catalog = crate::catalog::Catalog::embedded();
        for (id, profile) in profiles() {
            let spec = catalog.device(id).expect("the spec");
            for o in profile.objects {
                assert!(
                    spec.state.contains_key(&declared(o.path)),
                    "{id}: {} is not declared",
                    o.path
                );
            }
            let raw = ["get", "walk", "set", "refresh"];
            for name in spec.commands.keys() {
                assert!(
                    raw.contains(&name.as_str())
                        || profile.setters.iter().any(|s| s.command == name),
                    "{id}: {name} has no setter"
                );
            }
            for s in profile.setters {
                let command = &spec.commands[s.command];
                for index in s.indexes {
                    assert!(command.params.contains_key(*index), "{id}: {}", s.command);
                }
                assert!(command.params.contains_key("value"), "{id}: {}", s.command);
                assert!(s.reread.len() < s.oid.len() || s.reread == s.oid);
            }
        }
    }

    #[test]
    fn decoding_follows_the_kind() {
        assert_eq!(
            decode(Kind::Mac, &Value::OctetString(vec![0, 0xc0, 0xb7, 1, 2, 3])),
            Some(json!("00:c0:b7:01:02:03"))
        );
        assert_eq!(
            decode(Kind::Ip, &Value::IpAddress([10, 0, 0, 5])),
            Some(json!("10.0.0.5"))
        );
        assert_eq!(decode(Kind::Int, &Value::NoSuchInstance), None);
        assert_eq!(
            decode(Kind::Ticks, &Value::TimeTicks(360_000)),
            Some(json!(360_000))
        );
    }
}
