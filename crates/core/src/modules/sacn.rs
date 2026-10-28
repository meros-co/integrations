//! Streaming ACN (ANSI E1.31-2016, "Lightweight streaming protocol for
//! transport of DMX512 using ACN"): the core as an sACN source and receiver
//! on UDP 5568.
//!
//! - One socket on port 5568 (`Bind::Network`, so receivers and sources on
//!   this host can share it). Universe n is multicast to 239.255.n/256.n%256
//!   (§9), and unicast to the host and the unicast addresses as well.
//! - Output is held per universe with its priority and sent as E1.31 Data
//!   Packets: at once on a change (at most one frame per 23 ms, DMX512's
//!   44 Hz), then the same data twice more, then a keep-alive every second
//!   (§6.6.2). Releasing a universe sends three packets with
//!   Stream_Terminated set (§6.2.6). With a synchronization universe, data
//!   packets carry it and every burst is followed by an E1.31
//!   Synchronization Packet on it (§11). A Universe Discovery Packet listing
//!   the universes sent goes to 239.255.250.214 every 10 s (§4.3, §12).
//! - Receiving joins the multicast group of each universe `receive` names
//!   (and `receive_universe` adds) and keeps, per universe, every source's
//!   name, priority and address, and the levels of the highest priority,
//!   merged highest-takes-precedence among sources at that priority. A
//!   source silent 2.5 s, or terminating its stream, is removed (§6.7.1);
//!   out-of-order packets are discarded (§6.7.2). Sources' Universe Discovery
//!   is kept under `sources.<cid>`.
//! - Opening for commands only (monitor false) sends as usual but joins no
//!   group and keeps nothing received: receiving is monitoring.

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};

pub(crate) const PORT: u16 = 5568;
const SOCKET: Key = "sacn";
const RETRY: Key = "retry";
const FLUSH: Key = "flush";
const KEEPALIVE: Key = "keepalive";
const DISCOVERY: Key = "discovery";
const LOSS: Key = "loss";
const INPUTS: Key = "inputs";

const ACN_ID: [u8; 12] = *b"ASC-E1.17\0\0\0";
const VECTOR_ROOT_DATA: u32 = 0x0000_0004;
const VECTOR_ROOT_EXTENDED: u32 = 0x0000_0008;
const VECTOR_DATA_PACKET: u32 = 0x0000_0002;
const VECTOR_EXTENDED_SYNCHRONIZATION: u32 = 0x0000_0001;
const VECTOR_EXTENDED_DISCOVERY: u32 = 0x0000_0002;
const VECTOR_DISCOVERY_UNIVERSE_LIST: u32 = 0x0000_0001;
const VECTOR_DMP_SET_PROPERTY: u8 = 0x02;
pub(crate) const DISCOVERY_UNIVERSE: u16 = 64214;
const DISCOVERY_INTERVAL: Millis = 10_000;
const DATA_LOSS: Millis = 2500;
/// DMX512's fastest refresh, 44 Hz (§6.6.1).
const FRAME_GAP: Millis = 23;
/// Keep-alives go every 800 to 1000 ms (§6.6.2).
const KEEPALIVE_EVERY: Millis = 1000;
/// A changed universe is sent this many times before suppression (§6.6.2).
const REPEATS: u8 = 3;
/// Universes per discovery page (§8.5).
const PAGE: usize = 512;
/// A source whose Universe Discovery stops is dropped after two intervals
/// and a margin.
const SOURCE_GONE: Millis = 2 * DISCOVERY_INTERVAL + 5000;
const RETRY_MIN: Millis = 1000;
const RETRY_MAX: Millis = 30_000;

pub(crate) mod options {
    pub const PREVIEW: u8 = 0x80;
    pub const TERMINATED: u8 = 0x40;
    pub const FORCE_SYNC: u8 = 0x20;
}

/// The multicast group of a universe or synchronization address (§9.3).
pub(crate) fn group(universe: u16) -> Ipv4Addr {
    Ipv4Addr::new(239, 255, (universe >> 8) as u8, (universe & 0xff) as u8)
}

fn flags_length(len: usize) -> [u8; 2] {
    (0x7000 | (len as u16 & 0x0fff)).to_be_bytes()
}

/// The root layer (§5), its length filled in by `finish`.
fn root(vector: u32, cid: &[u8; 16]) -> Vec<u8> {
    let mut p = Vec::with_capacity(638);
    p.extend_from_slice(&0x0010u16.to_be_bytes());
    p.extend_from_slice(&0u16.to_be_bytes());
    p.extend_from_slice(&ACN_ID);
    p.extend_from_slice(&[0, 0]);
    p.extend_from_slice(&vector.to_be_bytes());
    p.extend_from_slice(cid);
    p
}

/// Fill in the root and framing layers' lengths: each counts from its
/// Flags and Length octets to the end of the packet (§5.4, §6.1).
fn finish(mut p: Vec<u8>) -> Vec<u8> {
    let total = p.len();
    p[16..18].copy_from_slice(&flags_length(total - 16));
    p[38..40].copy_from_slice(&flags_length(total - 38));
    p
}

fn name_field(p: &mut Vec<u8>, name: &str) {
    let mut bytes: Vec<u8> = name.as_bytes().iter().copied().take(63).collect();
    // Never cut a UTF-8 character in two.
    while std::str::from_utf8(&bytes).is_err() {
        bytes.pop();
    }
    bytes.resize(64, 0);
    p.extend_from_slice(&bytes);
}

