//! AES70 (OCA) devices over OCP.1 on TCP, as a controller: remote preamps on
//! AES67 and ST 2110 stageboxes, d&b audiotechnik amplifiers and DS
//! processors, and any other device that speaks OCP.1.
//!
//! The AES70 standard itself (AES70-1/2/3) is sold by the AES and was not
//! used. The module was written from what open implementations and the OCA
//! Alliance publish: tschiemer/ocac (MIT) and its class reference, PADL's
//! SwiftOCA (Apache-2.0), the OCA Alliance's OCAMicro reference device and
//! its AES70 pages; see the spec's sources. The layers are in their own
//! files: [`codec`] is OCP.1 framing and the base datatypes, [`classes`]
//! the classes and properties the module knows.
//!
//! - A device is a tree of objects, each with an object number (ONo) and a
//!   class. The root block is ONo 100; the device manager is ONo 1 and the
//!   subscription manager ONo 4. A block lists its members with GetMembers
//!   (3.5): their object numbers and class identifications. Every object
//!   has a role (GetRole, 1.5), a name unique within its block; the roles
//!   from the root down make a role path such as "Input 1/Gain".
//! - A command names its target object, a method id (level.index) and its
//!   parameters, and carries a handle; the device answers with a response
//!   carrying the handle and a status. Several messages may share a PDU.
//! - AddSubscription (3.1) on the subscription manager asks the device to
//!   send an object's PropertyChanged events (1.1) on this connection; each
//!   carries the property id, its new value and what changed.
//! - The controller sends a keep-alive with its heartbeat time; the device
//!   sends keep-alives at that interval when it has nothing else to send, and
//!   either side takes three intervals without hearing anything as the
//!   connection lost.
//!
//! On connecting the module walks the root block: it asks every block for
//! its members and every object for its role, then reads the properties of
//! the classes it knows and subscribes to their changes. State is the tree,
//! under `objects` keyed by role path, and the device manager's identity
//! under `device`. Commands address an object by role path or by ONo.
//!
//! Opened for commands only (`monitor` false), it walks nothing, reads
//! nothing and subscribes to nothing: a command addressing an object by role
//! path looks up just that path (GetMembers and GetRole on the blocks on the
//! way), and one addressing it by ONo asks only for its class when the
//! command needs it. Keep-alives stay, as the liveness check.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

#[path = "aes70_classes.rs"]
mod classes;
#[path = "aes70_codec.rs"]
mod codec;

use classes::{Prop, Ty};
use codec::{Command, Id, KeepAlive, Message, Reader, Writer};

const SOCKET: Key = "ocp1";
const KEEPALIVE: Key = "keepalive";
const REPLY: Key = "reply";
const RETRY: Key = "retry";
const POLL: Key = "poll";
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

pub(crate) const ROOT_BLOCK: u32 = 100;
pub(crate) const DEVICE_MANAGER_ONO: u32 = 1;
pub(crate) const SUBSCRIPTION_MANAGER_ONO: u32 = 4;

pub(crate) const GET_CLASS_IDENTIFICATION: Id = Id(1, 1);
pub(crate) const GET_ROLE: Id = Id(1, 5);
pub(crate) const GET_MEMBERS: Id = Id(3, 5);
pub(crate) const ADD_SUBSCRIPTION: Id = Id(3, 1);
pub(crate) const PROPERTY_CHANGED: Id = Id(1, 1);
/// The subscriber method named in each subscription. A controller has no
/// objects of its own; the device hands it back in each notification and
/// the module ignores it. 4096 is the first object number AES70 does not
/// reserve.
pub(crate) const SUBSCRIBER: (u32, Id) = (4096, Id(1, 1));

/// Messages per PDU the module sends at most.
const PDU_MESSAGES: usize = 64;

/// OcaStatus: its name and meaning.
pub(crate) fn status_name(status: u8) -> (&'static str, &'static str) {
    match status {
        0 => ("OK", "success"),
        1 => (
            "ProtocolVersionError",
            "the device cannot handle the protocol version",
        ),
        2 => ("DeviceError", "an internal device error"),
        3 => ("Locked", "the object is locked"),
        4 => ("BadFormat", "a parameter was in an invalid format"),
        5 => ("BadONo", "no object has that object number"),
        6 => ("ParameterError", "a parameter was unacceptable or missing"),
        7 => ("ParameterOutOfRange", "a parameter was out of range"),
        8 => ("NotImplemented", "the device does not implement the method"),
        9 => (
            "InvalidRequest",
            "the request is invalid in the current context",
        ),
        10 => (
            "ProcessingFailed",
            "processing failed, not through a device error",
        ),
        11 => ("BadMethod", "no such method"),
        12 => ("PartiallySucceeded", "the command partly succeeded"),
        13 => ("Timeout", "the device timed out processing the command"),
        14 => ("BufferOverflow", "the device had no room for the PDU"),
        15 => ("PermissionDenied", "the controller lacks permission"),
        16 => (
            "OutOfMemory",
            "the device had no memory to process the command",
        ),
        17 => (
            "Busy",
            "the device is temporarily unable to process the request",
        ),
        _ => ("Unknown", "an unknown status"),
    }
}

