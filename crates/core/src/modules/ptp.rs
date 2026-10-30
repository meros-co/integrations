//! PTP version 2 (IEEE 1588-2008 and 1588-2019, and their profiles such as
//! SMPTE ST 2059-2, AES67 and IEEE 802.1AS) monitored on UDP over IPv4: the
//! core listens, sends nothing, and keeps what the network's clocks say.
//!
//! - Two sockets on the event port (319: Sync, Delay_Req, Pdelay_Req,
//!   Pdelay_Resp) and the general port (320: Announce, Follow_Up,
//!   Delay_Resp, Pdelay_Resp_Follow_Up, Signaling, Management), shared with
//!   other programs on the host, joined to 224.0.1.129 (all PTP messages
//!   but peer delay) and 224.0.0.107 (peer delay).
//! - Every port heard is kept under `domains.<domain>.ports.<clock
//!   identity>:<port number>`, with its address, the role its traffic shows
//!   (a port sending Announce or Sync is a master, one sending Delay_Req a
//!   slave, one sending only peer delay messages a peer), its message
//!   intervals, the dataset its Announce messages carry, and Sync figures:
//!   the rate heard, the correction field, missed sequence numbers, and the
//!   host clock's offset from the PTP time the Sync (or its Follow_Up)
//!   carries. A port silent for `timeout_ms` is marked offline.
//! - Each domain's best master is chosen among the online ports' Announce
//!   messages with the dataset comparison (grandmaster priority 1, clock
//!   class, accuracy, variance, priority 2, identity, then steps removed and
//!   the sender's identity), and its grandmaster dataset kept under
//!   `domains.<domain>.grandmaster`.
//! - Sync figures change many times a second, so each port's are reported at
//!   most every `report_interval_ms`.
//!
//! Monitoring is the integration's whole purpose: opened for commands only
//! (`monitor` false) it still listens, since it has nothing else to do.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};

pub(crate) const EVENT_PORT: u16 = 319;
pub(crate) const GENERAL_PORT: u16 = 320;
/// IANA's PTP-primary and PTP-pdelay groups.
pub(crate) const PRIMARY: Ipv4Addr = Ipv4Addr::new(224, 0, 1, 129);
pub(crate) const PDELAY: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 107);
const EVENT: Key = "event";
const GENERAL: Key = "general";
const RETRY: Key = "retry";
const REPORT: Key = "report";
const SWEEP: Key = "sweep";
const RETRY_MIN: Millis = 1000;
const RETRY_MAX: Millis = 30_000;

pub(crate) const SYNC: u8 = 0x0;
pub(crate) const DELAY_REQ: u8 = 0x1;
pub(crate) const PDELAY_REQ: u8 = 0x2;
pub(crate) const PDELAY_RESP: u8 = 0x3;
pub(crate) const FOLLOW_UP: u8 = 0x8;
pub(crate) const DELAY_RESP: u8 = 0x9;
pub(crate) const PDELAY_RESP_FOLLOW_UP: u8 = 0xA;
pub(crate) const ANNOUNCE: u8 = 0xB;
pub(crate) const SIGNALING: u8 = 0xC;
pub(crate) const MANAGEMENT: u8 = 0xD;
const HEADER: usize = 34;

fn message_name(t: u8) -> &'static str {
    match t {
        SYNC => "sync",
        DELAY_REQ => "delay_req",
        PDELAY_REQ => "pdelay_req",
        PDELAY_RESP => "pdelay_resp",
        FOLLOW_UP => "follow_up",
        DELAY_RESP => "delay_resp",
        PDELAY_RESP_FOLLOW_UP => "pdelay_resp_follow_up",
        ANNOUNCE => "announce",
        SIGNALING => "signaling",
        MANAGEMENT => "management",
        _ => "other",
    }
}

/// A clock identity as IEEE's canonical EUI-64 text, 00-1D-C1-FF-FE-0A-1B-2C.
pub(crate) fn identity_text(id: &[u8]) -> String {
    id.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join("-")
}