pub(crate) struct DataHeader<'a> {
    pub cid: &'a [u8; 16],
    pub name: &'a str,
    pub priority: u8,
    pub sync: u16,
    pub sequence: u8,
    pub options: u8,
    pub universe: u16,
}

/// An E1.31 Data Packet (Table 1): the start code, then up to 512 slots.
pub(crate) fn data_packet(h: &DataHeader, start_code: u8, slots: &[u8]) -> Vec<u8> {
    let mut p = root(VECTOR_ROOT_DATA, h.cid);
    p.extend_from_slice(&[0, 0]);
    p.extend_from_slice(&VECTOR_DATA_PACKET.to_be_bytes());
    name_field(&mut p, h.name);
    p.push(h.priority);
    p.extend_from_slice(&h.sync.to_be_bytes());
    p.push(h.sequence);
    p.push(h.options);
    p.extend_from_slice(&h.universe.to_be_bytes());
    // DMP layer (§7)
    let dmp_len = 10 + 1 + slots.len();
    p.extend_from_slice(&flags_length(dmp_len));
    p.push(VECTOR_DMP_SET_PROPERTY);
    p.push(0xa1);
    p.extend_from_slice(&0u16.to_be_bytes());
    p.extend_from_slice(&1u16.to_be_bytes());
    p.extend_from_slice(&((slots.len() + 1) as u16).to_be_bytes());
    p.push(start_code);
    p.extend_from_slice(slots);
    finish(p)
}

/// An E1.31 Synchronization Packet (Table 2).
pub(crate) fn sync_packet(cid: &[u8; 16], sequence: u8, universe: u16) -> Vec<u8> {
    let mut p = root(VECTOR_ROOT_EXTENDED, cid);
    p.extend_from_slice(&[0, 0]);
    p.extend_from_slice(&VECTOR_EXTENDED_SYNCHRONIZATION.to_be_bytes());
    p.push(sequence);
    p.extend_from_slice(&universe.to_be_bytes());
    p.extend_from_slice(&[0, 0]);
    finish(p)
}

/// One page of an E1.31 Universe Discovery Packet (Table 3).
pub(crate) fn discovery_packet(
    cid: &[u8; 16],
    name: &str,
    page: u8,
    last: u8,
    universes: &[u16],
) -> Vec<u8> {
    let mut p = root(VECTOR_ROOT_EXTENDED, cid);
    p.extend_from_slice(&[0, 0]);
    p.extend_from_slice(&VECTOR_EXTENDED_DISCOVERY.to_be_bytes());
    name_field(&mut p, name);
    p.extend_from_slice(&[0; 4]);
    p.extend_from_slice(&flags_length(8 + 2 * universes.len()));
    p.extend_from_slice(&VECTOR_DISCOVERY_UNIVERSE_LIST.to_be_bytes());
    p.push(page);
    p.push(last);
    for u in universes {
        p.extend_from_slice(&u.to_be_bytes());
    }
    finish(p)
}

/// A received packet, decoded.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Packet {
    Data {
        cid: [u8; 16],
        name: String,
        priority: u8,
        sync: u16,
        sequence: u8,
        options: u8,
        universe: u16,
        start_code: u8,
        slots: Vec<u8>,
    },
    Sync {
        cid: [u8; 16],
        sequence: u8,
        universe: u16,
    },
    Discovery {
        cid: [u8; 16],
        name: String,
        page: u8,
        last: u8,
        universes: Vec<u16>,
    },
}

fn be16(p: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([p[at], p[at + 1]])
}

fn be32(p: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([p[at], p[at + 1], p[at + 2], p[at + 3]])
}