fn device_error(status: u8) -> CommandError {
    let (name, meaning) = status_name(status);
    CommandError::DeviceError {
        code: Some(status.to_string()),
        message: format!("{name}: {meaning}"),
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

// --- Settings --------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Levels {
    Off,
    Read,
    Poll,
    Subscribe,
}

#[derive(Debug, Clone)]
struct Settings {
    keepalive: Millis,
    request_timeout: Millis,
    walk: bool,
    walk_depth: u32,
    subscribe: bool,
    levels: Levels,
    level_poll: Millis,
    max_in_flight: usize,
}

impl Settings {
    fn from(p: &Params) -> Settings {
        let int = |k: &str, d: i64| p.get(k).and_then(Value::as_i64).unwrap_or(d);
        let flag = |k: &str, d: bool| p.get(k).and_then(Value::as_bool).unwrap_or(d);
        Settings {
            keepalive: int("keepalive_interval_ms", 1000).max(100) as Millis,
            request_timeout: int("request_timeout_ms", 5000).max(100) as Millis,
            walk: flag("walk", true),
            walk_depth: int("walk_depth", i64::from(u32::MAX)).clamp(1, i64::from(u32::MAX)) as u32,
            subscribe: flag("subscribe", true),
            levels: match p.get("level_sensors").and_then(Value::as_str) {
                Some("off") => Levels::Off,
                Some("read") => Levels::Read,
                Some("subscribe") => Levels::Subscribe,
                _ => Levels::Poll,
            },
            level_poll: int("level_poll_ms", 1000).max(50) as Millis,
            max_in_flight: int("max_in_flight", 32).clamp(1, 1024) as usize,
        }
    }
}

// --- Objects ---------------------------------------------------------------

/// What the module knows of one object.
#[derive(Debug, Clone, Default)]
struct Obj {
    class: Option<Vec<u16>>,
    version: u16,
    /// The block it is a member of.
    parent: Option<u32>,
    role: Option<String>,
    /// Its role path, once known; the key under `objects`.
    path: Option<String>,
    /// Block levels below the root block (its members are depth 1).
    depth: u32,
    /// Its OcaTimeInterval values are 64-bit floats.
    wide_time: bool,
    /// Its record is in state.
    emitted: bool,
    /// Last known property values, by state key.
    values: Map<String, Value>,
}

/// Where a command's result goes.
#[derive(Debug, Clone)]
enum Then {
    /// A setter: acknowledged; the value is then put in state.
    Set {
        ono: u32,
        prop: &'static Prop,
        value: Value,
    },
    /// A getter: the decoded value.
    Get { ono: u32, prop: &'static Prop },
    /// call_method: the status and the parameters as hex.
    Raw,
}

#[derive(Debug, Clone)]
enum Purpose {
    Members {
        block: u32,
        depth: u32,
        walk: bool,
    },
    Role {
        ono: u32,
        walk: bool,
    },
    Class {
        ono: u32,
    },
    Read {
        ono: u32,
        prop: &'static Prop,
        poll: bool,
    },
    Subscribe {
        ono: u32,
    },
    Command {
        id: CommandId,
        then: Then,
    },
    /// One property of get_device_info.
    Info {
        id: CommandId,
        prop: &'static Prop,
    },
}

#[derive(Debug)]
struct Request {
    target: u32,
    method: Id,
    count: u8,
    params: Vec<u8>,
    purpose: Purpose,
}

#[derive(Debug)]
struct InFlight {
    purpose: Purpose,
    target: u32,
    method: Id,
    sent: Millis,
    deadline: Millis,
}

/// A command waiting for the objects it names to be looked up.
#[derive(Debug)]
struct Deferred {
    id: CommandId,
    name: String,
    params: Params,
    deadline: Millis,
}

/// What a lookup found.
enum Found {
    Ono(u32),
    Wait,
}

pub(crate) struct Aes70 {
    device: SocketAddr,
    monitor: bool,
    settings: Settings,
    deframer: codec::Deframer,
    socket_open: bool,
    connected: bool,
    retry_after: Millis,
    last_heard: Millis,
    last_sent: Millis,
    next_handle: u32,
    queue: VecDeque<Request>,
    in_flight: BTreeMap<u32, InFlight>,
    outbox: Vec<Vec<u8>>,
    deferred: Vec<Deferred>,
    objects: BTreeMap<u32, Obj>,
    members: BTreeMap<u32, Vec<u32>>,
    paths: BTreeMap<String, u32>,
    failed: BTreeSet<u32>,
    asked_members: BTreeSet<u32>,
    asked_role: BTreeSet<u32>,
    asked_class: BTreeSet<u32>,
    subscribed: BTreeSet<u32>,
    polling: BTreeSet<u32>,
    walking: bool,
    walked: bool,
    /// Read values and subscribe once the walk is done.
    walk_reads: bool,
    walk_pending: usize,
    walk_waiters: Vec<CommandId>,
    gathers: BTreeMap<CommandId, (usize, Map<String, Value>)>,
    device_values: Map<String, Value>,
    subscribe_warned: bool,
    /// Objects found by a command's lookup, whose records go into state.
    to_emit: Vec<u32>,
}

impl Aes70 {
    pub(crate) fn new(ctx: OpenContext, port: u16) -> Aes70 {
        Aes70 {
            device: SocketAddr::new(ctx.host, port),
            monitor: ctx.monitor,
            settings: Settings::from(&ctx.settings),
            deframer: codec::Deframer::default(),
            socket_open: false,
            connected: false,
            retry_after: RETRY_MIN,
            last_heard: 0,
            last_sent: 0,
            next_handle: 1,
            queue: VecDeque::new(),
            in_flight: BTreeMap::new(),
            outbox: Vec::new(),
            deferred: Vec::new(),
            objects: BTreeMap::new(),
            members: BTreeMap::new(),
            paths: BTreeMap::new(),
            failed: BTreeSet::new(),
            asked_members: BTreeSet::new(),
            asked_role: BTreeSet::new(),
            asked_class: BTreeSet::new(),
            subscribed: BTreeSet::new(),
            polling: BTreeSet::new(),
            walking: false,
            walked: false,
            walk_reads: false,
            walk_pending: 0,
            walk_waiters: Vec::new(),
            gathers: BTreeMap::new(),
            device_values: Map::new(),
            subscribe_warned: false,
            to_emit: Vec::new(),
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    // --- Sending -----------------------------------------------------------

    fn request(&mut self, target: u32, method: Id, count: u8, params: Vec<u8>, purpose: Purpose) {
        self.queue.push_back(Request {
            target,
            method,
            count,
            params,
            purpose,
        });
    }

    /// A command's request goes ahead of the walk and the reads.
    fn request_first(
        &mut self,
        target: u32,
        method: Id,
        count: u8,
        params: Vec<u8>,
        purpose: Purpose,
    ) {
        self.queue.push_front(Request {
            target,
            method,
            count,
            params,
            purpose,
        });
    }

    /// Move queued requests into flight, up to the limit.
    fn pump(&mut self, cx: &mut Cx) {
        if !self.socket_open {
            return;
        }
        let mut sent = false;
        while self.in_flight.len() < self.settings.max_in_flight {
            let Some(req) = self.queue.pop_front() else {
                break;
            };
            let handle = self.next_handle;
            self.next_handle = self.next_handle.wrapping_add(1).max(1);
            self.outbox.push(
                Command {
                    handle,
                    target: req.target,
                    method: req.method,
                    param_count: req.count,
                    params: req.params,
                }
                .encode(),
            );
            self.in_flight.insert(
                handle,
                InFlight {
                    purpose: req.purpose,
                    target: req.target,
                    method: req.method,
                    sent: cx.now(),
                    deadline: cx.now() + self.settings.request_timeout,
                },
            );
            sent = true;
        }
        if sent {
            self.arm_reply_timer(cx);
        }
    }

    /// Send what is in the outbox, several commands to a PDU.
    fn flush(&mut self, cx: &mut Cx) {
        if self.outbox.is_empty() || !self.socket_open {
            self.outbox.clear();
            return;
        }
        for chunk in std::mem::take(&mut self.outbox).chunks(PDU_MESSAGES) {
            cx.tcp_send(SOCKET, codec::pdu(codec::TYPE_COMMAND_RRQ, chunk));
        }
        self.last_sent = cx.now();
    }

    fn arm_reply_timer(&self, cx: &mut Cx) {
        let next = self
            .in_flight
            .values()
            .map(|f| f.deadline)
            .chain(self.deferred.iter().map(|d| d.deadline))
            .min();
        match next {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now()).max(1)),
            None => cx.cancel_timer(REPLY),
        }
    }

    fn send_keepalive(&mut self, cx: &mut Cx) {
        cx.tcp_send(
            SOCKET,
            KeepAlive::for_interval(self.settings.keepalive).pdu(),
        );
        self.last_sent = cx.now();
    }

    // --- Connection --------------------------------------------------------

    /// Forget everything the connection held: the device may have been
    /// reconfigured, and subscriptions end with the connection.
    fn reset(&mut self) {
        self.deframer = codec::Deframer::default();
        self.queue.clear();
        self.in_flight.clear();
        self.outbox.clear();
        self.objects.clear();
        self.members.clear();
        self.paths.clear();
        self.failed.clear();
        self.asked_members.clear();
        self.asked_role.clear();
        self.asked_class.clear();
        self.subscribed.clear();
        self.polling.clear();
        self.to_emit.clear();
        self.walking = false;
        self.walked = false;
        self.walk_pending = 0;
    }

    fn fail_waiting(&mut self, cx: &mut Cx, error: &CommandError) {
        for (_, f) in std::mem::take(&mut self.in_flight) {
            if let Purpose::Command { id, .. } = f.purpose {
                cx.complete(id, Err(error.clone()));
            }
        }
        for r in std::mem::take(&mut self.queue) {
            if let Purpose::Command { id, .. } = r.purpose {
                cx.complete(id, Err(error.clone()));
            }
        }
        for d in std::mem::take(&mut self.deferred) {
            cx.complete(d.id, Err(error.clone()));
        }
        for (id, _) in std::mem::take(&mut self.gathers) {
            cx.complete(id, Err(error.clone()));
        }
        for id in std::mem::take(&mut self.walk_waiters) {
            cx.complete(id, Err(error.clone()));
        }
    }

    /// The connection failed or ended: try again, backing off to 30 s.
    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.fail_waiting(
            cx,
            &CommandError::Transport {
                message: reason.clone(),
            },
        );
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.connected = false;
        self.reset();
        for key in [KEEPALIVE, REPLY, POLL] {
            cx.cancel_timer(key);
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn connected_socket(&mut self, cx: &mut Cx) {
        self.reset();
        self.socket_open = true;
        self.last_heard = cx.now();
        // The keep-alive tells the device the heartbeat time it should keep
        // to; it is sent before anything else.
        self.send_keepalive(cx);
        cx.set_timer(KEEPALIVE, self.settings.keepalive);
        if self.monitor {
            self.read_device_manager();
            if self.settings.subscribe {
                self.subscribe(DEVICE_MANAGER_ONO);
            }
            if self.settings.walk {
                self.start_walk(cx, true);
            }
        }
    }

    // --- Walking -----------------------------------------------------------

    /// Walk the tree from the root block, and with `reads` read and follow
    /// the known objects once it is done.
    fn start_walk(&mut self, cx: &mut Cx, reads: bool) {
        self.walking = true;
        self.walk_reads = reads;
        self.walk_pending = 0;
        self.objects.retain(|ono, _| *ono == DEVICE_MANAGER_ONO);
        self.members.clear();
        self.paths.clear();
        self.failed.clear();
        self.asked_members.clear();
        self.asked_role.clear();
        self.asked_class.clear();
        cx.state(json!({"objects": null, "discovery": {"complete": false}}));
        self.ask_members(ROOT_BLOCK, 0, true);
    }

    fn ask_members(&mut self, block: u32, depth: u32, walk: bool) {
        if self.asked_members.insert(block) {
            if walk {
                self.walk_pending += 1;
            }
            self.request(
                block,
                GET_MEMBERS,
                0,
                vec![],
                Purpose::Members { block, depth, walk },
            );
        }
    }

    fn ask_role(&mut self, ono: u32, walk: bool) {
        if self.asked_role.insert(ono) {
            if walk {
                self.walk_pending += 1;
            }
            self.request(ono, GET_ROLE, 0, vec![], Purpose::Role { ono, walk });
        }
    }

    fn ask_class(&mut self, ono: u32) {
        if self.asked_class.insert(ono) {
            self.request(
                ono,
                GET_CLASS_IDENTIFICATION,
                0,
                vec![],
                Purpose::Class { ono },
            );
        }
    }

    fn walk_step_done(&mut self, cx: &mut Cx, walk: bool) {
        if walk {
            self.walk_pending = self.walk_pending.saturating_sub(1);
        }
        if self.walking && self.walk_pending == 0 {
            self.finish_walk(cx);
        }
    }

    /// The role path of an object, from its parents' roles.
    fn compute_path(&self, ono: u32) -> Option<String> {
        let o = self.objects.get(&ono)?;
        if let Some(path) = &o.path {
            return Some(path.clone());
        }
        let role = o.role.as_ref()?;
        match o.parent? {
            ROOT_BLOCK => Some(role.clone()),
            parent => Some(format!("{}/{role}", self.compute_path(parent)?)),
        }
    }

    /// Give an object its role path, if it can have one yet, and put its
    /// record in state.
    fn place(&mut self, cx: &mut Cx, ono: u32) {
        let Some(o) = self.objects.get(&ono) else {
            return;
        };
        if o.path.is_some() {
            return;
        }
        let Some(mut path) = self.compute_path(ono) else {
            return;
        };
        if self.paths.get(&path).is_some_and(|&other| other != ono) {
            // Roles are meant to be unique within a block; when they are
            // not, the later object is told apart by its number.
            path = format!("{path}#{ono}");
        }
        self.paths.insert(path.clone(), ono);
        let o = self.objects.get_mut(&ono).expect("present");
        o.path = Some(path.clone());
        if !o.emitted {
            o.emitted = true;
            cx.state(json!({"objects": {path: object_record(ono, o)}}));
        }
    }

    fn finish_walk(&mut self, cx: &mut Cx) {
        self.walking = false;
        self.walked = true;
        // Parents first, so a child's path builds on its parent's.
        let mut order: Vec<(u32, u32)> = self
            .objects
            .iter()
            .filter(|(ono, _)| **ono != DEVICE_MANAGER_ONO)
            .map(|(ono, o)| (o.depth, *ono))
            .collect();
        order.sort();
        for (_, ono) in &order {
            self.place(cx, *ono);
        }
        let placed = self.objects.values().filter(|o| o.path.is_some()).count();
        cx.state(json!({"discovery": {"complete": true, "objects": placed}}));
        cx.log(
            Level::Info,
            format!("found {placed} objects under the root block"),
        );
        for id in std::mem::take(&mut self.walk_waiters) {
            cx.complete(id, Ok(Outcome::Ack));
        }
        if self.walk_reads {
            self.read_and_follow(cx);
        }
    }

    /// Read every known object's properties and subscribe to its changes.
    fn read_and_follow(&mut self, cx: &mut Cx) {
        let known: Vec<(u32, Vec<u16>)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.path.is_some())
            .filter_map(|(ono, o)| o.class.clone().map(|c| (*ono, c)))
            .filter(|(_, c)| classes::is_known(c))
            .collect();
        for (ono, class) in known {
            let level = classes::is_a(&class, classes::LEVEL_SENSOR);
            if level && self.settings.levels == Levels::Off {
                continue;
            }
            for prop in classes::props_of(&class) {
                if prop.read {
                    if let Some(get) = prop.get {
                        self.request(
                            ono,
                            get,
                            0,
                            vec![],
                            Purpose::Read {
                                ono,
                                prop,
                                poll: false,
                            },
                        );
                    }
                }
            }
            let follow = !level || self.settings.levels == Levels::Subscribe;
            if follow && self.settings.subscribe {
                self.subscribe(ono);
            }
        }
        if self.settings.levels == Levels::Poll && self.has_level_sensors() {
            cx.set_timer(POLL, self.settings.level_poll);
        }
    }

    fn has_level_sensors(&self) -> bool {
        self.objects.values().any(|o| {
            o.path.is_some()
                && o.class
                    .as_deref()
                    .is_some_and(|c| classes::is_a(c, classes::LEVEL_SENSOR))
        })
    }

    fn read_device_manager(&mut self) {
        for prop in classes::props_of(classes::DEVICE_MANAGER) {
            if prop.read {
                if let Some(get) = prop.get {
                    self.request(
                        DEVICE_MANAGER_ONO,
                        get,
                        0,
                        vec![],
                        Purpose::Read {
                            ono: DEVICE_MANAGER_ONO,
                            prop,
                            poll: false,
                        },
                    );
                }
            }
        }
    }

    /// AddSubscription: the object's PropertyChanged events, delivered
    /// reliably on this connection.
    fn subscribe(&mut self, ono: u32) {
        if self.subscribed.contains(&ono) {
            return;
        }
        self.subscribed.insert(ono);
        self.request(
            SUBSCRIPTION_MANAGER_ONO,
            ADD_SUBSCRIPTION,
            5,
            subscription_params(ono),
            Purpose::Subscribe { ono },
        );
    }

    // --- Lookups -----------------------------------------------------------

    fn class_of(&self, ono: u32) -> Option<&[u16]> {
        if ono == DEVICE_MANAGER_ONO {
            return Some(classes::DEVICE_MANAGER);
        }
        self.objects.get(&ono)?.class.as_deref()
    }

    /// The object a command names, by `object` (role path) or `ono`. With
    /// `need_class`, an object named by number is looked up too.
    fn target(&mut self, p: &Params, need_class: bool) -> Result<Found, CommandError> {
        let path = p.get("object").and_then(Value::as_str);
        let ono = p.get("ono").and_then(Value::as_u64);
        match (path, ono) {
            (Some(_), Some(_)) => Err(invalid("give either 'object' or 'ono', not both")),
            (None, None) => Err(invalid("give 'object' (a role path) or 'ono'")),
            (None, Some(ono)) => {
                let ono = u32::try_from(ono).map_err(|_| invalid("'ono' is out of range"))?;
                if !need_class || self.class_of(ono).is_some() {
                    Ok(Found::Ono(ono))
                } else if self.failed.contains(&ono) {
                    self.asked_class.remove(&ono);
                    self.failed.remove(&ono);
                    Err(invalid(format!(
                        "the device did not say what object {ono} is (no such object?)"
                    )))
                } else if self.walked && !self.walking && self.monitor && self.settings.walk {
                    // The walk found every object in the tree.
                    Err(invalid(format!(
                        "no object {ono} was found in the device's tree"
                    )))
                } else {
                    self.ask_class(ono);
                    Ok(Found::Wait)
                }
            }
            (Some(path), None) => self.resolve_path(path),
        }
    }

    /// Look a role path up, asking the blocks on the way for what is missing.
    fn resolve_path(&mut self, path: &str) -> Result<Found, CommandError> {
        if let Some(&ono) = self.paths.get(path) {
            return Ok(Found::Ono(ono));
        }
        let segments: Vec<&str> = path.split('/').collect();
        if segments.iter().any(|s| s.is_empty()) {
            return Err(invalid(format!("'{path}' is not a role path")));
        }
        let mut block = ROOT_BLOCK;
        for (i, segment) in segments.iter().enumerate() {
            let Some(members) = self.members.get(&block).cloned() else {
                if self.walking && self.asked_members.contains(&block) {
                    return Ok(Found::Wait);
                }
                let depth = self.objects.get(&block).map_or(0, |o| o.depth);
                self.ask_members(block, depth, false);
                return Ok(Found::Wait);
            };
            let mut waiting = false;
            let mut hit = None;
            for m in members {
                let o = self.objects.get(&m);
                match o.and_then(|o| o.role.as_deref()) {
                    Some(role) if role == *segment => {
                        hit = Some(m);
                        break;
                    }
                    Some(_) => {}
                    None if self.failed.contains(&m) => {}
                    None => {
                        self.ask_role(m, false);
                        waiting = true;
                    }
                }
            }
            let Some(m) = hit else {
                if waiting {
                    return Ok(Found::Wait);
                }
                let within = segments[..i].join("/");
                return Err(invalid(if within.is_empty() {
                    format!("no object with role '{segment}' in the root block")
                } else {
                    format!("no object with role '{segment}' in '{within}'")
                }));
            };
            if i + 1 == segments.len() {
                if let Some(o) = self.objects.get_mut(&m) {
                    if o.path.is_none() {
                        o.path = Some(path.to_string());
                        self.paths.insert(path.to_string(), m);
                        self.to_emit.push(m);
                    }
                }
                return Ok(Found::Ono(m));
            }
            let is_block = self
                .class_of(m)
                .is_some_and(|c| classes::is_a(c, classes::BLOCK));
            if !is_block {
                return Err(invalid(format!(
                    "'{}' is not a block",
                    segments[..=i].join("/")
                )));
            }
            block = m;
        }
        unreachable!("a path has a segment")
    }

    fn describe(&self, ono: u32) -> String {
        match self.objects.get(&ono).and_then(|o| o.path.as_deref()) {
            Some(path) => format!("'{path}' (object {ono})"),
            None => format!("object {ono}"),
        }
    }

    fn class_name(class: &[u16]) -> String {
        classes::exact_name(class)
            .map(str::to_string)
            .unwrap_or_else(|| match classes::kind(class) {
                Some(kind) => format!("{} (a {kind})", codec::class_text(class)),
                None => codec::class_text(class),
            })
    }

    // --- Commands ----------------------------------------------------------

    /// Run a command: `Ok(true)` when it was sent or answered, `Ok(false)`
    /// when it waits for a lookup.
    fn run(
        &mut self,
        cx: &mut Cx,
        id: CommandId,
        name: &str,
        p: &Params,
    ) -> Result<bool, CommandError> {
        let value = |k: &str| {
            p.get(k)
                .cloned()
                .ok_or_else(|| invalid(format!("'{k}' is required")))
        };
        match name {
            "set_gain" => self.set(id, p, &[classes::GAIN], Id(4, 1), value("gain_db")?),
            "set_mute" => self.set(id, p, &[classes::MUTE], Id(4, 1), value("muted")?),
            "set_polarity" => self.set(id, p, &[classes::POLARITY], Id(4, 1), value("inverted")?),
            "set_delay" => self.set(id, p, &[classes::DELAY], Id(4, 1), value("delay_s")?),
            "set_switch" => self.set_switch(id, p),
            "set_bool" => self.set(
                id,
                p,
                &[classes::BOOLEAN_ACTUATOR],
                Id(5, 1),
                value("value")?,
            ),
            "set_int" => self.set(id, p, classes::INT_ACTUATORS, Id(5, 1), value("value")?),
            "set_float" => self.set(id, p, classes::FLOAT_ACTUATORS, Id(5, 1), value("value")?),
            "set_string" => self.set(
                id,
                p,
                &[classes::STRING_ACTUATOR],
                Id(5, 1),
                value("value")?,
            ),
            "identify" => {
                let mut p = p.clone();
                if p.get("object").is_none() && p.get("ono").is_none() {
                    let first = self
                        .objects
                        .iter()
                        .find(|(_, o)| {
                            o.class
                                .as_deref()
                                .is_some_and(|c| classes::is_a(c, classes::IDENTIFICATION))
                        })
                        .map(|(ono, _)| *ono);
                    match first {
                        Some(ono) => {
                            p.insert("ono".into(), json!(ono));
                        }
                        None => {
                            return Err(invalid(
                                "no OcaIdentificationActuator is known; give 'object' or 'ono'",
                            ))
                        }
                    }
                }
                self.set(
                    id,
                    &p,
                    &[classes::IDENTIFICATION],
                    Id(4, 1),
                    value("enabled")?,
                )
            }
            "get_property" => self.get_property(id, p),
            "set_property" => self.set_property(id, p),
            "call_method" => self.call_method(id, p),
            "refresh_objects" => {
                if !self.walking {
                    self.start_walk(cx, self.monitor);
                }
                self.walk_waiters.push(id);
                Ok(true)
            }
            "get_device_info" => {
                let props: Vec<&'static Prop> = classes::props_of(classes::DEVICE_MANAGER)
                    .into_iter()
                    .filter(|p| p.class == classes::DEVICE_MANAGER && p.get.is_some())
                    .collect();
                self.gathers.insert(id, (props.len(), Map::new()));
                for prop in props {
                    self.request_first(
                        DEVICE_MANAGER_ONO,
                        prop.get.expect("filtered"),
                        0,
                        vec![],
                        Purpose::Info { id, prop },
                    );
                }
                Ok(true)
            }
            other => Err(CommandError::UnknownCommand {
                command: other.into(),
            }),
        }
    }

    /// The object, checked to be one of `family`; `None` while it is looked up.
    fn fitting(
        &mut self,
        p: &Params,
        family: &[&[u16]],
    ) -> Result<Option<(u32, Vec<u16>)>, CommandError> {
        let ono = match self.target(p, true)? {
            Found::Wait => return Ok(None),
            Found::Ono(ono) => ono,
        };
        let class = self.class_of(ono).expect("looked up").to_vec();
        if !family.is_empty() && !family.iter().any(|f| classes::is_a(&class, f)) {
            let wanted: Vec<String> = family.iter().map(|f| Self::class_name(f)).collect();
            return Err(invalid(format!(
                "{} is {}, not {}",
                self.describe(ono),
                Self::class_name(&class),
                wanted.join(" or ")
            )));
        }
        Ok(Some((ono, class)))
    }

    fn set(
        &mut self,
        id: CommandId,
        p: &Params,
        family: &[&[u16]],
        prop_id: Id,
        value: Value,
    ) -> Result<bool, CommandError> {
        let Some((ono, class)) = self.fitting(p, family)? else {
            return Ok(false);
        };
        let prop = classes::prop(&class, prop_id).expect("a known class's property");
        self.send_set(id, ono, prop, value)
    }

    fn send_set(
        &mut self,
        id: CommandId,
        ono: u32,
        prop: &'static Prop,
        value: Value,
    ) -> Result<bool, CommandError> {
        let set = prop
            .set
            .ok_or_else(|| invalid(format!("property {} ({}) is read-only", prop.id, prop.key)))?;
        let wide = self.objects.get(&ono).is_some_and(|o| o.wide_time);
        let bytes = classes::encode(prop.ty, &value, wide)
            .map_err(|e| invalid(format!("{}: {e}", prop.key)))?;
        self.request_first(
            ono,
            set,
            1,
            bytes,
            Purpose::Command {
                id,
                then: Then::Set { ono, prop, value },
            },
        );
        Ok(true)
    }

    fn set_switch(&mut self, id: CommandId, p: &Params) -> Result<bool, CommandError> {
        let Some((ono, class)) = self.fitting(p, &[classes::SWITCH])? else {
            return Ok(false);
        };
        let position = match (p.get("position"), p.get("position_name")) {
            (Some(n), None) => n.clone(),
            (None, Some(name)) => {
                let names = self
                    .objects
                    .get(&ono)
                    .and_then(|o| o.values.get("position_names"))
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        invalid(format!(
                            "the position names of {} are not known; give 'position'",
                            self.describe(ono)
                        ))
                    })?;
                let index = names.iter().position(|n| n == name).ok_or_else(|| {
                    invalid(format!(
                        "{} has no position named {name}; it has {}",
                        self.describe(ono),
                        Value::Array(names.clone())
                    ))
                })?;
                json!(index)
            }
            _ => return Err(invalid("give 'position' or 'position_name'")),
        };
        let prop = classes::prop(&class, Id(4, 1)).expect("OcaSwitch position");
        self.send_set(id, ono, prop, position)
    }

    fn property_of(
        &self,
        ono: u32,
        class: &[u16],
        p: &Params,
    ) -> Result<&'static Prop, CommandError> {
        let text = p
            .get("property")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("'property' is required"))?;
        let pid = Id::parse(text).ok_or_else(|| invalid("'property' is level.index, e.g. 4.1"))?;
        classes::prop(class, pid).ok_or_else(|| {
            invalid(format!(
                "the module does not know property {pid} of {}, a {}; use call_method",
                self.describe(ono),
                Self::class_name(class)
            ))
        })
    }

    fn get_property(&mut self, id: CommandId, p: &Params) -> Result<bool, CommandError> {
        let Some((ono, class)) = self.fitting(p, &[])? else {
            return Ok(false);
        };
        let prop = self.property_of(ono, &class, p)?;
        let get = prop
            .get
            .ok_or_else(|| invalid("the property has no getter"))?;
        self.request_first(
            ono,
            get,
            0,
            vec![],
            Purpose::Command {
                id,
                then: Then::Get { ono, prop },
            },
        );
        Ok(true)
    }

    fn set_property(&mut self, id: CommandId, p: &Params) -> Result<bool, CommandError> {
        let Some((ono, class)) = self.fitting(p, &[])? else {
            return Ok(false);
        };
        let prop = self.property_of(ono, &class, p)?;
        let value = p
            .get("value")
            .cloned()
            .ok_or_else(|| invalid("'value' is required"))?;
        self.send_set(id, ono, prop, value)
    }

    fn call_method(&mut self, id: CommandId, p: &Params) -> Result<bool, CommandError> {
        let ono = match self.target(p, false)? {
            Found::Wait => return Ok(false),
            Found::Ono(ono) => ono,
        };
        let method = p
            .get("method")
            .and_then(Value::as_str)
            .and_then(Id::parse)
            .ok_or_else(|| invalid("'method' is level.index, e.g. 4.2"))?;
        let params = codec::unhex(p.get("params_hex").and_then(Value::as_str).unwrap_or(""))
            .ok_or_else(|| invalid("'params_hex' is not hex"))?;
        let count = match p.get("param_count").and_then(Value::as_u64) {
            Some(n) => u8::try_from(n).map_err(|_| invalid("'param_count' is at most 255"))?,
            None => u8::from(!params.is_empty()),
        };
        self.request_first(
            ono,
            method,
            count,
            params,
            Purpose::Command {
                id,
                then: Then::Raw,
            },
        );
        Ok(true)
    }

    /// Commands waiting on lookups: run again those that can now go.
    fn retry_deferred(&mut self, cx: &mut Cx) {
        let now = cx.now();
        for d in std::mem::take(&mut self.deferred) {
            match self.run(cx, d.id, &d.name, &d.params) {
                Ok(true) => {}
                Ok(false) if d.deadline <= now => cx.complete(d.id, Err(CommandError::Timeout)),
                Ok(false) => self.deferred.push(d),
                Err(e) => cx.complete(d.id, Err(e)),
            }
        }
    }

    // --- State -------------------------------------------------------------

    /// Put a property value in state and in the module's record.
    fn put(&mut self, cx: &mut Cx, ono: u32, key: &str, value: Value) {
        if ono == DEVICE_MANAGER_ONO {
            self.device_values.insert(key.into(), value.clone());
            cx.state(json!({"device": {key: value}}));
            return;
        }
        let Some(o) = self.objects.get_mut(&ono) else {
            return;
        };
        o.values.insert(key.into(), value.clone());
        if let Some(path) = &o.path {
            cx.state(json!({"objects": {path.clone(): {key: value}}}));
        }
    }

    fn put_reply(&mut self, cx: &mut Cx, ono: u32, prop: &Prop, bytes: &[u8]) -> Option<Value> {
        match classes::decode_reply(prop, bytes) {
            Ok(classes::Reply {
                value,
                min,
                max,
                wide,
            }) => {
                if prop.ty == Ty::TimeInterval {
                    if let Some(o) = self.objects.get_mut(&ono) {
                        o.wide_time = wide;
                    }
                }
                self.put(cx, ono, prop.key, value.clone());
                if let (Some(min), Some(max)) = (min, max) {
                    self.put(cx, ono, &format!("{}_min", prop.key), min.clone());
                    self.put(cx, ono, &format!("{}_max", prop.key), max.clone());
                    return Some(json!({"value": value, "min": min, "max": max}));
                }
                Some(value)
            }
            Err(e) => {
                cx.log(
                    Level::Warning,
                    format!(
                        "could not read {} of {}: {e} ({})",
                        prop.key,
                        self.describe(ono),
                        codec::hex(bytes)
                    ),
                );
                None
            }
        }
    }

    // --- Receiving ---------------------------------------------------------

    fn data(&mut self, cx: &mut Cx, bytes: &[u8]) {
        self.last_heard = cx.now();
        cx.alive();
        if !self.connected {
            self.connected = true;
            self.retry_after = RETRY_MIN;
            cx.connection(Connection::Connected);
        }
        let (pdus, discarded) = self.deframer.feed(bytes);
        for why in discarded {
            cx.log(Level::Warning, format!("discarded bytes: {why}"));
        }
        for pdu in pdus {
            match codec::parse_pdu(&pdu) {
                Ok((_, messages)) => {
                    for m in messages {
                        self.message(cx, m);
                    }
                }
                Err(e) => cx.log(
                    Level::Warning,
                    format!("discarded a PDU: {e} ({})", codec::hex(&pdu)),
                ),
            }
        }
    }

    fn message(&mut self, cx: &mut Cx, m: Message) {
        match m {
            Message::Response(r) => self.response(cx, r),
            Message::Notification(n) => match n.event {
                Some(e) => self.event(cx, e),
                None => cx.log(Level::Debug, "a notification with no event"),
            },
            Message::Notification2(n) => {
                if n.kind == 0 {
                    self.event(cx, n.event);
                } else {
                    cx.log(
                        Level::Warning,
                        format!(
                            "the device reported an exception for object {}'s event {} ({})",
                            n.event.emitter,
                            n.event.event,
                            codec::hex(&n.event.data)
                        ),
                    );
                }
            }
            Message::KeepAlive(_) => {}
            Message::Command(c) => cx.log(
                Level::Debug,
                format!(
                    "ignored a command from the device (method {} on {})",
                    c.method, c.target
                ),
            ),
        }
    }

    fn event(&mut self, cx: &mut Cx, e: codec::Event) {
        if e.event != PROPERTY_CHANGED {
            cx.log(
                Level::Debug,
                format!("ignored event {} from object {}", e.event, e.emitter),
            );
            return;
        }
        let (pid, value, change) = match codec::property_changed(&e.data) {
            Ok(parts) => parts,
            Err(err) => {
                cx.log(Level::Warning, format!("a PropertyChanged event: {err}"));
                return;
            }
        };
        let ono = e.emitter;
        let prop = self.class_of(ono).and_then(|c| classes::prop(c, pid));
        let Some(prop) = prop else {
            // A property the module does not know: kept as bytes.
            if let Some(path) = self.objects.get(&ono).and_then(|o| o.path.clone()) {
                cx.state(json!({"objects": {path: {"raw": {pid.to_string(): codec::hex(value)}}}}));
            }
            return;
        };
        match change {
            1..=3 => match classes::decode_event_value(prop, value) {
                Ok((v, wide)) => {
                    if prop.ty == Ty::TimeInterval {
                        if let Some(o) = self.objects.get_mut(&ono) {
                            o.wide_time = wide;
                        }
                    }
                    let key = match change {
                        1 => prop.key.to_string(),
                        2 => format!("{}_min", prop.key),
                        _ => format!("{}_max", prop.key),
                    };
                    self.put(cx, ono, &key, v);
                }
                Err(err) => cx.log(
                    Level::Warning,
                    format!("{} of {}: {err}", prop.key, self.describe(ono)),
                ),
            },
            // An item of a list was added, changed or deleted: read it all.
            _ => {
                if let Some(get) = prop.get {
                    self.request(
                        ono,
                        get,
                        0,
                        vec![],
                        Purpose::Read {
                            ono,
                            prop,
                            poll: false,
                        },
                    );
                }
            }
        }
    }

    fn response(&mut self, cx: &mut Cx, r: codec::Response) {
        let Some(f) = self.in_flight.remove(&r.handle) else {
            cx.log(
                Level::Debug,
                format!("a response to handle {} that is not waited for", r.handle),
            );
            return;
        };
        cx.round_trip(cx.now().saturating_sub(f.sent));
        let ok = r.status == 0;
        let params = r.params;
        match f.purpose {
            Purpose::Members { block, depth, walk } => {
                // During a walk, a block asked for by a command is walked too.
                let descend = walk || self.walking;
                let members = if ok {
                    decode_members(&params)
                } else {
                    Err(status_name(r.status).0.to_string())
                };
                match members {
                    Ok(list) => {
                        let mut onos = Vec::with_capacity(list.len());
                        for (ono, class, version) in list {
                            let is_block = classes::is_a(&class, classes::BLOCK);
                            let o = self.objects.entry(ono).or_default();
                            o.class = Some(class);
                            o.version = version;
                            o.parent = Some(block);
                            o.depth = depth + 1;
                            onos.push(ono);
                            if descend {
                                self.ask_role(ono, true);
                                if is_block && depth + 1 < self.settings.walk_depth {
                                    self.ask_members(ono, depth + 1, true);
                                }
                            }
                        }
                        self.members.insert(block, onos);
                    }
                    Err(e) => {
                        cx.log(
                            Level::Warning,
                            format!("GetMembers on block {block} failed: {e}"),
                        );
                        self.members.insert(block, Vec::new());
                        self.failed.insert(block);
                    }
                }
                self.walk_step_done(cx, walk);
                self.retry_deferred(cx);
            }
            Purpose::Role { ono, walk } => {
                let role = if ok {
                    Reader::new(&params).string()
                } else {
                    Err(status_name(r.status).0.to_string())
                };
                match role {
                    Ok(role) => {
                        self.objects.entry(ono).or_default().role = Some(role);
                        if self.walked && !self.walking {
                            self.place(cx, ono);
                        }
                    }
                    Err(e) => {
                        cx.log(Level::Debug, format!("GetRole on object {ono} failed: {e}"));
                        self.failed.insert(ono);
                    }
                }
                self.walk_step_done(cx, walk);
                self.retry_deferred(cx);
            }
            Purpose::Class { ono } => {
                let class = if ok {
                    let mut rd = Reader::new(&params);
                    rd.class_id().map(|c| (c, rd.u16().unwrap_or(0)))
                } else {
                    Err(status_name(r.status).0.to_string())
                };
                match class {
                    Ok((class, version)) => {
                        let o = self.objects.entry(ono).or_default();
                        o.class = Some(class);
                        o.version = version;
                    }
                    Err(_) => {
                        self.failed.insert(ono);
                    }
                }
                self.asked_class.remove(&ono);
                self.retry_deferred(cx);
            }
            Purpose::Read { ono, prop, poll } => {
                if poll {
                    self.polling.remove(&ono);
                }
                if ok {
                    self.put_reply(cx, ono, prop, &params);
                } else {
                    cx.log(
                        Level::Debug,
                        format!(
                            "reading {} of {} failed: {}",
                            prop.key,
                            self.describe(ono),
                            status_name(r.status).0
                        ),
                    );
                }
            }
            Purpose::Subscribe { ono } => {
                if !ok {
                    let level = if self.subscribe_warned {
                        Level::Debug
                    } else {
                        Level::Warning
                    };
                    self.subscribe_warned = true;
                    cx.log(
                        level,
                        format!(
                            "the device refused a subscription to {}'s changes: {}",
                            self.describe(ono),
                            status_name(r.status).0
                        ),
                    );
                }
            }
            Purpose::Info { id, prop } => {
                let value = if ok {
                    self.put_reply(cx, DEVICE_MANAGER_ONO, prop, &params)
                        .unwrap_or(Value::Null)
                } else {
                    Value::Null
                };
                self.gathered(cx, id, prop.key, value);
            }
            Purpose::Command { id, then } => {
                let result = match then {
                    Then::Raw => Ok(Outcome::Value {
                        value: json!({
                            "status": r.status,
                            "status_name": status_name(r.status).0,
                            "param_count": r.param_count,
                            "params_hex": codec::hex(&params),
                        }),
                    }),
                    _ if !ok => Err(device_error(r.status)),
                    Then::Set { ono, prop, value } => {
                        self.put(cx, ono, prop.key, value);
                        Ok(Outcome::Ack)
                    }
                    Then::Get { ono, prop } => {
                        let value = self
                            .put_reply(cx, ono, prop, &params)
                            .unwrap_or_else(|| json!({"hex": codec::hex(&params)}));
                        Ok(Outcome::Value { value })
                    }
                };
                cx.complete(id, result);
            }
        }
    }

    fn gathered(&mut self, cx: &mut Cx, id: CommandId, key: &str, value: Value) {
        let Some((remaining, map)) = self.gathers.get_mut(&id) else {
            return;
        };
        map.insert(key.into(), value);
        *remaining -= 1;
        if *remaining == 0 {
            let (_, map) = self.gathers.remove(&id).expect("present");
            cx.complete(
                id,
                Ok(Outcome::Value {
                    value: Value::Object(map),
                }),
            );
        }
    }

    /// Requests past their deadline.
    fn expire(&mut self, cx: &mut Cx) {
        let now = cx.now();
        let late: Vec<u32> = self
            .in_flight
            .iter()
            .filter(|(_, f)| f.deadline <= now)
            .map(|(h, _)| *h)
            .collect();
        for handle in late {
            let f = self.in_flight.remove(&handle).expect("listed");
            match f.purpose {
                Purpose::Command { id, .. } => cx.complete(id, Err(CommandError::Timeout)),
                Purpose::Info { id, prop } => self.gathered(cx, id, prop.key, Value::Null),
                Purpose::Members { block, walk, .. } => {
                    cx.log(
                        Level::Warning,
                        format!("no answer to GetMembers on block {block}"),
                    );
                    self.members.insert(block, Vec::new());
                    self.failed.insert(block);
                    self.walk_step_done(cx, walk);
                }
                Purpose::Role { ono, walk } => {
                    self.failed.insert(ono);
                    self.walk_step_done(cx, walk);
                }
                Purpose::Class { ono } => {
                    self.failed.insert(ono);
                    self.asked_class.remove(&ono);
                }
                Purpose::Read { ono, poll, .. } => {
                    if poll {
                        self.polling.remove(&ono);
                    }
                    cx.log(
                        Level::Debug,
                        format!("no answer to method {} on object {}", f.method, f.target),
                    );
                }
                Purpose::Subscribe { .. } => cx.log(
                    Level::Debug,
                    format!("no answer to a subscription for object {}", f.target),
                ),
            }
        }
        self.retry_deferred(cx);
    }

    fn poll_levels(&mut self, cx: &mut Cx) {
        let sensors: Vec<(u32, Vec<u16>)> = self
            .objects
            .iter()
            .filter(|(ono, o)| o.path.is_some() && !self.polling.contains(ono))
            .filter_map(|(ono, o)| o.class.clone().map(|c| (*ono, c)))
            .filter(|(_, c)| classes::is_a(c, classes::LEVEL_SENSOR))
            .collect();
        for (ono, class) in sensors {
            if let Some(prop) = classes::prop(&class, Id(4, 1)) {
                self.polling.insert(ono);
                self.request(
                    ono,
                    prop.get.expect("a getter"),
                    0,
                    vec![],
                    Purpose::Read {
                        ono,
                        prop,
                        poll: true,
                    },
                );
            }
        }
        cx.set_timer(POLL, self.settings.level_poll);
    }

    /// After every callback: send what is queued.
    fn settle(&mut self, cx: &mut Cx) {
        for ono in std::mem::take(&mut self.to_emit) {
            if let Some(o) = self.objects.get_mut(&ono) {
                if let (false, Some(path)) = (o.emitted, o.path.clone()) {
                    o.emitted = true;
                    cx.state(json!({"objects": {path: object_record(ono, o)}}));
                }
            }
        }
        self.pump(cx);
        self.flush(cx);
        if !self.deferred.is_empty() {
            self.arm_reply_timer(cx);
        }
    }
}