/// clockAccuracy (1588-2008 Table 6; 2019 adds 17h to 1Fh).
pub(crate) fn accuracy_text(a: u8) -> &'static str {
    match a {
        0x17 => "1 ps",
        0x18 => "2.5 ps",
        0x19 => "10 ps",
        0x1A => "25 ps",
        0x1B => "100 ps",
        0x1C => "250 ps",
        0x1D => "1 ns",
        0x1E => "2.5 ns",
        0x1F => "10 ns",
        0x20 => "25 ns",
        0x21 => "100 ns",
        0x22 => "250 ns",
        0x23 => "1 µs",
        0x24 => "2.5 µs",
        0x25 => "10 µs",
        0x26 => "25 µs",
        0x27 => "100 µs",
        0x28 => "250 µs",
        0x29 => "1 ms",
        0x2A => "2.5 ms",
        0x2B => "10 ms",
        0x2C => "25 ms",
        0x2D => "100 ms",
        0x2E => "250 ms",
        0x2F => "1 s",
        0x30 => "10 s",
        0x31 => "more than 10 s",
        0xFE => "unknown",
        _ => "reserved",
    }
}

/// timeSource (1588-2008 Table 7).
pub(crate) fn time_source_text(t: u8) -> &'static str {
    match t {
        0x10 => "atomic_clock",
        0x20 => "gps",
        0x30 => "terrestrial_radio",
        0x39 => "serial_time_code",
        0x40 => "ptp",
        0x50 => "ntp",
        0x60 => "hand_set",
        0x90 => "other",
        0xA0 => "internal_oscillator",
        _ => "reserved",
    }
}

/// The common header (1588-2008 §13.3).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Header {
    pub message_type: u8,
    pub version: u8,
    pub minor_version: u8,
    pub domain: u8,
    pub flags: u16,
    /// correctionField in nanoseconds (scaled by 2^16 on the wire).
    pub correction_ns: f64,
    pub clock: [u8; 8],
    pub port: u16,
    pub sequence: u16,
    pub log_interval: i8,
}

impl Header {
    pub(crate) fn port_key(&self) -> String {
        format!("{}:{}", identity_text(&self.clock), self.port)
    }

    fn two_step(&self) -> bool {
        self.flags & 0x0200 != 0
    }
}

pub(crate) fn parse_header(d: &[u8]) -> Option<Header> {
    if d.len() < HEADER {
        return None;
    }
    let version = d[1] & 0x0F;
    if version != 2 {
        return None;
    }
    let length = usize::from(u16::from_be_bytes([d[2], d[3]]));
    if length < HEADER || length > d.len() {
        return None;
    }
    let correction = i64::from_be_bytes(d[8..16].try_into().ok()?);
    Some(Header {
        message_type: d[0] & 0x0F,
        version,
        minor_version: d[1] >> 4,
        domain: d[4],
        flags: u16::from_be_bytes([d[6], d[7]]),
        correction_ns: correction as f64 / 65536.0,
        clock: d[20..28].try_into().ok()?,
        port: u16::from_be_bytes([d[28], d[29]]),
        sequence: u16::from_be_bytes([d[30], d[31]]),
        log_interval: d[33] as i8,
    })
}

/// A 10-byte timestamp: 48-bit seconds and 32-bit nanoseconds.
fn timestamp(d: &[u8]) -> Option<(u64, u32)> {
    let s = d.get(..10)?;
    let seconds = u64::from_be_bytes([0, 0, s[0], s[1], s[2], s[3], s[4], s[5]]);
    let nanos = u32::from_be_bytes([s[6], s[7], s[8], s[9]]);
    Some((seconds, nanos))
}

/// An Announce message's body (1588-2008 §13.5).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Announce {
    pub current_utc_offset: i16,
    pub priority1: u8,
    pub clock_class: u8,
    pub clock_accuracy: u8,
    pub variance: u16,
    pub priority2: u8,
    pub grandmaster: [u8; 8],
    pub steps_removed: u16,
    pub time_source: u8,
    pub flags: u16,
}