fn read_name(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

/// Decode a packet, discarding what the standard says receivers discard.
pub(crate) fn parse(p: &[u8]) -> Option<Packet> {
    if p.len() < 44 || be16(p, 0) != 0x0010 || be16(p, 2) != 0 || p[4..16] != ACN_ID {
        return None;
    }
    let mut cid = [0u8; 16];
    cid.copy_from_slice(&p[22..38]);
    match (be32(p, 18), be32(p, 40)) {
        (VECTOR_ROOT_DATA, VECTOR_DATA_PACKET) if p.len() >= 126 => {
            if p[117] != VECTOR_DMP_SET_PROPERTY
                || p[118] != 0xa1
                || be16(p, 119) != 0
                || be16(p, 121) != 1
            {
                return None;
            }
            let count = be16(p, 123) as usize;
            if count == 0 || count > 513 || p.len() < 125 + count {
                return None;
            }
            Some(Packet::Data {
                cid,
                name: read_name(&p[44..108]),
                priority: p[108],
                sync: be16(p, 109),
                sequence: p[111],
                options: p[112],
                universe: be16(p, 113),
                start_code: p[125],
                slots: p[126..125 + count].to_vec(),
            })
        }
        (VECTOR_ROOT_EXTENDED, VECTOR_EXTENDED_SYNCHRONIZATION) if p.len() >= 47 => {
            let universe = be16(p, 45);
            // A Synchronization Address of 0 is meaningless (§6.3.3.1).
            (universe != 0).then_some(Packet::Sync {
                cid,
                sequence: p[44],
                universe,
            })
        }
        (VECTOR_ROOT_EXTENDED, VECTOR_EXTENDED_DISCOVERY) if p.len() >= 120 => {
            if be32(p, 114) != VECTOR_DISCOVERY_UNIVERSE_LIST {
                return None;
            }
            let universes = p[120..]
                .chunks(2)
                .filter(|c| c.len() == 2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            Some(Packet::Discovery {
                cid,
                name: read_name(&p[44..108]),
                page: p[118],
                last: p[119],
                universes,
            })
        }
        _ => None,
    }
}

/// A CID in the RFC 4122 text form.
pub(crate) fn cid_text(cid: &[u8; 16]) -> String {
    let h: String = cid.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

fn parse_cid(text: &str) -> Option<[u8; 16]> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    let mut cid = [0u8; 16];
    for (i, b) in cid.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(cid)
}

/// A version 4 UUID (RFC 4122 §4.4) from what varies between openings, for
/// a source given no CID: "volatile components" make theirs as needed
/// (§5.6).
fn made_cid(seed: u64) -> [u8; 16] {
    let mut out = [0u8; 16];
    let mut x = seed ^ 0x9e37_79b9_7f4a_7c15;
    for chunk in out.chunks_mut(8) {
        // splitmix64
        x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        chunk.copy_from_slice(&z.to_be_bytes());
    }
    out[6] = (out[6] & 0x0f) | 0x40;
    out[8] = (out[8] & 0x3f) | 0x80;
    out
}

fn parse_universes(text: &str) -> Result<BTreeSet<u16>, String> {
    let mut set = BTreeSet::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        let bad = || format!("'{part}' is not a universe or range of universes 1-63999");
        let a: u16 = a.trim().parse().map_err(|_| bad())?;
        let b: u16 = b.trim().parse().map_err(|_| bad())?;
        if a == 0 || a > b || b > 63999 {
            return Err(bad());
        }
        set.extend(a..=b);
    }
    Ok(set)
}

fn parse_targets(text: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    text.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            t.parse::<SocketAddr>()
                .or_else(|_| t.parse::<IpAddr>().map(|ip| SocketAddr::new(ip, port)))
                .map_err(|_| format!("unicast address '{t}' is not an IP address or ip:port"))
        })
        .collect()
}

struct Output {
    slots: Vec<u8>,
    priority: u8,
    sequence: u8,
    /// Sends left before suppression: a change sets it to three.
    repeats: u8,
}

struct Source {
    name: String,
    ip: IpAddr,
    priority: u8,
    sequence: u8,
    preview: bool,
    sync: u16,
    slots: Vec<u8>,
    seen: Millis,
}

#[derive(Default)]
struct Input {
    sources: BTreeMap<[u8; 16], Source>,
    reported_at: Option<Millis>,
    stale: bool,
}

struct Discovered {
    name: String,
    ip: IpAddr,
    universes: Vec<u16>,
    seen: Millis,
}

pub(crate) struct Sacn {
    port: u16,
    address: Option<IpAddr>,
    cid: Option<[u8; 16]>,
    name: String,
    priority: u8,
    multicast: bool,
    unicast: Vec<SocketAddr>,
    sync_universe: u16,
    preview: bool,
    force_sync: bool,
    receive: BTreeSet<u16>,
    receive_interval: Millis,
    monitor: bool,
    listening: bool,
    retry_after: Millis,
    outputs: BTreeMap<u16, Output>,
    sync_sequence: u8,
    last_flush: Option<Millis>,
    flush_armed: bool,
    inputs: BTreeMap<u16, Input>,
    discovered: BTreeMap<[u8; 16], Discovered>,
}

impl Sacn {
    pub(crate) fn new(ctx: OpenContext) -> Result<Sacn, String> {
        let s = &ctx.settings;
        let text = |k: &str| {
            s.get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let int = |k: &str, d: i64| s.get(k).and_then(Value::as_i64).unwrap_or(d);
        let port = ctx.port.unwrap_or(PORT);
        let address = match text("bind_address").as_str() {
            "" => None,
            a => Some(
                a.parse::<IpAddr>()
                    .map_err(|_| format!("bind_address '{a}' is not an IP address"))?,
            ),
        };
        let cid = match text("cid").as_str() {
            "" => None,
            c => Some(parse_cid(c).ok_or(format!("cid '{c}' is not a UUID"))?),
        };
        let mut unicast = parse_targets(&text("unicast_addresses"), port)?;
        if !ctx.host.is_unspecified() {
            let host = SocketAddr::new(ctx.host, port);
            if !unicast.contains(&host) {
                unicast.push(host);
            }
        }
        let name = match text("source_name") {
            n if n.is_empty() => "Meros Integrations".to_string(),
            n => n,
        };
        Ok(Sacn {
            port,
            address,
            cid,
            name,
            priority: int("priority", 100).clamp(0, 200) as u8,
            multicast: s.get("multicast").and_then(Value::as_bool).unwrap_or(true),
            unicast,
            sync_universe: int("sync_universe", 0).clamp(0, 63999) as u16,
            preview: s.get("preview").and_then(Value::as_bool).unwrap_or(false),
            force_sync: s
                .get("force_sync")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            receive: parse_universes(&text("receive"))?,
            receive_interval: int("receive_interval_ms", 100).max(0) as Millis,
            monitor: ctx.monitor,
            listening: false,
            retry_after: RETRY_MIN,
            outputs: BTreeMap::new(),
            sync_sequence: 0,
            last_flush: None,
            flush_armed: false,
            inputs: BTreeMap::new(),
            discovered: BTreeMap::new(),
        })
    }

    fn cid(&self) -> [u8; 16] {
        self.cid.expect("made on starting")
    }

    fn destinations(&self, universe: u16) -> Vec<SocketAddr> {
        let mut out = Vec::new();
        if self.multicast {
            out.push(SocketAddr::new(IpAddr::V4(group(universe)), self.port));
        }
        out.extend(self.unicast.iter().copied());
        out
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
            cx.udp_join(SOCKET, group(DISCOVERY_UNIVERSE));
            for &u in &self.receive {
                cx.udp_join(SOCKET, group(u));
            }
            cx.set_timer(LOSS, 500);
        }
        cx.set_timer(KEEPALIVE, KEEPALIVE_EVERY);
        cx.set_timer(DISCOVERY, DISCOVERY_INTERVAL);
    }

