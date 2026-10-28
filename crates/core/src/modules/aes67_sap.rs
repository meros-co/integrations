//! AES67 stream discovery: Session Announcement Protocol (RFC 2974)
//! announcements of Session Description Protocol descriptions (RFC 8866),
//! the way AES67 senders, Dante devices in AES67 mode and RAVENNA devices
//! announce their multicast streams.
//!
//! - One socket on the SAP port (9875) that hears the group (239.255.255.255,
//!   the IPv4 Local Scope's SAP address, by default) on every interface the
//!   core uses. The core is the listener: there is no host.
//! - A stream is known by its SDP origin less the version (username, session
//!   id and unicast address), as RFC 8866 identifies a session; each is kept
//!   in state with its name, its media (address, port, encoding, rate,
//!   channels, packet time, clocks) and its whole description.
//! - An announcement is told apart from a repeat by its message identifier
//!   hash and originating source: a repeat only renews the stream. A stream
//!   goes when its announcer deletes it (from the same originating source),
//!   when its `t=` stop time passes, or when it has not been heard for ten
//!   of its announcement periods or the timeout setting (an hour, as RFC
//!   2974 sets), whichever is longer.
//! - Announcing is the other half: a description given to `announce` is sent
//!   now and every interval until withdrawn, and withdrawing or closing the
//!   integration sends its deletion.
//! - Encrypted announcements and payloads other than `application/sdp` are
//!   skipped; compressed ones (zlib) are inflated. Authentication data is
//!   skipped, not checked.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};

const SOCKET: Key = "sap";
const RETRY: Key = "retry";
const ANNOUNCE: Key = "announce";
const EXPIRE: Key = "expire";
/// The first retry of a port that cannot be had, doubling to the most.
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

pub(crate) const DEFAULT_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 255);
pub(crate) const DEFAULT_PORT: u16 = 9875;
/// How often streams are checked for expiry.
const EXPIRE_EVERY: Millis = 5_000;
/// Streams kept at once; announcements of more are skipped.
const MAX_STREAMS: usize = 1024;
/// The largest description inflated from a compressed announcement.
const MAX_INFLATED: usize = 64 * 1024;
/// Seconds between the NTP epoch (1900) and the Unix epoch (1970).
const NTP_UNIX_OFFSET: u64 = 2_208_988_800;
const MIME_SDP: &str = "application/sdp";

/// A SAP packet's header and payload, as RFC 2974 section 6 lays it out.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sap {
    pub deletion: bool,
    pub hash: u16,
    pub source: IpAddr,
    /// The description (or, for a deletion, at least its `o=` line).
    pub sdp: String,
}

/// Read one SAP packet. `Err` says why it is skipped.
pub(crate) fn parse_sap(data: &[u8]) -> Result<Sap, String> {
    if data.len() < 4 {
        return Err("shorter than a SAP header".into());
    }
    let flags = data[0];
    let version = flags >> 5;
    if version != 1 {
        return Err(format!("SAP version {version}, not 1"));
    }
    let ipv6 = flags & 0x10 != 0;
    let deletion = flags & 0x04 != 0;
    let encrypted = flags & 0x02 != 0;
    let compressed = flags & 0x01 != 0;
    let auth_len = data[1] as usize * 4;
    let hash = u16::from_be_bytes([data[2], data[3]]);
    let source_len = if ipv6 { 16 } else { 4 };
    let start = 4 + source_len + auth_len;
    if data.len() < start {
        return Err("shorter than its header says".into());
    }
    let source = if ipv6 {
        let mut b = [0u8; 16];
        b.copy_from_slice(&data[4..20]);
        IpAddr::V6(Ipv6Addr::from(b))
    } else {
        IpAddr::V4(Ipv4Addr::new(data[4], data[5], data[6], data[7]))
    };
    if encrypted {
        return Err("encrypted".into());
    }
    let inflated;
    let mut payload = &data[start..];
    if compressed {
        inflated = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(payload, MAX_INFLATED)
            .map_err(|e| format!("cannot inflate: {e:?}"))?;
        payload = &inflated;
    }
    // The payload type may be left out for SDP, which then starts at once.
    if !payload.starts_with(b"v=0") && !payload.starts_with(b"o=") {
        let Some(nul) = payload.iter().position(|&b| b == 0) else {
            return Err("no payload type and not SDP".into());
        };
        let mime = String::from_utf8_lossy(&payload[..nul]);
        if !mime.trim().eq_ignore_ascii_case(MIME_SDP) {
            return Err(format!("payload type '{mime}', not {MIME_SDP}"));
        }
        payload = &payload[nul + 1..];
    }
    Ok(Sap {
        deletion,
        hash,
        source,
        sdp: String::from_utf8_lossy(payload).into_owned(),
    })
}

/// Build a SAP packet with the `application/sdp` payload type.
pub(crate) fn build_sap(deletion: bool, hash: u16, source: IpAddr, sdp: &str) -> Vec<u8> {
    let mut flags = 0x20;
    if source.is_ipv6() {
        flags |= 0x10;
    }
    if deletion {
        flags |= 0x04;
    }
    let mut out = vec![flags, 0];
    out.extend_from_slice(&hash.to_be_bytes());
    match source {
        IpAddr::V4(v4) => out.extend_from_slice(&v4.octets()),
        IpAddr::V6(v6) => out.extend_from_slice(&v6.octets()),
    }
    out.extend_from_slice(MIME_SDP.as_bytes());
    out.push(0);
    out.extend_from_slice(sdp.as_bytes());
    out
}

/// A description's origin (`o=`), RFC 8866 section 5.2.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Origin {
    pub username: String,
    pub session_id: String,
    pub version: String,
    pub nettype: String,
    pub addrtype: String,
    pub address: String,
}

impl Origin {
    fn parse(value: &str) -> Option<Origin> {
        let f: Vec<&str> = value.split_whitespace().collect();
        if f.len() != 6 {
            return None;
        }
        Some(Origin {
            username: f[0].into(),
            session_id: f[1].into(),
            version: f[2].into(),
            nettype: f[3].into(),
            addrtype: f[4].into(),
            address: f[5].into(),
        })
    }