impl Announce {
    /// The dataset comparison's order: lower is better.
    fn rank(&self, sender: &Header) -> (u8, u8, u8, u16, u8, [u8; 8], u16, [u8; 8], u16) {
        (
            self.priority1,
            self.clock_class,
            self.clock_accuracy,
            self.variance,
            self.priority2,
            self.grandmaster,
            self.steps_removed,
            sender.clock,
            sender.port,
        )
    }

    fn json(&self) -> Value {
        let f = self.flags;
        json!({
            "identity": identity_text(&self.grandmaster),
            "priority1": self.priority1,
            "priority2": self.priority2,
            "clock_class": self.clock_class,
            "clock_accuracy": format!("0x{:02X}", self.clock_accuracy),
            "clock_accuracy_text": accuracy_text(self.clock_accuracy),
            "offset_scaled_log_variance": self.variance,
            "steps_removed": self.steps_removed,
            "time_source": format!("0x{:02X}", self.time_source),
            "time_source_text": time_source_text(self.time_source),
            "current_utc_offset": self.current_utc_offset,
            "current_utc_offset_valid": f & 0x0004 != 0,
            "leap61": f & 0x0001 != 0,
            "leap59": f & 0x0002 != 0,
            "ptp_timescale": f & 0x0008 != 0,
            "time_traceable": f & 0x0010 != 0,
            "frequency_traceable": f & 0x0020 != 0,
        })
    }
}

pub(crate) fn parse_announce(d: &[u8], header: &Header) -> Option<Announce> {
    let b = d.get(HEADER..HEADER + 30)?;
    Some(Announce {
        current_utc_offset: i16::from_be_bytes([b[10], b[11]]),
        priority1: b[13],
        clock_class: b[14],
        clock_accuracy: b[15],
        variance: u16::from_be_bytes([b[16], b[17]]),
        priority2: b[18],
        grandmaster: b[19..27].try_into().ok()?,
        steps_removed: u16::from_be_bytes([b[27], b[28]]),
        time_source: b[29],
        flags: header.flags,
    })
}

/// What is kept of one port.
#[derive(Debug, Default)]
struct Port {
    address: Option<IpAddr>,
    seen: Millis,
    online: bool,
    announce: Option<(Header, Announce)>,
    sent: BTreeMap<&'static str, u64>,
    /// The Sync awaiting its Follow_Up, by sequence: its correction.
    sync_pending: Option<(u16, f64, Millis, u64)>,
    last_sync_seq: Option<u16>,
    missed: u64,
    sync_count: u64,
    /// Sync messages since the last report, for the rate.
    window: (Millis, u64),
    /// Sync figures waiting to be reported.
    figures: Map<String, Value>,
    reported_at: Option<Millis>,
    due: bool,
}

pub(crate) struct Ptp {
    event_port: u16,
    general_port: u16,
    address: Option<IpAddr>,
    groups: Vec<Ipv4Addr>,
    domains: Option<Vec<u8>>,
    timeout: Millis,
    report_every: Millis,
    listening: bool,
    retry_after: Millis,
    ports: BTreeMap<(u8, String), Port>,
    /// Each domain's best master's port key, as last reported.
    best: BTreeMap<u8, String>,
}