    fn send(&self, cx: &mut Cx, universe: u16, packet: &[u8]) {
        for to in self.destinations(universe) {
            cx.udp_send(SOCKET, to, packet.to_vec());
        }
    }

    fn data(&mut self, cx: &mut Cx, universe: u16, options: u8) {
        let cid = self.cid();
        let mut preview = if self.preview { options::PREVIEW } else { 0 };
        if self.force_sync && self.sync_universe != 0 {
            preview |= options::FORCE_SYNC;
        }
        let Some(out) = self.outputs.get_mut(&universe) else {
            return;
        };
        out.sequence = out.sequence.wrapping_add(1);
        let packet = data_packet(
            &DataHeader {
                cid: &cid,
                name: &self.name,
                priority: out.priority,
                sync: self.sync_universe,
                sequence: out.sequence,
                options: options | preview,
                universe,
            },
            0,
            &out.slots,
        );
        self.send(cx, universe, &packet);
    }

    fn sync(&mut self, cx: &mut Cx, universe: u16) {
        self.sync_sequence = self.sync_sequence.wrapping_add(1);
        let packet = sync_packet(&self.cid(), self.sync_sequence, universe);
        self.send(cx, universe, &packet);
    }

    /// Send what has sends left (a change, or its two repeats), or every
    /// held universe for a keep-alive.
    fn flush(&mut self, cx: &mut Cx, keepalive: bool) {
        let due: Vec<u16> = self
            .outputs
            .iter()
            .filter(|(_, o)| keepalive || o.repeats > 0)
            .map(|(u, _)| *u)
            .collect();
        for &u in &due {
            self.data(cx, u, 0);
            let o = self.outputs.get_mut(&u).unwrap();
            o.repeats = o.repeats.saturating_sub(1);
        }
        if !due.is_empty() && self.sync_universe != 0 {
            self.sync(cx, self.sync_universe);
        }
        self.last_flush = Some(cx.now());
        // The repeats of a change go at the frame rate.
        if self.outputs.values().any(|o| o.repeats > 0) && !self.flush_armed {
            self.flush_armed = true;
            cx.set_timer(FLUSH, FRAME_GAP);
        }
    }

    fn changed(&mut self, cx: &mut Cx) {
        if self.flush_armed || !self.listening {
            return;
        }
        let since = self
            .last_flush
            .map(|at| cx.now().saturating_sub(at))
            .unwrap_or(FRAME_GAP);
        if since >= FRAME_GAP {
            self.flush(cx, false);
        } else {
            self.flush_armed = true;
            cx.set_timer(FLUSH, FRAME_GAP - since);
        }
    }

    fn advertise(&mut self, cx: &mut Cx) {
        let mut list: Vec<u16> = self.outputs.keys().copied().collect();
        if self.sync_universe != 0 && !list.is_empty() {
            list.push(self.sync_universe);
            list.sort_unstable();
            list.dedup();
        }
        // A source sending nothing need send no discovery (§12.1).
        if list.is_empty() {
            return;
        }
        let pages: Vec<&[u16]> = list.chunks(PAGE).collect();
        let last = (pages.len() - 1) as u8;
        let cid = self.cid();
        for (i, page) in pages.iter().enumerate() {
            let packet = discovery_packet(&cid, &self.name, i as u8, last, page);
            self.send(cx, DISCOVERY_UNIVERSE, &packet);
        }
    }

    fn output_state(&self, cx: &mut Cx, universe: u16) {
        if let Some(o) = self.outputs.get(&universe) {
            cx.state(json!({"outputs": {universe.to_string(): {
                "channels": o.slots, "priority": o.priority,
            }}}));
        }
    }

    fn release(&mut self, cx: &mut Cx, universe: u16) -> bool {
        if !self.outputs.contains_key(&universe) {
            return false;
        }
        // Three packets with Stream_Terminated (§6.2.6).
        for _ in 0..3 {
            self.data(cx, universe, options::TERMINATED);
        }
        self.outputs.remove(&universe);
        cx.state(json!({"outputs": {universe.to_string(): Value::Null}}));
        true
    }

