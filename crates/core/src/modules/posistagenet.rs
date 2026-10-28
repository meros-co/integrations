//! PosiStageNet (VYV and MA Lighting, protocol description v2.03, 2019): 3D
//! tracker positions on UDP multicast 236.10.10.10:56565.
//!
//! - One socket on the PSN port (`Bind::Network`, shared with other programs
//!   on the host) joined to the PSN group. Every PSN_INFO packet (system
//!   name, tracker names) and PSN_DATA packet (position, speed, orientation,
//!   validity, acceleration, target position, timestamp per tracker) is
//!   read into state under `servers.<ip>`, at most every
//!   `receive_interval_ms` per server. A tracker the server's PSN_INFO no
//!   longer lists is removed; a server silent 5 s is marked offline.
//! - The core can be a PSN server too: trackers set with `set_tracker` are
//!   sent as PSN_DATA at `send_rate_hz` (60 by default) and PSN_INFO once a
//!   second, to the group and to the host when one is given, split into
//!   packets of at most 1500 bytes as the protocol asks.
//! - Opening for commands only (monitor false) joins no group and keeps
//!   nothing received; sending works as usual.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};

pub(crate) const PORT: u16 = 56565;
pub(crate) const GROUP: Ipv4Addr = Ipv4Addr::new(236, 10, 10, 10);
const SOCKET: Key = "psn";
const RETRY: Key = "retry";
const DATA: Key = "data";
const INFO: Key = "info";
const REPORT: Key = "report";
const SILENCE: Key = "silence";

const INFO_PACKET: u16 = 0x6756;
const DATA_PACKET: u16 = 0x6755;
const PACKET_HEADER: u16 = 0x0000;
const INFO_SYSTEM_NAME: u16 = 0x0001;
const INFO_TRACKER_LIST: u16 = 0x0002;
const INFO_TRACKER_NAME: u16 = 0x0000;
const DATA_TRACKER_LIST: u16 = 0x0001;
const FIELD_POS: u16 = 0;
const FIELD_SPEED: u16 = 1;
const FIELD_ORI: u16 = 2;
const FIELD_STATUS: u16 = 3;
const FIELD_ACCEL: u16 = 4;
const FIELD_TARGET: u16 = 5;
const FIELD_TIMESTAMP: u16 = 6;
/// The largest UDP packet the protocol allows.
const MAX_PACKET: usize = 1500;
const VERSION: (u8, u8) = (2, 3);
const OFFLINE_AFTER: Millis = 5000;
const RETRY_MIN: Millis = 1000;
const RETRY_MAX: Millis = 30_000;

/// PSN_CHUNK_HEADER: id in bits 0-15, data length in bits 16-30,
/// has_subchunks in bit 31, little endian.
fn chunk(id: u16, data: &[u8], has_subchunks: bool) -> Vec<u8> {
    let header = id as u32 | ((data.len() as u32 & 0x7fff) << 16) | ((has_subchunks as u32) << 31);
    let mut out = header.to_le_bytes().to_vec();
    out.extend_from_slice(data);
    out
}

/// The chunks laid end to end in `data`: (id, has_subchunks, data).
fn chunks(mut data: &[u8]) -> Vec<(u16, bool, &[u8])> {
    let mut out = Vec::new();
    while data.len() >= 4 {
        let header = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let id = (header & 0xffff) as u16;
        let len = ((header >> 16) & 0x7fff) as usize;
        let sub = header >> 31 == 1;
        if data.len() < 4 + len {
            break;
        }
        out.push((id, sub, &data[4..4 + len]));
        data = &data[4 + len..];
    }
    out
}

fn packet_header(timestamp: u64, frame: u8, count: u8) -> Vec<u8> {
    let mut d = timestamp.to_le_bytes().to_vec();
    d.extend_from_slice(&[VERSION.0, VERSION.1, frame, count]);
    chunk(PACKET_HEADER, &d, false)
}

fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn read_f32(d: &[u8], i: usize) -> Option<f32> {
    d.get(4 * i..4 * i + 4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// A tracker the core sends.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Tracker {
    pub name: String,
    pub pos: Option<[f32; 3]>,
    pub speed: Option<[f32; 3]>,
    pub ori: Option<[f32; 3]>,
    pub status: Option<f32>,
    pub accel: Option<[f32; 3]>,
    pub target: Option<[f32; 3]>,
}

impl Tracker {
    fn data_chunk(&self, id: u16, timestamp: u64) -> Vec<u8> {
        let mut fields = Vec::new();
        for (field, v) in [
            (FIELD_POS, self.pos),
            (FIELD_SPEED, self.speed),
            (FIELD_ORI, self.ori),
            (FIELD_ACCEL, self.accel),
            (FIELD_TARGET, self.target),
        ] {
            if let Some(v) = v {
                fields.push((field, floats(&v)));
            }
        }
        if let Some(s) = self.status {
            fields.push((FIELD_STATUS, floats(&[s])));
        }
        fields.sort_by_key(|(f, _)| *f);
        let mut body = Vec::new();
        for (f, d) in fields {
            body.extend(chunk(f, &d, false));
        }
        body.extend(chunk(FIELD_TIMESTAMP, &timestamp.to_le_bytes(), false));
        chunk(id, &body, true)
    }

    fn info_chunk(&self, id: u16) -> Vec<u8> {
        chunk(
            id,
            &chunk(INFO_TRACKER_NAME, self.name.as_bytes(), false),
            true,
        )
    }
}

/// Packets of one frame: each holds the header and as many of `items`
/// (tracker chunks) as fit in 1500 bytes, under a list chunk; `fixed` (the
/// system name) goes in each packet before the list.
fn frame(
    root: u16,
    list: u16,
    fixed: &[u8],
    items: &[Vec<u8>],
    timestamp: u64,
    frame_id: u8,
) -> Vec<Vec<u8>> {
    // Root header 4, packet header 16, list header 4.
    let room = MAX_PACKET - 4 - 16 - fixed.len() - 4;
    let mut groups: Vec<Vec<u8>> = vec![Vec::new()];
    for item in items {
        let last = groups.last_mut().unwrap();
        if !last.is_empty() && last.len() + item.len() > room {
            groups.push(Vec::new());
        }
        groups.last_mut().unwrap().extend_from_slice(item);
    }
    let count = groups.len() as u8;
    groups
        .iter()
        .map(|g| {
            let mut body = packet_header(timestamp, frame_id, count);
            body.extend_from_slice(fixed);
            body.extend(chunk(list, g, true));
            chunk(root, &body, true)
        })
        .collect()
}

pub(crate) fn data_packets(
    trackers: &BTreeMap<u16, Tracker>,
    timestamp: u64,
    frame_id: u8,
) -> Vec<Vec<u8>> {
    let items: Vec<Vec<u8>> = trackers
        .iter()
        .map(|(id, t)| t.data_chunk(*id, timestamp))
        .collect();
    frame(
        DATA_PACKET,
        DATA_TRACKER_LIST,
        &[],
        &items,
        timestamp,
        frame_id,
    )
}

pub(crate) fn info_packets(
    system: &str,
    trackers: &BTreeMap<u16, Tracker>,
    timestamp: u64,
    frame_id: u8,
) -> Vec<Vec<u8>> {
    let items: Vec<Vec<u8>> = trackers.iter().map(|(id, t)| t.info_chunk(*id)).collect();
    let name = chunk(INFO_SYSTEM_NAME, system.as_bytes(), false);
    frame(
        INFO_PACKET,
        INFO_TRACKER_LIST,
        &name,
        &items,
        timestamp,
        frame_id,
    )
}

struct Server {
    online: bool,
    seen: Millis,
    /// Trackers listed by the last PSN_INFO frame, and those its packets
    /// so far have listed.
    listed: Option<Vec<u16>>,
    frame: Option<(u8, Vec<u16>, u8)>,
    pending: Map<String, Value>,
    reported_at: Option<Millis>,
    due: bool,
}

pub(crate) struct Psn {
    port: u16,
    address: Option<IpAddr>,
    group: Ipv4Addr,
    multicast: bool,
    unicast: Vec<SocketAddr>,
    system_name: String,
    send_every: Millis,
    receive_interval: Millis,
    monitor: bool,
    listening: bool,
    retry_after: Millis,
    trackers: BTreeMap<u16, Tracker>,
    data_frame: u8,
    info_frame: u8,
    servers: BTreeMap<IpAddr, Server>,
}

impl Psn {
    pub(crate) fn new(ctx: OpenContext) -> Result<Psn, String> {
        let s = &ctx.settings;
        let text = |k: &str| {
            s.get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let int = |k: &str, d: i64| s.get(k).and_then(Value::as_i64).unwrap_or(d);
        let address = match text("bind_address").as_str() {
            "" => None,
            a => Some(
                a.parse::<IpAddr>()
                    .map_err(|_| format!("bind_address '{a}' is not an IP address"))?,
            ),
        };
        let group = match text("multicast_group").as_str() {
            "" => GROUP,
            g => {
                let g: Ipv4Addr = g
                    .parse()
                    .map_err(|_| format!("multicast_group '{g}' is not an IPv4 address"))?;
                if !g.is_multicast() {
                    return Err(format!("multicast_group {g} is not a multicast address"));
                }
                g
            }
        };
        let port = ctx.port.unwrap_or(PORT);
        let mut unicast = Vec::new();
        for t in text("unicast_addresses")
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            let to = t
                .parse::<SocketAddr>()
                .or_else(|_| t.parse::<IpAddr>().map(|ip| SocketAddr::new(ip, port)))
                .map_err(|_| format!("unicast address '{t}' is not an IP address or ip:port"))?;
            unicast.push(to);
        }
        if !ctx.host.is_unspecified() {
            let host = SocketAddr::new(ctx.host, port);
            if !unicast.contains(&host) {
                unicast.push(host);
            }
        }
        let rate = int("send_rate_hz", 60).clamp(1, 1000) as Millis;
        let system_name = match text("system_name") {
            n if n.is_empty() => "Meros Integrations".to_string(),
            n => n,
        };
        Ok(Psn {
            port,
            address,
            group,
            multicast: s.get("multicast").and_then(Value::as_bool).unwrap_or(true),
            unicast,
            system_name,
            send_every: (1000 / rate).max(1),
            receive_interval: int("receive_interval_ms", 50).max(0) as Millis,
            monitor: ctx.monitor,
            listening: false,
            retry_after: RETRY_MIN,
            trackers: BTreeMap::new(),
            data_frame: 0,
            info_frame: 0,
            servers: BTreeMap::new(),
        })
    }

    fn listen(&mut self, cx: &mut Cx) {
        self.listening = true;
        cx.connection(Connection::Connected);
        cx.udp_open(
            SOCKET,
            Bind::Network {
                address: self.address,
                port: self.port,
            },
        );
        if self.monitor {
            cx.udp_join(SOCKET, self.group);
            cx.set_timer(SILENCE, 1000);
        }
        if !self.trackers.is_empty() {
            cx.set_timer(DATA, self.send_every);
            cx.set_timer(INFO, 1000);
        }
    }

    fn destinations(&self) -> Vec<SocketAddr> {
        let mut out = Vec::new();
        if self.multicast {
            out.push(SocketAddr::new(IpAddr::V4(self.group), self.port));
        }
        out.extend(self.unicast.iter().copied());
        out
    }

    fn send(&self, cx: &mut Cx, packets: Vec<Vec<u8>>) {
        for to in self.destinations() {
            for p in &packets {
                cx.udp_send(SOCKET, to, p.clone());
            }
        }
    }

    fn timestamp(cx: &Cx) -> u64 {
        cx.now().saturating_mul(1000)
    }

    fn send_data(&mut self, cx: &mut Cx) {
        self.data_frame = self.data_frame.wrapping_add(1);
        let packets = data_packets(&self.trackers, Self::timestamp(cx), self.data_frame);
        self.send(cx, packets);
    }

    fn send_info(&mut self, cx: &mut Cx) {
        self.info_frame = self.info_frame.wrapping_add(1);
        let packets = info_packets(
            &self.system_name,
            &self.trackers,
            Self::timestamp(cx),
            self.info_frame,
        );
        self.send(cx, packets);
    }

    fn tracker_state(&self, cx: &mut Cx, id: u16) {
        let t = &self.trackers[&id];
        let v3 = |v: Option<[f32; 3]>| v.map(|v| json!({"x": v[0], "y": v[1], "z": v[2]}));
        let mut m = Map::new();
        m.insert("name".into(), json!(t.name));
        for (k, v) in [
            ("position", t.pos),
            ("speed", t.speed),
            ("orientation", t.ori),
            ("acceleration", t.accel),
            ("target", t.target),
        ] {
            if let Some(v) = v3(v) {
                m.insert(k.into(), v);
            }
        }
        if let Some(s) = t.status {
            m.insert("validity".into(), json!(s));
        }
        cx.state(json!({"sending": {id.to_string(): m}}));
    }

    fn packet(&mut self, cx: &mut Cx, from: SocketAddr, data: &[u8]) {
        let Some(&(root, true, body)) = chunks(data).first() else {
            return;
        };
        if root != DATA_PACKET && root != INFO_PACKET {
            return;
        }
        cx.alive();
        if !self.monitor {
            return;
        }
        let now = cx.now();
        let ip = from.ip();
        let server = self.servers.entry(ip).or_insert(Server {
            online: false,
            seen: now,
            listed: None,
            frame: None,
            pending: Map::new(),
            reported_at: None,
            due: false,
        });
        server.seen = now;
        let mut top = Map::new();
        if !server.online {
            server.online = true;
            top.insert("online".into(), json!(true));
        }
        let mut header = None;
        for (id, sub, d) in chunks(body) {
            match (root, id, sub) {
                (_, PACKET_HEADER, false) if d.len() >= 12 => {
                    let ts = u64::from_le_bytes(d[..8].try_into().unwrap());
                    header = Some((ts, d[8], d[9], d[10], d[11]));
                }
                (INFO_PACKET, INFO_SYSTEM_NAME, _) => {
                    top.insert("system_name".into(), json!(String::from_utf8_lossy(d)));
                }
                (INFO_PACKET, INFO_TRACKER_LIST, true) => {
                    let mut ids = Vec::new();
                    for (tid, _, td) in chunks(d) {
                        ids.push(tid);
                        for (f, _, fd) in chunks(td) {
                            if f == INFO_TRACKER_NAME {
                                merge(
                                    &mut server.pending,
                                    tid,
                                    "name",
                                    json!(String::from_utf8_lossy(fd)),
                                );
                            }
                        }
                    }
                    // The trackers a whole PSN_INFO frame lists are the
                    // server's trackers: the rest are gone.
                    if let Some((_, _, _, frame_id, count)) = header {
                        let f = match server.frame.take() {
                            Some((fid, mut seen, n)) if fid == frame_id => {
                                seen.extend(&ids);
                                (fid, seen, n + 1)
                            }
                            _ => (frame_id, ids, 1),
                        };
                        if f.2 >= count.max(1) {
                            let listed = f.1;
                            if let Some(before) = &server.listed {
                                for gone in before.iter().filter(|t| !listed.contains(t)) {
                                    server.pending.insert(gone.to_string(), Value::Null);
                                }
                            }
                            server.listed = Some(listed);
                        } else {
                            server.frame = Some(f);
                        }
                    }
                }
                (DATA_PACKET, DATA_TRACKER_LIST, true) => {
                    for (tid, _, td) in chunks(d) {
                        for (f, _, fd) in chunks(td) {
                            let v3 = || {
                                Some(
                                    json!({"x": read_f32(fd, 0)?, "y": read_f32(fd, 1)?, "z": read_f32(fd, 2)?}),
                                )
                            };
                            let (key, value) = match f {
                                FIELD_POS => ("position", v3()),
                                FIELD_SPEED => ("speed", v3()),
                                FIELD_ORI => ("orientation", v3()),
                                FIELD_ACCEL => ("acceleration", v3()),
                                FIELD_TARGET => ("target", v3()),
                                FIELD_STATUS => ("validity", read_f32(fd, 0).map(|v| json!(v))),
                                FIELD_TIMESTAMP if fd.len() >= 8 => (
                                    "timestamp_us",
                                    Some(json!(u64::from_le_bytes(fd[..8].try_into().unwrap()))),
                                ),
                                _ => continue,
                            };
                            if let Some(value) = value {
                                merge(&mut server.pending, tid, key, value);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some((ts, vh, vl, _, _)) = header {
            top.insert("version".into(), json!(format!("{vh}.{vl:02}")));
            if root == DATA_PACKET {
                top.insert("timestamp_us".into(), json!(ts));
            }
        }
        let due = server
            .reported_at
            .is_none_or(|at| now.saturating_sub(at) >= self.receive_interval);
        let key = ip.to_string();
        if !top.is_empty() {
            cx.state(json!({"servers": {key.clone(): top}}));
        }
        if due {
            Self::report(cx, &key, server, now);
        } else if !server.due && !server.pending.is_empty() {
            server.due = true;
            let wait =
                self.receive_interval - now.saturating_sub(server.reported_at.unwrap_or(now));
            cx.set_timer(REPORT, wait.max(1));
        }
    }

    fn report(cx: &mut Cx, key: &str, server: &mut Server, now: Millis) {
        server.reported_at = Some(now);
        server.due = false;
        if server.pending.is_empty() {
            return;
        }
        let trackers = std::mem::take(&mut server.pending);
        cx.state(json!({"servers": {key: {"trackers": trackers}}}));
    }

    fn fail(cx: &mut Cx, id: CommandId, message: impl Into<String>) {
        cx.complete(
            id,
            Err(CommandError::InvalidParams {
                message: message.into(),
            }),
        );
    }
}

/// Add one tracker field to a pending patch, unless the tracker is being
/// removed in it.
fn merge(pending: &mut Map<String, Value>, tracker: u16, key: &str, value: Value) {
    let entry = pending
        .entry(tracker.to_string())
        .or_insert_with(|| json!({}));
    if entry.is_null() {
        *entry = json!({});
    }
    entry[key] = value;
}

impl Module for Psn {
    fn start(&mut self, cx: &mut Cx) {
        cx.state(json!({"port": self.port, "group": self.group.to_string(), "system_name": self.system_name}));
        self.listen(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.listening {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let tracker = params.get("tracker").and_then(Value::as_u64).unwrap_or(0) as u16;
        match name {
            "set_tracker" => {
                let f = |k: &str| params.get(k).and_then(Value::as_f64).map(|v| v as f32);
                let v3 = |p: &str| -> Result<Option<[f32; 3]>, String> {
                    match (
                        f(&format!("{p}_x")),
                        f(&format!("{p}_y")),
                        f(&format!("{p}_z")),
                    ) {
                        (Some(x), Some(y), Some(z)) => Ok(Some([x, y, z])),
                        (None, None, None) => Ok(None),
                        _ => Err(format!("give all of {p}_x, {p}_y and {p}_z, or none")),
                    }
                };
                let fields = (
                    v3("position"),
                    v3("speed"),
                    v3("orientation"),
                    v3("acceleration"),
                    v3("target"),
                );
                let (pos, speed, ori, accel, target) = match fields {
                    (Ok(a), Ok(b), Ok(c), Ok(d), Ok(e)) => (a, b, c, d, e),
                    (Err(e), ..)
                    | (_, Err(e), ..)
                    | (_, _, Err(e), ..)
                    | (_, _, _, Err(e), _)
                    | (.., Err(e)) => return Self::fail(cx, id, e),
                };
                let first = self.trackers.is_empty();
                let t = self.trackers.entry(tracker).or_insert_with(|| Tracker {
                    name: format!("Tracker {tracker}"),
                    pos: Some([0.0; 3]),
                    ..Default::default()
                });
                if let Some(n) = params
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|n| !n.is_empty())
                {
                    t.name = n.to_string();
                }
                t.pos = pos.or(t.pos);
                t.speed = speed.or(t.speed);
                t.ori = ori.or(t.ori);
                t.accel = accel.or(t.accel);
                t.target = target.or(t.target);
                t.status = f("validity").or(t.status);
                self.tracker_state(cx, tracker);
                self.send_data(cx);
                if first {
                    self.send_info(cx);
                    cx.set_timer(DATA, self.send_every);
                    cx.set_timer(INFO, 1000);
                }
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "remove_tracker" => {
                if self.trackers.remove(&tracker).is_some() {
                    cx.state(json!({"sending": {tracker.to_string(): Value::Null}}));
                    self.send_info(cx);
                }
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "clear_trackers" => {
                if !self.trackers.is_empty() {
                    let gone: Map<String, Value> = self
                        .trackers
                        .keys()
                        .map(|t| (t.to_string(), Value::Null))
                        .collect();
                    cx.state(json!({"sending": gone}));
                    self.trackers.clear();
                    self.send_info(cx);
                }
                cx.complete(id, Ok(Outcome::Unverified));
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        self.packet(cx, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        self.listening = false;
        cx.log(Level::Warning, format!("PSN port: {message}"));
        cx.connection(Connection::Disconnected {
            reason: message.to_string(),
        });
        for t in [DATA, INFO, SILENCE] {
            cx.cancel_timer(t);
        }
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.listen(cx),
            DATA if self.listening && !self.trackers.is_empty() => {
                self.send_data(cx);
                cx.set_timer(DATA, self.send_every);
            }
            INFO if self.listening && !self.trackers.is_empty() => {
                self.send_info(cx);
                cx.set_timer(INFO, 1000);
            }
            REPORT => {
                let now = cx.now();
                for (ip, server) in self.servers.iter_mut() {
                    if server.due {
                        Self::report(cx, &ip.to_string(), server, now);
                    }
                }
            }
            SILENCE if self.listening => {
                let now = cx.now();
                let mut patch = Map::new();
                for (ip, server) in self.servers.iter_mut() {
                    if server.online && now.saturating_sub(server.seen) >= OFFLINE_AFTER {
                        server.online = false;
                        patch.insert(ip.to_string(), json!({"online": false}));
                    }
                }
                if !patch.is_empty() {
                    cx.state(json!({"servers": patch}));
                }
                cx.set_timer(SILENCE, 1000);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.udp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;

    fn module(settings: Value, monitor: bool) -> Psn {
        Psn::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            host_name: None,
            port: None,
            model: "posistagenet".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        })
        .unwrap()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn sends(actions: &[Action]) -> Vec<(SocketAddr, Vec<u8>)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { to, data, .. } => Some((*to, data.clone())),
                _ => None,
            })
            .collect()
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn chunk_headers_pack_id_length_and_the_subchunk_bit() {
        assert_eq!(hex(&chunk(0x6755, &[], true)), "55670080");
        assert_eq!(hex(&chunk(0x0001, &[1, 2, 3], false)), "01000300010203");
        let packed = chunk(7, &[9; 5], true);
        let parsed = chunks(&packed);
        assert_eq!(parsed, vec![(7, true, &[9u8; 5][..])]);
    }

    #[test]
    fn a_tracker_is_sent_as_data_and_info() {
        let mut m = module(json!({"system_name": "Stage"}), true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpJoin {
            socket: SOCKET,
            group: GROUP
        }));
        let mut cx = Cx::new(2);
        m.command(&mut cx, 1, "set_tracker", &params(json!({"tracker": 3, "name": "Singer", "position_x": 1.0, "position_y": 2.0, "position_z": 3.0})));
        let a = cx.take();
        let s = sends(&a);
        assert_eq!(s.len(), 2, "data then info, to the group");
        assert_eq!(s[0].0, "236.10.10.10:56565".parse::<SocketAddr>().unwrap());
        let data = &s[0].1;
        // DATA root, header (timestamp 2000 us, version 2.03, frame 1 of 1).
        assert_eq!(
            hex(&data[..4]),
            format!(
                "5567{:02x}{:02x}",
                (data.len() - 4) & 0xff,
                0x80 | ((data.len() - 4) >> 8)
            )
        );
        assert_eq!(hex(&data[4..20]), "00000c00d00700000000000002030101");
        // Tracker list, tracker 3, its position then timestamp.
        assert_eq!(hex(&data[20..24]), "01002080");
        assert_eq!(hex(&data[24..28]), "03001c80");
        assert_eq!(hex(&data[28..32]), "00000c00");
        assert_eq!(&data[32..44], &floats(&[1.0, 2.0, 3.0])[..]);
        let info = &s[1].1;
        let root = chunks(info);
        assert_eq!(root[0].0, 0x6756);
        assert!(info.windows(5).any(|w| w == b"Stage"));
        assert!(info.windows(6).any(|w| w == b"Singer"));
        assert!(a.contains(&Action::SetTimer {
            key: DATA,
            after: 16
        }));
        assert!(a.contains(&Action::State(json!({"sending": {"3": {"name": "Singer", "position": {"x": 1.0, "y": 2.0, "z": 3.0}}}}))));
    }

    #[test]
    fn many_trackers_are_split_into_packets_of_at_most_1500_bytes() {
        let mut trackers = BTreeMap::new();
        for id in 0..100u16 {
            trackers.insert(
                id,
                Tracker {
                    name: "x".into(),
                    pos: Some([0.0; 3]),
                    speed: Some([0.0; 3]),
                    ..Default::default()
                },
            );
        }
        let packets = data_packets(&trackers, 0, 9);
        assert!(packets.len() > 1);
        for p in &packets {
            assert!(p.len() <= 1500);
            assert_eq!(p[18], 9, "frame id");
            assert_eq!(p[19] as usize, packets.len(), "frame packet count");
        }
    }

    #[test]
    fn received_frames_go_into_state_and_listed_trackers_are_kept() {
        let mut m = module(json!({"receive_interval_ms": 0}), true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let mut trackers = BTreeMap::new();
        trackers.insert(
            1u16,
            Tracker {
                name: "A".into(),
                pos: Some([1.5, 0.0, -2.0]),
                status: Some(1.0),
                ..Default::default()
            },
        );
        trackers.insert(
            2u16,
            Tracker {
                name: "B".into(),
                pos: Some([0.0; 3]),
                ..Default::default()
            },
        );
        let from: SocketAddr = "10.0.0.20:56565".parse().unwrap();
        let mut cx = Cx::new(10);
        m.datagram(
            &mut cx,
            SOCKET,
            from,
            &info_packets("Tracker server", &trackers, 5, 1)[0],
        );
        m.datagram(&mut cx, SOCKET, from, &data_packets(&trackers, 6, 1)[0]);
        let a = cx.take();
        assert!(a.contains(&Action::State(json!({"servers": {"10.0.0.20": {"online": true, "system_name": "Tracker server", "version": "2.03"}}}))));
        assert!(a.iter().any(|x| matches!(x, Action::State(p) if p["servers"]["10.0.0.20"]["trackers"]["1"]["position"] == json!({"x": 1.5, "y": 0.0, "z": -2.0})
            && p["servers"]["10.0.0.20"]["trackers"]["1"]["validity"] == json!(1.0))));
        // Tracker 2 is no longer listed: removed.
        trackers.remove(&2);
        let mut cx = Cx::new(1010);
        m.datagram(
            &mut cx,
            SOCKET,
            from,
            &info_packets("Tracker server", &trackers, 7, 2)[0],
        );
        assert!(cx.take().iter().any(|x| matches!(x, Action::State(p) if p["servers"]["10.0.0.20"]["trackers"].get("2") == Some(&Value::Null))));
        // Silent for 5 s: offline.
        let mut cx = Cx::new(6100);
        m.timer(&mut cx, SILENCE);
        assert!(cx.take().contains(&Action::State(
            json!({"servers": {"10.0.0.20": {"online": false}}})
        )));
    }

    #[test]
    fn commands_only_receives_nothing() {
        let mut m = module(json!({}), false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(!cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::UdpJoin { .. })));
        let mut trackers = BTreeMap::new();
        trackers.insert(1u16, Tracker::default());
        let mut cx = Cx::new(1);
        m.datagram(
            &mut cx,
            SOCKET,
            "10.0.0.1:56565".parse().unwrap(),
            &data_packets(&trackers, 0, 1)[0],
        );
        assert!(!cx.take().iter().any(|a| matches!(a, Action::State(_))));
    }
}