    /// The stream's key in state: the session's identity less the version.
    pub fn key(&self) -> String {
        if self.username == "-" {
            format!("{}:{}", self.address, self.session_id)
        } else {
            format!("{}@{}:{}", self.username, self.address, self.session_id)
        }
    }

    fn line(&self) -> String {
        format!(
            "o={} {} {} {} {} {}",
            self.username, self.session_id, self.version, self.nettype, self.addrtype, self.address
        )
    }
}

/// The `o=` line of a description or deletion payload.
fn origin_of(sdp: &str) -> Option<Origin> {
    sdp.lines()
        .map(|l| l.trim_end_matches('\r'))
        .find_map(|l| l.strip_prefix("o=").and_then(Origin::parse))
}

/// A parsed description: its origin, its state, and its latest stop time
/// (Unix milliseconds; `None` when unbounded).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Description {
    pub origin: Origin,
    pub state: Value,
    pub stop_unix_ms: Option<u64>,
}

fn number(text: &str) -> Value {
    if let Ok(n) = text.parse::<i64>() {
        return json!(n);
    }
    match text.parse::<f64>() {
        Ok(f) if f.is_finite() => json!(f),
        _ => json!(text),
    }
}

/// `c=IN IP4 239.69.11.44/32[/2]` or `c=IN IP6 ff15::1[/2]`.
fn connection(value: &str) -> Value {
    let f: Vec<&str> = value.split_whitespace().collect();
    if f.len() != 3 {
        return json!({ "raw": value });
    }
    let mut parts = f[2].split('/');
    let address = parts.next().unwrap_or_default();
    let mut out = Map::new();
    out.insert("address_type".into(), json!(f[1]));
    out.insert("address".into(), json!(address));
    let rest: Vec<&str> = parts.collect();
    let (ttl, count) = if f[1].eq_ignore_ascii_case("IP4") {
        (rest.first(), rest.get(1))
    } else {
        (None, rest.first())
    };
    if let Some(ttl) = ttl.and_then(|t| t.parse::<u64>().ok()) {
        out.insert("ttl".into(), json!(ttl));
    }
    out.insert(
        "count".into(),
        json!(count.and_then(|c| c.parse::<u64>().ok()).unwrap_or(1)),
    );
    Value::Object(out)
}

/// `a=ts-refclk:` (RFC 7273 section 4.8). Audinate's sample writes the PTP
/// domain as a bare number after the grandmaster, which is read too.
fn refclk(value: &str) -> Value {
    let (kind, rest) = value.split_once('=').unwrap_or((value, ""));
    let mut out = Map::new();
    out.insert("source".into(), json!(kind));
    match kind {
        "ptp" => {
            let mut parts = rest.splitn(3, ':');
            out.insert("version".into(), json!(parts.next().unwrap_or_default()));
            match parts.next() {
                Some("traceable") => {
                    out.insert("traceable".into(), json!(true));
                }
                Some(gm) => {
                    out.insert("grandmaster".into(), json!(gm));
                }
                None => {}
            }
            if let Some(domain) = parts.next() {
                let n = domain.strip_prefix("domain-nmbr=").unwrap_or(domain);
                match n.parse::<u64>() {
                    Ok(n) => {
                        out.insert("domain".into(), json!(n));
                    }
                    Err(_) => {
                        let name = domain.strip_prefix("domain-name=").unwrap_or(domain);
                        out.insert("domain_name".into(), json!(name));
                    }
                }
            }
        }
        "ntp" => {
            if rest == "/traceable/" {
                out.insert("traceable".into(), json!(true));
            } else {
                out.insert("server".into(), json!(rest));
            }
        }
        _ => {
            if !rest.is_empty() {
                out.insert("value".into(), json!(rest));
            }
            if kind.starts_with("private") && kind.ends_with(":traceable") {
                out.insert("source".into(), json!("private"));
                out.insert("traceable".into(), json!(true));
            }
        }
    }
    Value::Object(out)
}

/// `a=mediaclk:` (RFC 7273 section 5): `direct=0`, `direct=0 rate=1001/1000`,
/// `sender`, `IEEE1722=...`.
fn mediaclk(value: &str) -> Value {
    let mut out = Map::new();
    let mut words = value.split_whitespace().peekable();
    if let Some(id) = words.peek().and_then(|w| w.strip_prefix("id=")) {
        out.insert("id".into(), json!(id));
        words.next();
    }
    if let Some(clock) = words.next() {
        let (kind, arg) = clock.split_once('=').unwrap_or((clock, ""));
        out.insert("source".into(), json!(kind));
        if kind == "direct" && !arg.is_empty() {
            out.insert("offset".into(), number(arg));
        } else if !arg.is_empty() {
            out.insert("value".into(), json!(arg));
        }
    }
    for w in words {
        if let Some(rate) = w.strip_prefix("rate=") {
            out.insert("rate".into(), json!(rate));
        }
    }
    Value::Object(out)
}

/// `a=source-filter: incl IN IP4 239.69.11.44 10.0.1.2` (RFC 4570).
fn source_filter(value: &str) -> Value {
    let f: Vec<&str> = value.split_whitespace().collect();
    if f.len() < 5 {
        return json!({ "raw": value.trim() });
    }
    json!({
        "mode": f[0],
        "address_type": f[2],
        "destination": f[3],
        "sources": f[4..],
    })
}

/// Settings of one level (session or media) built up from its lines.
#[derive(Default)]
struct Section {
    map: Map<String, Value>,
    attributes: Vec<Value>,
    clocks: Vec<Value>,
    filters: Vec<Value>,
    bandwidths: Map<String, Value>,
    rtpmap: Map<String, Value>,
    fmtp: Map<String, Value>,
}

