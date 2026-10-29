//! Datavideo switchers over DVIP, Datavideo's Ethernet control protocol, and
//! their RS-232 ports. Written from Datavideo's own documents (see the spec's
//! sources); section numbers below are theirs.
//!
//! Three protocols, by model:
//!
//! - Processor-unit switchers (SE-1200MU, HS-1300, SE-3200, HS-3200,
//!   SE-4000, HS-4000), "Ethernet Control Protocol": every value is a 32-bit
//!   little-endian int or float, and every packet is its size in bytes
//!   (itself included), a command and its parameters (2.1.2.1). Four
//!   controllers each take a pair of ports, real-time 5001/5003/5005/5007 and
//!   command one higher (1.4); port 5009 hands out a free pair (1.5, 5.2).
//!   On the command port GET_CONTROL (0) reads controls, answered by a
//!   SET_CONTROL packet, and SET_CONTROL (1) writes them, answered by an
//!   8-byte OK (1) or ERROR (0) packet (2.1.2.2); other commands read names,
//!   mini pictures and files. The real-time port sends the whole machine
//!   state as control/value pairs in its first packet and every change after
//!   it, one packet a field, each to be answered by the controller with a
//!   packet of its own (3.2). The SE-1200MU and SE-4000 RS-232 ports carry a
//!   GVG100-style protocol (count, effects address, command, message) whose
//!   70/F0 commands wrap the same GET and SET of control IDs, so the same
//!   control table serves both links; over RS-232 nothing is pushed and
//!   state is polled.
//! - SE-2200, "DVIP Serial Control Protocol": blocks of sync AAH, a byte
//!   count, effect address FFH, command 20H (a panel button), 21H (T-bar) or
//!   22H (status) and a checksum, on RS-232/422 or through the DVIP board's
//!   TCP 5002 with DVIP's 2-byte length header. Nothing is pushed: the
//!   status is polled.
//! - Every DVIP device, "DVIP Ethernet Control Operation Guide" 3.5-3.6: UDP
//!   5002 packets of a 2-byte length, type 81H and a command, for network
//!   settings, identity and the device's presets. On the SE-650, the only
//!   protocol documented, so its liveness comes from these.
//!
//! Opened for commands only (`monitor` false), the real-time port is not
//! opened, nothing is polled and nothing is read on connecting; a cheap read
//! every 10 s (the connection status control, the SE-2200's status, or the
//! DVIP network information on the SE-650) shows the switcher is there.
//! Every answered request reports its round trip.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};

use base64::Engine as _;
use serde_json::{json, Map, Value};

use super::datavideo_switcher_table::{
    controls, id, inputs, keyers, present, Control, Family, Kind, Present, MEMORY_COMMAND,
    SECTION_MEMORY_CTRL, SECTION_STATUS, SECTION_STILL_CTRL, SECTION_TRANSITION_CTRL,
    STILL_COMMAND, TRANS_COMMAND,
};
use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, FileInput, Key, Level, Link, Millis, Module,
    OpenContext, Outcome, PollStamps, SerialPort, TcpInput,
};

const CMD: Key = "command";
const RT: Key = "realtime";
const REQ: Key = "request";
const UDP: Key = "dvip";
const FILE: Key = "file";

const REPLY_TIMER: Key = "reply";
const RETRY_TIMER: Key = "retry";
const RT_RETRY_TIMER: Key = "realtime-retry";
const PROBE_TIMER: Key = "probe";
const POLL_TIMER: Key = "poll";
const UDP_REPLY_TIMER: Key = "dvip-reply";
const UDP_POLL_TIMER: Key = "dvip-poll";

pub(crate) const REQUEST_PORT: u16 = 5009;
pub(crate) const DVIP_PORT: u16 = 5002;
const REPLY_TIMEOUT: Millis = 2_000;
const UDP_TIMEOUT: Millis = 1_000;
const PROBE_EVERY: Millis = 10_000;
const MISSES: u32 = 3;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// Bytes asked for per GET_FILE_DATA, and sent per STORE_FILE_DATA.
const CHUNK: usize = 1024;
/// GVG messages are at most 127 bytes: 2 + 4n for a read, 2 + 8n for a write.
const SERIAL_GETS: usize = 30;
const SERIAL_SETS: usize = 15;

const NULL_PACKET: [u8; 8] = [8, 0, 0, 0, 0, 0, 0, 0];

