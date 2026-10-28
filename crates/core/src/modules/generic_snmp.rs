//! A generic SNMP manager for one agent: GET, GETNEXT, GETBULK, walks and
//! typed SETs by OID, chosen objects polled into state, and the agent's
//! traps and informs received as events and state. The protocol is
//! modules/snmp.rs's (v1, v2c and v3 with the USM).
//!
//! - The system group (RFC 3418: sysDescr, sysObjectID, sysContact,
//!   sysName, sysLocation) is read on opening, and sysUpTime.0 with every
//!   poll, which is also the liveness check: the agent is connected while
//!   it answers, disconnected when a poll goes unanswered after its
//!   retries, and unauthorized when v3 reports the user, its keys or its
//!   level refused.
//! - Polled objects (`poll_oids`, each optionally named) are read with GET
//!   every `poll_interval_ms`, at most 16 per request; a request the agent
//!   finds too big is split, and an object a v1 agent does not have is set
//!   aside so the others are still read. GET and SET replies for a polled
//!   object update its state too.
//! - With `trap_port` set, traps and informs are received there: on the
//!   core's shared port, routed by sender so that several agents can share
//!   162, or with `trap_bind_address` on a port of this device's own.
//!   Opened with no host, the integration is a receiver for any sender.
//!   Each notification is a message event and the last one is in state.
//! - Opening for commands only reads no system group and polls nothing; one
//!   GET of sysUpTime.0 per interval stays as the liveness check, since
//!   SNMP over UDP gives no other sign of the agent. Traps are still
//!   received: the agent sends them unasked.
//! - Community strings and v3 passwords are never logged.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use serde_json::{json, Map, Value as Json};

use super::snmp::{
    self, command_error, parse_oids, varbind_json, Client, Notification, Reply, Request, Security,
    SnmpError, User, VarBind,
};
use super::snmp_ber::{hex, Oid, Value, SET_TYPES};
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};

const SOCKET: Key = "snmp";
const TIMER: Key = "snmp_retry";
const TRAPS: Key = "traps";
const TRAP_RETRY: Key = "trap_retry";
const POLL: Key = "poll";
/// Objects per polling GET.
const CHUNK: usize = 16;
/// Opening the notification port again after a failure: 1 s, doubling to
/// 30 s.
const RETRY_MIN: Millis = 1000;
const RETRY_MAX: Millis = 30_000;

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

/// What a request was for.
enum Purpose {
    Command(CommandId),
    /// A GET of polled objects: their names, in order.
    Poll(Vec<(String, Oid)>),
    /// A GET of the system group's objects still asked for.
    System(Vec<(&'static str, Oid)>),
    Liveness,
}

/// A polled object, by the name it has in state.
#[derive(Debug, Clone, PartialEq)]
struct Polled {
    name: String,
    oid: Oid,
}

pub(crate) struct GenericSnmp {
    agent: Option<SocketAddr>,
    security: Option<Security>,
    timeout: Millis,
    retries: u32,
    max_repetitions: u32,
    polled: Vec<Polled>,
    interval: Millis,
    monitor: bool,
    trap: Option<(Option<IpAddr>, u16)>,
    trap_community: Vec<u8>,
    trap_user: Option<User>,
    local_engine_id: Vec<u8>,
    seed: u64,
    client: Option<Client>,
    receiver: Option<snmp::Receiver>,
    next_tag: u64,
    purposes: HashMap<u64, Purpose>,
    connected: bool,
    trap_retry: Millis,
    trap_count: u64,
    /// `refresh` commands waiting for the reads they started.
    refreshing: Vec<(CommandId, Vec<u64>)>,
}

fn setting<'a>(s: &'a Params, name: &str) -> &'a str {
    s.get(name).and_then(Json::as_str).unwrap_or("").trim()
}

/// `poll_oids`: entries separated by commas, semicolons or white space,
/// each an OID or `name=OID`.
fn parse_polled(text: &str) -> Result<Vec<Polled>, String> {
    let mut out: Vec<Polled> = Vec::new();
    for entry in text.split([',', ';', ' ', '\t']).filter(|e| !e.is_empty()) {
        let (name, oid) = match entry.split_once('=') {
            Some((n, o)) => {
                let n = n.trim();
                if n.is_empty()
                    || !n
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    return Err(format!(
                        "poll_oids: '{n}' is not a name of letters, digits, _ and -"
                    ));
                }
                (Some(n.to_string()), o)
            }
            None => (None, entry),
        };
        let oid = Oid::parse(oid).map_err(|e| format!("poll_oids: {e}"))?;
        let name = name.unwrap_or_else(|| oid.to_string());
        if out.iter().any(|p| p.name == name) {
            return Err(format!("poll_oids: '{name}' is given twice"));
        }
        out.push(Polled { name, oid });
    }
    Ok(out)
}