/// AddSubscription's five parameters: the event (the object's
/// PropertyChanged), the subscriber method, an empty context, delivery mode
/// 1 (reliable, on this connection) and an empty destination address.
pub(crate) fn subscription_params(ono: u32) -> Vec<u8> {
    let mut w = Writer::new();
    w.u32(ono)
        .id(PROPERTY_CHANGED)
        .u32(SUBSCRIBER.0)
        .id(SUBSCRIBER.1);
    w.blob(&[]).expect("empty");
    w.u8(1);
    w.blob(&[]).expect("empty");
    w.buf
}

/// GetMembers' result: a list of object identifications, each the object
/// number, class id and class version.
fn decode_members(params: &[u8]) -> Result<Vec<(u32, Vec<u16>, u16)>, String> {
    let mut r = Reader::new(params);
    let n = r.u16()?;
    (0..n)
        .map(|_| Ok((r.u32()?, r.class_id()?, r.u16()?)))
        .collect()
}

fn object_record(ono: u32, o: &Obj) -> Value {
    let class = o.class.as_deref().unwrap_or(&[]);
    let mut record = json!({
        "ono": ono,
        "class": classes::exact_name(class)
            .map(str::to_string)
            .unwrap_or_else(|| codec::class_text(class)),
        "class_id": codec::class_text(class),
        "class_version": o.version,
    });
    if let Some(kind) = classes::kind(class) {
        record["kind"] = json!(kind);
    }
    let map = record.as_object_mut().expect("an object");
    for (k, v) in &o.values {
        map.insert(k.clone(), v.clone());
    }
    record
}