impl Section {
    fn attribute(&mut self, value: &str) {
        self.attributes.push(json!(value));
        let (name, arg) = match value.split_once(':') {
            Some((n, a)) => (n, Some(a)),
            None => (value, None),
        };
        match (name, arg) {
            ("recvonly" | "sendonly" | "sendrecv" | "inactive", None) => {
                self.map.insert("direction".into(), json!(name));
            }
            ("ts-refclk", Some(a)) => self.clocks.push(refclk(a.trim())),
            ("mediaclk", Some(a)) => {
                self.map.insert("media_clock".into(), mediaclk(a.trim()));
            }
            ("source-filter", Some(a)) => self.filters.push(source_filter(a)),
            ("ptime", Some(a)) => {
                self.map.insert("ptime_ms".into(), number(a.trim()));
            }
            ("maxptime", Some(a)) => {
                self.map.insert("maxptime_ms".into(), number(a.trim()));
            }
            ("framecount", Some(a)) => {
                self.map.insert("framecount".into(), number(a.trim()));
            }
            ("tool", Some(a)) => {
                self.map.insert("tool".into(), json!(a));
            }
            ("keywds", Some(a)) => {
                self.map.insert("keywords".into(), json!(a));
            }
            ("mid", Some(a)) => {
                self.map.insert("mid".into(), json!(a.trim()));
            }
            ("sync-time", Some(a)) => {
                self.map.insert("sync_time".into(), number(a.trim()));
            }
            ("clock-deviation", Some(a)) => {
                self.map.insert("clock_deviation".into(), json!(a.trim()));
            }
            ("clock-domain", Some(a)) => {
                let mut f = a.split_whitespace();
                let protocol = f.next().unwrap_or_default();
                let mut d = json!({ "protocol": protocol });
                if let Some(n) = f.next() {
                    d["domain"] = number(n);
                }
                self.map.insert("clock_domain".into(), d);
            }
            ("group", Some(a)) => {
                let mut f = a.split_whitespace();
                let semantics = f.next().unwrap_or_default();
                let mids: Vec<&str> = f.collect();
                let groups = self.map.entry("groups").or_insert_with(|| json!([]));
                if let Value::Array(g) = groups {
                    g.push(json!({ "semantics": semantics, "mids": mids }));
                }
            }
            ("rtpmap", Some(a)) => {
                if let Some((pt, enc)) = a.trim().split_once(' ') {
                    let mut p = enc.split('/');
                    let mut m = Map::new();
                    m.insert("encoding".into(), json!(p.next().unwrap_or_default()));
                    if let Some(rate) = p.next().and_then(|r| r.parse::<u64>().ok()) {
                        m.insert("clock_rate".into(), json!(rate));
                    }
                    let channels = p.next().and_then(|c| c.parse::<u64>().ok()).unwrap_or(1);
                    m.insert("channels".into(), json!(channels));
                    self.rtpmap.insert(pt.trim().into(), Value::Object(m));
                }
            }
            ("fmtp", Some(a)) => {
                if let Some((pt, params)) = a.trim().split_once(' ') {
                    self.fmtp.insert(pt.trim().into(), json!(params.trim()));
                }
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Map<String, Value> {
        if !self.clocks.is_empty() {
            self.map.insert("clocks".into(), Value::Array(self.clocks));
        }
        if !self.filters.is_empty() {
            self.map
                .insert("source_filters".into(), Value::Array(self.filters));
        }
        if !self.bandwidths.is_empty() {
            self.map
                .insert("bandwidths".into(), Value::Object(self.bandwidths));
        }
        if !self.rtpmap.is_empty() {
            self.map.insert("rtpmap".into(), Value::Object(self.rtpmap));
        }
        if !self.fmtp.is_empty() {
            self.map.insert("fmtp".into(), Value::Object(self.fmtp));
        }
        self.map
            .insert("attributes".into(), Value::Array(self.attributes));
        self.map
    }
}

/// Read a description into the state of its stream.
pub(crate) fn parse_sdp(sdp: &str) -> Result<Description, String> {
    let mut origin = None;
    let mut session = Section::default();
    let mut media: Vec<Section> = Vec::new();
    let mut times = Vec::new();
    let mut repeats = Vec::new();
    let mut emails = Vec::new();
    let mut phones = Vec::new();
    let mut version_seen = false;
    for line in sdp.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let bytes = line.as_bytes();
        if bytes.len() < 2 || bytes[1] != b'=' {
            continue;
        }
        let kind = bytes[0];
        let value = &line[2..];
        let level = media.last_mut().unwrap_or(&mut session);
        match kind {
            b'v' => {
                if value.trim() != "0" {
                    return Err(format!("SDP version {value}, not 0"));
                }
                version_seen = true;
            }
            b'o' => {
                origin = Some(Origin::parse(value).ok_or("an o= line without six fields")?);
            }
            b's' => {
                session.map.insert("name".into(), json!(value));
            }
            b'i' => {
                level.map.insert("info".into(), json!(value));
            }
            b'u' => {
                session.map.insert("uri".into(), json!(value));
            }
            b'e' => emails.push(json!(value)),
            b'p' => phones.push(json!(value)),
            b'c' => {
                level.map.insert("connection".into(), connection(value));
            }
            b'b' => {
                if let Some((t, v)) = value.split_once(':') {
                    level.bandwidths.insert(t.into(), number(v.trim()));
                }
            }
            b't' => {
                let mut f = value.split_whitespace();
                let start = f.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
                let stop = f.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
                times.push((start, stop));
            }
            b'r' => repeats.push(json!(value)),
            b'a' => level.attribute(value),
            b'm' => {
                let f: Vec<&str> = value.split_whitespace().collect();
                let mut m = Section::default();
                if f.len() >= 3 {
                    let (port, ports) = match f[1].split_once('/') {
                        Some((p, n)) => (p, n.parse::<u64>().unwrap_or(1)),
                        None => (f[1], 1),
                    };
                    m.map.insert("media".into(), json!(f[0]));
                    m.map.insert("port".into(), number(port));
                    m.map.insert("port_count".into(), json!(ports));
                    m.map.insert("protocol".into(), json!(f[2]));
                    m.map.insert("formats".into(), json!(f[3..]));
                } else {
                    m.map.insert("raw".into(), json!(value));
                }
                media.push(m);
            }
            _ => {}
        }
    }
    if !version_seen {
        return Err("no v=0 line".into());
    }
    let origin = origin.ok_or("no o= line")?;
    let stop_unix_ms = if times.is_empty() || times.iter().any(|&(_, stop)| stop == 0) {
        None
    } else {
        times
            .iter()
            .map(|&(_, stop)| stop.saturating_sub(NTP_UNIX_OFFSET) * 1000)
            .max()
    };
    let mut state = session.finish();
    state.insert(
        "origin".into(),
        json!({
            "username": origin.username,
            "session_id": origin.session_id,
            "version": origin.version,
            "network_type": origin.nettype,
            "address_type": origin.addrtype,
            "address": origin.address,
        }),
    );
    state.insert(
        "times".into(),
        Value::Array(
            times
                .iter()
                .map(|&(start, stop)| json!({ "start": start, "stop": stop }))
                .collect(),
        ),
    );
    if !repeats.is_empty() {
        state.insert("repeats".into(), Value::Array(repeats));
    }
    if !emails.is_empty() {
        state.insert("emails".into(), Value::Array(emails));
    }
    if !phones.is_empty() {
        state.insert("phones".into(), Value::Array(phones));
    }
    let session_connection = state.get("connection").cloned();
    let media: Vec<Value> = media
        .into_iter()
        .map(|m| {
            let mut m = m.finish();
            // The first format's rtpmap names the encoding, rate and
            // channels; a static payload type has none.
            let first = m
                .get("formats")
                .and_then(|f| f.get(0))
                .and_then(Value::as_str)
                .map(str::to_string);
            if let Some(map) =
                first.and_then(|pt| m.get("rtpmap").and_then(|r| r.get(&pt)).cloned())
            {
                for (k, v) in map.as_object().into_iter().flatten() {
                    m.insert(k.clone(), v.clone());
                }
            }
            if !m.contains_key("connection") {
                if let Some(c) = &session_connection {
                    m.insert("connection".into(), c.clone());
                }
            }
            Value::Object(m)
        })
        .collect();
    // The second leg of a redundant pair (a=group:DUP, RFC 7104) carries
    // the same channels as the first.
    let copies: Vec<&str> = state
        .get("groups")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|g| g["semantics"] == "DUP")
        .filter_map(|g| g["mids"].as_array())
        .flat_map(|mids| mids.iter().skip(1).filter_map(Value::as_str))
        .collect();
    let channels: u64 = media
        .iter()
        .filter(|m| m.get("media").and_then(Value::as_str) == Some("audio"))
        .filter(|m| {
            !m.get("mid")
                .and_then(Value::as_str)
                .is_some_and(|mid| copies.contains(&mid))
        })
        .filter_map(|m| m.get("channels").and_then(Value::as_u64))
        .sum();
    state.insert("channels".into(), json!(channels));
    state.insert("media".into(), Value::Array(media));
    state.insert("sdp".into(), json!(sdp));
    Ok(Description {
        origin,
        state: Value::Object(state),
        stop_unix_ms,
    })
}

/// A 16-bit message identifier hash of a description, never 0 (which
/// listeners may discard).
fn hash16(text: &str) -> u16 {
    let mut h: u32 = 0x811c_9dc5;
    for b in text.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    match ((h >> 16) ^ (h & 0xffff)) as u16 {
        0 => 1,
        n => n,
    }
}

/// A description with CRLF line ends, as RFC 8866 writes them.
fn crlf(sdp: &str) -> String {
    let mut out = String::with_capacity(sdp.len() + 32);
    for line in sdp.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out
}

struct Heard {
    hash: u16,
    announcer: IpAddr,
    last: Millis,
    /// The time between its last two announcements.
    period: Option<Millis>,
    stop_unix_ms: Option<u64>,
}

struct Own {
    origin: Origin,
    source: IpAddr,
    hash: u16,
    sdp: String,
}

pub(crate) struct Aes67Sap {
    group: Ipv4Addr,
    port: u16,
    timeout: Millis,
    interval: Millis,
    listening: bool,
    retry_after: Millis,
    streams: HashMap<String, Heard>,
    own: Vec<(String, Own)>,
    full_logged: bool,
}

impl Aes67Sap {
    pub(crate) fn new(ctx: OpenContext) -> Result<Aes67Sap, String> {
        let s = &ctx.settings;
        let group = match s.get("group").and_then(Value::as_str) {
            None | Some("") => DEFAULT_GROUP,
            Some(g) => g
                .trim()
                .parse::<Ipv4Addr>()
                .ok()
                .filter(Ipv4Addr::is_multicast)
                .ok_or_else(|| format!("group '{g}' is not an IPv4 multicast address"))?,
        };
        let seconds = |name: &str, default: u64| -> Result<Millis, String> {
            match s.get(name) {
                None | Some(Value::Null) => Ok(default * 1000),
                Some(v) => v
                    .as_f64()
                    .filter(|n| *n >= 1.0)
                    .map(|n| (n * 1000.0) as Millis)
                    .ok_or_else(|| format!("{name} must be at least 1 second")),
            }
        };
        Ok(Aes67Sap {
            group,
            port: ctx.port.unwrap_or(DEFAULT_PORT),
            timeout: seconds("timeout", 3600)?,
            interval: seconds("announce_interval", 30)?,
            listening: false,
            retry_after: RETRY_MIN,
            streams: HashMap::new(),
            own: Vec::new(),
            full_logged: false,
        })
    }