impl Ptp {
    pub(crate) fn new(ctx: OpenContext) -> Result<Ptp, String> {
        let s = &ctx.settings;
        let text = |k: &str| {
            s.get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let int = |k: &str, d: u64| s.get(k).and_then(Value::as_u64).unwrap_or(d);
        let address = match text("bind_address").as_str() {
            "" => None,
            a => Some(
                a.parse::<IpAddr>()
                    .map_err(|_| format!("bind_address '{a}' is not an IP address"))?,
            ),
        };
        let mut domains = Vec::new();
        for d in text("domains")
            .split([',', ' ', ';'])
            .map(str::trim)
            .filter(|d| !d.is_empty())
        {
            domains.push(
                d.parse::<u8>()
                    .map_err(|_| format!("domain '{d}' is not 0 to 255"))?,
            );
        }
        let mut groups = vec![PRIMARY];
        if s.get("peer_delay").and_then(Value::as_bool).unwrap_or(true) {
            groups.push(PDELAY);
        }
        Ok(Ptp {
            event_port: int("event_port", u64::from(EVENT_PORT)) as u16,
            general_port: int("general_port", u64::from(GENERAL_PORT)) as u16,
            address,
            groups,
            domains: (!domains.is_empty()).then_some(domains),
            timeout: int("timeout_ms", 6000).max(100),
            report_every: int("report_interval_ms", 1000),
            listening: false,
            retry_after: RETRY_MIN,
            ports: BTreeMap::new(),
            best: BTreeMap::new(),
        })
    }

    fn listen(&mut self, cx: &mut Cx) {
        self.listening = true;
        for (socket, port) in [(EVENT, self.event_port), (GENERAL, self.general_port)] {
            cx.udp_open(
                socket,
                Bind::Network {
                    address: self.address,
                    port,
                },
            );
            for g in &self.groups {
                cx.udp_join(socket, *g);
            }
        }
        cx.connection(Connection::Connected);
        cx.set_timer(SWEEP, 1000);
    }

    fn port_patch(cx: &mut Cx, domain: u8, key: &str, value: Value) {
        cx.state(json!({"domains": {domain.to_string(): {"ports": {key: value}}}}));
    }

    pub(crate) fn received(&mut self, cx: &mut Cx, from: SocketAddr, data: &[u8]) {
        let Some(h) = parse_header(data) else {
            return;
        };
        if let Some(domains) = &self.domains {
            if !domains.contains(&h.domain) {
                return;
            }
        }
        cx.alive();
        let now = cx.now();
        let key = h.port_key();
        let domain = h.domain;
        let port = self.ports.entry((domain, key.clone())).or_default();
        let mut patch = Map::new();
        port.seen = now;
        if !port.online {
            port.online = true;
            patch.insert("online".into(), json!(true));
        }
        if port.address != Some(from.ip()) {
            port.address = Some(from.ip());
            patch.insert("address".into(), json!(from.ip().to_string()));
        }
        let name = message_name(h.message_type);
        let count = port.sent.entry(name).or_insert(0);
        *count += 1;
        let first = *count == 1;
        let role = |p: &Port| {
            if p.sent.contains_key("announce") || p.sent.contains_key("sync") {
                "master"
            } else if p.sent.contains_key("delay_req") {
                "slave"
            } else if p.sent.keys().any(|k| k.starts_with("pdelay")) {
                "peer"
            } else {
                "unknown"
            }
        };
        if first {
            patch.insert("role".into(), json!(role(port)));
            patch.insert(
                "version".into(),
                json!(format!("{}.{}", h.version, h.minor_version)),
            );
        }
        let interval_key = match h.message_type {
            ANNOUNCE => Some("log_announce_interval"),
            SYNC => Some("log_sync_interval"),
            DELAY_RESP => Some("log_min_delay_req_interval"),
            PDELAY_REQ => Some("log_min_pdelay_req_interval"),
            _ => None,
        };
        // 7Fh is "not given".
        if let (Some(k), true) = (interval_key, h.log_interval != 0x7F) {
            patch.insert(k.into(), json!(h.log_interval));
        }
        let mut reselect = false;
        match h.message_type {
            ANNOUNCE => {
                if let Some(a) = parse_announce(data, &h) {
                    let changed = port.announce.as_ref().map(|(_, old)| old) != Some(&a);
                    if changed {
                        patch.insert("announce".into(), a.json());
                        reselect = true;
                    }
                    port.announce = Some((h.clone(), a));
                }
            }
            SYNC => {
                port.sync_count += 1;
                if let Some(last) = port.last_sync_seq {
                    let gap = h.sequence.wrapping_sub(last);
                    if gap > 1 && gap < 0x8000 {
                        port.missed += u64::from(gap - 1);
                    }
                }
                port.last_sync_seq = Some(h.sequence);
                port.figures.insert("two_step".into(), json!(h.two_step()));
                if h.two_step() {
                    port.sync_pending = Some((h.sequence, h.correction_ns, now, cx.unix_millis()));
                } else if let Some(origin) = data.get(HEADER..).and_then(timestamp) {
                    port.figures
                        .insert("correction_ns".into(), json!(h.correction_ns));
                    Self::origin(port, origin, h.correction_ns, cx.unix_millis());
                }
                port.due = true;
            }
            FOLLOW_UP => {
                if let Some((seq, sync_correction, _, received)) = port.sync_pending {
                    if seq == h.sequence {
                        if let Some(origin) = data.get(HEADER..).and_then(timestamp) {
                            let correction = sync_correction + h.correction_ns;
                            port.figures.insert("two_step".into(), json!(true));
                            port.figures
                                .insert("correction_ns".into(), json!(correction));
                            Self::origin(port, origin, correction, received);
                            port.due = true;
                        }
                        port.sync_pending = None;
                    }
                }
            }
            _ => {}
        }
        if !patch.is_empty() {
            Self::port_patch(cx, domain, &key, Value::Object(patch));
        }
        if reselect {
            self.select(cx, domain);
        }
        self.report_due(cx, domain, &key, now);
    }

    /// The host clock's offset from the PTP time a Sync carried, with the
    /// grandmaster's UTC offset when its timescale is PTP.
    fn origin(port: &mut Port, (seconds, nanos): (u64, u32), correction_ns: f64, received: Millis) {
        let ptp_ms = seconds as f64 * 1000.0 + f64::from(nanos) / 1e6 + correction_ns / 1e6;
        port.figures
            .insert("origin".into(), json!(format!("{seconds}.{nanos:09}")));
        let utc_ms = match &port.announce {
            Some((_, a)) if a.flags & 0x0008 != 0 => {
                Some(ptp_ms - f64::from(a.current_utc_offset) * 1000.0)
            }
            _ => None,
        };
        match utc_ms {
            Some(utc) => {
                port.figures
                    .insert("host_offset_ms".into(), json!(received as f64 - utc));
            }
            None => {
                port.figures.remove("host_offset_ms");
            }
        }
    }

    fn report_due(&mut self, cx: &mut Cx, domain: u8, key: &str, now: Millis) {
        let every = self.report_every;
        let Some(port) = self.ports.get_mut(&(domain, key.to_string())) else {
            return;
        };
        if !port.due {
            return;
        }
        if port
            .reported_at
            .is_none_or(|at| now.saturating_sub(at) >= every)
        {
            Self::report(cx, domain, key, port, now);
        } else {
            cx.set_timer(REPORT, every);
        }
    }

    fn report(cx: &mut Cx, domain: u8, key: &str, port: &mut Port, now: Millis) {
        let mut sync = std::mem::take(&mut port.figures);
        let (since, count) = port.window;
        let elapsed = now.saturating_sub(since);
        if port.reported_at.is_some() && elapsed > 0 {
            let rate = (port.sync_count - count) as f64 * 1000.0 / elapsed as f64;
            sync.insert("rate_hz".into(), json!((rate * 100.0).round() / 100.0));
        }
        sync.insert("count".into(), json!(port.sync_count));
        sync.insert("missed".into(), json!(port.missed));
        port.window = (now, port.sync_count);
        port.reported_at = Some(now);
        port.due = false;
        Self::port_patch(cx, domain, key, json!({ "sync": sync }));
    }

    /// Choose a domain's best master among its online announcing ports.
    fn select(&mut self, cx: &mut Cx, domain: u8) {
        let best = self
            .ports
            .iter()
            .filter(|((d, _), p)| *d == domain && p.online)
            .filter_map(|((_, k), p)| p.announce.as_ref().map(|(h, a)| (a.rank(h), k, a)))
            .min_by(|x, y| x.0.cmp(&y.0));
        let d = domain.to_string();
        match best {
            Some((_, key, a)) => {
                if self.best.get(&domain) != Some(key) {
                    cx.log(
                        Level::Info,
                        format!(
                            "PTP domain {domain}: best master {key}, grandmaster {}",
                            identity_text(&a.grandmaster)
                        ),
                    );
                }
                self.best.insert(domain, key.clone());
                cx.state(json!({"domains": {d: {"best_master": key, "grandmaster": a.json()}}}));
            }
            None => {
                if self.best.remove(&domain).is_some() {
                    cx.state(
                        json!({"domains": {d: {"best_master": "", "grandmaster": Value::Null}}}),
                    );
                }
            }
        }
    }
}

impl Module for Ptp {
    fn start(&mut self, cx: &mut Cx) {
        cx.state(json!({
            "event_port": self.event_port,
            "general_port": self.general_port,
        }));
        self.listen(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, _params: &Params) {
        match name {
            "reset" => {
                self.ports.clear();
                self.best.clear();
                cx.state(json!({"domains": Value::Null}));
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
        self.received(cx, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        if !self.listening {
            return;
        }
        self.listening = false;
        let other = if socket == EVENT { GENERAL } else { EVENT };
        cx.udp_close(other);
        cx.log(Level::Warning, format!("PTP {socket} port: {message}"));
        cx.connection(Connection::Disconnected {
            reason: message.to_string(),
        });
        cx.cancel_timer(SWEEP);
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.listen(cx),
            REPORT => {
                let now = cx.now();
                let every = self.report_every;
                let mut again = false;
                for ((domain, key), port) in self.ports.iter_mut() {
                    if !port.due {
                        continue;
                    }
                    if port
                        .reported_at
                        .is_none_or(|at| now.saturating_sub(at) >= every)
                    {
                        Self::report(cx, *domain, key, port, now);
                    } else {
                        again = true;
                    }
                }
                if again {
                    cx.set_timer(REPORT, every);
                }
            }
            SWEEP if self.listening => {
                let now = cx.now();
                let mut lost = Vec::new();
                for ((domain, key), port) in self.ports.iter_mut() {
                    if port.online && now.saturating_sub(port.seen) >= self.timeout {
                        port.online = false;
                        lost.push(*domain);
                        Self::port_patch(cx, *domain, key, json!({"online": false}));
                    }
                }
                lost.sort_unstable();
                lost.dedup();
                for domain in lost {
                    self.select(cx, domain);
                }
                cx.set_timer(SWEEP, 1000);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.udp_close(EVENT);
        cx.udp_close(GENERAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::Action;
    use crate::session::merge_patch;

    fn module(settings: Value) -> Ptp {
        let spec = Catalog::source_tree().devices["ptp"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        let mut m = Ptp::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: None,
            model: "ptp".into(),
            channels: None,
            settings,
            monitor: true,
        })
        .unwrap();
        m.start(&mut Cx::new(0));
        m
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

    /// A message: the 34-byte header and a body.
    pub(crate) fn message(
        kind: u8,
        domain: u8,
        flags: u16,
        clock: [u8; 8],
        seq: u16,
        log: i8,
        body: &[u8],
    ) -> Vec<u8> {
        let mut m = vec![kind, 0x02];
        m.extend(((34 + body.len()) as u16).to_be_bytes());
        m.extend([domain, 0]);
        m.extend(flags.to_be_bytes());
        m.extend(0i64.to_be_bytes());
        m.extend([0; 4]);
        m.extend(clock);
        m.extend(1u16.to_be_bytes());
        m.extend(seq.to_be_bytes());
        m.extend([0, log as u8]);
        m.extend(body);
        m
    }

    fn announce_body(priority1: u8, class: u8, gm: [u8; 8]) -> Vec<u8> {
        let mut b = vec![0; 10];
        b.extend(37i16.to_be_bytes());
        b.extend([0, priority1, class, 0x21]);
        b.extend(0x4E5Du16.to_be_bytes());
        b.push(128);
        b.extend(gm);
        b.extend(0u16.to_be_bytes());
        b.push(0x20);
        b
    }

    const GM: [u8; 8] = [0x00, 0x1D, 0xC1, 0xFF, 0xFE, 0x0A, 0x1B, 0x2C];
    const OTHER: [u8; 8] = [0x00, 0x1D, 0xC1, 0xFF, 0xFE, 0x00, 0x00, 0x01];
    const SLAVE: [u8; 8] = [0xAC, 0xDE, 0x48, 0xFF, 0xFE, 0x00, 0x11, 0x22];

    fn heard(m: &mut Ptp, now: Millis, unix: Millis, from: [u8; 4], data: &[u8]) -> Vec<Action> {
        let mut cx = Cx::at(now, unix);
        m.datagram(
            &mut cx,
            GENERAL,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::from(from)), 320),
            data,
        );
        cx.take()
    }

    #[test]
    fn it_joins_both_groups_on_both_ports() {
        let mut m = module(json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        for socket in [EVENT, GENERAL] {
            assert!(a.contains(&Action::UdpJoin {
                socket,
                group: PRIMARY
            }));
            assert!(a.contains(&Action::UdpJoin {
                socket,
                group: PDELAY
            }));
        }
        assert!(a.contains(&Action::UdpOpen {
            socket: EVENT,
            bind: Bind::Network {
                address: None,
                port: 319
            }
        }));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
    }

    #[test]
    fn announce_messages_choose_the_grandmaster() {
        let mut m = module(json!({}));
        let a = heard(
            &mut m,
            0,
            0,
            [10, 0, 0, 2],
            &message(
                ANNOUNCE,
                0,
                0x000C,
                OTHER,
                1,
                1,
                &announce_body(128, 248, OTHER),
            ),
        );
        let s = state(&a);
        let d = &s["domains"]["0"];
        assert_eq!(d["best_master"], "00-1D-C1-FF-FE-00-00-01:1");
        let port = &d["ports"]["00-1D-C1-FF-FE-00-00-01:1"];
        assert_eq!(port["role"], "master");
        assert_eq!(port["address"], "10.0.0.2");
        assert_eq!(port["log_announce_interval"], 1);
        assert_eq!(port["announce"]["clock_class"], 248);
        // A better clock (class 6) wins.
        let a = heard(
            &mut m,
            10,
            10,
            [10, 0, 0, 1],
            &message(ANNOUNCE, 0, 0x003C, GM, 1, 1, &announce_body(128, 6, GM)),
        );
        let gm = &state(&a)["domains"]["0"]["grandmaster"];
        assert_eq!(gm["identity"], "00-1D-C1-FF-FE-0A-1B-2C");
        assert_eq!(gm["clock_class"], 6);
        assert_eq!(gm["clock_accuracy"], "0x21");
        assert_eq!(gm["clock_accuracy_text"], "100 ns");
        assert_eq!(gm["time_source_text"], "gps");
        assert_eq!(gm["current_utc_offset"], 37);
        assert_eq!(gm["ptp_timescale"], true);
        assert_eq!(gm["time_traceable"], true);
        assert_eq!(gm["leap61"], false);
        // Silent past the timeout: it is offline and the other wins again.
        heard(
            &mut m,
            5_000,
            5_000,
            [10, 0, 0, 2],
            &message(
                ANNOUNCE,
                0,
                0x000C,
                OTHER,
                2,
                1,
                &announce_body(128, 248, OTHER),
            ),
        );
        let mut cx = Cx::new(7_000);
        m.timer(&mut cx, SWEEP);
        let s = state(&cx.take());
        assert_eq!(
            s["domains"]["0"]["ports"]["00-1D-C1-FF-FE-0A-1B-2C:1"]["online"],
            false
        );
        assert_eq!(
            s["domains"]["0"]["best_master"],
            "00-1D-C1-FF-FE-00-00-01:1"
        );
    }

    #[test]
    fn two_step_sync_pairs_with_its_follow_up() {
        let mut m = module(json!({"report_interval_ms": 0}));
        heard(
            &mut m,
            0,
            0,
            [10, 0, 0, 1],
            &message(ANNOUNCE, 0, 0x000C, GM, 1, 1, &announce_body(128, 6, GM)),
        );
        // Sync, two-step, sequence 7, received at 1700000000.503 s on the
        // host's clock; its Follow_Up carries 1700000037.5 s TAI, which is
        // 1700000000.5 s UTC with the offset of 37 s: the host is 3 ms ahead.
        let mut body = vec![0x00, 0x00];
        body.extend(1_700_000_037u32.to_be_bytes());
        body.extend(500_000_000u32.to_be_bytes());
        heard(
            &mut m,
            100,
            1_700_000_000_503,
            [10, 0, 0, 1],
            &message(SYNC, 0, 0x0200, GM, 7, -3, &[0; 10]),
        );
        let a = heard(
            &mut m,
            101,
            1_700_000_000_504,
            [10, 0, 0, 1],
            &message(FOLLOW_UP, 0, 0, GM, 7, -3, &body),
        );
        let s = state(&a);
        let sync = &s["domains"]["0"]["ports"]["00-1D-C1-FF-FE-0A-1B-2C:1"]["sync"];
        assert_eq!(sync["origin"], "1700000037.500000000");
        assert_eq!(sync["host_offset_ms"], 3.0);
        assert_eq!(sync["two_step"], true);
        // A skipped sequence number is counted.
        heard(
            &mut m,
            200,
            0,
            [10, 0, 0, 1],
            &message(SYNC, 0, 0x0200, GM, 9, -3, &[0; 10]),
        );
        let mut cx = Cx::new(2000);
        m.timer(&mut cx, REPORT);
        let a = heard(
            &mut m,
            2000,
            0,
            [10, 0, 0, 1],
            &message(SYNC, 0, 0x0200, GM, 10, -3, &[0; 10]),
        );
        let s = state(&a);
        assert_eq!(
            s["domains"]["0"]["ports"]["00-1D-C1-FF-FE-0A-1B-2C:1"]["sync"]["missed"],
            1
        );
    }

    #[test]
    fn slaves_and_other_domains_are_told_apart() {
        let mut m = module(json!({"domains": "127"}));
        let a = heard(
            &mut m,
            0,
            0,
            [10, 0, 0, 9],
            &message(DELAY_REQ, 127, 0, SLAVE, 1, 0x7F, &[0; 10]),
        );
        let port = &state(&a)["domains"]["127"]["ports"]["AC-DE-48-FF-FE-00-11-22:1"];
        assert_eq!(port["role"], "slave");
        // The default domain is not kept here.
        let a = heard(
            &mut m,
            0,
            0,
            [10, 0, 0, 1],
            &message(SYNC, 0, 0, GM, 1, 0, &[0; 10]),
        );
        assert!(a.is_empty());
        // Version 1 messages and short datagrams are not PTP version 2.
        let mut v1 = message(SYNC, 127, 0, GM, 1, 0, &[0; 10]);
        v1[1] = 1;
        assert!(heard(&mut m, 0, 0, [10, 0, 0, 1], &v1).is_empty());
        assert!(heard(&mut m, 0, 0, [10, 0, 0, 1], &[0; 20]).is_empty());
    }

    #[test]
    fn reset_forgets_what_was_heard() {
        let mut m = module(json!({}));
        heard(
            &mut m,
            0,
            0,
            [10, 0, 0, 1],
            &message(ANNOUNCE, 0, 0, GM, 1, 1, &announce_body(128, 6, GM)),
        );
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "reset", &Params::new());
        let a = cx.take();
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));
        assert!(m.ports.is_empty());
    }
}