/// A polled object in state: {oid, type, number, text, hex}, so each key
/// keeps one JSON type.
fn state_json(vb: &VarBind) -> Json {
    let v = &vb.value;
    let number = match v {
        Value::Integer(_)
        | Value::Counter32(_)
        | Value::Gauge32(_)
        | Value::TimeTicks(_)
        | Value::Counter64(_) => v.to_json(),
        _ => Json::Null,
    };
    let text = match v {
        Value::Opaque(b) => json!(hex(b)),
        _ if v.is_exception() || *v == Value::Null => Json::Null,
        _ => match v.to_json() {
            Json::String(s) => json!(s),
            other => json!(other.to_string()),
        },
    };
    json!({
        "oid": vb.oid.to_string(),
        "type": v.type_name(),
        "number": number,
        "text": text,
        "hex": v.bytes().map(hex),
    })
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

impl GenericSnmp {
    pub(crate) fn new(ctx: OpenContext) -> Result<GenericSnmp, String> {
        let s = snmp::Settings::parse(&ctx, 5000)?;
        Ok(GenericSnmp {
            agent: s.agent,
            security: Some(s.security),
            timeout: s.timeout,
            retries: s.retries,
            max_repetitions: s.max_repetitions,
            polled: parse_polled(setting(&ctx.settings, "poll_oids"))?,
            interval: s.interval,
            monitor: ctx.monitor,
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
            trap_retry: RETRY_MIN,
            trap_count: 0,
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

    fn listen(&mut self, cx: &mut Cx) {
        let Some((address, port)) = self.trap else {
            return;
        };
        let bind = snmp::listen_bind(address, port, self.agent.is_none());
        cx.udp_open(TRAPS, bind);
        if self.agent.is_none() {
            cx.connection(Connection::Connected);
        }
    }

    /// Read what state keeps: the polled objects (and, when `system`, the
    /// system group). Returns the request tags.
    fn poll(&mut self, cx: &mut Cx, system: bool) -> Vec<u64> {
        let mut tags = Vec::new();
        if system {
            tags.push(self.read_system(cx, system_group()));
        } else {
            let up = Oid(vec![1, 3, 6, 1, 2, 1, 1, 3, 0]);
            tags.push(self.send(cx, Purpose::Liveness, Request::Get(vec![up])));
        }
        if self.monitor {
            let chunks: Vec<Vec<Polled>> =
                self.polled.chunks(CHUNK).map(<[Polled]>::to_vec).collect();
            for chunk in chunks {
                tags.push(
                    self.poll_chunk(cx, chunk.into_iter().map(|p| (p.name, p.oid)).collect()),
                );
            }
        }
        tags
    }

    fn read_system(&mut self, cx: &mut Cx, objects: Vec<(&'static str, Oid)>) -> u64 {
        let oids = objects.iter().map(|(_, o)| o.clone()).collect();
        self.send(cx, Purpose::System(objects), Request::Get(oids))
    }

    fn poll_chunk(&mut self, cx: &mut Cx, chunk: Vec<(String, Oid)>) -> u64 {
        let oids = chunk.iter().map(|(_, o)| o.clone()).collect();
        self.send(cx, Purpose::Poll(chunk), Request::Get(oids))
    }

    /// Patch state for any polled objects among these variable bindings.
    fn apply_polled(&self, cx: &mut Cx, varbinds: &[VarBind]) {
        let mut values = Map::new();
        for vb in varbinds {
            for p in self.polled.iter().filter(|p| p.oid == vb.oid) {
                values.insert(p.name.clone(), state_json(vb));
            }
        }
        if !values.is_empty() {
            cx.state(json!({ "values": values }));
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
            cx.state(json!({ "engine": { "id": hex(&id), "boots": boots, "time": time } }));
        }
    }

    fn down(&mut self, cx: &mut Cx, e: &SnmpError) {
        self.connected = false;
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
                (Purpose::Command(id), Ok(varbinds)) => {
                    self.up(cx, reply.round_trip);
                    self.apply_polled(cx, &varbinds);
                    let list: Vec<Json> = varbinds.iter().map(varbind_json).collect();
                    cx.complete(
                        id,
                        Ok(Outcome::Value {
                            value: json!({ "varbinds": list }),
                        }),
                    );
                }
                (Purpose::Command(id), Err(e)) => {
                    if !matches!(e, SnmpError::Timeout) {
                        cx.alive();
                    }
                    cx.complete(id, Err(command_error(&e)));
                }
                (Purpose::System(_) | Purpose::Liveness, Ok(varbinds)) => {
                    self.up(cx, reply.round_trip);
                    let mut system = Map::new();
                    for (name, oid) in system_group() {
                        if let Some(vb) = varbinds.iter().find(|v| v.oid == oid) {
                            if !vb.value.is_exception() {
                                system.insert(name.into(), vb.value.to_json());
                            }
                        }
                    }
                    cx.state(json!({ "system": system }));
                    self.apply_polled(cx, &varbinds);
                }
                // A v1 agent without one of the objects refuses the GET:
                // it answered, and the others are asked for again.
                (Purpose::System(mut objects), Err(SnmpError::Status { index, .. })) => {
                    self.up(cx, reply.round_trip);
                    let i = usize::try_from(index).unwrap_or(0);
                    if (1..=objects.len()).contains(&i) && objects.len() > 1 {
                        objects.remove(i - 1);
                        let again = self.read_system(cx, objects);
                        for (_, waiting) in self.refreshing.iter_mut() {
                            if waiting.contains(&tag) {
                                waiting.push(again);
                            }
                        }
                    }
                }
                (Purpose::Liveness, Err(SnmpError::Status { .. })) => self.up(cx, reply.round_trip),
                (Purpose::System(_) | Purpose::Liveness, Err(e)) => self.down(cx, &e),
                (Purpose::Poll(_), Ok(varbinds)) => {
                    self.up(cx, reply.round_trip);
                    self.apply_polled(cx, &varbinds);
                }
                (Purpose::Poll(chunk), Err(SnmpError::Status { status, index, .. })) => {
                    cx.alive();
                    self.poll_error(cx, tag, chunk, status, index);
                }
                (Purpose::Poll(_), Err(e)) => self.down(cx, &e),
            }
            self.refreshed(cx, tag);
        }
    }

    /// A polling GET was refused: split a request too big, set aside the
    /// object a v1 agent does not have, and read the rest.
    fn poll_error(
        &mut self,
        cx: &mut Cx,
        tag: u64,
        chunk: Vec<(String, Oid)>,
        status: i64,
        index: i64,
    ) {
        let mut retry = Vec::new();
        if status == 1 && chunk.len() > 1 {
            let (a, b) = chunk.split_at(chunk.len() / 2);
            retry.push(a.to_vec());
            retry.push(b.to_vec());
        } else if let Some(i) = usize::try_from(index)
            .ok()
            .filter(|i| (1..=chunk.len()).contains(i))
        {
            let (name, oid) = &chunk[i - 1];
            cx.state(json!({ "values": { name.as_str(): {
                "oid": oid.to_string(), "type": snmp::status_name(status),
                "number": Json::Null, "text": Json::Null, "hex": Json::Null,
            } } }));
            let rest: Vec<(String, Oid)> = chunk
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i - 1)
                .map(|(_, p)| p.clone())
                .collect();
            if !rest.is_empty() {
                retry.push(rest);
            }
        } else {
            cx.log(
                Level::Warning,
                format!("polling refused: {}", snmp::status_name(status)),
            );
        }
        let mut tags = Vec::new();
        for part in retry {
            tags.push(self.poll_chunk(cx, part));
        }
        // A refresh waiting for the refused read waits for its parts.
        for (_, waiting) in self.refreshing.iter_mut() {
            if waiting.contains(&tag) {
                waiting.extend(tags.iter().copied());
            }
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

    fn notification(&mut self, cx: &mut Cx, n: Notification) {
        cx.alive();
        self.trap_count += 1;
        let varbinds: Vec<Json> = n.varbinds.iter().map(varbind_json).collect();
        let mut body = json!({
            "kind": n.kind,
            "version": n.version.name(),
            "source": n.source.to_string(),
            "trap_oid": n.trap_oid.as_ref().map(Oid::to_string),
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
        // A trap may carry a polled object's new value.
        self.apply_polled(cx, &n.varbinds);
        cx.state(json!({ "traps": { "count": self.trap_count, "last": body } }));
    }
}

impl Module for GenericSnmp {
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
            self.poll(cx, self.monitor);
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
        let text = |k: &str| params.get(k).and_then(Json::as_str).unwrap_or("");
        let int = |k: &str| params.get(k).and_then(Json::as_u64);
        let request = match name {
            "get" | "get_next" | "get_bulk" => {
                let oids = match parse_oids(text("oids")) {
                    Ok(o) => o,
                    Err(e) => return cx.complete(id, Err(invalid(e))),
                };
                match name {
                    "get" => Request::Get(oids),
                    "get_next" => Request::GetNext(oids),
                    _ => Request::GetBulk {
                        non_repeaters: int("non_repeaters").unwrap_or(0) as u32,
                        max_repetitions: int("max_repetitions")
                            .map_or(self.max_repetitions, |m| m as u32),
                        oids,
                    },
                }
            }
            "walk" => {
                let root = match Oid::parse(text("oid")) {
                    Ok(o) => o,
                    Err(e) => return cx.complete(id, Err(invalid(e))),
                };
                let use_getnext = params
                    .get("use_getnext")
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                Request::Walk {
                    root,
                    bulk: (!use_getnext)
                        .then(|| int("max_repetitions").map_or(self.max_repetitions, |m| m as u32)),
                    limit: int("limit").map(|l| l as usize),
                }
            }
            "set" => match Oid::parse(text("oid")).and_then(|oid| {
                Ok(VarBind {
                    oid,
                    value: Value::parse(text("type"), text("value"))?,
                })
            }) {
                Ok(vb) => Request::Set(vec![vb]),
                Err(e) => return cx.complete(id, Err(invalid(e))),
            },
            "set_multiple" => {
                let list = params
                    .get("varbinds")
                    .and_then(Json::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut vbs = Vec::new();
                for (i, item) in list.iter().enumerate() {
                    let field = |k: &str| item.get(k).and_then(Json::as_str).unwrap_or("");
                    let kind = field("type");
                    if !SET_TYPES.contains(&kind) {
                        return cx.complete(
                            id,
                            Err(invalid(format!(
                                "varbinds[{i}].type '{kind}' is not one of {}",
                                SET_TYPES.join(", ")
                            ))),
                        );
                    }
                    match Oid::parse(field("oid")).and_then(|oid| {
                        Ok(VarBind {
                            oid,
                            value: Value::parse(kind, field("value"))?,
                        })
                    }) {
                        Ok(vb) => vbs.push(vb),
                        Err(e) => {
                            return cx.complete(id, Err(invalid(format!("varbinds[{i}]: {e}"))))
                        }
                    }
                }
                if vbs.is_empty() {
                    return cx.complete(id, Err(invalid("give at least one {oid, type, value}")));
                }
                Request::Set(vbs)
            }
            "refresh" => {
                let tags = self.poll(cx, true);
                self.refreshing.push((id, tags));
                return;
            }
            other => {
                return cx.complete(
                    id,
                    Err(CommandError::UnknownCommand {
                        command: other.into(),
                    }),
                );
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
                // On a port of its own, a device hears only its agent.
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
                // Open it again; the next poll uses it.
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
                let polling = self.purposes.values().any(|p| {
                    matches!(p, Purpose::Poll(_) | Purpose::System(_) | Purpose::Liveness)
                });
                if !polling {
                    // The system group again while it has never been read.
                    let system = self.monitor && !self.connected;
                    self.poll(cx, system);
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
    use crate::module::{Action, Bind};
    use crate::modules::snmp::agent::Agent;
    use std::net::Ipv4Addr;

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 30));

    fn open(settings: Json, host: IpAddr, monitor: bool) -> GenericSnmp {
        GenericSnmp::new(OpenContext {
            host,
            host_name: None,
            port: None,
            model: "generic".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        })
        .unwrap()
    }

    fn agent() -> Agent {
        Agent::new(&[
            (
                "1.3.6.1.2.1.1.1.0",
                Value::OctetString(b"Rack PDU".to_vec()),
            ),
            ("1.3.6.1.2.1.1.3.0", Value::TimeTicks(500)),
            ("1.3.6.1.2.1.1.5.0", Value::OctetString(b"pdu-1".to_vec())),
            ("1.3.6.1.4.1.99.1.1.0", Value::Gauge32(230)),
            ("1.3.6.1.4.1.99.1.2.0", Value::Integer(1)),
        ])
    }

    /// Answer every request the module sends until it sends no more.
    fn run(m: &mut GenericSnmp, agent: &mut Agent, actions: Vec<Action>) -> Vec<Action> {
        let mut all = actions.clone();
        let mut pending = actions;
        for step in 0..20 {
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
    fn the_system_group_and_polled_objects_reach_state() {
        let mut m = open(
            json!({ "poll_oids": "voltage=1.3.6.1.4.1.99.1.1.0, 1.3.6.1.4.1.99.1.9.0" }),
            HOST,
            true,
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let start = cx.take();
        assert!(start.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Ephemeral
        }));
        let mut a = agent();
        let actions = run(&mut m, &mut a, start);
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        let s = state(&actions);
        assert_eq!(s["version"], "v2c");
        assert_eq!(s["system"]["description"], "Rack PDU");
        assert_eq!(s["system"]["uptime"], 500);
        assert_eq!(
            s["values"]["voltage"],
            json!({ "oid": "1.3.6.1.4.1.99.1.1.0", "type": "gauge32", "number": 230, "text": "230" })
        );
        assert_eq!(
            s["values"]["1.3.6.1.4.1.99.1.9.0"]["type"],
            "no_such_object"
        );
    }

    #[test]
    fn a_v1_agent_missing_one_polled_object_still_gives_the_others() {
        let mut m = open(
            json!({ "version": "v1", "poll_oids": "a=1.3.6.1.4.1.99.1.9.0 b=1.3.6.1.4.1.99.1.2.0" }),
            HOST,
            true,
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let mut a = agent();
        let s = state(&run(&mut m, &mut a, cx.take()));
        assert_eq!(s["values"]["a"]["type"], "noSuchName");
        assert_eq!(s["values"]["b"]["number"], 1);
    }

    #[test]
    fn commands_return_varbinds_and_a_set_is_read_back_into_state() {
        let mut m = open(
            json!({ "poll_oids": "mode=1.3.6.1.4.1.99.1.2.0", "write_community": "private" }),
            HOST,
            true,
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let mut a = agent();
        run(&mut m, &mut a, cx.take());
        let mut cx = Cx::new(200);
        let params = json!({ "oid": "1.3.6.1.4.1.99.1.2.0", "type": "integer", "value": "3" });
        m.command(&mut cx, 1, "set", params.as_object().unwrap());
        let actions = run(&mut m, &mut a, cx.take());
        assert_eq!(state(&actions)["values"]["mode"]["number"], 3);
        let done = actions.iter().find_map(|x| match x {
            Action::Complete { id: 1, result } => Some(result.clone()),
            _ => None,
        });
        assert_eq!(
            done,
            Some(Ok(Outcome::Value {
                value: json!({ "varbinds": [{ "oid": "1.3.6.1.4.1.99.1.2.0", "type": "integer", "value": 3 }] })
            }))
        );
        // A walk, and a GET of an octet string with its hex.
        let mut cx = Cx::new(300);
        m.command(
            &mut cx,
            2,
            "walk",
            json!({ "oid": "1.3.6.1.4.1.99" }).as_object().unwrap(),
        );
        m.command(
            &mut cx,
            3,
            "get",
            json!({ "oids": "1.3.6.1.2.1.1.5.0" }).as_object().unwrap(),
        );
        let actions = run(&mut m, &mut a, cx.take());
        let result = |want: CommandId| {
            actions.iter().find_map(|x| match x {
                Action::Complete {
                    id,
                    result: Ok(Outcome::Value { value }),
                } if *id == want => Some(value.clone()),
                _ => None,
            })
        };
        assert_eq!(result(2).unwrap()["varbinds"].as_array().unwrap().len(), 2);
        assert_eq!(result(3).unwrap()["varbinds"][0]["hex"], "7064752d31");
        // A bad type is refused before anything is sent.
        let mut cx = Cx::new(400);
        m.command(
            &mut cx,
            4,
            "set",
            json!({ "oid": "1.3.6.1.4.1.99.1.2.0", "type": "integer", "value": "x" })
                .as_object()
                .unwrap(),
        );
        assert!(matches!(
            &cx.take()[..],
            [Action::Complete {
                result: Err(CommandError::InvalidParams { .. }),
                ..
            }]
        ));
    }

    #[test]
    fn commands_only_polls_nothing_but_the_liveness_check() {
        let mut m = open(
            json!({ "poll_oids": "v=1.3.6.1.4.1.99.1.1.0" }),
            HOST,
            false,
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let sends: Vec<Vec<u8>> = cx
            .take()
            .into_iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } => Some(data),
                _ => None,
            })
            .collect();
        assert_eq!(sends.len(), 1);
        match snmp::decode(&sends[0]).unwrap() {
            snmp::Message::Community {
                pdu: snmp::AnyPdu::Pdu(p),
                ..
            } => {
                assert_eq!(p.varbinds.len(), 1);
                assert_eq!(p.varbinds[0].oid.to_string(), "1.3.6.1.2.1.1.3.0");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_unanswered_poll_disconnects_and_v3_refusals_are_unauthorized() {
        let mut m = open(json!({ "retries": 0, "timeout_ms": 500 }), HOST, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(500);
        m.timer(&mut cx, TIMER);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        let mut cx = Cx::new(0);
        m.down(&mut cx, &SnmpError::Auth("usmStatsUnknownUserNames".into()));
        assert!(matches!(
            &cx.take()[..],
            [Action::Connection(Connection::Unauthorized { .. })]
        ));
    }

    #[test]
    fn traps_become_message_events_and_state() {
        let mut m = open(json!({ "trap_port": 1162 }), HOST, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx.take().contains(&Action::UdpOpen {
            socket: TRAPS,
            bind: Bind::Shared(1162)
        }));
        let mut a = agent();
        let trap = a.notification(
            false,
            1,
            "1.3.6.1.4.1.99.0.5",
            vec![VarBind {
                oid: Oid::parse("1.3.6.1.4.1.99.1.1.0").unwrap(),
                value: Value::Gauge32(0),
            }],
            None,
        );
        let mut cx = Cx::new(10);
        m.datagram(&mut cx, TRAPS, SocketAddr::new(HOST, 40000), &trap);
        let actions = cx.take();
        let event = actions.iter().find_map(|x| match x {
            Action::Message { address, args, .. } => Some((address.clone(), args.clone())),
            _ => None,
        });
        let (address, args) = event.unwrap();
        assert_eq!(address, "1.3.6.1.4.1.99.0.5");
        assert_eq!(args["kind"], "trap");
        assert_eq!(args["varbinds"][0]["value"], 0);
        assert_eq!(state(&actions)["traps"]["count"], 1);
        // Another sender is not this device's agent.
        let mut cx = Cx::new(11);
        m.datagram(&mut cx, TRAPS, "192.0.2.99:40000".parse().unwrap(), &trap);
        assert!(cx.take().is_empty());
    }

    #[test]
    fn opened_without_a_host_it_receives_from_any_sender() {
        let no_host = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        let settings = json!({ "trap_port": 1162, "trap_bind_address": "127.0.0.1" });
        let mut m = open(settings, no_host, true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let actions = cx.take();
        assert!(actions.contains(&Action::UdpOpen {
            socket: TRAPS,
            bind: Bind::Any {
                address: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
                port: 1162
            }
        }));
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        let mut cx = Cx::new(1);
        m.command(
            &mut cx,
            1,
            "get",
            json!({ "oids": "1.3.6.1.2.1.1.3.0" }).as_object().unwrap(),
        );
        assert!(matches!(
            &cx.take()[..],
            [Action::Complete {
                result: Err(CommandError::NotConnected),
                ..
            }]
        ));
        assert!(GenericSnmp::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: None,
            model: "generic".into(),
            channels: None,
            settings: Params::new(),
            monitor: true,
        })
        .is_err());
    }

    #[test]
    fn poll_oids_are_parsed_with_names() {
        let p = parse_polled("a=1.3.6.1.2.1.1.3.0; .1.3.6.1.2.1.1.5.0\tb-2=1.3.6.1.4.1.9").unwrap();
        assert_eq!(
            p.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
            ["a", "1.3.6.1.2.1.1.5.0", "b-2"]
        );
        assert!(parse_polled("a=1.3.6, a=1.3.7").is_err());
        assert!(parse_polled("bad name=1.3.6").is_err());
    }
}