    fn destination(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(self.group), self.port)
    }

    fn listen(&mut self, cx: &mut Cx) {
        self.listening = true;
        cx.connection(Connection::Connected);
        cx.udp_open(
            SOCKET,
            Bind::Network {
                address: None,
                port: self.port,
            },
        );
        cx.udp_join(SOCKET, self.group);
    }

    fn count(&self, patch: &mut Map<String, Value>) {
        patch.insert("stream_count".into(), json!(self.streams.len()));
    }

    fn remove(&mut self, cx: &mut Cx, key: &str, why: &str) {
        if self.streams.remove(key).is_some() {
            cx.log(Level::Debug, format!("stream {key} removed: {why}"));
            let mut patch = Map::new();
            patch.insert("streams".into(), json!({ key: Value::Null }));
            self.count(&mut patch);
            cx.state(Value::Object(patch));
        }
    }

    fn packet(&mut self, cx: &mut Cx, from: SocketAddr, data: &[u8]) {
        let sap = match parse_sap(data) {
            Ok(sap) => sap,
            Err(why) => {
                cx.log(
                    Level::Debug,
                    format!("skipped a SAP packet from {from}: {why}"),
                );
                return;
            }
        };
        let now = cx.now();
        if sap.deletion {
            let Some(origin) = origin_of(&sap.sdp) else {
                cx.log(
                    Level::Debug,
                    format!("a deletion from {from} without an o= line"),
                );
                return;
            };
            let key = origin.key();
            match self.streams.get(&key) {
                Some(heard) if heard.announcer == sap.source => {
                    self.remove(cx, &key, "deleted by its announcer");
                }
                Some(_) => cx.log(
                    Level::Debug,
                    format!("ignored a deletion of {key} from another originating source"),
                ),
                None => {}
            }
            return;
        }
        // A repeat only renews the stream.
        if let Some(origin) = origin_of(&sap.sdp) {
            if let Some(heard) = self.streams.get_mut(&origin.key()) {
                if sap.hash != 0 && heard.hash == sap.hash && heard.announcer == sap.source {
                    heard.period = Some(now.saturating_sub(heard.last));
                    heard.last = now;
                    return;
                }
            }
        }
        let description = match parse_sdp(&sap.sdp) {
            Ok(d) => d,
            Err(why) => {
                cx.log(
                    Level::Debug,
                    format!("skipped a description from {from}: {why}"),
                );
                return;
            }
        };
        let key = description.origin.key();
        let previous = self.streams.get(&key).map(|p| p.last);
        if previous.is_none() && self.streams.len() >= MAX_STREAMS {
            if !self.full_logged {
                self.full_logged = true;
                cx.log(
                    Level::Warning,
                    format!("{MAX_STREAMS} streams are kept; announcements of more are skipped"),
                );
            }
            return;
        }
        let period = previous.map(|last| now.saturating_sub(last));
        self.streams.insert(
            key.clone(),
            Heard {
                hash: sap.hash,
                announcer: sap.source,
                last: now,
                period,
                stop_unix_ms: description.stop_unix_ms,
            },
        );
        let mut stream = description.state;
        stream["announcer"] = json!(sap.source.to_string());
        stream["sender"] = json!(from.ip().to_string());
        stream["message_hash"] = json!(sap.hash);
        stream["local"] = json!(self.own.iter().any(|(k, _)| *k == key));
        // Replace the whole entry: lists and fields that went must go too.
        let mut patch = Map::new();
        if previous.is_some() {
            cx.state(json!({ "streams": { key.clone(): Value::Null } }));
        }
        patch.insert("streams".into(), json!({ key: stream }));
        self.count(&mut patch);
        cx.state(Value::Object(patch));
    }