fn bad(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn device_error(code: Option<String>, message: impl Into<String>) -> CommandError {
    CommandError::DeviceError {
        code,
        message: message.into(),
    }
}

fn int(params: &Params, name: &str) -> Result<i64, CommandError> {
    params
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

fn opt_int(params: &Params, name: &str) -> Option<i64> {
    params.get(name).and_then(Value::as_i64)
}

fn boolean(params: &Params, name: &str) -> Result<bool, CommandError> {
    params
        .get(name)
        .and_then(Value::as_bool)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

fn text<'a>(params: &'a Params, name: &str) -> Result<&'a str, CommandError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

/// Put a value at a dotted path of a state patch.
fn put(patch: &mut Map<String, Value>, path: &str, value: Value) {
    match path.split_once('.') {
        None => {
            patch.insert(path.to_string(), value);
        }
        Some((head, rest)) => {
            let entry = patch
                .entry(head.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !entry.is_object() {
                *entry = Value::Object(Map::new());
            }
            put(entry.as_object_mut().unwrap(), rest, value);
        }
    }
}

fn le32(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// A float as the shortest decimal that reads back as the same f32.
fn float_value(raw: u32) -> Value {
    let f = f32::from_bits(raw);
    if !f.is_finite() {
        return Value::Null;
    }
    let shortest: f64 = format!("{f}").parse().unwrap_or(f as f64);
    json!(shortest)
}

/// A control's raw value as its state value.
pub(crate) fn decode(kind: &Kind, raw: u32) -> Value {
    match kind {
        Kind::Int { .. } => json!(raw as i32),
        Kind::Float { .. } => float_value(raw),
        Kind::Flag => json!(raw != 0),
        Kind::FloatFlag => json!(f32::from_bits(raw) != 0.0),
        Kind::Enum(values) => {
            let v = raw as i32 as i64;
            match values.iter().find(|(_, n)| *n == v) {
                Some((name, _)) => json!(name),
                None => json!(v),
            }
        }
    }
}

/// A value given as text (`set_control`) as a control's raw value.
pub(crate) fn encode(kind: &Kind, value: &str) -> Result<u32, String> {
    let value = value.trim();
    let flag = || match value.to_ascii_lowercase().as_str() {
        "true" | "on" | "1" | "yes" => Ok(true),
        "false" | "off" | "0" | "no" => Ok(false),
        _ => Err(format!("'{value}' is not true or false")),
    };
    match kind {
        Kind::Int { min, max } => {
            let n: i64 = value
                .parse()
                .map_err(|_| format!("'{value}' is not a whole number"))?;
            if n < *min || n > *max {
                return Err(format!("{n} is outside {min} to {max}"));
            }
            Ok(n as i32 as u32)
        }
        Kind::Float { min, max } => {
            let f: f64 = value
                .parse()
                .map_err(|_| format!("'{value}' is not a number"))?;
            if !f.is_finite() || f < *min || f > *max {
                return Err(format!("{value} is outside {min} to {max}"));
            }
            Ok((f as f32).to_bits())
        }
        Kind::Flag => flag().map(u32::from),
        Kind::FloatFlag => flag().map(|on| if on { 1.0f32 } else { 0.0 }.to_bits()),
        Kind::Enum(values) => {
            if let Some((_, n)) = values.iter().find(|(name, _)| *name == value) {
                return Ok(*n as i32 as u32);
            }
            match value.parse::<i64>() {
                Ok(n) if values.iter().any(|(_, v)| *v == n) => Ok(n as i32 as u32),
                _ => Err(format!(
                    "'{value}' is not one of {}",
                    values
                        .iter()
                        .map(|(n, _)| *n)
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
    }
}

fn enum_raw(table: &[(&str, i64)], name: &str) -> u32 {
    table
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| *v as u32)
        .unwrap_or(0)
}

/// UTF-16LE text, up to a NUL.
fn utf16(bytes: &[u8], chars: usize) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .take(chars)
        .map(|b| u16::from_le_bytes(*b))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// ASCII text, up to a NUL.
fn ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| {
            if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                '?'
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// DVIP UDP management (Operation Guide 3.5, 3.6).

pub(crate) fn dvip_packet(command: u8, params: &[u8]) -> Vec<u8> {
    // The length counts the whole packet, as the guide's replies show
    // (3.6.2: 00 06 for six bytes).
    let len = 4 + params.len();
    let mut out = vec![(len >> 8) as u8, len as u8, 0x81, command];
    out.extend_from_slice(params);
    out
}

#[derive(Debug, Clone, PartialEq)]
enum DvipKind {
    Network,
    Firmware,
    Model,
    ModelName,
    Version,
    PresetCount { names: bool },
    PresetName { number: u8, last: bool },
    Ack,
}

#[derive(Debug, Clone)]
struct DvipJob {
    id: Option<CommandId>,
    command: u8,
    params: Vec<u8>,
    kind: DvipKind,
}

impl DvipJob {
    fn new(id: Option<CommandId>, command: u8, tag: &str, extra: &[u8], kind: DvipKind) -> Self {
        let mut params = tag.as_bytes().to_vec();
        params.extend_from_slice(extra);
        DvipJob {
            id,
            command,
            params,
            kind,
        }
    }
}

fn ipv4(params: &Params, name: &str) -> Result<[u8; 4], CommandError> {
    let s = text(params, name)?;
    s.trim()
        .parse::<std::net::Ipv4Addr>()
        .map(|a| a.octets())
        .map_err(|_| bad(format!("{name} '{s}' is not an IPv4 address")))
}

struct Dvip {
    to: SocketAddr,
    /// Whether DVIP is the device's only protocol, and so its liveness.
    standalone: bool,
    monitor: bool,
    poll_ms: Millis,
    opened: bool,
    queue: VecDeque<DvipJob>,
    in_flight: Option<(DvipJob, Millis)>,
    reachable: bool,
    misses: u32,
    preset_names: Vec<Value>,
}

impl Dvip {
    fn new(to: SocketAddr, standalone: bool, monitor: bool, poll_ms: Millis) -> Dvip {
        Dvip {
            to,
            standalone,
            monitor,
            poll_ms,
            opened: false,
            queue: VecDeque::new(),
            in_flight: None,
            reachable: false,
            misses: 0,
            preset_names: Vec::new(),
        }
    }

    fn identity_reads() -> Vec<DvipJob> {
        vec![
            DvipJob::new(None, 0x00, "ETH_REQ", &[], DvipKind::Network),
            DvipJob::new(None, 0x01, "FWVER_REQ", &[], DvipKind::Firmware),
            DvipJob::new(None, 0x40, "GET_MODEL", &[], DvipKind::Model),
            DvipJob::new(None, 0x41, "GET_MODEL_NAME", &[], DvipKind::ModelName),
            DvipJob::new(None, 0x42, "GET_VERSION", &[], DvipKind::Version),
            DvipJob::new(
                None,
                0x43,
                "GET_PRESET_COUNT",
                &[],
                DvipKind::PresetCount { names: true },
            ),
        ]
    }

    fn start(&mut self, cx: &mut Cx) {
        if self.standalone {
            cx.connection(Connection::Connecting);
        }
        if self.standalone || self.monitor {
            self.open(cx);
            if self.monitor {
                self.queue.extend(Self::identity_reads());
            } else {
                // Commands only: the network information is the cheapest
                // sign the device is there.
                self.queue
                    .push_back(DvipJob::new(None, 0x00, "ETH_REQ", &[], DvipKind::Network));
            }
            if self.standalone {
                let every = if self.monitor {
                    self.poll_ms
                } else {
                    PROBE_EVERY
                };
                cx.set_timer(UDP_POLL_TIMER, every);
            }
            self.pump(cx);
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        if !self.opened {
            cx.udp_open(UDP, Bind::Ephemeral);
            self.opened = true;
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) -> bool {
        let job = match name {
            "get_network" => DvipJob::new(Some(id), 0x00, "ETH_REQ", &[], DvipKind::Network),
            "get_identity" => {
                for job in [
                    DvipJob::new(None, 0x01, "FWVER_REQ", &[], DvipKind::Firmware),
                    DvipJob::new(None, 0x40, "GET_MODEL", &[], DvipKind::Model),
                    DvipJob::new(None, 0x41, "GET_MODEL_NAME", &[], DvipKind::ModelName),
                ] {
                    self.queue.push_back(job);
                }
                DvipJob::new(Some(id), 0x42, "GET_VERSION", &[], DvipKind::Version)
            }
            "get_presets" => DvipJob::new(
                Some(id),
                0x43,
                "GET_PRESET_COUNT",
                &[],
                DvipKind::PresetCount { names: true },
            ),
            "recall_preset" => match int(params, "preset") {
                Ok(n) => DvipJob::new(Some(id), 0x45, "RECALL_PRESET", &[n as u8], DvipKind::Ack),
                Err(e) => {
                    cx.complete(id, Err(e));
                    return true;
                }
            },
            "set_dhcp" => match boolean(params, "enabled") {
                Ok(on) => DvipJob::new(Some(id), 0x02, "SET_DHCPMODE", &[on as u8], DvipKind::Ack),
                Err(e) => {
                    cx.complete(id, Err(e));
                    return true;
                }
            },
            "set_ip_address" => match (ipv4(params, "address"), ipv4(params, "gateway")) {
                (Ok(a), Ok(g)) => {
                    let mut extra = a.to_vec();
                    extra.extend_from_slice(&g);
                    DvipJob::new(Some(id), 0x03, "SET_IPADR", &extra, DvipKind::Ack)
                }
                (Err(e), _) | (_, Err(e)) => {
                    cx.complete(id, Err(e));
                    return true;
                }
            },
            "set_netmask" => match ipv4(params, "netmask") {
                Ok(m) => DvipJob::new(Some(id), 0x0B, "SET_NETMASK", &m, DvipKind::Ack),
                Err(e) => {
                    cx.complete(id, Err(e));
                    return true;
                }
            },
            "set_gateway" => match ipv4(params, "gateway") {
                Ok(g) => DvipJob::new(Some(id), 0x0C, "SET_GATEWAY", &g, DvipKind::Ack),
                Err(e) => {
                    cx.complete(id, Err(e));
                    return true;
                }
            },
            "set_dns" => {
                let primary = params.get("primary").and_then(Value::as_str).is_some();
                let secondary = params.get("secondary").and_then(Value::as_str).is_some();
                if !primary && !secondary {
                    cx.complete(id, Err(bad("give primary, secondary or both")));
                    return true;
                }
                let mut jobs = Vec::new();
                for (given, name, code, tag) in [
                    (primary, "primary", 0x0D, "SET_PRIDNS"),
                    (secondary, "secondary", 0x0E, "SET_SECDNS"),
                ] {
                    if given {
                        match ipv4(params, name) {
                            Ok(a) => jobs.push(DvipJob::new(None, code, tag, &a, DvipKind::Ack)),
                            Err(e) => {
                                cx.complete(id, Err(e));
                                return true;
                            }
                        }
                    }
                }
                let mut last = jobs.pop().unwrap();
                last.id = Some(id);
                self.queue.extend(jobs);
                last
            }
            "set_host_name" => {
                let name = match text(params, "name") {
                    Ok(n) => n,
                    Err(e) => {
                        cx.complete(id, Err(e));
                        return true;
                    }
                };
                if name.len() > 15 || !name.bytes().all(|b| b.is_ascii_graphic()) {
                    cx.complete(
                        id,
                        Err(bad(
                            "the host name is at most 15 printable ASCII characters",
                        )),
                    );
                    return true;
                }
                let mut extra = name.as_bytes().to_vec();
                extra.push(0);
                DvipJob::new(Some(id), 0x09, "SET_DHCPNAME", &extra, DvipKind::Ack)
            }
            "reboot" => DvipJob::new(Some(id), 0x04, "RESET", &[], DvipKind::Ack),
            _ => return false,
        };
        self.open(cx);
        self.queue.push_back(job);
        self.pump(cx);
        true
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() {
            return;
        }
        let Some(job) = self.queue.pop_front() else {
            return;
        };
        cx.udp_send(UDP, self.to, dvip_packet(job.command, &job.params));
        cx.set_timer(UDP_REPLY_TIMER, UDP_TIMEOUT);
        self.in_flight = Some((job, cx.now()));
    }

    fn datagram(&mut self, cx: &mut Cx, from: SocketAddr, data: &[u8]) {
        if from.ip() != self.to.ip() || data.len() < 4 || data[2] != 0x81 {
            return;
        }
        if !self
            .in_flight
            .as_ref()
            .is_some_and(|(j, _)| j.command == data[3])
        {
            return;
        }
        let Some((job, sent_at)) = self.in_flight.take() else {
            return;
        };
        cx.cancel_timer(UDP_REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.misses = 0;
        if self.standalone {
            cx.alive();
            if !self.reachable {
                self.reachable = true;
                cx.connection(Connection::Connected);
            }
        }
        let body = &data[4..];
        let mut patch = Map::new();
        let result: Result<Value, CommandError> = match job.kind.clone() {
            DvipKind::Network if body.len() >= 43 => {
                let ip = |at: usize| {
                    std::net::Ipv4Addr::new(body[at], body[at + 1], body[at + 2], body[at + 3])
                        .to_string()
                };
                let mac = body[17..23]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(":");
                let network = json!({
                    "dhcp": body[0] != 0,
                    "host_name": ascii(&body[1..17]),
                    "mac_address": mac,
                    "ip_address": ip(23),
                    "netmask": ip(27),
                    "gateway": ip(31),
                    "dns_primary": ip(35),
                    "dns_secondary": ip(39),
                    "connections": body.get(43).copied().map(Value::from).unwrap_or(Value::Null),
                });
                put(&mut patch, "dvip.network", network.clone());
                Ok(network)
            }
            DvipKind::Firmware if body.len() >= 2 => {
                let v = format!("{}.{}", body[0], body[1]);
                put(&mut patch, "dvip.firmware", json!(v));
                Ok(json!(v))
            }
            DvipKind::Model if body.len() >= 4 => {
                let vendor = u16::from_be_bytes([body[0], body[1]]);
                let product = u16::from_be_bytes([body[2], body[3]]);
                put(&mut patch, "dvip.vendor_id", json!(vendor));
                put(&mut patch, "dvip.product_id", json!(product));
                Ok(json!({"vendor_id": vendor, "product_id": product}))
            }
            DvipKind::ModelName => {
                let name = ascii(&body[..body.len().min(16)]);
                put(&mut patch, "dvip.model_name", json!(name));
                Ok(json!(name))
            }
            DvipKind::Version if body.len() >= 16 => {
                let hex = |at: usize| {
                    body[at..at + 4]
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                };
                let versions = json!({
                    "firmware_a": hex(0),
                    "firmware_b": hex(4),
                    "firmware_c": hex(8),
                    "firmware_d": hex(12),
                });
                put(&mut patch, "dvip.versions", versions.clone());
                Ok(versions)
            }
            DvipKind::PresetCount { names } => match body {
                [0x06, count, ..] => {
                    put(&mut patch, "dvip.preset_count", json!(count));
                    if names {
                        self.preset_names.clear();
                        // Read every name, the last carrying the command.
                        let count = *count;
                        for n in (1..=count).rev() {
                            self.queue.push_front(DvipJob::new(
                                if n == count { job.id } else { None },
                                0x44,
                                "GET_PRESET_NAME",
                                &[n],
                                DvipKind::PresetName {
                                    number: n,
                                    last: n == count,
                                },
                            ));
                        }
                        if count > 0 {
                            if !patch.is_empty() {
                                cx.state(Value::Object(patch));
                            }
                            self.pump(cx);
                            return;
                        }
                    }
                    Ok(json!([]))
                }
                _ => Err(device_error(
                    Some("NACK".into()),
                    "the device refused the preset count (NACK)",
                )),
            },
            DvipKind::PresetName { number, last } => {
                let name = ascii(&body[..body.len().min(32)]);
                put(
                    &mut patch,
                    &format!("dvip.presets.{number}.name"),
                    json!(name),
                );
                self.preset_names
                    .push(json!({"preset": number, "name": name}));
                if !last {
                    cx.state(Value::Object(patch));
                    self.pump(cx);
                    return;
                }
                Ok(Value::Array(std::mem::take(&mut self.preset_names)))
            }
            DvipKind::Ack => match body.first() {
                Some(0x06) => Ok(Value::Null),
                Some(0x15) => Err(device_error(
                    Some("NACK".into()),
                    "the device refused (NACK)",
                )),
                _ => Err(device_error(None, format!("unexpected reply {data:02x?}"))),
            },
            _ => Err(device_error(None, format!("short reply {data:02x?}"))),
        };
        if !patch.is_empty() {
            cx.state(Value::Object(patch));
        }
        if let Some(id) = job.id {
            cx.complete(
                id,
                result.map(|value| match job.kind {
                    DvipKind::Ack => Outcome::Ack,
                    _ => Outcome::Value { value },
                }),
            );
        }
        self.pump(cx);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            UDP_REPLY_TIMER => {
                if let Some((job, _)) = self.in_flight.take() {
                    if let Some(id) = job.id {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.standalone && self.misses >= MISSES && self.reachable {
                        self.reachable = false;
                        cx.connection(Connection::Disconnected {
                            reason: "no reply to DVIP requests".into(),
                        });
                    }
                }
                self.pump(cx);
            }
            UDP_POLL_TIMER => {
                if self.in_flight.is_none() && self.queue.is_empty() {
                    self.queue.push_back(DvipJob::new(
                        None,
                        0x00,
                        "ETH_REQ",
                        &[],
                        DvipKind::Network,
                    ));
                    self.pump(cx);
                }
                let every = if self.monitor {
                    self.poll_ms
                } else {
                    PROBE_EVERY
                };
                cx.set_timer(UDP_POLL_TIMER, every);
            }
            _ => {}
        }
    }

    fn failed(&mut self, cx: &mut Cx, message: &str) {
        self.opened = false;
        for job in self
            .in_flight
            .take()
            .map(|(j, _)| j)
            .into_iter()
            .chain(self.queue.drain(..))
        {
            if let Some(id) = job.id {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: message.to_string(),
                    }),
                );
            }
        }
        cx.cancel_timer(UDP_REPLY_TIMER);
        if self.standalone {
            self.reachable = false;
            cx.connection(Connection::Disconnected {
                reason: format!("udp: {message}"),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Processor-unit switchers: the Ethernet Control Protocol and its RS-232 form.

const OPEN_DATA_FILE: u32 = 2;
const CLOSE_DATA_FILE: u32 = 4;
const GET_FILE_DATA: u32 = 5;
const STORE_FILE_DATA: u32 = 6;
const FILE_EVENT: u32 = 7;
const GET_MINI_PIC: u32 = 8;
const GET_INPUT_NAME: u32 = 9;
const GET_FILE_NAME: u32 = 11;
const SET_FILE_NAME: u32 = 12;
const CHROMA_KEYER_AUTO: u32 = 20;

const FILE_TYPES: &[(&str, u32)] = &[
    ("memory", 0),
    ("still", 1),
    ("clip", 2),
    ("logo", 3),
    ("animated_logo", 4),
];

fn file_type(params: &Params) -> Result<u32, CommandError> {
    let name = text(params, "type")?;
    FILE_TYPES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| *v)
        .ok_or_else(|| bad(format!("type '{name}' is not a file type")))
}

#[derive(Debug, Clone, PartialEq)]
enum Req {
    Set(Vec<(u32, u32)>),
    Get(Vec<u32>),
    InputName(u32),
    FileName(u32, u32),
    SetFileName(u32, u32, String),
    MiniPic(u32),
    ChromaAuto(u32),
    OpenData {
        kind: u32,
        number: u32,
        write: bool,
    },
    ReadData,
    StoreData {
        chunk: u32,
        data: Vec<u8>,
    },
    CloseData,
    FileEvent(u32),
    /// A GVG message over RS-232 answered by a status byte.
    Gvg(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Returns {
    Ack,
    Value,
    /// Completed when the exported file is closed on the host.
    File,
}

#[derive(Debug)]
struct Job {
    id: Option<CommandId>,
    steps: VecDeque<Req>,
    returns: Returns,
    value: Option<Value>,
}

impl Job {
    fn internal(req: Req) -> Job {
        Job {
            id: None,
            steps: VecDeque::from([req]),
            returns: Returns::Ack,
            value: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum PLink {
    Tcp(IpAddr),
    Serial(SerialPort),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Requesting,
    Connecting,
    Open,
}

struct Processor {
    family: Family,
    link: PLink,
    controller: Option<u16>,
    port: Option<u16>,
    monitor: bool,
    /// When each poll was last answered (`polled.<group>`, SPEC.md §8).
    stamps: PollStamps,
    poll_ms: Millis,
    table: Vec<Control>,
    by_path: HashMap<String, usize>,
    by_id: HashMap<u32, usize>,
    present: Vec<Present>,
    present_words: HashMap<u32, u32>,
    fast: Vec<u32>,
    cursor: usize,
    rt_port: u16,
    cmd_port: u16,
    phase: Phase,
    cmd_buf: Vec<u8>,
    rt_buf: Vec<u8>,
    req_buf: Vec<u8>,
    rt_open: bool,
    rt_first: bool,
    rt_alive: Millis,
    rt_retry_after: Millis,
    queue: VecDeque<Job>,
    in_flight: Option<(Job, Req, Millis)>,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    /// An export waiting for its host file to close, or an import for its
    /// host file to be read.
    file_job: Option<(CommandId, FileOp)>,
    /// Why the host file of a running export failed, if it did.
    file_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum FileOp {
    Export,
    Import { kind: u32, number: u32 },
}

impl Processor {
    fn new(family: Family, link: PLink, ctx: &OpenContext, poll_ms: Millis) -> Processor {
        let controller = match ctx.settings.get("controller").and_then(Value::as_str) {
            Some(n) if n != "auto" => n.parse().ok(),
            _ => None,
        };
        let table = controls(family);
        let by_path = table
            .iter()
            .enumerate()
            .map(|(i, c)| (c.path.clone(), i))
            .collect();
        let by_id = table.iter().enumerate().map(|(i, c)| (c.id, i)).collect();
        let fast = table
            .iter()
            .filter(|c| {
                let p = c.path.as_str();
                matches!(p, "program" | "preview" | "transition.type" | "wipe.level")
                    || p.starts_with("tally.")
                    || p.starts_with("ftb.")
                    || (p.starts_with("keyers.") && p.ends_with(".on"))
                    || (p.starts_with("dsks.") && p.ends_with(".on"))
                    || (p.starts_with("transitions.") && p.ends_with(".state"))
            })
            .map(|c| c.id)
            .collect();
        Processor {
            family,
            link,
            controller,
            port: ctx.port,
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            poll_ms,
            table,
            by_path,
            by_id,
            present: present(family),
            present_words: HashMap::new(),
            fast,
            cursor: 0,
            rt_port: 5001,
            cmd_port: 5002,
            phase: Phase::Closed,
            cmd_buf: Vec::new(),
            rt_buf: Vec::new(),
            req_buf: Vec::new(),
            rt_open: false,
            rt_first: false,
            rt_alive: 0,
            rt_retry_after: RETRY_MIN,
            queue: VecDeque::new(),
            in_flight: None,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            file_job: None,
            file_error: None,
        }
    }

    fn serial(&self) -> bool {
        matches!(self.link, PLink::Serial(_))
    }

    fn control(&self, path: &str) -> Result<&Control, CommandError> {
        self.by_path
            .get(path)
            .map(|&i| &self.table[i])
            .ok_or_else(|| bad(format!("'{path}' is not a control of this model")))
    }

    fn id_of(&self, path: &str) -> u32 {
        self.table[self.by_path[path]].id
    }

    fn open(&mut self, cx: &mut Cx) {
        self.cmd_buf.clear();
        self.req_buf.clear();
        cx.connection(Connection::Connecting);
        match &self.link {
            PLink::Serial(port) => {
                self.phase = Phase::Connecting;
                Link::Serial(port.clone()).open(cx, CMD);
            }
            PLink::Tcp(host) => {
                let host = *host;
                if let Some(port) = self.port {
                    self.cmd_port = port;
                    self.rt_port = port.saturating_sub(1);
                } else if let Some(n) = self.controller {
                    self.rt_port = 5001 + 2 * (n - 1);
                    self.cmd_port = self.rt_port + 1;
                } else {
                    self.phase = Phase::Requesting;
                    cx.tcp_open(REQ, SocketAddr::new(host, REQUEST_PORT));
                    return;
                }
                self.phase = Phase::Connecting;
                cx.tcp_open(CMD, SocketAddr::new(host, self.cmd_port));
            }
        }
    }

    fn host(&self) -> Option<IpAddr> {
        match &self.link {
            PLink::Tcp(h) => Some(*h),
            PLink::Serial(_) => None,
        }
    }

    // --- Requests on the wire.

    fn encode(&self, req: &Req) -> Vec<u8> {
        if self.serial() {
            return match req {
                Req::Set(pairs) => {
                    let mut out = vec![(2 + 8 * pairs.len()) as u8, 0x00, 0xF0];
                    for (i, v) in pairs {
                        out.extend_from_slice(&i.to_le_bytes());
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                    out
                }
                Req::Get(ids) => {
                    let mut out = vec![(2 + 4 * ids.len()) as u8, 0x00, 0x70];
                    for i in ids {
                        out.extend_from_slice(&i.to_le_bytes());
                    }
                    out
                }
                Req::Gvg(message) => message.clone(),
                // Planned only for TCP.
                _ => Vec::new(),
            };
        }
        let words: Vec<u32> = match req {
            Req::Get(ids) => std::iter::once(0).chain(ids.iter().copied()).collect(),
            Req::Set(pairs) => std::iter::once(1)
                .chain(pairs.iter().flat_map(|(i, v)| [*i, *v]))
                .collect(),
            Req::InputName(n) => vec![GET_INPUT_NAME, *n],
            Req::FileName(kind, n) => vec![GET_FILE_NAME, *kind, *n],
            Req::MiniPic(n) => vec![GET_MINI_PIC, *n],
            Req::ChromaAuto(k) => vec![CHROMA_KEYER_AUTO, *k],
            Req::OpenData {
                kind,
                number,
                write,
            } => vec![OPEN_DATA_FILE, *kind, *number, *write as u32],
            Req::ReadData => vec![GET_FILE_DATA, CHUNK as u32],
            Req::CloseData => vec![CLOSE_DATA_FILE],
            Req::FileEvent(kind) => vec![FILE_EVENT, *kind],
            Req::SetFileName(kind, n, name) => {
                let mut units: Vec<u16> = name.encode_utf16().collect();
                let chars = units.len() as u32;
                units.push(0);
                let mut out = Vec::new();
                for w in [SET_FILE_NAME, *kind, *n, chars] {
                    out.extend_from_slice(&w.to_le_bytes());
                }
                for u in units {
                    out.extend_from_slice(&u.to_le_bytes());
                }
                return sized(out);
            }
            Req::StoreData { chunk, data } => {
                let mut out = Vec::new();
                for w in [STORE_FILE_DATA, data.len() as u32, *chunk] {
                    out.extend_from_slice(&w.to_le_bytes());
                }
                out.extend_from_slice(data);
                return sized(out);
            }
            Req::Gvg(_) => Vec::new(),
        };
        sized(words.iter().flat_map(|w| w.to_le_bytes()).collect())
    }

    fn enqueue(&mut self, cx: &mut Cx, job: Job) {
        // Commands go ahead of background reads.
        let at = self.queue.iter().position(|j| j.id.is_none());
        match at {
            Some(at) => self.queue.insert(at, job),
            None => self.queue.push_back(job),
        }
        self.pump(cx);
    }

    fn queue_read(&mut self, ids: Vec<u32>) {
        let limit = if self.serial() { SERIAL_GETS } else { 256 };
        for chunk in ids.chunks(limit) {
            self.queue
                .push_back(Job::internal(Req::Get(chunk.to_vec())));
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || self.phase != Phase::Open {
            return;
        }
        let Some(mut job) = self.queue.pop_front() else {
            return;
        };
        let Some(req) = job.steps.pop_front() else {
            self.finish(cx, job);
            self.pump(cx);
            return;
        };
        cx.tcp_send(CMD, self.encode(&req));
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        self.in_flight = Some((job, req, cx.now()));
    }

    fn finish(&mut self, cx: &mut Cx, job: Job) {
        let Some(id) = job.id else {
            return;
        };
        match job.returns {
            Returns::Ack => cx.complete(id, Ok(Outcome::Ack)),
            Returns::Value => cx.complete(
                id,
                Ok(Outcome::Value {
                    value: job.value.unwrap_or(Value::Null),
                }),
            ),
            Returns::File => {
                cx.file_close(FILE);
                match self.file_error.take() {
                    Some(message) => cx.complete(id, Err(CommandError::Transport { message })),
                    None => self.file_job = Some((id, FileOp::Export)),
                }
            }
        }
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.misses = 0;
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
    }

    /// Apply control values to state.
    fn apply(&mut self, cx: &mut Cx, values: &[(u32, u32)]) -> Option<Value> {
        let mut patch = Map::new();
        let mut first = None;
        let mut present_changed = BTreeSet::new();
        for &(cid, raw) in values {
            if let Some(&i) = self.by_id.get(&cid) {
                let c = &self.table[i];
                let v = decode(&c.kind, raw);
                if first.is_none() {
                    first = Some(v.clone());
                }
                put(&mut patch, &c.path, v);
            } else if let Some(p) = self
                .present
                .iter()
                .position(|p| cid >> 16 == p.section && (cid & 0xFFFF) < p.words)
            {
                if self.present_words.insert(cid, raw) != Some(raw) {
                    present_changed.insert(p);
                }
            }
        }
        for p in present_changed {
            let Present {
                path,
                section,
                words,
            } = self.present[p].clone();
            let mut stored = Vec::new();
            for w in 0..words {
                let bits = self
                    .present_words
                    .get(&id(section, w))
                    .copied()
                    .unwrap_or(0);
                for b in 0..32 {
                    if bits & (1 << b) != 0 {
                        stored.push(json!(w * 32 + b));
                    }
                }
            }
            put(&mut patch, path, Value::Array(stored));
        }
        if !patch.is_empty() {
            cx.state(Value::Object(patch));
        }
        first
    }

    // --- Command port replies.

    fn cmd_data(&mut self, cx: &mut Cx, bytes: &[u8]) {
        self.cmd_buf.extend_from_slice(bytes);
        loop {
            let packet = if self.serial() {
                // GVG: the count, then that many bytes.
                let Some(&count) = self.cmd_buf.first() else {
                    return;
                };
                let total = 1 + count as usize;
                if count == 0 {
                    self.cmd_buf.remove(0);
                    continue;
                }
                if self.cmd_buf.len() < total {
                    return;
                }
                self.cmd_buf.drain(..total).collect::<Vec<u8>>()
            } else {
                let Some(size) = le32(&self.cmd_buf, 0) else {
                    return;
                };
                let size = size as usize;
                if !(8..=16 * 1024 * 1024).contains(&size) {
                    // Out of step: nothing can be trusted, start again.
                    cx.log(Level::Warning, format!("bad packet size {size}, resyncing"));
                    self.cmd_buf.clear();
                    return;
                }
                if self.cmd_buf.len() < size {
                    return;
                }
                self.cmd_buf.drain(..size).collect::<Vec<u8>>()
            };
            self.reply(cx, &packet);
        }
    }

    fn reply(&mut self, cx: &mut Cx, packet: &[u8]) {
        let Some((mut job, req, sent_at)) = self.in_flight.take() else {
            return;
        };
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.heard(cx);
        let result = if self.serial() {
            self.serial_reply(cx, &req, packet)
        } else {
            self.tcp_reply(cx, &mut job, &req, packet)
        };
        if result.is_ok() && job.id.is_none() && self.monitor {
            self.stamps.answered(cx, "status");
        }
        match result {
            Err(e) => {
                if job.returns == Returns::File {
                    cx.file_close(FILE);
                }
                // A failed export or import still closes the device's file.
                if matches!(
                    req,
                    Req::ReadData | Req::StoreData { .. } | Req::FileEvent(_)
                ) {
                    self.queue.push_front(Job::internal(Req::CloseData));
                }
                if let Some(id) = job.id {
                    cx.complete(id, Err(e));
                }
            }
            Ok(value) => {
                if value.is_some() {
                    job.value = value;
                }
                if job.steps.is_empty() {
                    self.finish(cx, job);
                } else {
                    self.queue.push_front(job);
                }
            }
        }
        self.pump(cx);
    }

    fn serial_reply(
        &mut self,
        cx: &mut Cx,
        req: &Req,
        packet: &[u8],
    ) -> Result<Option<Value>, CommandError> {
        match (req, packet) {
            (Req::Get(ids), [_, _, 0xF0, values @ ..]) => {
                let pairs: Vec<(u32, u32)> = ids
                    .iter()
                    .enumerate()
                    .filter_map(|(n, &i)| le32(values, 4 * n).map(|v| (i, v)))
                    .collect();
                Ok(self.apply(cx, &pairs))
            }
            (_, [1, status]) => {
                if status & 0x80 != 0 {
                    Ok(None)
                } else if status & 0x40 != 0 {
                    Err(device_error(
                        Some(format!("{status:02x}")),
                        "the switcher reported a protocol error",
                    ))
                } else {
                    Err(device_error(
                        Some(format!("{status:02x}")),
                        "the switcher ignored the command (inhibited or invalid)",
                    ))
                }
            }
            _ => Err(device_error(
                None,
                format!("unexpected reply {packet:02x?}"),
            )),
        }
    }

    fn tcp_reply(
        &mut self,
        cx: &mut Cx,
        job: &mut Job,
        req: &Req,
        packet: &[u8],
    ) -> Result<Option<Value>, CommandError> {
        let size = packet.len();
        let command = le32(packet, 4).unwrap_or(u32::MAX);
        if size == 8 {
            return match command {
                1 => Ok(None),
                0 => Err(device_error(
                    Some("ERROR".into()),
                    "the switcher answered DV_COMMAND_RESULT_ERROR",
                )),
                other => Err(device_error(None, format!("unexpected result {other}"))),
            };
        }
        let body = &packet[8..];
        match req {
            Req::Get(ids) => {
                // A SET_CONTROL packet: pairs, in the order asked, possibly
                // followed by padding (5.3's example ends in two zero words).
                let mut pairs = Vec::new();
                for (n, want) in ids.iter().enumerate() {
                    match (le32(body, 8 * n), le32(body, 8 * n + 4)) {
                        (Some(i), Some(v)) if i == *want => pairs.push((i, v)),
                        _ => break,
                    }
                }
                Ok(self.apply(cx, &pairs))
            }
            Req::InputName(n) => {
                let chars = le32(body, 0).unwrap_or(0) as usize;
                let name = utf16(&body[4.min(body.len())..], chars);
                let mut patch = Map::new();
                put(&mut patch, &format!("inputs.{}.name", n + 1), json!(name));
                cx.state(Value::Object(patch));
                Ok(Some(json!(name)))
            }
            Req::FileName(..) => {
                let chars = le32(body, 0).unwrap_or(0) as usize;
                Ok(Some(json!(utf16(&body[4.min(body.len())..], chars))))
            }
            Req::MiniPic(_) => {
                let version = le32(body, 0).unwrap_or(0);
                let data = &body[4.min(body.len())..];
                Ok(Some(json!({
                    "width": 96,
                    "height": 54,
                    "version": version,
                    "format": "rgbx",
                    "data": base64::engine::general_purpose::STANDARD.encode(data),
                })))
            }
            Req::ReadData => {
                let len = le32(body, 0).unwrap_or(0) as usize;
                let data = body.get(4..4 + len).unwrap_or(&body[4.min(body.len())..]);
                if !data.is_empty() {
                    cx.file_write(FILE, data.to_vec());
                }
                if len >= CHUNK {
                    job.steps.push_front(Req::ReadData);
                }
                Ok(None)
            }
            _ => Err(device_error(
                None,
                format!("unexpected reply {packet:02x?}"),
            )),
        }
    }

    // --- Real-time port.

    fn rt_data(&mut self, cx: &mut Cx, bytes: &[u8]) {
        self.rt_buf.extend_from_slice(bytes);
        loop {
            let Some(size) = le32(&self.rt_buf, 0) else {
                return;
            };
            let size = size as usize;
            if !(8..=16 * 1024 * 1024).contains(&size) || !size.is_multiple_of(4) {
                cx.log(
                    Level::Warning,
                    format!("bad real-time packet size {size}, reconnecting"),
                );
                self.rt_buf.clear();
                cx.tcp_close(RT);
                self.rt_lost(cx);
                return;
            }
            if self.rt_buf.len() < size {
                return;
            }
            let packet: Vec<u8> = self.rt_buf.drain(..size).collect();
            // Every packet is answered, or the switcher sends no more (3.2.5).
            cx.tcp_send(RT, NULL_PACKET.to_vec());
            if cx.now() >= self.rt_alive + 1_000 {
                self.rt_alive = cx.now();
                cx.alive();
            }
            let pairs: Vec<(u32, u32)> = packet[8..]
                .as_chunks::<8>()
                .0
                .iter()
                .filter_map(|c| Some((le32(c, 0)?, le32(c, 4)?)))
                .collect();
            if self.rt_first {
                // The first packet is the whole state against all zeros
                // (3.2.3): every control not in it is zero.
                self.rt_first = false;
                let mut all: Vec<(u32, u32)> = self.table.iter().map(|c| (c.id, 0)).collect();
                for p in &self.present {
                    for w in 0..p.words {
                        all.push((id(p.section, w), 0));
                    }
                }
                // Present words absent from the map are rebuilt as changed.
                self.present_words.clear();
                let given: HashMap<u32, u32> = pairs.iter().copied().collect();
                for pair in all.iter_mut() {
                    if let Some(v) = given.get(&pair.0) {
                        pair.1 = *v;
                    }
                }
                self.apply(cx, &all);
            } else if !pairs.is_empty() {
                self.apply(cx, &pairs);
            }
        }
    }

    fn rt_lost(&mut self, cx: &mut Cx) {
        self.rt_open = false;
        if self.phase == Phase::Open && self.monitor {
            cx.set_timer(RT_RETRY_TIMER, self.rt_retry_after);
            self.rt_retry_after = (self.rt_retry_after * 2).min(RETRY_MAX);
        }
    }

    fn open_rt(&mut self, cx: &mut Cx) {
        if let Some(host) = self.host() {
            self.rt_buf.clear();
            cx.tcp_open(RT, SocketAddr::new(host, self.rt_port));
        }
    }

    // --- Connection life.

    fn connected(&mut self, cx: &mut Cx) {
        self.phase = Phase::Open;
        self.retry_after = RETRY_MIN;
        self.cmd_buf.clear();
        // The first read shows the switcher answers.
        self.queue
            .push_front(Job::internal(Req::Get(vec![id(SECTION_STATUS, 0)])));
        if self.monitor {
            if self.serial() {
                self.queue_poll();
                cx.set_timer(POLL_TIMER, self.poll_ms);
            } else {
                for n in 0..inputs(self.family) {
                    self.queue.push_back(Job::internal(Req::InputName(n)));
                }
                self.open_rt(cx);
            }
        }
        cx.set_timer(PROBE_TIMER, PROBE_EVERY);
        self.pump(cx);
    }

    /// Over RS-232: the fast-changing controls, and the next part of the
    /// rest, so every control is read in turn.
    fn poll(&mut self, cx: &mut Cx) {
        if self.queue.iter().any(|j| j.id.is_none()) {
            return;
        }
        self.queue_poll();
        self.pump(cx);
    }

    fn queue_poll(&mut self) {
        self.queue_read(self.fast.clone());
        let mut rest: Vec<u32> = self.table.iter().map(|c| c.id).collect();
        for p in &self.present {
            rest.extend((0..p.words).map(|w| id(p.section, w)));
        }
        if self.cursor >= rest.len() {
            self.cursor = 0;
        }
        let end = (self.cursor + SERIAL_GETS).min(rest.len());
        self.queue_read(rest[self.cursor..end].to_vec());
        self.cursor = end;
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.phase = Phase::Closed;
        self.reachable = false;
        self.cmd_buf.clear();
        for job in self
            .in_flight
            .take()
            .map(|(j, _, _)| j)
            .into_iter()
            .chain(self.queue.drain(..))
        {
            if job.returns == Returns::File {
                cx.file_close(FILE);
            }
            if let Some(id) = job.id {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        for key in [REPLY_TIMER, PROBE_TIMER, POLL_TIMER, RT_RETRY_TIMER] {
            cx.cancel_timer(key);
        }
        if self.rt_open {
            cx.tcp_close(RT);
            self.rt_open = false;
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        match (socket, input) {
            (REQ, TcpInput::Connected) => {
                // 2.1.1: size 8, connection request 0x55aa0001.
                cx.tcp_send(REQ, vec![8, 0, 0, 0, 0x01, 0x00, 0xAA, 0x55]);
            }
            (REQ, TcpInput::Data(bytes)) => {
                self.req_buf.extend_from_slice(&bytes);
                if self.req_buf.len() < 8 {
                    return;
                }
                let info = le32(&self.req_buf, 4).unwrap_or(0);
                self.req_buf.clear();
                cx.tcp_close(REQ);
                match info {
                    0xAA55_FFFF => self.lost(cx, "no controller port is free (port 5009)".into()),
                    i if i >> 16 == 0xAA55 => {
                        self.rt_port = (i & 0xFFFF) as u16;
                        self.cmd_port = self.rt_port + 1;
                        self.phase = Phase::Connecting;
                        if let Some(host) = self.host() {
                            cx.tcp_open(CMD, SocketAddr::new(host, self.cmd_port));
                        }
                    }
                    other => self.lost(
                        cx,
                        format!("port 5009 answered {other:#010x}, not a free port"),
                    ),
                }
            }
            (REQ, TcpInput::Closed { reason }) => {
                if self.phase == Phase::Requesting {
                    self.lost(cx, format!("port 5009: {reason}"));
                }
            }
            (CMD, TcpInput::Connected) => self.connected(cx),
            (CMD, TcpInput::Data(bytes)) => self.cmd_data(cx, &bytes),
            (CMD, TcpInput::Closed { reason }) => self.lost(cx, reason),
            (RT, TcpInput::Connected) => {
                self.rt_open = true;
                self.rt_first = true;
                self.rt_retry_after = RETRY_MIN;
            }
            (RT, TcpInput::Data(bytes)) => self.rt_data(cx, &bytes),
            (RT, TcpInput::Closed { reason }) => {
                cx.log(Level::Info, format!("real-time port closed: {reason}"));
                self.rt_lost(cx);
            }
            _ => {}
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => self.open(cx),
            RT_RETRY_TIMER if self.phase == Phase::Open && !self.rt_open => self.open_rt(cx),
            REPLY_TIMER => {
                if let Some((job, req, _)) = self.in_flight.take() {
                    if job.returns == Returns::File {
                        cx.file_close(FILE);
                    }
                    if matches!(req, Req::ReadData | Req::StoreData { .. }) {
                        self.queue.push_front(Job::internal(Req::CloseData));
                    }
                    if let Some(id) = job.id {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES && self.reachable {
                        self.reachable = false;
                        cx.connection(Connection::Disconnected {
                            reason: "no reply from the switcher".into(),
                        });
                    }
                }
                self.pump(cx);
            }
            PROBE_TIMER if self.phase == Phase::Open => {
                if self.in_flight.is_none() && self.queue.is_empty() {
                    self.queue
                        .push_back(Job::internal(Req::Get(vec![id(SECTION_STATUS, 0)])));
                    self.pump(cx);
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
            }
            POLL_TIMER if self.phase == Phase::Open && self.monitor => {
                self.poll(cx);
                cx.set_timer(POLL_TIMER, self.poll_ms);
            }
            _ => {}
        }
    }

    fn file(&mut self, cx: &mut Cx, input: FileInput) {
        match (self.file_job.take(), input) {
            (Some((id, FileOp::Export)), FileInput::Closed { bytes }) => {
                cx.complete(
                    id,
                    Ok(Outcome::Value {
                        value: json!({"bytes": bytes}),
                    }),
                );
            }
            (Some((id, FileOp::Export)), FileInput::Failed { message }) => {
                cx.complete(id, Err(CommandError::Transport { message }));
            }
            (Some((id, FileOp::Import { kind, number })), FileInput::Read { data }) => {
                let mut steps = VecDeque::from([Req::OpenData {
                    kind,
                    number,
                    write: true,
                }]);
                for (n, chunk) in data.chunks(CHUNK).enumerate() {
                    steps.push_back(Req::StoreData {
                        chunk: n as u32,
                        data: chunk.to_vec(),
                    });
                }
                steps.push_back(Req::CloseData);
                steps.push_back(Req::FileEvent(kind));
                self.enqueue(
                    cx,
                    Job {
                        id: Some(id),
                        steps,
                        returns: Returns::Value,
                        value: Some(json!({"bytes": data.len()})),
                    },
                );
            }
            (Some((id, FileOp::Import { .. })), FileInput::Failed { message }) => {
                cx.complete(id, Err(CommandError::Transport { message }));
            }
            (None, FileInput::Failed { message }) => self.file_error = Some(message),
            (job, _) => self.file_job = job,
        }
    }

    // --- Commands.

    /// Set controls by path and raw value, then read them back.
    fn set_job(&self, id: CommandId, values: Vec<(u32, u32)>) -> Job {
        let ids: Vec<u32> = values.iter().map(|(i, _)| *i).collect();
        let mut steps = VecDeque::new();
        let limit = if self.serial() { SERIAL_SETS } else { 256 };
        for chunk in values.chunks(limit) {
            steps.push_back(Req::Set(chunk.to_vec()));
        }
        // Read back what was written.
        let readable: Vec<u32> = ids;
        let limit = if self.serial() { SERIAL_GETS } else { 256 };
        for chunk in readable.chunks(limit) {
            steps.push_back(Req::Get(chunk.to_vec()));
        }
        Job {
            id: Some(id),
            steps,
            returns: Returns::Ack,
            value: None,
        }
    }

    fn raw(&self, path: &str, value: u32) -> Result<(u32, u32), CommandError> {
        Ok((self.control(path)?.id, value))
    }

    fn keyer(&self, params: &Params) -> Result<u32, CommandError> {
        let k = int(params, "keyer")?;
        if k < 1 || k > keyers(self.family) as i64 {
            return Err(bad(format!(
                "keyer {k} is not 1 to {}",
                keyers(self.family)
            )));
        }
        Ok(k as u32)
    }

    fn input(&self, params: &Params) -> Result<u32, CommandError> {
        let n = int(params, "input")?;
        if n < 1 || n > inputs(self.family) as i64 {
            return Err(bad(format!(
                "input {n} is not 1 to {}",
                inputs(self.family)
            )));
        }
        Ok(n as u32)
    }

    fn tcp_only(&self, name: &str) -> Result<(), CommandError> {
        if self.serial() {
            Err(bad(format!(
                "'{name}' is part of the Ethernet protocol only: open the switcher with link tcp"
            )))
        } else {
            Ok(())
        }
    }

    fn plan(
        &self,
        cx: &mut Cx,
        cid: CommandId,
        name: &str,
        p: &Params,
    ) -> Result<Option<Job>, CommandError> {
        let src = |v: i64| v as u32;
        let set = |values: Vec<(u32, u32)>| Ok(Some(self.set_job(cid, values)));
        let me = |cmd: &str| (id(SECTION_TRANSITION_CTRL, 0), enum_raw(TRANS_COMMAND, cmd));
        match name {
            "set_control" => {
                let path = text(p, "control")?;
                let c = self.control(path)?;
                if !c.writable {
                    return Err(bad(format!("'{path}' is read only")));
                }
                let raw = encode(&c.kind, text(p, "value")?).map_err(bad)?;
                set(vec![(c.id, raw)])
            }
            "get_control" => {
                let path = text(p, "control")?;
                let c = self.control(path)?;
                Ok(Some(Job {
                    id: Some(cid),
                    steps: VecDeque::from([Req::Get(vec![c.id])]),
                    returns: Returns::Value,
                    value: None,
                }))
            }
            "set_program" => set(vec![self.raw("program", src(int(p, "source")?))?]),
            "set_preview" => set(vec![self.raw("preview", src(int(p, "source")?))?]),
            "cut" => set(vec![me("goto_end")]),
            "auto" => set(vec![me("run")]),
            "set_transition" => {
                let c = self.control("transition.type")?;
                let raw = encode(&c.kind, text(p, "type")?).map_err(bad)?;
                set(vec![(c.id, raw)])
            }
            "set_transition_duration" => {
                let engine = text(p, "engine").unwrap_or("me");
                let frames = int(p, "frames")?;
                set(vec![self.raw(
                    &format!("transitions.{engine}.duration"),
                    frames as u32,
                )?])
            }
            "set_wipe_pattern" => set(vec![self.raw("wipe.pattern", int(p, "pattern")? as u32)?]),
            "set_transition_position" => {
                let pos = p
                    .get("position")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| bad("'position' is required"))?;
                set(vec![self.raw("wipe.level", (pos as f32).to_bits())?])
            }
            "set_next_transition" => {
                let mut values = Vec::new();
                if let Some(on) = p.get("background").and_then(Value::as_bool) {
                    values.push(self.raw("next_transition.background", on as u32)?);
                }
                for k in 1..=4 {
                    if let Some(on) = p.get(&format!("key{k}")).and_then(Value::as_bool) {
                        values.push(self.raw(&format!("next_transition.key{k}"), on as u32)?);
                    }
                }
                if values.is_empty() {
                    return Err(bad("give background or a key"));
                }
                set(values)
            }
            "set_keyer" => {
                let k = self.keyer(p)?;
                set(vec![
                    self.raw(&format!("keyers.{k}.on"), boolean(p, "on")? as u32)?
                ])
            }
            "set_keyer_sources" => {
                let k = self.keyer(p)?;
                let mut values = Vec::new();
                if let Some(s) = opt_int(p, "key_source") {
                    values.push(self.raw(&format!("keyers.{k}.key_source"), src(s))?);
                }
                if let Some(s) = opt_int(p, "split_source") {
                    values.push(self.raw(&format!("keyers.{k}.split_source"), src(s))?);
                }
                if values.is_empty() {
                    return Err(bad("give key_source, split_source or both"));
                }
                set(values)
            }
            "set_dsk" => {
                let d = int(p, "dsk")?;
                set(vec![
                    self.raw(&format!("dsks.{d}.on"), boolean(p, "on")? as u32)?
                ])
            }
            "dsk_auto" => set(vec![(
                id(SECTION_TRANSITION_CTRL, 5),
                enum_raw(TRANS_COMMAND, "run"),
            )]),
            "fade_to_black" => set(vec![
                self.raw("ftb.enabled", 1)?,
                (
                    id(SECTION_TRANSITION_CTRL, 10),
                    enum_raw(TRANS_COMMAND, "run"),
                ),
            ]),
            "set_aux" => {
                let a = int(p, "aux")?;
                set(vec![
                    self.raw(&format!("aux.{a}.source"), src(int(p, "source")?))?
                ])
            }
            "set_input_freeze" => {
                let n = self.input(p)?;
                let c = self.control(&format!("inputs.{n}.freeze"))?;
                let raw = encode(&c.kind, text(p, "mode")?).map_err(bad)?;
                set(vec![(c.id, raw)])
            }
            "load_memory" | "store_memory" | "delete_memory" => {
                let verb = name.trim_end_matches("_memory");
                set(vec![
                    (id(SECTION_MEMORY_CTRL, 0), int(p, "memory")? as u32),
                    (id(SECTION_MEMORY_CTRL, 1), enum_raw(MEMORY_COMMAND, verb)),
                ])
            }
            "load_still" | "store_still" | "grab_still" | "delete_still" => {
                let verb = name.trim_end_matches("_still");
                let mut values = Vec::new();
                if let Some(s) = opt_int(p, "still") {
                    values.push((id(SECTION_STILL_CTRL, 0), s as u32));
                } else if verb != "grab" {
                    return Err(bad("'still' is required"));
                }
                if let Some(b) = opt_int(p, "buffer") {
                    values.push((id(SECTION_STILL_CTRL, 1), b as u32));
                } else if verb != "delete" {
                    return Err(bad("'buffer' is required"));
                }
                values.push((id(SECTION_STILL_CTRL, 2), enum_raw(STILL_COMMAND, verb)));
                set(values)
            }
            "chroma_key_auto" => {
                self.tcp_only(name)?;
                let k = self.keyer(p)?;
                Ok(Some(Job {
                    id: Some(cid),
                    steps: VecDeque::from([
                        Req::ChromaAuto(k - 1),
                        Req::Get(
                            [
                                "hue",
                                "saturation",
                                "luma",
                                "key_acceptance",
                                "key_gain",
                                "key_lift",
                                "chroma_acceptance",
                                "suppression",
                            ]
                            .iter()
                            .map(|f| self.id_of(&format!("keyers.{k}.chroma.{f}")))
                            .collect(),
                        ),
                    ]),
                    returns: Returns::Ack,
                    value: None,
                }))
            }
            "get_input_name" => {
                self.tcp_only(name)?;
                let n = self.input(p)?;
                Ok(Some(Job {
                    id: Some(cid),
                    steps: VecDeque::from([Req::InputName(n - 1)]),
                    returns: Returns::Value,
                    value: None,
                }))
            }
            "get_file_name" => {
                self.tcp_only(name)?;
                Ok(Some(Job {
                    id: Some(cid),
                    steps: VecDeque::from([Req::FileName(file_type(p)?, int(p, "number")? as u32)]),
                    returns: Returns::Value,
                    value: None,
                }))
            }
            "set_file_name" => {
                self.tcp_only(name)?;
                let kind = file_type(p)?;
                let number = int(p, "number")? as u32;
                let new = text(p, "name")?.to_string();
                Ok(Some(Job {
                    id: Some(cid),
                    steps: VecDeque::from([
                        Req::SetFileName(kind, number, new),
                        Req::FileName(kind, number),
                    ]),
                    returns: Returns::Ack,
                    value: None,
                }))
            }
            "get_mini_pic" => {
                self.tcp_only(name)?;
                Ok(Some(Job {
                    id: Some(cid),
                    steps: VecDeque::from([Req::MiniPic(int(p, "number")? as u32)]),
                    returns: Returns::Value,
                    value: None,
                }))
            }
            "export_file" => {
                self.tcp_only(name)?;
                let kind = file_type(p)?;
                let number = int(p, "number")? as u32;
                let path = text(p, "path")?;
                if self.file_job.is_some() || self.queue.iter().any(|j| j.returns == Returns::File)
                {
                    return Err(bad("a file transfer is already running"));
                }
                cx.file_open(FILE, path);
                Ok(Some(Job {
                    id: Some(cid),
                    steps: VecDeque::from([
                        Req::OpenData {
                            kind,
                            number,
                            write: false,
                        },
                        Req::ReadData,
                        Req::CloseData,
                    ]),
                    returns: Returns::File,
                    value: None,
                }))
            }
            "press_button" | "set_lamp" | "menu_select" | "all_stop" => {
                if !self.serial() {
                    return Err(bad(format!(
                        "'{name}' is part of the RS-232 protocol only: open the switcher with link serial"
                    )));
                }
                let message = match name {
                    "press_button" => vec![0x03, 0x00, 0xFB, int(p, "button")? as u8],
                    "set_lamp" => vec![
                        0x03,
                        0x00,
                        if boolean(p, "on")? { 0xC6 } else { 0xC7 },
                        int(p, "button")? as u8,
                    ],
                    "menu_select" => {
                        let menu = int(p, "menu")? as u8;
                        let sub = opt_int(p, "sub_menu").unwrap_or(0) as u8;
                        vec![0x03, 0x00, 0xDE, (sub << 4) | (menu & 0x0F)]
                    }
                    _ => vec![0x03, 0x00, 0xF2, 0x00],
                };
                Ok(Some(Job {
                    id: Some(cid),
                    steps: VecDeque::from([Req::Gvg(message)]),
                    returns: Returns::Ack,
                    value: None,
                }))
            }
            other => Err(CommandError::UnknownCommand {
                command: other.to_string(),
            }),
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.phase != Phase::Open {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        if name == "import_file" {
            match (
                self.tcp_only(name),
                file_type(params),
                int(params, "number"),
                text(params, "path"),
            ) {
                (Ok(()), Ok(kind), Ok(number), Ok(path)) => {
                    if self.file_job.is_some()
                        || self.queue.iter().any(|j| j.returns == Returns::File)
                    {
                        cx.complete(id, Err(bad("a file transfer is already running")));
                        return;
                    }
                    cx.file_read(FILE, path);
                    self.file_job = Some((
                        id,
                        FileOp::Import {
                            kind,
                            number: number as u32,
                        },
                    ));
                }
                (Err(e), ..) | (_, Err(e), ..) | (_, _, Err(e), _) | (.., Err(e)) => {
                    cx.complete(id, Err(e))
                }
            }
            return;
        }
        if name == "export_file" {
            self.file_error = None;
        }
        match self.plan(cx, id, name, params) {
            Ok(Some(job)) => self.enqueue(cx, job),
            Ok(None) => {}
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(CMD);
        if self.rt_open {
            cx.tcp_close(RT);
        }
        if self.phase == Phase::Requesting {
            cx.tcp_close(REQ);
        }
    }
}

fn sized(mut body: Vec<u8>) -> Vec<u8> {
    let size = (body.len() + 4) as u32;
    let mut out = size.to_le_bytes().to_vec();
    out.append(&mut body);
    out
}

// ---------------------------------------------------------------------------
// SE-2200: the DVIP serial control protocol.

/// Panel buttons (Table 6), by name.
pub(crate) const SE2200_BUTTONS: &[(&str, u8)] = &[
    ("wipe_1", 0x06),
    ("wipe_2", 0x07),
    ("wipe_3", 0x08),
    ("wipe_4", 0x09),
    ("wipe_5", 0x0A),
    ("wipe_6", 0x0B),
    ("f1", 0x0C),
    ("f2", 0x0D),
    ("pip_1_program", 0x0E),
    ("pip_2_program", 0x0F),
    ("dsk_program", 0x10),
    ("multiscreen", 0x11),
    ("wipe_7", 0x16),
    ("wipe_8", 0x17),
    ("wipe_9", 0x18),
    ("wipe_10", 0x19),
    ("wipe_11", 0x1A),
    ("wipe_12", 0x1B),
    ("f3", 0x1C),
    ("f4", 0x1D),
    ("pip_1_preview", 0x1E),
    ("pip_2_preview", 0x1F),
    ("dsk_preview", 0x20),
    ("program_black", 0x21),
    ("program_1", 0x22),
    ("program_2", 0x23),
    ("program_3", 0x24),
    ("program_4", 0x25),
    ("program_5", 0x26),
    ("program_6", 0x29),
    ("program_background", 0x2A),
    ("freeze", 0x2E),
    ("mix", 0x2F),
    ("wipe", 0x30),
    ("preset_black", 0x31),
    ("preset_1", 0x32),
    ("preset_2", 0x33),
    ("preset_3", 0x34),
    ("preset_4", 0x35),
    ("preset_5", 0x36),
    ("preset_6", 0x39),
    ("preset_background", 0x3A),
    ("cut", 0x3E),
    ("ftb", 0x3F),
    ("auto", 0x40),
    ("timer", 0x41),
    ("audio_fixed", 0x42),
    ("audio_follow_video", 0x43),
    ("wipe_13", 0x46),
    ("wipe_14", 0x47),
    ("wipe_15", 0x48),
    ("wipe_16", 0x49),
    ("wipe_17", 0x4A),
    ("border", 0x4B),
    ("logo_1", 0x4C),
    ("logo_2", 0x4F),
    ("clock", 0x50),
    ("speed_1", 0x5E),
    ("speed_2", 0x5F),
    ("speed_3", 0x60),
];

/// The toggles the status reports: (feature, status byte from 1, bit, button).
const SE2200_FEATURES: &[(&str, usize, u8, &str)] = &[
    ("logo_1", 3, 0, "logo_1"),
    ("logo_2", 3, 1, "logo_2"),
    ("border", 3, 2, "border"),
    ("f1", 3, 3, "f1"),
    ("f2", 3, 4, "f2"),
    ("f3", 3, 5, "f3"),
    ("f4", 3, 6, "f4"),
    ("pip_1_program", 4, 0, "pip_1_program"),
    ("pip_2_program", 4, 1, "pip_2_program"),
    ("dsk_program", 4, 2, "dsk_program"),
    ("pip_1_preview", 4, 3, "pip_1_preview"),
    ("pip_2_preview", 4, 4, "pip_2_preview"),
    ("dsk_preview", 4, 5, "dsk_preview"),
    ("clock", 4, 6, "clock"),
    ("timer", 4, 7, "timer"),
    ("audio_fixed", 5, 0, "audio_fixed"),
    ("audio_follow_video", 5, 1, "audio_follow_video"),
];

fn button(name: &str) -> Option<u8> {
    SE2200_BUTTONS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
}

pub(crate) fn se2200_block(command: u8, params: &[u8]) -> Vec<u8> {
    let count = 3 + params.len() as u8;
    let mut out = vec![0xAA, count, 0xFF, command];
    out.extend_from_slice(params);
    let sum = out[1..].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    out.push(sum);
    out
}

pub(crate) fn se2200_status(bytes: &[u8]) -> Value {
    let bit = |byte: usize, b: u8| bytes[byte - 1] & (1 << b) != 0;
    let mut patch = Map::new();
    put(&mut patch, "program", json!(bytes[0] & 0x0F));
    put(&mut patch, "preview", json!(bytes[0] >> 4));
    put(&mut patch, "wipe.pattern", json!(bytes[1] & 0x1F));
    put(&mut patch, "transition.speed", json!(bytes[1] >> 5));
    for (feature, byte, b, _) in SE2200_FEATURES {
        put(
            &mut patch,
            &format!("features.{feature}"),
            json!(bit(*byte, *b)),
        );
    }
    put(&mut patch, "multiscreen", json!(!bit(5, 2)));
    if bit(5, 3) {
        put(&mut patch, "transition.type", json!("wipe"));
    } else if bit(5, 4) {
        put(&mut patch, "transition.type", json!("mix"));
    }
    put(&mut patch, "ftb.active", json!(bit(5, 5)));
    put(
        &mut patch,
        "t_bar",
        json!(u16::from_le_bytes([bytes[5], bytes[6]])),
    );
    Value::Object(patch)
}

#[derive(Debug, Clone, PartialEq)]
enum S2Req {
    Button(u8),
    TBar(u16),
    Status,
    /// Press the button only when the status shows the feature otherwise.
    Toggle {
        byte: usize,
        bit: u8,
        on: bool,
        code: u8,
    },
}

struct S2Job {
    id: Option<CommandId>,
    steps: VecDeque<S2Req>,
    returns_status: bool,
}

struct Se2200 {
    link: Link,
    tcp: bool,
    monitor: bool,
    poll_ms: Millis,
    open: bool,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    buf: Vec<u8>,
    queue: VecDeque<S2Job>,
    in_flight: Option<(S2Job, S2Req, Millis)>,
    stamps: PollStamps,
    status: Option<[u8; 7]>,
}

impl Se2200 {
    fn new(link: Link, monitor: bool, poll_ms: Millis) -> Se2200 {
        let tcp = matches!(link, Link::Tcp(_));
        Se2200 {
            link,
            tcp,
            monitor,
            poll_ms,
            open: false,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            buf: Vec::new(),
            queue: VecDeque::new(),
            in_flight: None,
            status: None,
            stamps: PollStamps::default(),
        }
    }

    fn open_link(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.buf.clear();
        self.link.open(cx, CMD);
    }

    fn wrap(&self, block: Vec<u8>) -> Vec<u8> {
        if self.tcp {
            // DVIP's header: the whole packet's length, big-endian.
            let len = block.len() + 2;
            let mut out = vec![(len >> 8) as u8, len as u8];
            out.extend(block);
            out
        } else {
            block
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        while self.in_flight.is_none() && self.open {
            let Some(mut job) = self.queue.pop_front() else {
                return;
            };
            let Some(req) = job.steps.pop_front() else {
                if let Some(id) = job.id {
                    cx.complete(id, Ok(Outcome::Ack));
                }
                continue;
            };
            let block = match &req {
                S2Req::Button(code) => se2200_block(0x20, &[*code]),
                S2Req::TBar(pos) => se2200_block(0x21, &pos.to_le_bytes()),
                S2Req::Status => se2200_block(0x22, &[]),
                S2Req::Toggle {
                    byte,
                    bit,
                    on,
                    code,
                } => match self.status {
                    None => {
                        job.steps.push_front(req.clone());
                        job.steps.push_front(S2Req::Status);
                        self.queue.push_front(job);
                        continue;
                    }
                    Some(s) if (s[byte - 1] & (1 << bit) != 0) == *on => {
                        // Already so: nothing to press.
                        self.queue.push_front(job);
                        continue;
                    }
                    Some(_) => se2200_block(0x20, &[*code]),
                },
            };
            cx.tcp_send(CMD, self.wrap(block));
            cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
            self.in_flight = Some((job, req, cx.now()));
        }
    }

    fn enqueue(&mut self, cx: &mut Cx, job: S2Job) {
        let at = self.queue.iter().position(|j| j.id.is_none());
        match at {
            Some(at) => self.queue.insert(at, job),
            None => self.queue.push_back(job),
        }
        self.pump(cx);
    }

    fn read_status(&mut self, cx: &mut Cx) {
        if !self.queue.iter().any(|j| j.id.is_none()) && self.in_flight.is_none() {
            self.queue.push_back(S2Job {
                id: None,
                steps: VecDeque::from([S2Req::Status]),
                returns_status: false,
            });
            self.pump(cx);
        }
    }

    fn data(&mut self, cx: &mut Cx, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        loop {
            let message: Vec<u8> = if self.tcp {
                if self.buf.len() < 2 {
                    return;
                }
                let len = u16::from_be_bytes([self.buf[0], self.buf[1]]) as usize;
                if len < 3 {
                    self.buf.clear();
                    return;
                }
                if self.buf.len() < len {
                    return;
                }
                self.buf.drain(..len).skip(2).collect()
            } else {
                let Some(&count) = self.buf.first() else {
                    return;
                };
                let count = count as usize;
                if count < 2 {
                    self.buf.remove(0);
                    continue;
                }
                if self.buf.len() < count {
                    return;
                }
                self.buf.drain(..count).collect()
            };
            self.reply(cx, &message);
        }
    }

    fn reply(&mut self, cx: &mut Cx, message: &[u8]) {
        let Some((job, req, sent_at)) = self.in_flight.take() else {
            return;
        };
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.misses = 0;
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
        let status = message.get(1).copied().unwrap_or(0);
        if job.id.is_none() && self.monitor && status & 0x80 != 0 {
            self.stamps.answered(cx, "status");
        }
        let result = if status & 0x80 == 0 {
            Err(device_error(
                Some(format!("{status:02x}")),
                if status & 0x40 != 0 {
                    "the switcher reported a protocol error"
                } else {
                    "the switcher ignored the command (is it in PC CTRL remote mode?)"
                },
            ))
        } else if req == S2Req::Status {
            match message.get(2..9) {
                Some(s) => {
                    let mut bytes = [0u8; 7];
                    bytes.copy_from_slice(s);
                    self.status = Some(bytes);
                    let state = se2200_status(&bytes);
                    cx.state(state.clone());
                    Ok(Some(state))
                }
                None => Err(device_error(None, format!("short status {message:02x?}"))),
            }
        } else {
            Ok(None)
        };
        match result {
            Err(e) => {
                if let Some(id) = job.id {
                    cx.complete(id, Err(e));
                }
            }
            Ok(state) => {
                if job.steps.is_empty() {
                    if let Some(id) = job.id {
                        let outcome = match (job.returns_status, state) {
                            (true, Some(value)) => Outcome::Value { value },
                            _ => Outcome::Ack,
                        };
                        cx.complete(id, Ok(outcome));
                    }
                } else {
                    self.queue.push_front(job);
                }
            }
        }
        self.pump(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, p: &Params) {
        if !self.open {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let steps: Result<Vec<S2Req>, CommandError> = (|| {
            let press = |name: &str| {
                button(name)
                    .map(S2Req::Button)
                    .ok_or_else(|| bad(format!("'{name}' is not a button")))
            };
            Ok(match name {
                "get_status" => vec![],
                "press" => vec![press(text(p, "button")?)?],
                "set_program" | "set_preview" => {
                    let bus = if name == "set_program" {
                        "program"
                    } else {
                        "preset"
                    };
                    let s = int(p, "source")?;
                    let b = match s {
                        1..=6 => format!("{bus}_{s}"),
                        7 => format!("{bus}_black"),
                        8 => format!("{bus}_background"),
                        _ => {
                            return Err(bad(
                                "the SE-2200's sources are 1-6, 7 (black) and 8 (background)",
                            ))
                        }
                    };
                    vec![press(&b)?]
                }
                "cut" => vec![press("cut")?],
                "auto" => vec![press("auto")?],
                "fade_to_black" => vec![press("ftb")?],
                "set_transition" => match text(p, "type")? {
                    t @ ("mix" | "wipe") => vec![press(t)?],
                    other => return Err(bad(format!("the SE-2200 has no '{other}' transition"))),
                },
                "set_wipe_pattern" => {
                    let n = int(p, "pattern")?;
                    if !(1..=17).contains(&n) {
                        return Err(bad("the SE-2200's wipes are 1 to 17"));
                    }
                    vec![press(&format!("wipe_{n}"))?]
                }
                "set_transition_speed" => vec![press(&format!("speed_{}", int(p, "speed")?))?],
                "set_t_bar" => vec![S2Req::TBar(int(p, "position")? as u16)],
                "set_feature" => {
                    let feature = text(p, "feature")?;
                    let on = boolean(p, "on")?;
                    let (_, byte, bit, b) = SE2200_FEATURES
                        .iter()
                        .find(|(f, ..)| *f == feature)
                        .ok_or_else(|| bad(format!("'{feature}' is not a feature")))?;
                    vec![S2Req::Toggle {
                        byte: *byte,
                        bit: *bit,
                        on,
                        code: button(b).unwrap(),
                    }]
                }
                other => {
                    return Err(CommandError::UnknownCommand {
                        command: other.to_string(),
                    })
                }
            })
        })();
        match steps {
            Ok(mut steps) => {
                // Every command ends in a status read, so state follows it.
                steps.push(S2Req::Status);
                self.enqueue(
                    cx,
                    S2Job {
                        id: Some(id),
                        steps: steps.into(),
                        returns_status: name == "get_status",
                    },
                );
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.open = true;
                self.retry_after = RETRY_MIN;
                self.buf.clear();
                self.read_status(cx);
                let every = if self.monitor {
                    self.poll_ms
                } else {
                    PROBE_EVERY
                };
                cx.set_timer(POLL_TIMER, every);
            }
            TcpInput::Data(bytes) => self.data(cx, &bytes),
            TcpInput::Closed { reason } => {
                self.open = false;
                self.reachable = false;
                for job in self
                    .in_flight
                    .take()
                    .map(|(j, _, _)| j)
                    .into_iter()
                    .chain(self.queue.drain(..))
                {
                    if let Some(id) = job.id {
                        cx.complete(
                            id,
                            Err(CommandError::Transport {
                                message: reason.clone(),
                            }),
                        );
                    }
                }
                cx.cancel_timer(REPLY_TIMER);
                cx.cancel_timer(POLL_TIMER);
                cx.connection(Connection::Disconnected { reason });
                cx.set_timer(RETRY_TIMER, self.retry_after);
                self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => self.open_link(cx),
            REPLY_TIMER => {
                if let Some((job, _, _)) = self.in_flight.take() {
                    if let Some(id) = job.id {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES && self.reachable {
                        self.reachable = false;
                        cx.connection(Connection::Disconnected {
                            reason: "no reply from the switcher".into(),
                        });
                    }
                }
                self.pump(cx);
            }
            POLL_TIMER if self.open => {
                self.read_status(cx);
                let every = if self.monitor {
                    self.poll_ms
                } else {
                    PROBE_EVERY
                };
                cx.set_timer(POLL_TIMER, every);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------

enum Protocol {
    Processor(Box<Processor>),
    Se2200(Box<Se2200>),
    DvipOnly,
}

pub(crate) struct DatavideoSwitcher {
    protocol: Protocol,
    dvip: Dvip,
}

pub(crate) fn family_of(model: &str) -> Option<Family> {
    match model {
        "se-1200mu" | "hs-1300" => Some(Family::Se1200),
        "se-3200" | "hs-3200" => Some(Family::Se3200),
        "se-4000" | "hs-4000" => Some(Family::Se4000),
        _ => None,
    }
}

/// The models whose RS-232 port takes the documented protocol.
fn has_serial(model: &str) -> bool {
    matches!(model, "se-1200mu" | "se-4000" | "se-2200")
}

impl DatavideoSwitcher {
    pub(crate) fn new(ctx: OpenContext) -> Result<DatavideoSwitcher, String> {
        let s = &ctx.settings;
        let poll_ms = s
            .get("poll_ms")
            .and_then(Value::as_u64)
            .unwrap_or(1_000)
            .max(100);
        let serial = s.get("link").and_then(Value::as_str) == Some("serial");
        if serial && !has_serial(&ctx.model) {
            return Err(format!(
                "the {} has no RS-232 control port Datavideo documents: use link tcp",
                ctx.model.to_uppercase()
            ));
        }
        let dvip_port = s
            .get("dvip_port")
            .and_then(Value::as_u64)
            .map(|p| p as u16)
            .unwrap_or(DVIP_PORT);
        let standalone = ctx.model == "se-650";
        if (standalone || !serial) && ctx.host.is_unspecified() {
            return Err("give the switcher's address as host".into());
        }
        let dvip = Dvip::new(
            SocketAddr::new(ctx.host, dvip_port),
            standalone,
            ctx.monitor,
            poll_ms.max(1_000),
        );
        let protocol = if let Some(family) = family_of(&ctx.model) {
            let link = if serial {
                PLink::Serial(SerialPort::from_settings(&ctx.settings)?)
            } else {
                PLink::Tcp(ctx.host)
            };
            Protocol::Processor(Box::new(Processor::new(family, link, &ctx, poll_ms)))
        } else if ctx.model == "se-2200" {
            Protocol::Se2200(Box::new(Se2200::new(
                Link::from_settings(&ctx, DVIP_PORT)?,
                ctx.monitor,
                poll_ms,
            )))
        } else if standalone {
            Protocol::DvipOnly
        } else {
            return Err(format!("no Datavideo model '{}'", ctx.model));
        };
        Ok(DatavideoSwitcher { protocol, dvip })
    }
}

const DVIP_COMMANDS: &[&str] = &[
    "get_network",
    "get_identity",
    "get_presets",
    "recall_preset",
    "set_dhcp",
    "set_ip_address",
    "set_netmask",
    "set_gateway",
    "set_dns",
    "set_host_name",
    "reboot",
];

impl Module for DatavideoSwitcher {
    fn start(&mut self, cx: &mut Cx) {
        match &mut self.protocol {
            Protocol::Processor(p) => p.open(cx),
            Protocol::Se2200(s) => s.open_link(cx),
            Protocol::DvipOnly => {}
        }
        // On a serial link the switcher's address may be unknown.
        if !self.dvip.to.ip().is_unspecified() {
            self.dvip.start(cx);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if DVIP_COMMANDS.contains(&name) {
            if self.dvip.to.ip().is_unspecified() {
                cx.complete(
                    id,
                    Err(bad("DVIP commands need the switcher's address as host")),
                );
                return;
            }
            self.dvip.command(cx, id, name, params);
            return;
        }
        match &mut self.protocol {
            Protocol::Processor(p) => p.command(cx, id, name, params),
            Protocol::Se2200(s) => s.command(cx, id, name, params),
            Protocol::DvipOnly => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.to_string(),
                }),
            ),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        self.dvip.datagram(cx, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        if socket == UDP {
            self.dvip.failed(cx, message);
        }
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        match &mut self.protocol {
            Protocol::Processor(p) => p.tcp(cx, socket, input),
            Protocol::Se2200(s) => s.tcp(cx, input),
            Protocol::DvipOnly => {}
        }
    }

    fn file(&mut self, cx: &mut Cx, _file: Key, input: FileInput) {
        if let Protocol::Processor(p) = &mut self.protocol {
            p.file(cx, input);
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            UDP_REPLY_TIMER | UDP_POLL_TIMER => self.dvip.timer(cx, key),
            _ => match &mut self.protocol {
                Protocol::Processor(p) => p.timer(cx, key),
                Protocol::Se2200(s) => s.timer(cx, key),
                Protocol::DvipOnly => {}
            },
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        match &mut self.protocol {
            Protocol::Processor(p) => p.stop(cx),
            Protocol::Se2200(_) => cx.tcp_close(CMD),
            Protocol::DvipOnly => {}
        }
        if self.dvip.opened {
            cx.udp_close(UDP);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::Ipv4Addr;

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 9));
    const OK: &str = "08 00 00 00 01 00 00 00";
    const ERROR: &str = "08 00 00 00 00 00 00 00";

    fn context(model: &str, monitor: bool, settings: Value) -> OpenContext {
        let spec = Catalog::source_tree().devices["datavideo-switcher"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        OpenContext {
            host: HOST,
            host_name: None,
            port: None,
            model: model.into(),
            channels: None,
            settings,
            monitor,
        }
    }

    fn module(model: &str, monitor: bool, settings: Value) -> DatavideoSwitcher {
        DatavideoSwitcher::new(context(model, monitor, settings)).unwrap()
    }

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    fn sent(actions: &[Action], socket: Key) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { socket: s, data } if *s == socket => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    fn udp_sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    fn opened(actions: &[Action]) -> Vec<(Key, u16)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpOpen { socket, to } => Some((*socket, to.port())),
                _ => None,
            })
            .collect()
    }

    /// The state the actions report, without the poll's `polled` stamp.
    fn state(actions: &[Action]) -> Value {
        let mut merged = json!({});
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(&mut merged, p);
            }
        }
        if let Some(m) = merged.as_object_mut() {
            m.remove("polled");
        }
        merged
    }

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn tcp(m: &mut DatavideoSwitcher, now: Millis, socket: Key, input: TcpInput) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, socket, input);
        cx.take()
    }

    fn data(m: &mut DatavideoSwitcher, now: Millis, socket: Key, bytes: &[u8]) -> Vec<Action> {
        tcp(m, now, socket, TcpInput::Data(bytes.to_vec()))
    }

    fn udp(m: &mut DatavideoSwitcher, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.datagram(&mut cx, UDP, SocketAddr::new(HOST, 5002), bytes);
        cx.take()
    }

    fn run(m: &mut DatavideoSwitcher, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn start(m: &mut DatavideoSwitcher) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take()
    }

    /// An SE-3200 on controller 1, connected and past its first read.
    fn se3200(monitor: bool) -> DatavideoSwitcher {
        let mut m = module("se-3200", monitor, json!({"controller": "1"}));
        assert_eq!(opened(&start(&mut m)), [(CMD, 5002)]);
        let a = tcp(&mut m, 0, CMD, TcpInput::Connected);
        // The first read is the connection status control.
        assert_eq!(sent(&a, CMD), [hex("0c 00 00 00 00 00 00 00 00 00 00 00")]);
        let a = data(
            &mut m,
            3,
            CMD,
            &hex("10 00 00 00 01 00 00 00 00 00 00 00 02 00 00 00"),
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(3)));
        assert_eq!(state(&a), json!({"status": {"connection": "connected"}}));
        m
    }

    #[test]
    fn values_decode_and_encode_by_kind() {
        let pct = Kind::Float {
            min: 0.0,
            max: 100.0,
        };
        assert_eq!(decode(&pct, 37.5f32.to_bits()), json!(37.5));
        assert_eq!(decode(&pct, 0.1f32.to_bits()), json!(0.1));
        assert_eq!(decode(&Kind::Flag, 1), json!(true));
        assert_eq!(decode(&Kind::FloatFlag, 1.0f32.to_bits()), json!(true));
        assert_eq!(decode(&Kind::Enum(TRANS_COMMAND), 5), json!("goto_end"));
        assert_eq!(decode(&Kind::Enum(TRANS_COMMAND), 42), json!(42));
        assert_eq!(
            decode(&Kind::Int { min: -5, max: 5 }, (-3i32) as u32),
            json!(-3)
        );
        assert_eq!(encode(&Kind::Enum(TRANS_COMMAND), "run"), Ok(1));
        assert_eq!(encode(&Kind::Enum(TRANS_COMMAND), "6"), Ok(6));
        assert!(encode(&Kind::Enum(TRANS_COMMAND), "fly").is_err());
        assert_eq!(encode(&Kind::Flag, "on"), Ok(1));
        assert_eq!(encode(&Kind::FloatFlag, "true"), Ok(1.0f32.to_bits()));
        assert_eq!(encode(&pct, "12.5"), Ok(12.5f32.to_bits()));
        assert!(encode(&pct, "101").is_err());
        assert!(encode(&Kind::Int { min: 1, max: 32 }, "0").is_err());
    }

    #[test]
    fn port_5009_hands_out_the_controller_ports() {
        let mut m = module("se-3200", true, json!({}));
        assert_eq!(opened(&start(&mut m)), [(REQ, REQUEST_PORT)]);
        // 5.2: request 08 00 00 00 01 00 AA 55, answered real-time port 5001.
        let a = tcp(&mut m, 0, REQ, TcpInput::Connected);
        assert_eq!(sent(&a, REQ), [hex("08 00 00 00 01 00 aa 55")]);
        let a = data(&mut m, 1, REQ, &hex("08 00 00 00 89 13 55 aa"));
        assert!(a.contains(&Action::TcpClose { socket: REQ }));
        assert_eq!(opened(&a), [(CMD, 5002)]);

        let mut m = module("se-3200", true, json!({}));
        start(&mut m);
        tcp(&mut m, 0, REQ, TcpInput::Connected);
        let a = data(&mut m, 1, REQ, &hex("08 00 00 00 8d 13 55 aa"));
        assert_eq!(opened(&a), [(CMD, 5006)]);

        let mut m = module("se-3200", true, json!({}));
        start(&mut m);
        tcp(&mut m, 0, REQ, TcpInput::Connected);
        let a = data(&mut m, 1, REQ, &hex("08 00 00 00 ff ff 55 aa"));
        assert!(opened(&a).is_empty());
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
    }

    #[test]
    fn commands_match_the_documents_examples() {
        let mut m = se3200(false);
        // 5.3: set program to input 3, read back; the reply ends in two zero
        // words.
        let a = run(&mut m, 1, "set_program", json!({"source": 3}));
        assert_eq!(
            sent(&a, CMD),
            [hex("10 00 00 00 01 00 00 00 94 00 02 00 03 00 00 00")]
        );
        let a = data(&mut m, 0, CMD, &hex(OK));
        assert_eq!(sent(&a, CMD), [hex("0c 00 00 00 00 00 00 00 94 00 02 00")]);
        let a = data(
            &mut m,
            0,
            CMD,
            &hex("18 00 00 00 01 00 00 00 94 00 02 00 03 00 00 00 00 00 00 00 00 00 00 00"),
        );
        assert_eq!(state(&a), json!({"program": 3}));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));

        let cases = [
            (
                "set_preview",
                json!({"source": 4}),
                "10 00 00 00 01 00 00 00 95 00 02 00 04 00 00 00",
            ),
            (
                "cut",
                json!({}),
                "10 00 00 00 01 00 00 00 00 00 07 00 05 00 00 00",
            ),
            (
                "auto",
                json!({}),
                "10 00 00 00 01 00 00 00 00 00 07 00 01 00 00 00",
            ),
            (
                "fade_to_black",
                json!({}),
                "18 00 00 00 01 00 00 00 c3 00 02 00 01 00 00 00 0a 00 07 00 01 00 00 00",
            ),
            (
                "set_transition",
                json!({"type": "wipe"}),
                "10 00 00 00 01 00 00 00 96 00 02 00 01 00 00 00",
            ),
            (
                "set_transition_duration",
                json!({"engine": "ftb", "frames": 70}),
                "10 00 00 00 01 00 00 00 0d 00 07 00 46 00 00 00",
            ),
            (
                "set_keyer",
                json!({"keyer": 1, "on": true}),
                "10 00 00 00 01 00 00 00 13 00 02 00 01 00 00 00",
            ),
            (
                "load_memory",
                json!({"memory": 3}),
                "18 00 00 00 01 00 00 00 00 00 08 00 03 00 00 00 01 00 08 00 01 00 00 00",
            ),
            (
                "set_input_freeze",
                json!({"input": 7, "mode": "freeze"}),
                "10 00 00 00 01 00 00 00 66 00 03 00 01 00 00 00",
            ),
            (
                "set_control",
                json!({"control": "keyers.1.pinp.position_x", "value": "0.5"}),
                "10 00 00 00 01 00 00 00 01 01 02 00 00 00 00 3f",
            ),
        ];
        for (n, (name, params, wire)) in cases.into_iter().enumerate() {
            let id = 10 + n as u64;
            let a = run(&mut m, id, name, params);
            assert_eq!(sent(&a, CMD), [hex(wire)], "{name}");
            // The set is acknowledged; an error to the read-back still ends
            // the command.
            data(&mut m, 0, CMD, &hex(OK));
            let a = data(&mut m, 0, CMD, &hex(ERROR));
            assert!(completed(&a, id).is_some(), "{name}");
        }

        let a = run(
            &mut m,
            30,
            "set_control",
            json!({"control": "status.board_version", "value": "1"}),
        );
        assert!(matches!(
            completed(&a, 30),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(
            &mut m,
            31,
            "set_control",
            json!({"control": "dve.size_x", "value": "1"}),
        );
        assert!(matches!(
            completed(&a, 31),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(&mut m, 32, "press_button", json!({"button": 74}));
        assert!(matches!(
            completed(&a, 32),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
    }

    #[test]
    fn get_control_returns_the_value_and_errors_fail() {
        let mut m = se3200(false);
        let a = run(&mut m, 1, "get_control", json!({"control": "wipe.level"}));
        assert_eq!(sent(&a, CMD), [hex("0c 00 00 00 00 00 00 00 01 00 02 00")]);
        let mut reply = hex("10 00 00 00 01 00 00 00 01 00 02 00");
        reply.extend(42.5f32.to_le_bytes());
        let a = data(&mut m, 0, CMD, &reply);
        assert_eq!(state(&a), json!({"wipe": {"level": 42.5}}));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(42.5) }))
        );
        let a = run(&mut m, 2, "set_wipe_pattern", json!({"pattern": 9}));
        assert_eq!(sent(&a, CMD).len(), 1);
        let a = data(&mut m, 0, CMD, &hex(ERROR));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn the_real_time_port_pushes_state_and_is_answered() {
        let mut m = module("se-3200", true, json!({"controller": "2"}));
        assert_eq!(opened(&start(&mut m)), [(CMD, 5004)]);
        let a = tcp(&mut m, 0, CMD, TcpInput::Connected);
        assert_eq!(opened(&a), [(RT, 5003)]);
        tcp(&mut m, 0, RT, TcpInput::Connected);
        // First packet: program input 5, key 1 on, wipe level 25, memory 3
        // stored (present word 0, bit 3).
        let mut first = hex(
            "28 00 00 00 00 00 00 00 94 00 02 00 05 00 00 00 13 00 02 00 01 00 00 00 01 00 02 00",
        );
        first.extend(25.0f32.to_le_bytes());
        first.extend(hex("00 00 09 00 08 00 00 00"));
        let a = data(&mut m, 10, RT, &first);
        assert_eq!(sent(&a, RT), [NULL_PACKET.to_vec()]);
        let s = state(&a);
        assert_eq!(s["program"], json!(5));
        assert_eq!(s["preview"], json!(0));
        assert_eq!(s["keyers"]["1"]["on"], json!(true));
        assert_eq!(s["keyers"]["2"]["on"], json!(false));
        assert_eq!(s["wipe"]["level"], json!(25.0));
        assert_eq!(s["transitions"]["me"]["command"], json!("stop"));
        assert_eq!(s["memory"]["stored"], json!([3]));
        assert_eq!(s["stills"]["stored"], json!([]));
        // A null packet changes nothing and is answered; a change split
        // across two reads applies once whole.
        let a = data(&mut m, 30, RT, &NULL_PACKET);
        assert_eq!(sent(&a, RT), [NULL_PACKET.to_vec()]);
        assert_eq!(state(&a), json!({}));
        let change = hex("10 00 00 00 00 00 00 00 95 00 02 00 02 00 00 00");
        assert!(sent(&data(&mut m, 40, RT, &change[..5]), RT).is_empty());
        let a = data(&mut m, 41, RT, &change[5..]);
        assert_eq!(state(&a), json!({"preview": 2}));
        // Losing the real-time port retries it.
        let a = tcp(
            &mut m,
            50,
            RT,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        assert!(a.iter().any(|x| matches!(
            x,
            Action::SetTimer {
                key: RT_RETRY_TIMER,
                ..
            }
        )));
    }

    #[test]
    fn commands_only_opens_no_real_time_port_and_reads_nothing() {
        let mut m = module("se-3200", false, json!({"controller": "1"}));
        assert!(udp_sent(&start(&mut m)).is_empty());
        let a = tcp(&mut m, 0, CMD, TcpInput::Connected);
        assert!(opened(&a).is_empty());
        assert_eq!(sent(&a, CMD).len(), 1);
        let a = data(
            &mut m,
            0,
            CMD,
            &hex("10 00 00 00 01 00 00 00 00 00 00 00 02 00 00 00"),
        );
        assert!(sent(&a, CMD).is_empty());
    }

    #[test]
    fn names_and_files_use_their_own_commands() {
        let mut m = se3200(false);
        let a = run(&mut m, 1, "get_input_name", json!({"input": 2}));
        assert_eq!(sent(&a, CMD), [hex("0c 00 00 00 09 00 00 00 01 00 00 00")]);
        let mut reply = hex("16 00 00 00 09 00 00 00 05 00 00 00");
        reply.extend("Cam 2".encode_utf16().flat_map(|u| u.to_le_bytes()));
        let a = data(&mut m, 0, CMD, &reply);
        assert_eq!(state(&a), json!({"inputs": {"2": {"name": "Cam 2"}}}));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!("Cam 2")
            }))
        );

        let a = run(
            &mut m,
            2,
            "set_file_name",
            json!({"type": "still", "number": 4, "name": "Logo"}),
        );
        let mut wire = hex("1e 00 00 00 0c 00 00 00 01 00 00 00 04 00 00 00 04 00 00 00");
        wire.extend("Logo\0".encode_utf16().flat_map(|u| u.to_le_bytes()));
        assert_eq!(sent(&a, CMD), [wire]);
        let a = data(&mut m, 0, CMD, &hex(OK));
        assert_eq!(
            sent(&a, CMD),
            [hex("10 00 00 00 0b 00 00 00 01 00 00 00 04 00 00 00")]
        );

        // An export: open for reading, chunks until a short one, close; the
        // command ends when the host file is closed.
        let mut m = se3200(false);
        let a = run(
            &mut m,
            3,
            "export_file",
            json!({"type": "still", "number": 2, "path": "/x/s.bin"}),
        );
        assert!(a.contains(&Action::FileOpen {
            file: FILE,
            path: "/x/s.bin".into()
        }));
        assert_eq!(
            sent(&a, CMD),
            [hex(
                "14 00 00 00 02 00 00 00 01 00 00 00 02 00 00 00 00 00 00 00"
            )]
        );
        let a = data(&mut m, 0, CMD, &hex(OK));
        let read = hex("0c 00 00 00 05 00 00 00 00 04 00 00");
        assert_eq!(sent(&a, CMD), std::slice::from_ref(&read));
        let mut full = hex("0c 04 00 00 06 00 00 00 00 04 00 00");
        full.extend(vec![7u8; CHUNK]);
        let a = data(&mut m, 0, CMD, &full);
        assert!(a.contains(&Action::FileWrite {
            file: FILE,
            data: vec![7u8; CHUNK]
        }));
        assert_eq!(sent(&a, CMD), [read]);
        let a = data(
            &mut m,
            0,
            CMD,
            &hex("0e 00 00 00 06 00 00 00 02 00 00 00 01 02"),
        );
        assert!(a.contains(&Action::FileWrite {
            file: FILE,
            data: vec![1, 2]
        }));
        assert_eq!(sent(&a, CMD), [hex("08 00 00 00 04 00 00 00")]);
        let a = data(&mut m, 0, CMD, &hex(OK));
        assert!(a.contains(&Action::FileClose { file: FILE }));
        let mut cx = Cx::new(0);
        m.file(&mut cx, FILE, FileInput::Closed { bytes: 1026 });
        assert_eq!(
            completed(&cx.take(), 3),
            Some(Ok(Outcome::Value {
                value: json!({"bytes": 1026})
            }))
        );

        // An import: the host file is read, then written in chunks.
        let a = run(
            &mut m,
            4,
            "import_file",
            json!({"type": "logo", "number": 1, "path": "/x/l.bin"}),
        );
        assert!(a.contains(&Action::FileRead {
            file: FILE,
            path: "/x/l.bin".into()
        }));
        let mut cx = Cx::new(0);
        m.file(
            &mut cx,
            FILE,
            FileInput::Read {
                data: vec![9, 8, 7],
            },
        );
        assert_eq!(
            sent(&cx.take(), CMD),
            [hex(
                "14 00 00 00 02 00 00 00 03 00 00 00 01 00 00 00 01 00 00 00"
            )]
        );
        let a = data(&mut m, 0, CMD, &hex(OK));
        assert_eq!(
            sent(&a, CMD),
            [hex(
                "13 00 00 00 06 00 00 00 03 00 00 00 00 00 00 00 09 08 07"
            )]
        );
        let a = data(&mut m, 0, CMD, &hex(OK));
        assert_eq!(sent(&a, CMD), [hex("08 00 00 00 04 00 00 00")]);
        let a = data(&mut m, 0, CMD, &hex(OK));
        assert_eq!(sent(&a, CMD), [hex("0c 00 00 00 07 00 00 00 03 00 00 00")]);
        let a = data(&mut m, 0, CMD, &hex(OK));
        assert_eq!(
            completed(&a, 4),
            Some(Ok(Outcome::Value {
                value: json!({"bytes": 3})
            }))
        );
    }

    #[test]
    fn an_export_whose_host_file_fails_fails() {
        let mut m = se3200(false);
        run(
            &mut m,
            1,
            "export_file",
            json!({"type": "memory", "number": 0, "path": "/no/such/dir/m.bin"}),
        );
        let mut cx = Cx::new(0);
        m.file(
            &mut cx,
            FILE,
            FileInput::Failed {
                message: "no such directory".into(),
            },
        );
        data(&mut m, 0, CMD, &hex(OK));
        data(&mut m, 0, CMD, &hex("0c 00 00 00 06 00 00 00 00 00 00 00"));
        let a = data(&mut m, 0, CMD, &hex(OK));
        assert!(matches!(
            completed(&a, 1),
            Some(Err(CommandError::Transport { .. }))
        ));
    }

    fn serial_settings() -> Value {
        json!({"link": "serial", "serial_port": "COM9"})
    }

    #[test]
    fn rs232_wraps_the_same_controls_and_polls() {
        let mut m = module("se-4000", true, serial_settings());
        assert!(start(&mut m)
            .iter()
            .any(|x| matches!(x, Action::SerialOpen { .. })));
        let a = tcp(&mut m, 0, CMD, TcpInput::Connected);
        // 3.4.16: get controls, a count of 02 + 4n, effects address 00, 70.
        assert_eq!(sent(&a, CMD), [hex("06 00 70 00 00 00 00")]);
        let a = data(&mut m, 2, CMD, &hex("06 00 f0 02 00 00 00"));
        assert_eq!(state(&a), json!({"status": {"connection": "connected"}}));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // Then the poll, at most 30 controls a message.
        let polls = sent(&a, CMD);
        assert_eq!(polls.len(), 1);
        assert_eq!(polls[0][2], 0x70);
        assert!(polls[0].len() <= 127);

        let mut m = module("se-1200mu", false, serial_settings());
        start(&mut m);
        tcp(&mut m, 0, CMD, TcpInput::Connected);
        data(&mut m, 0, CMD, &hex("06 00 f0 02 00 00 00"));
        // Program 2 (SE-1200 control 86): F0, then read back with 70.
        let a = run(&mut m, 1, "set_program", json!({"source": 2}));
        assert_eq!(sent(&a, CMD), [hex("0a 00 f0 56 00 02 00 02 00 00 00")]);
        let a = data(&mut m, 0, CMD, &hex("01 80"));
        assert_eq!(sent(&a, CMD), [hex("06 00 70 56 00 02 00")]);
        let a = data(&mut m, 0, CMD, &hex("06 00 f0 02 00 00 00"));
        assert_eq!(state(&a), json!({"program": 2}));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        // Panel buttons exist only here.
        let a = run(&mut m, 2, "press_button", json!({"button": 0x4a}));
        assert_eq!(sent(&a, CMD), [hex("03 00 fb 4a")]);
        let a = data(&mut m, 0, CMD, &hex("01 40"));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
        let a = run(&mut m, 3, "menu_select", json!({"menu": 1, "sub_menu": 2}));
        assert_eq!(sent(&a, CMD), [hex("03 00 de 21")]);
        let a = run(&mut m, 4, "get_input_name", json!({"input": 1}));
        assert!(matches!(
            completed(&a, 4),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
    }

    #[test]
    fn models_without_a_serial_port_refuse_it() {
        assert!(DatavideoSwitcher::new(context("se-3200", true, serial_settings())).is_err());
        assert!(DatavideoSwitcher::new(context("hs-4000", true, serial_settings())).is_err());
    }

    #[test]
    fn se2200_blocks_follow_its_examples() {
        // 1.4.2.1 and 1.4.3.1.
        assert_eq!(se2200_block(0x20, &[0x23]), hex("aa 04 ff 20 23 46"));
        assert_eq!(se2200_block(0x20, &[0x36]), hex("aa 04 ff 20 36 59"));
        assert_eq!(se2200_block(0x21, &[0, 0]), hex("aa 05 ff 21 00 00 25"));
        assert_eq!(se2200_block(0x22, &[]), hex("aa 03 ff 22 24"));
        let s = se2200_status(&hex("52 a3 05 09 1a 34 12"));
        assert_eq!(s["program"], json!(2));
        assert_eq!(s["preview"], json!(5));
        assert_eq!(s["wipe"]["pattern"], json!(3));
        assert_eq!(s["transition"]["speed"], json!(5));
        assert_eq!(s["features"]["logo_1"], json!(true));
        assert_eq!(s["features"]["border"], json!(true));
        assert_eq!(s["features"]["pip_1_program"], json!(true));
        assert_eq!(s["features"]["pip_1_preview"], json!(true));
        assert_eq!(s["features"]["audio_follow_video"], json!(true));
        assert_eq!(s["transition"]["type"], json!("wipe"));
        assert_eq!(s["multiscreen"], json!(true));
        assert_eq!(s["ftb"]["active"], json!(false));
        assert_eq!(s["t_bar"], json!(0x1234));
    }

    #[test]
    fn se2200_over_rs232_and_dvip_tcp() {
        let mut m = module(
            "se-2200",
            true,
            json!({"link": "serial", "serial_port": "COM3", "baud_rate": "115200", "parity": "none"}),
        );
        start(&mut m);
        let a = tcp(&mut m, 0, CMD, TcpInput::Connected);
        assert_eq!(sent(&a, CMD), [hex("aa 03 ff 22 24")]);
        let a = data(&mut m, 4, CMD, &hex("09 80 21 01 00 00 00 00 00"));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(4)));
        assert_eq!(state(&a)["program"], json!(1));
        // set_feature presses only when the status differs: logo 1 is off.
        let a = run(
            &mut m,
            1,
            "set_feature",
            json!({"feature": "logo_1", "on": false}),
        );
        assert_eq!(sent(&a, CMD), [hex("aa 03 ff 22 24")]);
        run(
            &mut m,
            2,
            "set_feature",
            json!({"feature": "logo_1", "on": true}),
        );
        let a = data(&mut m, 0, CMD, &hex("09 80 21 01 00 00 00 00 00"));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a, CMD), [hex("aa 04 ff 20 4c 6f")]);
        let a = data(&mut m, 0, CMD, &hex("02 80"));
        assert_eq!(sent(&a, CMD), [hex("aa 03 ff 22 24")]);
        let a = data(&mut m, 0, CMD, &hex("09 80 21 01 01 00 00 00 00"));
        assert_eq!(completed(&a, 2), Some(Ok(Outcome::Ack)));
        assert_eq!(state(&a)["features"]["logo_1"], json!(true));
        // Black on program is source 7; the switcher ignores it outside
        // remote mode.
        let a = run(&mut m, 3, "set_program", json!({"source": 7}));
        assert_eq!(sent(&a, CMD), [hex("aa 04 ff 20 21 44")]);
        let a = data(&mut m, 0, CMD, &hex("02 20"));
        assert!(matches!(
            completed(&a, 3),
            Some(Err(CommandError::DeviceError { .. }))
        ));

        // Over the DVIP board: TCP 5002 with DVIP's length header.
        let mut m = module("se-2200", false, json!({}));
        assert_eq!(opened(&start(&mut m)), [(CMD, DVIP_PORT)]);
        let a = tcp(&mut m, 0, CMD, TcpInput::Connected);
        assert_eq!(sent(&a, CMD), [hex("00 07 aa 03 ff 22 24")]);
        let a = data(&mut m, 0, CMD, &hex("00 0b 09 80 33 02 00 00 00 00 00"));
        assert_eq!(state(&a)["preview"], json!(3));
        let a = run(&mut m, 1, "set_t_bar", json!({"position": 256}));
        assert_eq!(sent(&a, CMD), [hex("00 09 aa 05 ff 21 00 01 26")]);
    }

    #[test]
    fn dvip_management_over_udp() {
        assert_eq!(
            dvip_packet(0x00, b"ETH_REQ"),
            hex("00 0b 81 00 45 54 48 5f 52 45 51")
        );
        let mut m = module("se-650", true, json!({}));
        let a = start(&mut m);
        assert!(a.contains(&Action::Connection(Connection::Connecting)));
        assert_eq!(udp_sent(&a), [dvip_packet(0x00, b"ETH_REQ")]);
        let mut reply = hex("00 30 81 00 01");
        let mut name = b"se650".to_vec();
        name.resize(16, 0);
        reply.extend(name);
        reply.extend(hex(
            "00 11 22 33 44 55 c0 a8 64 65 ff ff ff 00 c0 a8 64 01 08 08 08 08 00 00 00 00 02",
        ));
        let a = udp(&mut m, 5, &reply);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(5)));
        let s = state(&a);
        assert_eq!(s["dvip"]["network"]["ip_address"], json!("192.168.100.101"));
        assert_eq!(s["dvip"]["network"]["host_name"], json!("se650"));
        assert_eq!(
            s["dvip"]["network"]["mac_address"],
            json!("00:11:22:33:44:55")
        );
        assert_eq!(s["dvip"]["network"]["connections"], json!(2));
        assert_eq!(udp_sent(&a), [dvip_packet(0x01, b"FWVER_REQ")]);

        // Presets: the count, then each name; the command ends with the list.
        let mut m = module("se-650", false, json!({}));
        start(&mut m);
        let mut cx = Cx::new(0);
        m.timer(&mut cx, UDP_REPLY_TIMER);
        run(&mut m, 1, "get_presets", json!({}));
        let a = udp(&mut m, 0, &hex("00 06 81 43 06 01"));
        assert_eq!(udp_sent(&a), [dvip_packet(0x44, b"GET_PRESET_NAME\x01")]);
        let mut reply = hex("00 24 81 44");
        let mut name = b"Opening".to_vec();
        name.resize(32, 0);
        reply.extend(name);
        let a = udp(&mut m, 0, &reply);
        assert_eq!(state(&a)["dvip"]["presets"]["1"]["name"], json!("Opening"));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!([{"preset": 1, "name": "Opening"}])
            }))
        );
        let a = run(&mut m, 2, "recall_preset", json!({"preset": 1}));
        assert_eq!(udp_sent(&a), [dvip_packet(0x45, b"RECALL_PRESET\x01")]);
        let a = udp(&mut m, 0, &hex("00 05 81 45 15"));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
        let a = run(
            &mut m,
            3,
            "set_ip_address",
            json!({"address": "10.0.0.5", "gateway": "10.0.0.1"}),
        );
        let mut want = b"SET_IPADR".to_vec();
        want.extend([10, 0, 0, 5, 10, 0, 0, 1]);
        assert_eq!(udp_sent(&a), [dvip_packet(0x03, &want)]);
    }

    /// Every path the module writes is declared in the spec, with its type.
    #[test]
    fn every_control_is_declared_in_the_spec() {
        let spec = Catalog::source_tree().devices["datavideo-switcher"].clone();
        let declared = |path: &str| {
            let parts: Vec<&str> = path.split('.').collect();
            spec.state.iter().find_map(|(pattern, field)| {
                let p: Vec<&str> = pattern.split('.').collect();
                (p.len() == parts.len() && p.iter().zip(&parts).all(|(a, b)| *a == "*" || a == b))
                    .then(|| field.kind.clone())
            })
        };
        for family in [Family::Se1200, Family::Se3200, Family::Se4000] {
            for c in controls(family) {
                let want = match c.kind {
                    Kind::Int { .. } => "int",
                    Kind::Float { .. } => "float",
                    Kind::Flag | Kind::FloatFlag => "bool",
                    Kind::Enum(_) => "string",
                };
                assert_eq!(
                    declared(&c.path).as_deref(),
                    Some(want),
                    "{family:?} {}",
                    c.path
                );
            }
            for p in present(family) {
                assert_eq!(declared(p.path).as_deref(), Some("array"));
            }
        }
        for path in [
            "inputs.3.name",
            "ftb.active",
            "multiscreen",
            "t_bar",
            "transition.speed",
            "features.clock",
            "dvip.network.dns_secondary",
            "dvip.presets.4.name",
            "dvip.versions.firmware_d",
            "dvip.model_name",
        ] {
            assert!(declared(path).is_some(), "{path}");
        }
    }
}