    fn packet(&mut self, cx: &mut Cx, from: SocketAddr, data: &[u8]) {
        let Some(packet) = parse(data) else {
            return;
        };
        let own = self.cid();
        match packet {
            Packet::Data { cid, .. } | Packet::Sync { cid, .. } | Packet::Discovery { cid, .. }
                if cid == own =>
            {
                // This source's own multicast, looped back.
            }
            Packet::Data {
                cid,
                name,
                priority,
                sync,
                sequence,
                options: opts,
                universe,
                start_code,
                slots,
            } => {
                cx.alive();
                if !self.monitor || !self.receive.contains(&universe) {
                    return;
                }
                if start_code != 0 {
                    cx.message(
                        "sacn/start_code",
                        None,
                        json!({"universe": universe, "start_code": start_code, "cid": cid_text(&cid),
                               "data": slots.iter().map(|b| format!("{b:02x}")).collect::<String>()}),
                        from,
                    );
                    return;
                }
                self.data_in(
                    cx, from, cid, name, priority, sync, sequence, opts, universe, slots,
                );
            }
            Packet::Sync { cid, universe, .. } => {
                cx.alive();
                cx.message(
                    "sacn/sync",
                    None,
                    json!({"universe": universe, "cid": cid_text(&cid)}),
                    from,
                );
            }
            Packet::Discovery {
                cid,
                name,
                page,
                last,
                universes,
            } => {
                cx.alive();
                if !self.monitor {
                    return;
                }
                let now = cx.now();
                let entry = self.discovered.entry(cid).or_insert(Discovered {
                    name: name.clone(),
                    ip: from.ip(),
                    universes: Vec::new(),
                    seen: now,
                });
                if page == 0 {
                    entry.universes.clear();
                }
                entry.universes.extend(universes);
                entry.name = name;
                entry.ip = from.ip();
                entry.seen = now;
                let _ = last;
                cx.state(json!({"sources": {cid_text(&cid): {
                    "name": entry.name, "ip": entry.ip.to_string(), "universes": entry.universes,
                }}}));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn data_in(
        &mut self,
        cx: &mut Cx,
        from: SocketAddr,
        cid: [u8; 16],
        name: String,
        priority: u8,
        sync: u16,
        sequence: u8,
        opts: u8,
        universe: u16,
        slots: Vec<u8>,
    ) {
        let now = cx.now();
        let input = self.inputs.entry(universe).or_default();
        if opts & options::TERMINATED != 0 {
            if input.sources.remove(&cid).is_some() {
                cx.state(json!({"inputs": {universe.to_string(): {"sources": {cid_text(&cid): Value::Null}}}}));
                Self::report(cx, universe, input, now);
            }
            return;
        }
        if let Some(previous) = input.sources.get(&cid) {
            // Out of sequence: B - A in (-20, 0] is discarded (§6.7.2).
            let d = sequence.wrapping_sub(previous.sequence) as i8;
            if d <= 0 && d > -20 {
                return;
            }
        }
        let fresh = input.sources.get(&cid).is_none_or(|s| {
            s.name != name
                || s.priority != priority
                || s.preview != (opts & options::PREVIEW != 0)
                || s.sync != sync
                || s.ip != from.ip()
        });
        let changed = fresh || input.sources.get(&cid).is_none_or(|s| s.slots != slots);
        input.sources.insert(
            cid,
            Source {
                name,
                ip: from.ip(),
                priority,
                sequence,
                preview: opts & options::PREVIEW != 0,
                sync,
                slots,
                seen: now,
            },
        );
        if fresh {
            let s = &input.sources[&cid];
            cx.state(
                json!({"inputs": {universe.to_string(): {"sources": {cid_text(&cid): {
                    "name": s.name, "ip": s.ip.to_string(), "priority": s.priority,
                    "preview": s.preview, "sync_universe": s.sync,
                }}}}}),
            );
        }
        if !changed {
            return;
        }
        let due = input
            .reported_at
            .is_none_or(|at| now.saturating_sub(at) >= self.receive_interval);
        if due || fresh {
            Self::report(cx, universe, input, now);
        } else if !input.stale {
            input.stale = true;
            let wait = self.receive_interval - now.saturating_sub(input.reported_at.unwrap_or(now));
            cx.set_timer(INPUTS, wait.max(1));
        }
    }

    /// The merged levels of a universe: the highest priority's sources, slot
    /// by slot highest takes precedence; preview data is left out.
    fn report(cx: &mut Cx, universe: u16, input: &mut Input, now: Millis) {
        input.reported_at = Some(now);
        input.stale = false;
        let live: Vec<&Source> = input.sources.values().filter(|s| !s.preview).collect();
        let top = live.iter().map(|s| s.priority).max();
        let mut merged: Vec<u8> = Vec::new();
        let mut winners = 0;
        for s in live.iter().filter(|s| Some(s.priority) == top) {
            winners += 1;
            if merged.len() < s.slots.len() {
                merged.resize(s.slots.len(), 0);
            }
            for (m, v) in merged.iter_mut().zip(&s.slots) {
                *m = (*m).max(*v);
            }
        }
        cx.state(json!({"inputs": {universe.to_string(): {
            "channels": merged, "priority": top.unwrap_or(0), "merged_sources": winners,
        }}}));
    }

    fn data_loss(&mut self, cx: &mut Cx) {
        let now = cx.now();
        for (u, input) in self.inputs.iter_mut() {
            let gone: Vec<[u8; 16]> = input
                .sources
                .iter()
                .filter(|(_, s)| now.saturating_sub(s.seen) >= DATA_LOSS)
                .map(|(c, _)| *c)
                .collect();
            if gone.is_empty() {
                continue;
            }
            let mut patch = Map::new();
            for c in gone {
                input.sources.remove(&c);
                patch.insert(cid_text(&c), Value::Null);
            }
            cx.state(json!({"inputs": {u.to_string(): {"sources": patch}}}));
            Self::report(cx, *u, input, now);
        }
        let gone: Vec<[u8; 16]> = self
            .discovered
            .iter()
            .filter(|(_, d)| now.saturating_sub(d.seen) >= SOURCE_GONE)
            .map(|(c, _)| *c)
            .collect();
        if !gone.is_empty() {
            let mut patch = Map::new();
            for c in gone {
                self.discovered.remove(&c);
                patch.insert(cid_text(&c), Value::Null);
            }
            cx.state(json!({"sources": patch}));
        }
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

fn byte_values(v: Option<&Value>, max: usize) -> Result<Vec<u8>, String> {
    let list = v
        .and_then(Value::as_array)
        .ok_or("values must be a list of integers 0-255")?;
    if list.len() > max {
        return Err(format!("at most {max} values"));
    }
    list.iter()
        .map(|x| {
            x.as_u64()
                .filter(|n| *n <= 255)
                .map(|n| n as u8)
                .ok_or_else(|| format!("{x} is not an integer 0-255"))
        })
        .collect()
}

impl Module for Sacn {
    fn start(&mut self, cx: &mut Cx) {
        if self.cid.is_none() {
            self.cid = Some(made_cid(cx.unix_millis() ^ (self.port as u64) << 48));
        }
        let receive: Vec<u16> = self.receive.iter().copied().collect();
        cx.state(json!({
            "cid": cid_text(&self.cid()), "source_name": self.name, "port": self.port,
            "receive": receive,
        }));
        self.listen(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        let int = |k: &str| params.get(k).and_then(Value::as_i64).unwrap_or(0);
        if !self.listening {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let universe = int("universe") as u16;
        match name {
            "set_channel" | "set_channels" | "set_universe" | "clear_universe" => {
                let values = match name {
                    "set_channel" => vec![int("value") as u8],
                    "clear_universe" => vec![0; 512],
                    "set_universe" => match byte_values(params.get("values"), 512) {
                        Ok(v) => v,
                        Err(e) => return Self::fail(cx, id, e),
                    },
                    _ => {
                        match byte_values(params.get("values"), 513 - int("start").max(1) as usize)
                        {
                            Ok(v) => v,
                            Err(e) => return Self::fail(cx, id, e),
                        }
                    }
                };
                let start = match name {
                    "set_channel" => int("channel") as usize,
                    "set_channels" => int("start") as usize,
                    _ => 1,
                };
                let priority = self.priority;
                let out = self.outputs.entry(universe).or_insert(Output {
                    slots: vec![0; 512],
                    priority,
                    sequence: 0,
                    repeats: REPEATS,
                });
                if name == "set_universe" {
                    out.slots = vec![0; 512];
                }
                for (i, v) in values.iter().enumerate() {
                    out.slots[start - 1 + i] = *v;
                }
                out.repeats = REPEATS;
                self.output_state(cx, universe);
                self.changed(cx);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "set_universe_priority" => {
                let priority = int("priority") as u8;
                let out = self.outputs.entry(universe).or_insert(Output {
                    slots: vec![0; 512],
                    priority,
                    sequence: 0,
                    repeats: REPEATS,
                });
                out.priority = priority;
                out.repeats = REPEATS;
                self.output_state(cx, universe);
                self.changed(cx);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "release_universe" => {
                self.release(cx, universe);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "release_all" => {
                let all: Vec<u16> = self.outputs.keys().copied().collect();
                for u in all {
                    self.release(cx, u);
                }
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "send_sync" => {
                let u = if universe == 0 {
                    self.sync_universe
                } else {
                    universe
                };
                if u == 0 {
                    return Self::fail(cx, id, "give a universe, or set sync_universe");
                }
                self.sync(cx, u);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "send_start_code" => {
                let start_code = int("start_code") as u8;
                let values = match byte_values(params.get("values"), 512) {
                    Ok(v) => v,
                    Err(e) => return Self::fail(cx, id, e),
                };
                let cid = self.cid();
                let packet = data_packet(
                    &DataHeader {
                        cid: &cid,
                        name: &self.name,
                        priority: self
                            .outputs
                            .get(&universe)
                            .map_or(self.priority, |o| o.priority),
                        sync: 0,
                        sequence: 0,
                        options: if self.preview { options::PREVIEW } else { 0 },
                        universe,
                    },
                    start_code,
                    &values,
                );
                self.send(cx, universe, &packet);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "receive_universe" => {
                let on = params
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let changed = if on {
                    self.receive.insert(universe)
                } else {
                    self.receive.remove(&universe)
                };
                if changed && self.monitor {
                    if on {
                        cx.udp_join(SOCKET, group(universe));
                    } else {
                        cx.udp_leave(SOCKET, group(universe));
                        if self.inputs.remove(&universe).is_some() {
                            cx.state(json!({"inputs": {universe.to_string(): Value::Null}}));
                        }
                    }
                }
                let receive: Vec<u16> = self.receive.iter().copied().collect();
                cx.state(json!({"receive": receive}));
                cx.complete(id, Ok(Outcome::Ack));
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
        cx.log(Level::Warning, format!("sACN port: {message}"));
        cx.connection(Connection::Disconnected {
            reason: message.to_string(),
        });
        for t in [KEEPALIVE, DISCOVERY, LOSS] {
            cx.cancel_timer(t);
        }
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.listen(cx),
            FLUSH => {
                self.flush_armed = false;
                if self.listening {
                    self.flush(cx, false);
                }
            }
            KEEPALIVE if self.listening => {
                // Universes still repeating a change are not due a keep-alive.
                if self.outputs.values().all(|o| o.repeats == 0) {
                    self.flush(cx, true);
                }
                cx.set_timer(KEEPALIVE, KEEPALIVE_EVERY);
            }
            DISCOVERY if self.listening => {
                self.advertise(cx);
                cx.set_timer(DISCOVERY, DISCOVERY_INTERVAL);
            }
            LOSS if self.listening => {
                self.data_loss(cx);
                cx.set_timer(LOSS, 500);
            }
            INPUTS => {
                let now = cx.now();
                for (u, input) in self.inputs.iter_mut() {
                    if input.stale {
                        Self::report(cx, *u, input, now);
                    }
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        // Terminate every stream, so receivers need not wait out the
        // network data loss timeout.
        if self.listening {
            let all: Vec<u16> = self.outputs.keys().copied().collect();
            for u in all {
                self.release(cx, u);
            }
        }
        cx.udp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;

    const CID: &str = "ef07c8dd-0064-4401-a3a2-459ef8e6143e";

    fn module(settings: Value, monitor: bool) -> Sacn {
        let mut s = settings;
        if s.get("cid").is_none() {
            s["cid"] = json!(CID);
        }
        Sacn::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            host_name: None,
            port: None,
            model: "sacn".into(),
            channels: None,
            settings: s.as_object().unwrap().clone(),
            monitor,
        })
        .unwrap()
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

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    fn other_cid() -> [u8; 16] {
        parse_cid("11111111-2222-4333-8444-555555555555").unwrap()
    }

    #[test]
    fn a_data_packet_has_the_layout_of_table_1() {
        let cid = parse_cid(CID).unwrap();
        let p = data_packet(
            &DataHeader {
                cid: &cid,
                name: "Source_A",
                priority: 100,
                sync: 7962,
                sequence: 154,
                options: 0,
                universe: 1,
            },
            0,
            &[0; 512],
        );
        assert_eq!(p.len(), 638);
        assert_eq!(hex(&p[..16]), "001000004153432d45312e3137000000");
        // Root, framing and DMP lengths count from their own Flags and
        // Length to the end: 622, 600 and 523.
        assert_eq!(hex(&p[16..22]), "726e00000004");
        assert_eq!(hex(&p[22..38]), "ef07c8dd00644401a3a2459ef8e6143e");
        assert_eq!(hex(&p[38..44]), "725800000002");
        assert_eq!(&p[44..52], b"Source_A");
        assert_eq!(
            hex(&p[108..126]),
            "641f1a9a000001720b02a1000000010201".to_string() + "00"
        );
    }

    #[test]
    fn sync_and_discovery_packets_have_their_layouts() {
        let cid = parse_cid(CID).unwrap();
        let s = sync_packet(&cid, 7, 7962);
        assert_eq!(s.len(), 49);
        assert_eq!(hex(&s[16..22]), "702100000008");
        assert_eq!(hex(&s[38..49]), "700b00000001071f1a0000");
        let d = discovery_packet(&cid, "A", 0, 0, &[1, 2]);
        assert_eq!(d.len(), 124);
        assert_eq!(hex(&d[16..22]), "706c00000008");
        assert_eq!(hex(&d[38..44]), "705600000002");
        assert_eq!(hex(&d[112..124]), "700c00000001000000010002");
        assert_eq!(
            parse(&d),
            Some(Packet::Discovery {
                cid,
                name: "A".into(),
                page: 0,
                last: 0,
                universes: vec![1, 2],
            })
        );
    }

    #[test]
    fn output_is_multicast_three_times_then_kept_alive() {
        let mut m = module(json!({}), true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Network {
                address: None,
                port: 5568
            }
        }));
        assert!(a.contains(&Action::UdpJoin {
            socket: SOCKET,
            group: Ipv4Addr::new(239, 255, 250, 214)
        }));
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "set_channel",
            &params(json!({"universe": 258, "channel": 1, "value": 9})),
        );
        let a = cx.take();
        let s = sends(&a);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].0, "239.255.1.2:5568".parse::<SocketAddr>().unwrap());
        assert_eq!(s[0].1[111], 1, "sequence");
        assert_eq!(s[0].1[126], 9);
        assert!(a.contains(&Action::SetTimer {
            key: FLUSH,
            after: 23
        }));
        let mut cx = Cx::new(33);
        m.timer(&mut cx, FLUSH);
        let mut cx2 = Cx::new(56);
        m.timer(&mut cx2, FLUSH);
        assert_eq!(
            sends(&cx.take()).len() + sends(&cx2.take()).len(),
            2,
            "two repeats"
        );
        let mut cx = Cx::new(79);
        m.timer(&mut cx, FLUSH);
        assert!(sends(&cx.take()).is_empty(), "then suppressed");
        let mut cx = Cx::new(1000);
        m.timer(&mut cx, KEEPALIVE);
        let s = sends(&cx.take());
        assert_eq!(s.len(), 1, "a keep-alive");
        assert_eq!(s[0].1[111], 4);
        // Discovery lists it.
        let mut cx = Cx::new(10_000);
        m.timer(&mut cx, DISCOVERY);
        let s = sends(&cx.take());
        assert_eq!(
            s[0].0,
            "239.255.250.214:5568".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(hex(&s[0].1[120..]), "0102");
        // Releasing terminates the stream with three packets.
        let mut cx = Cx::new(10_100);
        m.command(
            &mut cx,
            2,
            "release_universe",
            &params(json!({"universe": 258})),
        );
        let s = sends(&cx.take());
        assert_eq!(s.len(), 3);
        assert!(s.iter().all(|(_, p)| p[112] == options::TERMINATED));
    }

    #[test]
    fn sync_universe_follows_each_burst_and_priority_is_per_universe() {
        let mut m = module(
            json!({"sync_universe": 7962, "priority": 120, "unicast_addresses": "10.0.0.5"}),
            true,
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "set_universe_priority",
            &params(json!({"universe": 1, "priority": 150})),
        );
        let s = sends(&cx.take());
        // Data to the multicast group and the unicast address, then sync.
        assert_eq!(s.len(), 4);
        assert_eq!(s[1].0, "10.0.0.5:5568".parse::<SocketAddr>().unwrap());
        assert_eq!(s[0].1[108], 150);
        assert_eq!(hex(&s[0].1[109..111]), "1f1a");
        assert_eq!(s[2].0, "239.255.31.26:5568".parse::<SocketAddr>().unwrap());
        assert_eq!(s[2].1.len(), 49);
    }

    #[test]
    fn received_sources_merge_by_priority_then_htp_and_time_out() {
        let mut m = module(json!({"receive": "1", "receive_interval_ms": 0}), true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx.take().contains(&Action::UdpJoin {
            socket: SOCKET,
            group: Ipv4Addr::new(239, 255, 0, 1)
        }));
        let a = other_cid();
        let b = parse_cid("aaaaaaaa-2222-4333-8444-555555555555").unwrap();
        let packet = |cid: &[u8; 16], prio: u8, seq: u8, slots: &[u8]| {
            data_packet(
                &DataHeader {
                    cid,
                    name: "x",
                    priority: prio,
                    sync: 0,
                    sequence: seq,
                    options: 0,
                    universe: 1,
                },
                0,
                slots,
            )
        };
        let from: SocketAddr = "10.0.0.7:5568".parse().unwrap();
        let mut cx = Cx::new(10);
        m.datagram(&mut cx, SOCKET, from, &packet(&a, 100, 1, &[10, 0]));
        m.datagram(&mut cx, SOCKET, from, &packet(&b, 100, 1, &[0, 20]));
        let last = |actions: Vec<Action>| {
            actions.into_iter().rev().find_map(|a| match a {
                Action::State(p) if p["inputs"]["1"].get("channels").is_some() => {
                    Some(p["inputs"]["1"].clone())
                }
                _ => None,
            })
        };
        let got = last(cx.take()).unwrap();
        assert_eq!(got["channels"], json!([10, 20]));
        assert_eq!(got["merged_sources"], 2);
        // A higher priority wins outright.
        let mut cx = Cx::new(20);
        m.datagram(&mut cx, SOCKET, from, &packet(&b, 150, 2, &[0, 30]));
        assert_eq!(last(cx.take()).unwrap()["channels"], json!([0, 30]));
        // Out of sequence: discarded.
        let mut cx = Cx::new(30);
        m.datagram(&mut cx, SOCKET, from, &packet(&b, 150, 1, &[0, 99]));
        assert!(last(cx.take()).is_none());
        // b falls silent: data loss leaves a.
        let mut cx = Cx::new(2600);
        m.datagram(&mut cx, SOCKET, from, &packet(&a, 100, 2, &[10, 0]));
        cx.take();
        let mut cx = Cx::new(2700);
        m.timer(&mut cx, LOSS);
        let a_ = cx.take();
        assert!(a_.iter().any(|x| matches!(x, Action::State(p) if p["inputs"]["1"]["sources"][cid_text(&b)].is_null() && p["inputs"]["1"]["sources"].get(cid_text(&b)).is_some())));
        assert_eq!(last(a_).unwrap()["channels"], json!([10, 0]));
    }

    #[test]
    fn discovery_is_kept_and_own_packets_are_ignored() {
        let mut m = module(json!({}), true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let from: SocketAddr = "10.0.0.7:5568".parse().unwrap();
        let mut cx = Cx::new(5);
        m.datagram(
            &mut cx,
            SOCKET,
            from,
            &discovery_packet(&other_cid(), "Desk", 0, 0, &[1, 5]),
        );
        assert!(cx
            .take()
            .contains(&Action::State(json!({"sources": {cid_text(&other_cid()): {
            "name": "Desk", "ip": "10.0.0.7", "universes": [1, 5]}}}))));
        let own = parse_cid(CID).unwrap();
        let mut cx = Cx::new(6);
        m.datagram(
            &mut cx,
            SOCKET,
            from,
            &discovery_packet(&own, "Me", 0, 0, &[1]),
        );
        assert!(cx.take().is_empty());
    }

    #[test]
    fn commands_only_joins_nothing_and_still_sends() {
        let mut m = module(json!({"receive": "1-3"}), false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(!a.iter().any(|x| matches!(x, Action::UdpJoin { .. })));
        let mut cx = Cx::new(1);
        m.command(
            &mut cx,
            1,
            "set_channel",
            &params(json!({"universe": 1, "channel": 1, "value": 1})),
        );
        assert_eq!(sends(&cx.take()).len(), 1);
    }

    #[test]
    fn a_cid_is_made_when_none_is_given() {
        let mut m = Sacn::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            host_name: None,
            port: None,
            model: "sacn".into(),
            channels: None,
            settings: Params::new(),
            monitor: true,
        })
        .unwrap();
        let mut cx = Cx::at(0, 1_800_000_000_000);
        m.start(&mut cx);
        let cid = m.cid();
        assert_eq!(cid[6] >> 4, 4, "version 4");
        assert_eq!(cid[8] >> 6, 2, "RFC 4122 variant");
    }
}