    fn expire(&mut self, cx: &mut Cx) {
        let now = cx.now();
        let unix = cx.unix_millis();
        let mut gone = Vec::new();
        for (key, heard) in &self.streams {
            let limit = self
                .timeout
                .max(heard.period.unwrap_or(0).saturating_mul(10));
            if now.saturating_sub(heard.last) > limit {
                gone.push((key.clone(), "not announced within its timeout"));
            } else if heard.stop_unix_ms.is_some_and(|stop| unix > stop) {
                gone.push((key.clone(), "its stop time has passed"));
            }
        }
        for (key, why) in gone {
            self.remove(cx, &key, why);
        }
    }

    /// The next announcement time: the interval, less or more a third at
    /// random (RFC 2974 section 3.1), so announcers do not keep step.
    fn next_announce(&self, cx: &Cx) -> Millis {
        let span = (self.interval * 2 / 3).max(1);
        let mut x = cx.now() ^ cx.unix_millis().rotate_left(17) ^ 0x9e37_79b9_7f4a_7c15;
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        (self.interval - self.interval / 3 + x % span).max(1)
    }

    fn send_all(&self, cx: &mut Cx) {
        let to = self.destination();
        for (_, own) in &self.own {
            cx.udp_send(SOCKET, to, build_sap(false, own.hash, own.source, &own.sdp));
        }
    }

    fn announce(&mut self, cx: &mut Cx, params: &Params) -> Result<Outcome, CommandError> {
        let invalid = |message: String| CommandError::InvalidParams { message };
        let sdp = params.get("sdp").and_then(Value::as_str).unwrap_or("");
        let sdp = crlf(sdp);
        let description =
            parse_sdp(&sdp).map_err(|e| invalid(format!("not a description: {e}")))?;
        let source = description.origin.address.parse::<IpAddr>().map_err(|_| {
            invalid(
                "the o= line must give the sender's IP address, which SAP announces it from".into(),
            )
        })?;
        if !self.listening {
            return Err(CommandError::NotConnected);
        }
        let key = description.origin.key();
        let own = Own {
            origin: description.origin,
            source,
            hash: hash16(&sdp),
            sdp,
        };
        cx.udp_send(
            SOCKET,
            self.destination(),
            build_sap(false, own.hash, own.source, &own.sdp),
        );
        let first = self.own.is_empty();
        match self.own.iter_mut().find(|(k, _)| *k == key) {
            Some((_, existing)) => *existing = own,
            None => self.own.push((key.clone(), own)),
        }
        if self.streams.contains_key(&key) {
            cx.state(json!({ "streams": { key.clone(): { "local": true } } }));
        }
        if first {
            let next = self.next_announce(cx);
            cx.set_timer(ANNOUNCE, next);
        }
        cx.state(
            json!({ "announced": self.own.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>() }),
        );
        Ok(Outcome::Value { value: json!(key) })
    }