impl Module for Aes70 {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.socket_open {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.run(cx, id, name, params) {
            Ok(true) => {}
            Ok(false) => self.deferred.push(Deferred {
                id,
                name: name.to_string(),
                params: params.clone(),
                deadline: cx.now() + 4 * self.settings.request_timeout,
            }),
            Err(e) => cx.complete(id, Err(e)),
        }
        self.settle(cx);
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => self.connected_socket(cx),
            TcpInput::Data(data) => {
                if self.socket_open {
                    self.data(cx, &data);
                }
            }
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                self.lost(cx, reason);
            }
        }
        self.settle(cx);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        let now = cx.now();
        match key {
            RETRY => {
                if !self.socket_open {
                    self.open(cx);
                }
            }
            KEEPALIVE => {
                if !self.socket_open {
                    return;
                }
                let interval = self.settings.keepalive;
                let quiet = now.saturating_sub(self.last_heard);
                if quiet >= 3 * interval {
                    self.lost(
                        cx,
                        format!(
                            "nothing from the device for {quiet} ms, three keep-alive intervals"
                        ),
                    );
                    return;
                }
                if now.saturating_sub(self.last_sent) >= interval {
                    self.send_keepalive(cx);
                }
                let to_send = interval.saturating_sub(now.saturating_sub(self.last_sent));
                let to_lose = (3 * interval).saturating_sub(quiet);
                cx.set_timer(KEEPALIVE, to_send.min(to_lose).max(1));
            }
            REPLY => {
                self.expire(cx);
                self.arm_reply_timer(cx);
            }
            POLL if self.socket_open && self.monitor => self.poll_levels(cx),
            _ => {}
        }
        self.settle(cx);
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
    use super::codec::{pdu, Notification, Response, TYPE_NOTIFICATION, TYPE_RESPONSE};
    use super::*;
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn module(settings: Value, monitor: bool) -> Aes70 {
        Aes70::new(
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 70)),
                port: Some(50000),
                model: "aes70-device".into(),
                channels: None,
                settings: params(settings),
                monitor,
            },
            50000,
        )
    }

    /// The commands the module sent, from every PDU in the actions.
    fn sent(actions: &[Action]) -> Vec<Command> {
        let mut out = Vec::new();
        for a in actions {
            if let Action::TcpSend { data, .. } = a {
                let mut d = codec::Deframer::default();
                for p in d.feed(data).0 {
                    for m in codec::parse_pdu(&p).unwrap().1 {
                        if let Message::Command(c) = m {
                            out.push(c);
                        }
                    }
                }
            }
        }
        out
    }

    fn keepalives(actions: &[Action]) -> Vec<KeepAlive> {
        let mut out = Vec::new();
        for a in actions {
            if let Action::TcpSend { data, .. } = a {
                for p in codec::Deframer::default().feed(data).0 {
                    for m in codec::parse_pdu(&p).unwrap().1 {
                        if let Message::KeepAlive(k) = m {
                            out.push(k);
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

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn connect(m: &mut Aes70, now: Millis) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    fn feed(m: &mut Aes70, now: Millis, bytes: Vec<u8>) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes));
        cx.take()
    }

    fn run(m: &mut Aes70, now: Millis, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, &params(p));
        cx.take()
    }

    fn timer(m: &mut Aes70, now: Millis, key: Key) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.timer(&mut cx, key);
        cx.take()
    }

    // --- A small device ----------------------------------------------------

    /// A device: root block 100 holding "In1" (a block, 200) and "Out" (an
    /// OcaGain, 300); "In1" holds "Gain" (an OcaGain, 201), "Mute" (an
    /// OcaMute, 202), "Meter" (an OcaLevelSensor, 203) and "Ident" (an
    /// OcaIdentificationActuator, 204).
    struct Fake {
        gain: f32,
        muted: bool,
    }

    fn ident(ono: u32, class: &[u16]) -> Vec<u8> {
        let mut w = Writer::new();
        w.u32(ono).class_id(class).u16(2);
        w.buf
    }

    fn members_of(block: u32) -> Option<Vec<u8>> {
        let list: Vec<(u32, &[u16])> = match block {
            100 => vec![(200, classes::BLOCK), (300, classes::GAIN)],
            200 => vec![
                (201, classes::GAIN),
                (202, classes::MUTE),
                (203, classes::LEVEL_SENSOR),
                (204, classes::IDENTIFICATION),
            ],
            _ => return None,
        };
        let mut w = Writer::new();
        w.u16(list.len() as u16);
        for (ono, class) in list {
            w.bytes(&ident(ono, class));
        }
        Some(w.buf)
    }

    fn role_of(ono: u32) -> Option<&'static str> {
        Some(match ono {
            200 => "In1",
            300 => "Out",
            201 => "Gain",
            202 => "Mute",
            203 => "Meter",
            204 => "Ident",
            _ => return None,
        })
    }

    impl Fake {
        /// The status and parameters for a command.
        fn answer(&mut self, c: &Command) -> (u8, u8, Vec<u8>) {
            let mut w = Writer::new();
            match (c.target, c.method) {
                (100 | 200, GET_MEMBERS) => (0, 1, members_of(c.target).unwrap()),
                (_, GET_ROLE) => match role_of(c.target) {
                    Some(r) => {
                        w.string(r).unwrap();
                        (0, 1, w.buf)
                    }
                    None => (5, 0, vec![]),
                },
                (_, GET_CLASS_IDENTIFICATION) => {
                    let class = match c.target {
                        201 | 300 => classes::GAIN,
                        202 => classes::MUTE,
                        _ => return (5, 0, vec![]),
                    };
                    w.class_id(class).u16(2);
                    (0, 1, w.buf)
                }
                (201 | 300, Id(4, 1)) => {
                    w.f32(self.gain).f32(-120.0).f32(12.0);
                    (0, 3, w.buf)
                }
                (201 | 300, Id(4, 2)) => {
                    self.gain = Reader::new(&c.params).f32().unwrap();
                    (0, 0, vec![])
                }
                (202, Id(4, 1)) => (0, 1, vec![if self.muted { 1 } else { 2 }]),
                (202, Id(4, 2)) => {
                    self.muted = c.params == [1];
                    (0, 0, vec![])
                }
                (203, Id(4, 1)) => {
                    w.f32(-20.0).f32(-60.0).f32(0.0);
                    (0, 3, w.buf)
                }
                (204, Id(4, 1)) => (0, 1, vec![0]),
                (204, Id(4, 2)) => (0, 0, vec![]),
                (1, _) => (8, 0, vec![]),
                (SUBSCRIPTION_MANAGER_ONO, ADD_SUBSCRIPTION) => (0, 0, vec![]),
                _ => (11, 0, vec![]),
            }
        }

        /// The response PDU to every command in the actions.
        fn respond(&mut self, actions: &[Action]) -> Vec<u8> {
            let replies: Vec<Vec<u8>> = sent(actions)
                .iter()
                .map(|c| {
                    let (status, param_count, params) = self.answer(c);
                    Response {
                        handle: c.handle,
                        status,
                        param_count,
                        params,
                    }
                    .encode()
                })
                .collect();
            if replies.is_empty() {
                return vec![];
            }
            pdu(TYPE_RESPONSE, &replies)
        }
    }

    /// Connect, and answer until the device has nothing more to be asked.
    fn walked(settings: Value) -> (Aes70, Fake, Value, Vec<Command>) {
        let mut m = module(settings, true);
        let mut fake = Fake {
            gain: -6.0,
            muted: false,
        };
        let mut st = json!({});
        let mut actions = connect(&mut m, 0);
        let mut all = Vec::new();
        for round in 0..20 {
            state(&mut st, &actions);
            all.extend(sent(&actions));
            let reply = fake.respond(&actions);
            if reply.is_empty() {
                break;
            }
            actions = feed(&mut m, 10 + round, reply);
        }
        (m, fake, st, all)
    }

    #[test]
    fn walks_the_tree_reads_known_objects_and_subscribes() {
        let (_, _, st, all) = walked(json!({}));
        assert_eq!(st["discovery"], json!({"complete": true, "objects": 6}));
        assert_eq!(
            st["objects"]["In1/Gain"],
            json!({
                "ono": 201, "class": "OcaGain", "class_id": "1.1.1.5", "class_version": 2,
                "kind": "OcaGain", "gain_db": -6.0, "gain_db_min": -120.0, "gain_db_max": 12.0
            })
        );
        assert_eq!(st["objects"]["In1"]["class"], "OcaBlock");
        assert_eq!(st["objects"]["In1/Mute"]["muted"], false);
        assert_eq!(st["objects"]["In1/Meter"]["reading_db"], -20.0);
        assert_eq!(st["objects"]["In1/Ident"]["active"], false);
        assert_eq!(st["objects"]["Out"]["gain_db"], -6.0);
        // GetMembers on both blocks, a role for each of six objects.
        let count = |t: Option<u32>, m: Id| {
            all.iter()
                .filter(|c| t.is_none_or(|t| c.target == t) && c.method == m)
                .count()
        };
        assert_eq!(count(Some(100), GET_MEMBERS), 1);
        assert_eq!(count(Some(200), GET_MEMBERS), 1);
        assert_eq!(count(None, GET_ROLE), 6);
        // Subscriptions to the device manager, both gains, the mute and the
        // identification actuator; not the meter, which is polled.
        let subs: Vec<u32> = all
            .iter()
            .filter(|c| c.target == SUBSCRIPTION_MANAGER_ONO && c.method == ADD_SUBSCRIPTION)
            .map(|c| Reader::new(&c.params).u32().unwrap())
            .collect();
        assert_eq!(subs, vec![1, 201, 202, 204, 300]);
        let sub = all
            .iter()
            .find(|c| c.target == SUBSCRIPTION_MANAGER_ONO)
            .unwrap();
        assert_eq!(sub.method, ADD_SUBSCRIPTION);
        assert_eq!(sub.param_count, 5);
        assert_eq!(
            codec::hex(&sub.params),
            concat!(
                "00000001", "00010001", // event: object 1, PropertyChanged
                "00001000", "00010001", // subscriber method
                "0000",     // context
                "01",       // reliable delivery
                "0000",     // destination
            )
        );
    }

    #[test]
    fn many_requests_share_a_pdu() {
        let mut m = module(json!({}), true);
        let actions = connect(&mut m, 0);
        let pdus: Vec<&Vec<u8>> = actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(data),
                _ => None,
            })
            .collect();
        // The keep-alive, then one PDU with the device manager reads, its
        // subscription and the root block's GetMembers.
        assert_eq!(pdus.len(), 2);
        let (_, messages) = codec::parse_pdu(pdus[1]).unwrap();
        assert!(messages.len() > 5);
    }

    #[test]
    fn set_gain_by_path_and_by_number() {
        let (mut m, mut fake, mut st, _) = walked(json!({}));
        let a = run(
            &mut m,
            100,
            1,
            "set_gain",
            json!({"object": "In1/Gain", "gain_db": -12.5}),
        );
        let c = sent(&a);
        assert_eq!(c.len(), 1);
        assert_eq!(
            (c[0].target, c[0].method, c[0].param_count),
            (201, Id(4, 2), 1)
        );
        assert_eq!(c[0].params, (-12.5f32).to_be_bytes());
        let a = feed(&mut m, 105, fake.respond(&a));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert!(a.contains(&Action::RoundTrip(5)));
        state(&mut st, &a);
        assert_eq!(st["objects"]["In1/Gain"]["gain_db"], -12.5);
        assert_eq!(fake.gain, -12.5);

        let a = run(
            &mut m,
            110,
            2,
            "set_gain",
            json!({"ono": 300, "gain_db": 3.0}),
        );
        assert_eq!(sent(&a)[0].target, 300);
        let a = feed(&mut m, 111, fake.respond(&a));
        assert_eq!(completed(&a, 2), Some(Ok(Outcome::Ack)));
    }

    #[test]
    fn commands_are_checked_against_the_object() {
        let (mut m, _, _, _) = walked(json!({}));
        let a = run(
            &mut m,
            100,
            1,
            "set_gain",
            json!({"object": "In1/Mute", "gain_db": 0.0}),
        );
        assert!(sent(&a).is_empty());
        match completed(&a, 1) {
            Some(Err(CommandError::InvalidParams { message })) => {
                assert!(message.contains("OcaMute"), "{message}");
                assert!(message.contains("OcaGain"), "{message}");
            }
            other => panic!("{other:?}"),
        }
        let a = run(
            &mut m,
            100,
            2,
            "set_mute",
            json!({"object": "In1/Nope", "muted": true}),
        );
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(
            &mut m,
            100,
            3,
            "set_mute",
            json!({"ono": 999, "muted": true}),
        );
        assert!(matches!(
            completed(&a, 3),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(&mut m, 100, 4, "set_mute", json!({"muted": true}));
        assert!(matches!(
            completed(&a, 4),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(
            &mut m,
            100,
            5,
            "set_gain",
            json!({"object": "Out/x", "gain_db": 0.0}),
        );
        match completed(&a, 5) {
            Some(Err(CommandError::InvalidParams { message })) => {
                assert!(message.contains("not a block"), "{message}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_device_status_fails_the_command_with_its_name() {
        let (mut m, _, _, _) = walked(json!({}));
        let a = run(
            &mut m,
            100,
            1,
            "set_mute",
            json!({"object": "In1/Mute", "muted": true}),
        );
        let handle = sent(&a)[0].handle;
        let reply = pdu(
            TYPE_RESPONSE,
            &[Response {
                handle,
                status: 3,
                param_count: 0,
                params: vec![],
            }
            .encode()],
        );
        let a = feed(&mut m, 120, reply);
        assert_eq!(
            completed(&a, 1),
            Some(Err(CommandError::DeviceError {
                code: Some("3".into()),
                message: "Locked: the object is locked".into()
            }))
        );
        assert!(a.contains(&Action::RoundTrip(20)));
    }

    #[test]
    fn property_changes_update_state() {
        let (mut m, _, mut st, _) = walked(json!({}));
        let mut data = Writer::new();
        data.id(Id(4, 1)).f32(-3.0).u8(1);
        let n = Notification {
            target: SUBSCRIBER.0,
            method: SUBSCRIBER.1,
            param_count: 2,
            context: vec![],
            event: Some(codec::Event {
                emitter: 201,
                event: PROPERTY_CHANGED,
                data: data.buf,
            }),
        };
        let a = feed(&mut m, 200, pdu(TYPE_NOTIFICATION, &[n.encode()]));
        state(&mut st, &a);
        assert_eq!(st["objects"]["In1/Gain"]["gain_db"], -3.0);
        // A new maximum.
        let mut data = Writer::new();
        data.id(Id(4, 1)).f32(6.0).u8(3);
        let n = Notification {
            event: Some(codec::Event {
                emitter: 201,
                event: PROPERTY_CHANGED,
                data: data.buf,
            }),
            ..n
        };
        let a = feed(&mut m, 201, pdu(TYPE_NOTIFICATION, &[n.encode()]));
        state(&mut st, &a);
        assert_eq!(st["objects"]["In1/Gain"]["gain_db_max"], 6.0);
        // The mute, and a property the module does not know.
        let mut data = Writer::new();
        data.id(Id(4, 1)).u8(1).u8(1);
        let mut unknown = Writer::new();
        unknown.id(Id(4, 9)).u16(7).u8(1);
        let ns: Vec<Vec<u8>> = [(202, data.buf), (202, unknown.buf)]
            .into_iter()
            .map(|(emitter, data)| {
                Notification {
                    target: 0,
                    method: Id(0, 0),
                    param_count: 2,
                    context: vec![],
                    event: Some(codec::Event {
                        emitter,
                        event: PROPERTY_CHANGED,
                        data,
                    }),
                }
                .encode()
            })
            .collect();
        let a = feed(&mut m, 202, pdu(TYPE_NOTIFICATION, &ns));
        state(&mut st, &a);
        assert_eq!(st["objects"]["In1/Mute"]["muted"], true);
        assert_eq!(st["objects"]["In1/Mute"]["raw"]["4.9"], "0007");
    }

    #[test]
    fn get_property_set_property_and_call_method() {
        let (mut m, mut fake, _, _) = walked(json!({}));
        let a = run(
            &mut m,
            100,
            1,
            "get_property",
            json!({"object": "Out", "property": "4.1"}),
        );
        let a = feed(&mut m, 101, fake.respond(&a));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!({"value": -6.0, "min": -120.0, "max": 12.0})
            }))
        );
        let a = run(
            &mut m,
            102,
            2,
            "get_property",
            json!({"object": "Out", "property": "9.9"}),
        );
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(
            &mut m,
            103,
            3,
            "set_property",
            json!({"object": "In1/Mute", "property": "4.1", "value": true}),
        );
        assert_eq!(sent(&a)[0].params, vec![1]);
        let a = feed(&mut m, 104, fake.respond(&a));
        assert_eq!(completed(&a, 3), Some(Ok(Outcome::Ack)));
        assert!(fake.muted);
        // Read-only.
        let a = run(
            &mut m,
            105,
            4,
            "set_property",
            json!({"object": "In1/Meter", "property": "4.1", "value": 1.0}),
        );
        assert!(matches!(
            completed(&a, 4),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        // An unknown method answers BadMethod, returned with its status.
        let a = run(
            &mut m,
            106,
            5,
            "call_method",
            json!({"ono": 300, "method": "4.77", "params_hex": "0102"}),
        );
        let c = &sent(&a)[0];
        assert_eq!(
            (c.method, c.param_count, c.params.clone()),
            (Id(4, 77), 1, vec![1, 2])
        );
        let a = feed(&mut m, 107, fake.respond(&a));
        assert_eq!(
            completed(&a, 5),
            Some(Ok(Outcome::Value {
                value: json!({"status": 11, "status_name": "BadMethod", "param_count": 0, "params_hex": ""})
            }))
        );
    }

    #[test]
    fn identify_finds_the_identification_actuator() {
        let (mut m, mut fake, _, _) = walked(json!({}));
        let a = run(&mut m, 100, 1, "identify", json!({"enabled": true}));
        let c = &sent(&a)[0];
        assert_eq!(
            (c.target, c.method, c.params.clone()),
            (204, Id(4, 2), vec![1])
        );
        let a = feed(&mut m, 101, fake.respond(&a));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
    }

    #[test]
    fn level_sensors_are_polled() {
        let (mut m, _, _, _) = walked(json!({"level_poll_ms": 500}));
        let a = timer(&mut m, 600, POLL);
        let c = sent(&a);
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].target, c[0].method), (203, Id(4, 1)));
        // Not again while that read is unanswered.
        let a = timer(&mut m, 1100, POLL);
        assert!(sent(&a).is_empty());
        assert!(a.contains(&Action::SetTimer {
            key: POLL,
            after: 500
        }));
        // Off.
        let (_, _, st, all) = walked(json!({"level_sensors": "off"}));
        assert!(st["objects"]["In1/Meter"].get("reading_db").is_none());
        assert!(!all.iter().any(|c| c.target == 203 && c.method == Id(4, 1)));
    }

    #[test]
    fn keepalives_and_loss() {
        let mut m = module(json!({"keepalive_interval_ms": 2000}), false);
        let a = connect(&mut m, 0);
        assert_eq!(keepalives(&a), vec![KeepAlive::Seconds(2)]);
        // Commands only: nothing but the keep-alive goes out.
        assert!(sent(&a).is_empty());
        // The device's keep-alive: connected.
        let a = feed(&mut m, 500, KeepAlive::Seconds(2).pdu());
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // Quiet on our side for an interval: a keep-alive.
        let a = timer(&mut m, 2000, KEEPALIVE);
        assert_eq!(keepalives(&a), vec![KeepAlive::Seconds(2)]);
        let a = timer(&mut m, 4000, KEEPALIVE);
        assert_eq!(keepalives(&a).len(), 1);
        // Three intervals since the device was last heard (at 500).
        let a = timer(&mut m, 6400, KEEPALIVE);
        assert!(!a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        let a = timer(&mut m, 6500, KEEPALIVE);
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        assert!(a.contains(&Action::TcpClose { socket: SOCKET }));
        // A millisecond interval uses the second form.
        let mut m = module(json!({"keepalive_interval_ms": 1500}), false);
        assert_eq!(
            keepalives(&connect(&mut m, 0)),
            vec![KeepAlive::Millis(1500)]
        );
    }

    #[test]
    fn commands_only_resolves_a_path_on_demand() {
        let mut m = module(json!({}), false);
        let mut fake = Fake {
            gain: 0.0,
            muted: false,
        };
        let a = connect(&mut m, 0);
        assert!(sent(&a).is_empty(), "no walk, read or subscription");
        let a = run(
            &mut m,
            10,
            1,
            "set_gain",
            json!({"object": "In1/Gain", "gain_db": -1.0}),
        );
        // The root block's members first.
        let c = sent(&a);
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].target, c[0].method), (100, GET_MEMBERS));
        let mut actions = feed(&mut m, 11, fake.respond(&a));
        let mut all = Vec::new();
        for t in 12..30 {
            all.extend(sent(&actions));
            if completed(&actions, 1).is_some() {
                break;
            }
            actions = feed(&mut m, t, fake.respond(&actions));
        }
        assert_eq!(completed(&actions, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(fake.gain, -1.0);
        // Only what the path needed: roles in the root block and in In1, the
        // members of In1, then the set. No reads, no subscriptions.
        assert!(!all.iter().any(|c| c.target == SUBSCRIPTION_MANAGER_ONO));
        assert!(!all.iter().any(|c| c.method == Id(4, 1)));
        assert!(!all.iter().any(|c| c.target == 300 && c.method != GET_ROLE));
        // Now the path is known: straight to the set.
        let a = run(
            &mut m,
            40,
            2,
            "set_gain",
            json!({"object": "In1/Gain", "gain_db": -2.0}),
        );
        assert_eq!(sent(&a).len(), 1);
        // By number: the class is asked for first.
        let mut m = module(json!({}), false);
        let _ = connect(&mut m, 45);
        let a = run(
            &mut m,
            50,
            3,
            "set_mute",
            json!({"ono": 202, "muted": true}),
        );
        let c = sent(&a);
        assert_eq!((c[0].target, c[0].method), (202, GET_CLASS_IDENTIFICATION));
        let a = feed(&mut m, 51, fake.respond(&a));
        let c = sent(&a);
        assert_eq!((c[0].target, c[0].method), (202, Id(4, 2)));
        let a = feed(&mut m, 52, fake.respond(&a));
        assert_eq!(completed(&a, 3), Some(Ok(Outcome::Ack)));
        // call_method by number needs nothing first.
        let a = run(
            &mut m,
            60,
            4,
            "call_method",
            json!({"ono": 77, "method": "1.1"}),
        );
        let c = sent(&a);
        assert_eq!(
            (c[0].target, c[0].method, c[0].param_count),
            (77, Id(1, 1), 0)
        );
        // An unknown number: the class lookup fails, and so does the command.
        let a = run(
            &mut m,
            70,
            5,
            "set_mute",
            json!({"ono": 999, "muted": true}),
        );
        let a = feed(&mut m, 71, fake.respond(&a));
        assert!(matches!(
            completed(&a, 5),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
    }

    #[test]
    fn commands_time_out() {
        let (mut m, _, _, _) = walked(json!({"request_timeout_ms": 1000}));
        let _ = run(
            &mut m,
            100,
            1,
            "set_gain",
            json!({"object": "Out", "gain_db": 0.0}),
        );
        let a = timer(&mut m, 1100, REPLY);
        assert_eq!(completed(&a, 1), Some(Err(CommandError::Timeout)));
    }

    #[test]
    fn refresh_objects_walks_again() {
        let (mut m, mut fake, _, _) = walked(json!({}));
        let mut actions = run(&mut m, 100, 1, "refresh_objects", json!({}));
        assert!(actions.contains(&Action::State(
            json!({"objects": null, "discovery": {"complete": false}})
        )));
        for t in 101..120 {
            if completed(&actions, 1).is_some() {
                break;
            }
            actions = feed(&mut m, t, fake.respond(&actions));
        }
        assert_eq!(completed(&actions, 1), Some(Ok(Outcome::Ack)));
    }

    #[test]
    fn get_device_info_gathers_the_device_manager() {
        let (mut m, _, _, _) = walked(json!({}));
        let a = run(&mut m, 100, 1, "get_device_info", json!({}));
        let c = sent(&a);
        assert!(c.iter().all(|c| c.target == 1));
        let mut w = Writer::new();
        w.string("AMP-01").unwrap();
        let replies: Vec<Vec<u8>> = c
            .iter()
            .map(|c| {
                if c.method == Id(3, 3) {
                    Response {
                        handle: c.handle,
                        status: 0,
                        param_count: 1,
                        params: w.buf.clone(),
                    }
                    .encode()
                } else {
                    Response {
                        handle: c.handle,
                        status: 8,
                        param_count: 0,
                        params: vec![],
                    }
                    .encode()
                }
            })
            .collect();
        let a = feed(&mut m, 101, pdu(TYPE_RESPONSE, &replies));
        match completed(&a, 1) {
            Some(Ok(Outcome::Value { value })) => {
                assert_eq!(value["serial_number"], "AMP-01");
                assert_eq!(value["name"], Value::Null);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_lost_connection_fails_commands_and_retries() {
        let (mut m, _, _, _) = walked(json!({}));
        let _ = run(
            &mut m,
            100,
            1,
            "set_gain",
            json!({"object": "Out", "gain_db": 0.0}),
        );
        let mut cx = Cx::new(110);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        let a = cx.take();
        assert!(matches!(
            completed(&a, 1),
            Some(Err(CommandError::Transport { .. }))
        ));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        let a = run(
            &mut m,
            120,
            2,
            "set_gain",
            json!({"object": "Out", "gain_db": 0.0}),
        );
        assert_eq!(completed(&a, 2), Some(Err(CommandError::NotConnected)));
    }
}