    fn withdraw(&mut self, cx: &mut Cx, params: &Params) -> Result<Outcome, CommandError> {
        let key = params.get("stream").and_then(Value::as_str).unwrap_or("");
        let Some(at) = self.own.iter().position(|(k, _)| k == key) else {
            return Err(CommandError::InvalidParams {
                message: format!("'{key}' is not announced by this integration"),
            });
        };
        let (key, own) = self.own.remove(at);
        if self.listening {
            cx.udp_send(SOCKET, self.destination(), deletion(&own));
        }
        if self.own.is_empty() {
            cx.cancel_timer(ANNOUNCE);
        }
        self.remove(cx, &key, "withdrawn here");
        cx.state(
            json!({ "announced": self.own.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>() }),
        );
        Ok(Outcome::Unverified)
    }
}

/// A deletion of one of this integration's announcements: the same hash and
/// source, and the description's `o=` line as payload (RFC 2974 section 4).
fn deletion(own: &Own) -> Vec<u8> {
    build_sap(
        true,
        own.hash,
        own.source,
        &format!("{}\r\n", own.origin.line()),
    )
}

impl Module for Aes67Sap {
    fn start(&mut self, cx: &mut Cx) {
        cx.state(json!({
            "group": self.group.to_string(),
            "port": self.port,
            "stream_count": 0,
            "announced": [],
        }));
        self.listen(cx);
        cx.set_timer(EXPIRE, EXPIRE_EVERY);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        let result = match name {
            "announce" => self.announce(cx, params),
            "withdraw" => self.withdraw(cx, params),
            other => Err(CommandError::UnknownCommand {
                command: other.into(),
            }),
        };
        cx.complete(id, result);
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        cx.alive();
        self.packet(cx, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        self.listening = false;
        cx.log(Level::Warning, format!("SAP socket: {message}"));
        cx.connection(Connection::Disconnected {
            reason: message.to_string(),
        });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => {
                self.listen(cx);
                if !self.own.is_empty() {
                    self.send_all(cx);
                }
            }
            ANNOUNCE => {
                if self.listening {
                    self.send_all(cx);
                }
                if !self.own.is_empty() {
                    let next = self.next_announce(cx);
                    cx.set_timer(ANNOUNCE, next);
                }
            }
            EXPIRE => {
                self.expire(cx);
                cx.set_timer(EXPIRE, EXPIRE_EVERY);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.listening {
            let to = self.destination();
            for (_, own) in &self.own {
                cx.udp_send(SOCKET, to, deletion(own));
            }
        }
        cx.udp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;

    /// Audinate's sample multicast description (Dante Domain Manager user
    /// guide, "Sample SDP Specification").
    const SAMPLE: &str = "v=0\r\no=- 123456 123458 IN IP4 10.0.1.2\r\ns=My sample flow\r\n\
        i=4 channels: c1, c2, c3, c4\r\nt=0 0\r\na=recvonly\r\nm=audio 5004 RTP/AVP 98\r\n\
        c=IN IP4 239.69.11.44/32\r\na=rtpmap:98 L24/48000/4\r\na=ptime:1\r\n\
        a=ts-refclk:ptp=IEEE1588-2008:00-11-22-FF-FE-33-44-55:0\r\na=mediaclk:direct=0\r\n";

    const ANNOUNCER: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 1, 2));

    fn sap(settings: Value) -> Aes67Sap {
        Aes67Sap::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: None,
            model: "generic".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: true,
        })
        .unwrap()
    }

    fn started() -> Aes67Sap {
        let mut s = sap(json!({}));
        let mut cx = Cx::new(0);
        s.start(&mut cx);
        s
    }

    fn from() -> SocketAddr {
        SocketAddr::new(ANNOUNCER, 9875)
    }

    /// Every state patch, merged in order as a consumer would.
    fn merged(actions: &[Action], into: &mut Value) {
        fn merge(target: &mut Value, patch: &Value) {
            if let Value::Object(p) = patch {
                if !target.is_object() {
                    *target = json!({});
                }
                let t = target.as_object_mut().unwrap();
                for (k, v) in p {
                    if v.is_null() {
                        t.remove(k);
                    } else {
                        merge(t.entry(k.clone()).or_insert(Value::Null), v);
                    }
                }
            } else {
                *target = patch.clone();
            }
        }
        for a in actions {
            if let Action::State(p) = a {
                merge(into, p);
            }
        }
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

    #[test]
    fn listens_on_the_local_scope_sap_group() {
        let mut s = sap(json!({}));
        let mut cx = Cx::new(0);
        s.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Network {
                address: None,
                port: 9875
            }
        }));
        assert!(a.contains(&Action::UdpJoin {
            socket: SOCKET,
            group: Ipv4Addr::new(239, 255, 255, 255)
        }));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(Aes67Sap::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: None,
            model: "generic".into(),
            channels: None,
            settings: json!({"group": "10.0.0.1"}).as_object().unwrap().clone(),
            monitor: true,
        })
        .is_err());
    }

    #[test]
    fn the_sample_description_is_read_into_a_stream() {
        let mut s = started();
        let mut cx = Cx::new(10);
        s.datagram(
            &mut cx,
            SOCKET,
            from(),
            &build_sap(false, 0x1234, ANNOUNCER, SAMPLE),
        );
        let mut state = json!({});
        merged(&cx.take(), &mut state);
        assert_eq!(state["stream_count"], 1);
        let stream = &state["streams"]["10.0.1.2:123456"];
        assert_eq!(stream["name"], "My sample flow");
        assert_eq!(stream["info"], "4 channels: c1, c2, c3, c4");
        assert_eq!(stream["direction"], "recvonly");
        assert_eq!(stream["channels"], 4);
        assert_eq!(stream["origin"]["version"], "123458");
        assert_eq!(stream["announcer"], "10.0.1.2");
        assert_eq!(stream["message_hash"], 0x1234);
        assert_eq!(stream["local"], false);
        assert_eq!(stream["times"], json!([{"start": 0, "stop": 0}]));
        let m = &stream["media"][0];
        assert_eq!(m["media"], "audio");
        assert_eq!(m["port"], 5004);
        assert_eq!(m["protocol"], "RTP/AVP");
        assert_eq!(m["encoding"], "L24");
        assert_eq!(m["clock_rate"], 48000);
        assert_eq!(m["channels"], 4);
        assert_eq!(m["ptime_ms"], 1);
        assert_eq!(
            m["connection"],
            json!({"address_type": "IP4", "address": "239.69.11.44", "ttl": 32, "count": 1})
        );
        assert_eq!(
            m["clocks"],
            json!([{"source": "ptp", "version": "IEEE1588-2008",
                    "grandmaster": "00-11-22-FF-FE-33-44-55", "domain": 0}])
        );
        assert_eq!(m["media_clock"], json!({"source": "direct", "offset": 0}));
        assert_eq!(stream["sdp"], SAMPLE);
    }

    #[test]
    fn header_variants_are_read_and_unusable_packets_skipped() {
        // An IPv6 originating source with two words of authentication data,
        // and no payload type.
        let mut p = vec![0x30, 2, 0, 7];
        p.extend_from_slice(&"fd00::2".parse::<Ipv6Addr>().unwrap().octets());
        p.extend_from_slice(&[0x21, 0, 0, 0, 0, 0, 0, 0]);
        p.extend_from_slice(SAMPLE.as_bytes());
        let got = parse_sap(&p).unwrap();
        assert_eq!(got.source, "fd00::2".parse::<IpAddr>().unwrap());
        assert_eq!(got.hash, 7);
        assert_eq!(got.sdp, SAMPLE);
        // Compressed (zlib) with the payload type inside.
        let mut inner = b"application/sdp\0".to_vec();
        inner.extend_from_slice(SAMPLE.as_bytes());
        let mut p = vec![0x21, 0, 0, 9, 10, 0, 1, 2];
        p.extend_from_slice(&miniz_oxide::deflate::compress_to_vec_zlib(&inner, 6));
        assert_eq!(parse_sap(&p).unwrap().sdp, SAMPLE);
        // Encrypted, another version, another payload type: skipped.
        let mut p = build_sap(false, 1, ANNOUNCER, SAMPLE);
        p[0] |= 0x02;
        assert!(parse_sap(&p).is_err());
        let mut p = build_sap(false, 1, ANNOUNCER, SAMPLE);
        p[0] = 0x00;
        assert!(parse_sap(&p).is_err());
        let mut p = vec![0x20, 0, 0, 1, 10, 0, 1, 2];
        p.extend_from_slice(b"text/plain\0hello");
        assert!(parse_sap(&p).is_err());
        assert!(parse_sap(&[0x20, 0]).is_err());
    }

    #[test]
    fn repeats_renew_changes_replace_and_deletions_remove() {
        let mut s = started();
        let mut state = json!({});
        let mut cx = Cx::new(1_000);
        s.datagram(
            &mut cx,
            SOCKET,
            from(),
            &build_sap(false, 1, ANNOUNCER, SAMPLE),
        );
        merged(&cx.take(), &mut state);
        // The same announcement again: no state.
        let mut cx = Cx::new(31_000);
        s.datagram(
            &mut cx,
            SOCKET,
            from(),
            &build_sap(false, 1, ANNOUNCER, SAMPLE),
        );
        assert!(!cx.take().iter().any(|a| matches!(a, Action::State(_))));
        // A changed description under a new hash replaces the stream.
        let changed = SAMPLE
            .replace("123458", "123459")
            .replace("a=ptime:1\r\n", "a=ptime:0.125\r\n");
        let mut cx = Cx::new(61_000);
        s.datagram(
            &mut cx,
            SOCKET,
            from(),
            &build_sap(false, 2, ANNOUNCER, &changed),
        );
        merged(&cx.take(), &mut state);
        let stream = &state["streams"]["10.0.1.2:123456"];
        assert_eq!(stream["media"][0]["ptime_ms"], 0.125);
        assert_eq!(stream["origin"]["version"], "123459");
        // A deletion from another originating source is ignored; its own
        // announcer's removes the stream.
        let o = "o=- 123456 123459 IN IP4 10.0.1.2\r\n";
        let other = IpAddr::V4(Ipv4Addr::new(10, 0, 1, 9));
        let mut cx = Cx::new(62_000);
        s.datagram(&mut cx, SOCKET, from(), &build_sap(true, 2, other, o));
        assert!(!cx.take().iter().any(|a| matches!(a, Action::State(_))));
        let mut cx = Cx::new(63_000);
        s.datagram(&mut cx, SOCKET, from(), &build_sap(true, 2, ANNOUNCER, o));
        merged(&cx.take(), &mut state);
        assert_eq!(state["streams"], json!({}));
        assert_eq!(state["stream_count"], 0);
    }

    #[test]
    fn silent_streams_and_ended_sessions_expire() {
        let mut s = sap(json!({"timeout": 60}));
        let mut cx = Cx::new(0);
        s.start(&mut cx);
        let mut state = json!({});
        let mut cx = Cx::at(0, 0);
        s.datagram(
            &mut cx,
            SOCKET,
            from(),
            &build_sap(false, 1, ANNOUNCER, SAMPLE),
        );
        // Ends at NTP 3,900,000,000 (Unix 1,691,011,200).
        let ended = SAMPLE
            .replace("o=- 123456", "o=- 777")
            .replace("t=0 0", "t=0 3900000000");
        s.datagram(
            &mut cx,
            SOCKET,
            from(),
            &build_sap(false, 3, ANNOUNCER, &ended),
        );
        merged(&cx.take(), &mut state);
        assert_eq!(state["stream_count"], 2);
        let mut cx = Cx::at(30_000, 1_691_011_200_000 + 1);
        s.timer(&mut cx, EXPIRE);
        merged(&cx.take(), &mut state);
        assert!(state["streams"].get("10.0.1.2:777").is_none());
        assert!(state["streams"].get("10.0.1.2:123456").is_some());
        let mut cx = Cx::at(61_000, 0);
        s.timer(&mut cx, EXPIRE);
        merged(&cx.take(), &mut state);
        assert_eq!(state["streams"], json!({}));
    }

    #[test]
    fn announce_sends_repeats_and_withdraw_deletes() {
        let mut s = started();
        let params = json!({"sdp": SAMPLE.replace("\r\n", "\n")})
            .as_object()
            .unwrap()
            .clone();
        let mut cx = Cx::new(100);
        s.command(&mut cx, 1, "announce", &params);
        let a = cx.take();
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value {
                value: json!("10.0.1.2:123456")
            })
        }));
        let sent = sends(&a);
        assert_eq!(sent.len(), 1);
        let (to, packet) = &sent[0];
        assert_eq!(*to, "239.255.255.255:9875".parse().unwrap());
        let hash = hash16(SAMPLE);
        let mut expect = vec![0x20, 0];
        expect.extend_from_slice(&hash.to_be_bytes());
        expect.extend_from_slice(&[10, 0, 1, 2]);
        expect.extend_from_slice(b"application/sdp\0");
        expect.extend_from_slice(SAMPLE.as_bytes());
        assert_eq!(*packet, expect);
        let after = a.iter().find_map(|a| match a {
            Action::SetTimer { key, after } if *key == ANNOUNCE => Some(*after),
            _ => None,
        });
        assert!(
            after.is_some_and(|t| (20_000..=40_000).contains(&t)),
            "{after:?}"
        );

        let mut cx = Cx::new(30_000);
        s.timer(&mut cx, ANNOUNCE);
        let a = cx.take();
        assert_eq!(sends(&a).len(), 1);
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::SetTimer { key, .. } if *key == ANNOUNCE)));

        // Heard back through multicast loopback: marked local.
        let mut state = json!({});
        let mut cx = Cx::new(30_001);
        s.datagram(&mut cx, SOCKET, from(), &expect);
        merged(&cx.take(), &mut state);
        assert_eq!(state["streams"]["10.0.1.2:123456"]["local"], true);

        let params = json!({"stream": "10.0.1.2:123456"})
            .as_object()
            .unwrap()
            .clone();
        let mut cx = Cx::new(31_000);
        s.command(&mut cx, 2, "withdraw", &params);
        let a = cx.take();
        let sent = sends(&a);
        let mut deletion = vec![0x24, 0];
        deletion.extend_from_slice(&hash.to_be_bytes());
        deletion.extend_from_slice(&[10, 0, 1, 2]);
        deletion.extend_from_slice(b"application/sdp\0o=- 123456 123458 IN IP4 10.0.1.2\r\n");
        assert_eq!(sent, [("239.255.255.255:9875".parse().unwrap(), deletion)]);
        assert!(a.contains(&Action::CancelTimer { key: ANNOUNCE }));
        merged(&a, &mut state);
        assert_eq!(state["streams"], json!({}));
        let mut cx = Cx::new(32_000);
        s.command(&mut cx, 3, "withdraw", &params);
        assert!(matches!(
            cx.take().as_slice(),
            [Action::Complete {
                result: Err(CommandError::InvalidParams { .. }),
                ..
            }]
        ));
    }

    #[test]
    fn closing_deletes_what_was_announced_and_bad_descriptions_are_refused() {
        let mut s = started();
        let named = SAMPLE.replace("IN IP4 10.0.1.2", "IN IP4 sender.example");
        let params = json!({"sdp": named}).as_object().unwrap().clone();
        let mut cx = Cx::new(1);
        s.command(&mut cx, 1, "announce", &params);
        assert!(matches!(
            cx.take().as_slice(),
            [Action::Complete {
                result: Err(CommandError::InvalidParams { .. }),
                ..
            }]
        ));
        let params = json!({"sdp": "s=no version"}).as_object().unwrap().clone();
        let mut cx = Cx::new(2);
        s.command(&mut cx, 2, "announce", &params);
        assert!(matches!(
            cx.take().as_slice(),
            [Action::Complete {
                result: Err(CommandError::InvalidParams { .. }),
                ..
            }]
        ));
        let params = json!({"sdp": SAMPLE}).as_object().unwrap().clone();
        let mut cx = Cx::new(3);
        s.command(&mut cx, 3, "announce", &params);
        cx.take();
        let mut cx = Cx::new(4);
        s.stop(&mut cx);
        let a = cx.take();
        let sent = sends(&a);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].1[0], 0x24);
        assert!(a.contains(&Action::UdpClose { socket: SOCKET }));
    }

    #[test]
    fn session_level_attributes_and_redundant_media_are_read() {
        let sdp = "v=0\r\no=user 42 1 IN IP4 192.168.1.5\r\ns=Stage box\r\n\
            c=IN IP4 239.1.2.3/16\r\nt=0 0\r\na=clock-domain:PTPv2 0\r\na=group:DUP p s\r\n\
            a=ts-refclk:ptp=IEEE1588-2008:traceable\r\n\
            m=audio 5004 RTP/AVP 96\r\na=rtpmap:96 L16/48000/2\r\na=mid:p\r\na=framecount:48\r\n\
            a=source-filter: incl IN IP4 239.1.2.3 192.168.1.5\r\n\
            m=audio 5004 RTP/AVP 96\r\nc=IN IP4 239.1.2.4/16\r\na=rtpmap:96 L16/48000/2\r\na=mid:s\r\n";
        let d = parse_sdp(sdp).unwrap();
        assert_eq!(d.origin.key(), "user@192.168.1.5:42");
        let s = &d.state;
        assert_eq!(s["clock_domain"], json!({"protocol": "PTPv2", "domain": 0}));
        assert_eq!(
            s["groups"],
            json!([{"semantics": "DUP", "mids": ["p", "s"]}])
        );
        assert_eq!(
            s["clocks"],
            json!([{"source": "ptp", "version": "IEEE1588-2008", "traceable": true}])
        );
        // A redundant pair's channels count once.
        assert_eq!(s["channels"], 2);
        assert_eq!(s["media"][0]["connection"]["address"], "239.1.2.3");
        assert_eq!(s["media"][1]["connection"]["address"], "239.1.2.4");
        assert_eq!(s["media"][0]["framecount"], 48);
        assert_eq!(
            s["media"][0]["source_filters"],
            json!([{"mode": "incl", "address_type": "IP4", "destination": "239.1.2.3",
                    "sources": ["192.168.1.5"]}])
        );
        assert_eq!(d.stop_unix_ms, None);
    }
}
