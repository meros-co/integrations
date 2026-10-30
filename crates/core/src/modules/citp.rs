//! CITP/MSEX media server status, as the CITP protocol suite specification
//! (Lars Wernlund, revision 2020-11-27) lays it out.
//!
//! - Every message starts with the 20-byte CITP header (cookie `CITP`,
//!   version 1.0, a request index, the whole message's size, its fragment
//!   count and index, and the second layer's cookie), all little-endian.
//!   MSEX messages add a version and their own cookie; PINF messages their
//!   cookie. Strings are null-terminated, single-byte (`ucs1`) or UTF-16LE
//!   (`ucs2`).
//! - The core is the lighting console of the specification: it connects to
//!   the media server's TCP port, sends PINF/PNam and MSEX/CInf (supporting
//!   MSEX 1.0, 1.1 and 1.2), and takes the server's SInf, which a 1.0 or 1.1
//!   server sends on connecting and a 1.2 server in answer to CInf. MSEX 1.2
//!   requests go to a server whose SInf is 1.2 or later; an older one gets
//!   1.1 requests (the version it cannot state, its SInf being 1.0), or the
//!   version the `msex_version` setting forces.
//! - LSta (sent by the server about four times a second) keeps each layer's
//!   media, position and play flags; the element libraries are read with
//!   GELI (each level, at most three) and GEIn (each library's elements,
//!   answered by MEIn, EEIn or GLEI) and read again where ELUp says they
//!   changed; the video sources with GVSr. Thumbnails are commands (GELT,
//!   GETh); the `live` stream subscribes to a video source with RqSt and
//!   publishes its JPEG frames, which the server multicasts as StFr on
//!   239.224.0.180 (or the older 224.0.0.180) port 4809.
//! - A server whose port is not given is found by its PINF/PLoc, multicast
//!   on the same groups and port, from the device's address.
//!
//! Requests go one at a time, each answered by its own message or Nack (MSEX
//! 1.2), or given up after 5 s; each answer reports its round trip. Opened
//! for commands only (`monitor` false), only the greeting (PNam, CInf) is
//! sent: no library, element or source is read, while what the server sends
//! unasked (SInf, LSta) still reaches state, and its LSta is the liveness
//! check. A connection silent for 15 s is opened again.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::{
    json,
    value::{Map, Value},
};
use base64::Engine as _;

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const TCP: Key = "citp";
const UDP: Key = "citp-multicast";
const RETRY: Key = "retry";
const SILENCE: Key = "silence";
const REPLY: Key = "reply";
const SINF_WAIT: Key = "sinf";
const RENEW: Key = "stream";
const LIVE: &str = "live";

/// The PINF/PLoc and StFr port, and its groups (the current one first).
pub(crate) const MULTICAST_PORT: u16 = 4809;
pub(crate) const GROUPS: [Ipv4Addr; 2] = [
    Ipv4Addr::new(239, 224, 0, 180),
    Ipv4Addr::new(224, 0, 0, 180),
];

const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const QUIET: Millis = 15_000;
const REPLY_TIMEOUT: Millis = 5_000;
/// How long a 1.0 SInf waits for a 1.2 one before the libraries are read.
const SINF_GRACE: Millis = 1_000;
/// RqSt asks for this many seconds of frames, and is sent again sooner.
const STREAM_TIMEOUT_S: u8 = 5;
const STREAM_RENEW: Millis = 3_000;

const HEADER: usize = 20;

/// The library types, numbered as MSEX numbers them (from 1).
pub(crate) const LIBRARY_TYPES: [&str; 9] = [
    "media",
    "effects",
    "cues",
    "crossfades",
    "masks",
    "blend_presets",
    "effect_presets",
    "image_presets",
    "meshes",
];

fn type_name(t: u8) -> Option<&'static str> {
    LIBRARY_TYPES.get(usize::from(t).checked_sub(1)?).copied()
}

fn type_number(name: &str) -> Option<u8> {
    LIBRARY_TYPES
        .iter()
        .position(|n| *n == name)
        .map(|i| i as u8 + 1)
}

/// A library identifier: its level (0 for the root) and up to three
/// 0-based indexes, written `a`, `a-b` or `a-b-c`. In MSEX 1.0 a library is
/// a number, kept as level 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct LibId {
    level: u8,
    path: [u8; 3],
}

impl LibId {
    const ROOT: LibId = LibId {
        level: 0,
        path: [0; 3],
    };

    fn parse(text: &str) -> Option<LibId> {
        let parts: Vec<&str> = text.split('-').collect();
        if parts.is_empty() || parts.len() > 3 {
            return None;
        }
        let mut path = [0u8; 3];
        for (i, p) in parts.iter().enumerate() {
            path[i] = p.parse().ok()?;
        }
        Some(LibId {
            level: parts.len() as u8,
            path,
        })
    }

    fn from_bytes(b: [u8; 4]) -> LibId {
        let level = b[0].min(3);
        let mut path = [b[1], b[2], b[3]];
        for p in path.iter_mut().skip(usize::from(level)) {
            *p = 0;
        }
        LibId { level, path }
    }

    fn bytes(self) -> [u8; 4] {
        [self.level, self.path[0], self.path[1], self.path[2]]
    }

    fn key(self) -> String {
        self.path[..usize::from(self.level)]
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join("-")
    }

    fn parent(self) -> LibId {
        if self.level == 0 {
            return self;
        }
        let mut path = self.path;
        path[usize::from(self.level) - 1] = 0;
        LibId {
            level: self.level - 1,
            path,
        }
    }

    /// Whether `self` lies under `other` (or is it).
    fn under(self, other: LibId) -> bool {
        self.level >= other.level
            && self.path[..usize::from(other.level)] == other.path[..usize::from(other.level)]
    }
}

/// A thumbnail or frame format, as its cookie.
fn format_cookie(name: &str) -> Option<[u8; 4]> {
    Some(match name {
        "jpeg" => *b"JPEG",
        "png" => *b"PNG ",
        "rgb8" => *b"RGB8",
        _ => return None,
    })
}

fn format_name(cookie: &[u8]) -> String {
    match cookie {
        b"JPEG" => "jpeg".into(),
        b"PNG " => "png".into(),
        b"RGB8" => "rgb8".into(),
        b"fJPG" => "fjpg".into(),
        b"fPNG" => "fpng".into(),
        other => String::from_utf8_lossy(other).trim().to_string(),
    }
}

// ── Encoding ────────────────────────────────────────────────────────────

/// A whole CITP message: the header, then the second layer's bytes.
pub(crate) fn citp(layer: &[u8; 4], body: &[u8], request_index: u16) -> Vec<u8> {
    let mut m = Vec::with_capacity(HEADER + body.len());
    m.extend_from_slice(b"CITP");
    m.extend_from_slice(&[1, 0]);
    m.extend_from_slice(&request_index.to_le_bytes());
    m.extend_from_slice(&((HEADER + body.len()) as u32).to_le_bytes());
    m.extend_from_slice(&1u16.to_le_bytes());
    m.extend_from_slice(&0u16.to_le_bytes());
    m.extend_from_slice(layer);
    m.extend_from_slice(body);
    m
}

/// An MSEX message of this version and cookie.
pub(crate) fn msex(version: (u8, u8), cookie: &[u8; 4], body: &[u8], index: u16) -> Vec<u8> {
    let mut b = vec![version.0, version.1];
    b.extend_from_slice(cookie);
    b.extend_from_slice(body);
    citp(b"MSEX", &b, index)
}

fn ucs1(text: &str) -> Vec<u8> {
    let mut b: Vec<u8> = text
        .chars()
        .map(|c| if (c as u32) < 256 { c as u8 } else { b'?' })
        .collect();
    b.push(0);
    b
}

/// PINF/PNam with this name.
pub(crate) fn pnam(name: &str) -> Vec<u8> {
    let mut b = b"PNam".to_vec();
    b.extend(ucs1(name));
    citp(b"PINF", &b, 0)
}

/// MSEX 1.2 CInf: the versions this side supports, 1.0, 1.1 and 1.2.
pub(crate) fn cinf() -> Vec<u8> {
    let mut b = vec![3u8];
    for (major, minor) in [(1u8, 0u8), (1, 1), (1, 2)] {
        // MSB the major version, LSB the minor, sent little-endian.
        b.extend_from_slice(&u16::from_be_bytes([major, minor]).to_le_bytes());
    }
    msex((1, 2), b"CInf", &b, 0)
}

/// A count field: one byte before MSEX 1.2, two from it.
fn count(b: &mut Vec<u8>, version: (u8, u8), n: u16) {
    if version >= (1, 2) {
        b.extend_from_slice(&n.to_le_bytes());
    } else {
        b.push(n as u8);
    }
}

/// A library reference: its number in MSEX 1.0, its identifier after.
fn library(b: &mut Vec<u8>, version: (u8, u8), lib: LibId) {
    if version < (1, 1) {
        b.push(lib.path[0]);
    } else {
        b.extend_from_slice(&lib.bytes());
    }
}

/// GELI: every library of a type under `parent` (all libraries in 1.0).
pub(crate) fn geli(version: (u8, u8), t: u8, parent: LibId, index: u16) -> Vec<u8> {
    let mut b = vec![t];
    if version >= (1, 1) {
        b.extend_from_slice(&parent.bytes());
    }
    count(&mut b, version, 0);
    msex(version, b"GELI", &b, index)
}

/// GEIn: every element of a library.
pub(crate) fn gein(version: (u8, u8), t: u8, lib: LibId, index: u16) -> Vec<u8> {
    let mut b = vec![t];
    library(&mut b, version, lib);
    count(&mut b, version, 0);
    msex(version, b"GEIn", &b, index)
}

/// The size and format of a thumbnail asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Thumb {
    format: [u8; 4],
    width: u16,
    height: u16,
    preserve_aspect: bool,
}

/// GELT: one library's thumbnail.
pub(crate) fn gelt(version: (u8, u8), t: u8, lib: LibId, thumb: Thumb, index: u16) -> Vec<u8> {
    let mut b = thumb.format.to_vec();
    b.extend_from_slice(&thumb.width.to_le_bytes());
    b.extend_from_slice(&thumb.height.to_le_bytes());
    b.push(u8::from(thumb.preserve_aspect));
    b.push(t);
    count(&mut b, version, 1);
    library(&mut b, version, lib);
    msex(version, b"GELT", &b, index)
}

/// GETh: one element's thumbnail.
pub(crate) fn geth(
    version: (u8, u8),
    t: u8,
    lib: LibId,
    element: u8,
    thumb: Thumb,
    index: u16,
) -> Vec<u8> {
    let mut b = thumb.format.to_vec();
    b.extend_from_slice(&thumb.width.to_le_bytes());
    b.extend_from_slice(&thumb.height.to_le_bytes());
    b.push(u8::from(thumb.preserve_aspect));
    b.push(t);
    library(&mut b, version, lib);
    count(&mut b, version, 1);
    b.push(element);
    msex(version, b"GETh", &b, index)
}

/// RqSt: frames of a source for `STREAM_TIMEOUT_S` seconds.
pub(crate) fn rqst(
    version: (u8, u8),
    source: u16,
    size: (u16, u16),
    fps: u8,
    index: u16,
) -> Vec<u8> {
    let mut b = source.to_le_bytes().to_vec();
    b.extend_from_slice(b"JPEG");
    b.extend_from_slice(&size.0.to_le_bytes());
    b.extend_from_slice(&size.1.to_le_bytes());
    b.push(fps);
    b.push(STREAM_TIMEOUT_S);
    msex(version, b"RqSt", &b, index)
}

// ── Decoding ────────────────────────────────────────────────────────────

/// Reads little-endian fields and null-terminated strings in turn.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let b = self.data.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(b)
    }

    fn u8(&mut self) -> Option<u8> {
        self.bytes(1).map(|b| b[0])
    }

    fn u16(&mut self) -> Option<u16> {
        self.bytes(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Option<u32> {
        self.bytes(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u64(&mut self) -> Option<u64> {
        let b = self.bytes(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Some(u64::from_le_bytes(a))
    }

    fn cookie(&mut self) -> Option<[u8; 4]> {
        let b = self.bytes(4)?;
        Some([b[0], b[1], b[2], b[3]])
    }

    fn lib(&mut self) -> Option<LibId> {
        self.cookie().map(LibId::from_bytes)
    }

    /// A count: one byte before MSEX 1.2, two from it.
    fn count(&mut self, version: (u8, u8)) -> Option<u16> {
        if version >= (1, 2) {
            self.u16()
        } else {
            self.u8().map(u16::from)
        }
    }

    fn ucs1(&mut self) -> Option<String> {
        let rest = self.data.get(self.pos..)?;
        let end = rest.iter().position(|b| *b == 0)?;
        self.pos += end + 1;
        Some(rest[..end].iter().map(|b| char::from(*b)).collect())
    }

    fn ucs2(&mut self) -> Option<String> {
        let mut units = Vec::new();
        loop {
            let u = self.u16()?;
            if u == 0 {
                break;
            }
            units.push(u);
        }
        Some(String::from_utf16_lossy(&units))
    }
}

/// One whole CITP message off a stream, and the bytes it took.
#[derive(Debug, PartialEq)]
pub(crate) enum Framed {
    Message {
        layer: [u8; 4],
        index: u16,
        part: (u16, u16),
        body: Vec<u8>,
        used: usize,
    },
    /// Bytes that start no message, to drop.
    Skip(usize),
    Incomplete,
}

pub(crate) fn frame(buf: &[u8]) -> Framed {
    let Some(at) = buf.windows(4).position(|w| w == b"CITP") else {
        // Keep a possible start of a cookie.
        return match buf.len() {
            0..=3 => Framed::Incomplete,
            n => Framed::Skip(n - 3),
        };
    };
    if at > 0 {
        return Framed::Skip(at);
    }
    if buf.len() < HEADER {
        return Framed::Incomplete;
    }
    let size = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]) as usize;
    if size < HEADER {
        return Framed::Skip(4);
    }
    if buf.len() < size {
        return Framed::Incomplete;
    }
    Framed::Message {
        layer: [buf[16], buf[17], buf[18], buf[19]],
        index: u16::from_le_bytes([buf[6], buf[7]]),
        part: (
            u16::from_le_bytes([buf[12], buf[13]]),
            u16::from_le_bytes([buf[14], buf[15]]),
        ),
        body: buf[HEADER..size].to_vec(),
        used: size,
    }
}

// ── The module ──────────────────────────────────────────────────────────

/// What a request waits for.
#[derive(Debug, Clone, PartialEq)]
enum Ask {
    /// GELI: answered by ELIn.
    Libraries { t: u8, parent: LibId },
    /// GEIn: answered by MEIn, EEIn or GLEI.
    Elements { t: u8, lib: LibId },
    /// GELT: answered by ELTh.
    LibraryThumb { t: u8, lib: LibId },
    /// GETh: answered by EThn.
    ElementThumb { t: u8, lib: LibId, element: u8 },
    /// GVSr: answered by VSrc.
    Sources,
}

impl Ask {
    /// The request's own cookie, which a Nack names.
    fn cookie(&self) -> &'static [u8; 4] {
        match self {
            Ask::Libraries { .. } => b"GELI",
            Ask::Elements { .. } => b"GEIn",
            Ask::LibraryThumb { .. } => b"GELT",
            Ask::ElementThumb { .. } => b"GETh",
            Ask::Sources => b"GVSr",
        }
    }
}

#[derive(Debug, Clone)]
struct Request {
    ask: Ask,
    thumb: Option<Thumb>,
    command: Option<CommandId>,
    /// Part of a whole reading of the libraries: what it lists is read
    /// further.
    walk: bool,
}

/// A library this connection has read.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Lib {
    libraries: u16,
    elements: u16,
}

/// A video source, as VSrc gives it.
#[derive(Debug, Clone, PartialEq)]
struct Source {
    id: u16,
}

pub(crate) struct Citp {
    host: IpAddr,
    port: Option<u16>,
    bind: Option<IpAddr>,
    monitor: bool,
    client_name: String,
    forced: Option<(u8, u8)>,
    stream_source: Option<u16>,
    stream_size: (u16, u16),
    stream_fps: u8,
    /// The version requests are sent in, once the server's SInf is in.
    version: Option<(u8, u8)>,
    /// The library types the server offers (MSEX 1.2 SInf), bit 0 media.
    library_types: Option<u16>,
    uuid: Option<String>,
    connected: bool,
    connecting: bool,
    udp_open: bool,
    retry_after: Millis,
    buf: Vec<u8>,
    /// Fragments of a message sent in parts, by part index.
    parts: BTreeMap<u16, Vec<u8>>,
    request_index: u16,
    queue: VecDeque<Request>,
    in_flight: Option<(Request, Millis, u16)>,
    libs: BTreeMap<(u8, LibId), Lib>,
    walked: bool,
    /// Commands waiting for a whole reading (refresh) to end.
    walks: Vec<CommandId>,
    sources: Vec<Source>,
    layers: HashMap<u8, Value>,
    watching: bool,
    fragments: Option<(u32, u16, BTreeMap<u32, Vec<u8>>)>,
    warned_format: bool,
}

fn setting_str<'a>(s: &'a Params, name: &str) -> Option<&'a str> {
    s.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

impl Citp {
    pub(crate) fn new(ctx: OpenContext) -> Result<Citp, String> {
        if ctx.host.is_unspecified() {
            return Err("give the media server's address as host".into());
        }
        let s = &ctx.settings;
        let bind = match setting_str(s, "bind_address") {
            None => None,
            Some(a) => Some(
                a.parse::<IpAddr>()
                    .map_err(|_| format!("bind_address '{a}' is not an IP address"))?,
            ),
        };
        let forced = match setting_str(s, "msex_version") {
            None | Some("auto") => None,
            Some("1.0") => Some((1, 0)),
            Some("1.1") => Some((1, 1)),
            Some("1.2") => Some((1, 2)),
            Some(other) => {
                return Err(format!(
                    "msex_version '{other}' is not auto, 1.0, 1.1 or 1.2"
                ))
            }
        };
        let int = |name: &str, default: u64| s.get(name).and_then(Value::as_u64).unwrap_or(default);
        Ok(Citp {
            host: ctx.host,
            port: ctx.port,
            bind,
            monitor: ctx.monitor,
            client_name: setting_str(s, "client_name")
                .unwrap_or("CITP client")
                .to_string(),
            forced,
            stream_source: s
                .get("stream_source")
                .and_then(Value::as_u64)
                .map(|v| v as u16),
            stream_size: (
                int("stream_width", 640).min(u64::from(u16::MAX)) as u16,
                int("stream_height", 360).min(u64::from(u16::MAX)) as u16,
            ),
            stream_fps: int("stream_fps", 10).min(255) as u8,
            version: None,
            library_types: None,
            uuid: None,
            connected: false,
            connecting: false,
            udp_open: false,
            retry_after: RETRY_MIN,
            buf: Vec::new(),
            parts: BTreeMap::new(),
            request_index: 0,
            queue: VecDeque::new(),
            in_flight: None,
            libs: BTreeMap::new(),
            walked: false,
            walks: Vec::new(),
            sources: Vec::new(),
            layers: HashMap::new(),
            watching: false,
            fragments: None,
            warned_format: false,
        })
    }

    fn next_index(&mut self) -> u16 {
        self.request_index = self.request_index.wrapping_add(1).max(1);
        self.request_index
    }

    /// The multicast socket: for finding the server's port, and for frames.
    fn open_udp(&mut self, cx: &mut Cx) {
        if self.udp_open {
            return;
        }
        self.udp_open = true;
        cx.udp_open(
            UDP,
            Bind::Network {
                address: self.bind,
                port: MULTICAST_PORT,
            },
        );
        for g in GROUPS {
            cx.udp_join(UDP, g);
        }
    }

    fn connect(&mut self, cx: &mut Cx) {
        let Some(port) = self.port else {
            // Wait for the server's PLoc to name its port.
            cx.connection(Connection::Connecting);
            self.open_udp(cx);
            return;
        };
        self.connecting = true;
        cx.connection(Connection::Connecting);
        cx.tcp_open(TCP, SocketAddr::new(self.host, port));
    }

    fn dropped(&mut self, cx: &mut Cx, reason: String) {
        let was = self.connected || self.connecting;
        self.connected = false;
        self.connecting = false;
        self.buf.clear();
        self.parts.clear();
        self.version = None;
        self.walked = false;
        cx.cancel_timer(SILENCE);
        cx.cancel_timer(REPLY);
        cx.cancel_timer(SINF_WAIT);
        cx.cancel_timer(RENEW);
        if let Some((r, _, _)) = self.in_flight.take() {
            if let Some(id) = r.command {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        for r in self.queue.drain(..) {
            if let Some(id) = r.command {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        for id in self.walks.drain(..) {
            cx.complete(
                id,
                Err(CommandError::Transport {
                    message: reason.clone(),
                }),
            );
        }
        if was {
            cx.connection(Connection::Disconnected { reason });
        }
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn send(&mut self, cx: &mut Cx, bytes: Vec<u8>) {
        cx.tcp_send(TCP, bytes);
    }

    fn enqueue(&mut self, cx: &mut Cx, r: Request) {
        if r.command.is_none() {
            let waiting = self
                .queue
                .iter()
                .any(|q| q.command.is_none() && q.ask == r.ask)
                || self
                    .in_flight
                    .as_ref()
                    .is_some_and(|(q, _, _)| q.command.is_none() && q.ask == r.ask);
            if waiting {
                return;
            }
            self.queue.push_back(r);
        } else {
            // Commands go ahead of the libraries being read.
            let at = self.queue.iter().position(|q| q.command.is_none());
            match at {
                Some(at) => self.queue.insert(at, r),
                None => self.queue.push_back(r),
            }
        }
        self.pump(cx);
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || !self.connected {
            return;
        }
        let Some(version) = self.version else {
            return;
        };
        let Some(r) = self.queue.pop_front() else {
            self.walk_done(cx);
            return;
        };
        let index = self.next_index();
        let bytes = match (&r.ask, r.thumb) {
            (Ask::Libraries { t, parent }, _) => geli(version, *t, *parent, index),
            (Ask::Elements { t, lib }, _) => gein(version, *t, *lib, index),
            (Ask::LibraryThumb { t, lib }, Some(th)) => gelt(version, *t, *lib, th, index),
            (Ask::ElementThumb { t, lib, element }, Some(th)) => {
                geth(version, *t, *lib, *element, th, index)
            }
            (Ask::Sources, _) => msex(version, b"GVSr", &[], index),
            _ => return,
        };
        self.send(cx, bytes);
        cx.set_timer(REPLY, REPLY_TIMEOUT);
        self.in_flight = Some((r, cx.now(), index));
    }

    /// Whole readings asked for by commands end when nothing is left to read.
    fn walk_done(&mut self, cx: &mut Cx) {
        if self.in_flight.is_none() && self.queue.is_empty() {
            for id in self.walks.drain(..) {
                cx.complete(id, Ok(Outcome::Ack));
            }
        }
    }

    /// The request in flight is answered: report it and send the next.
    fn answered(&mut self, cx: &mut Cx) -> Option<Request> {
        let (r, at, _) = self.in_flight.take()?;
        cx.cancel_timer(REPLY);
        cx.round_trip(cx.now().saturating_sub(at));
        Some(r)
    }

    fn types(&self) -> Vec<u8> {
        (1..=9u8)
            .filter(|t| match self.library_types {
                Some(flags) => flags & (1 << (t - 1)) != 0,
                // Before MSEX 1.2 a server names none: ask for the eight
                // types the specification knew then.
                None => *t <= 8,
            })
            .collect()
    }

    /// Read every library of every type, level by level, with its elements.
    fn walk(&mut self, cx: &mut Cx) {
        self.walked = true;
        for t in self.types() {
            self.enqueue(
                cx,
                Request {
                    ask: Ask::Libraries {
                        t,
                        parent: LibId::ROOT,
                    },
                    thumb: None,
                    command: None,
                    walk: true,
                },
            );
        }
        self.enqueue(
            cx,
            Request {
                ask: Ask::Sources,
                thumb: None,
                command: None,
                walk: true,
            },
        );
    }

    /// The server's information is in: requests can go.
    fn ready(&mut self, cx: &mut Cx, version: (u8, u8)) {
        let use_version = self
            .forced
            .unwrap_or(if version >= (1, 2) { (1, 2) } else { (1, 1) });
        self.version = Some(use_version);
        cx.state(
            json!({"server": {"msex_version": format!("{}.{}", use_version.0, use_version.1)}}),
        );
        if self.monitor && !self.walked {
            self.walk(cx);
        }
        if self.watching {
            self.request_stream(cx);
        }
        self.pump(cx);
    }

    fn on_message(&mut self, cx: &mut Cx, layer: [u8; 4], body: &[u8], via_udp: bool) {
        match &layer {
            b"PINF" => self.on_pinf(cx, body, via_udp),
            b"MSEX" => self.on_msex(cx, body),
            _ => {}
        }
    }

    fn on_pinf(&mut self, cx: &mut Cx, body: &[u8], via_udp: bool) {
        let mut r = Reader::new(body);
        let Some(cookie) = r.cookie() else {
            return;
        };
        match &cookie {
            b"PNam" => {
                if let Some(name) = r.ucs1() {
                    cx.state(json!({"server": {"peer_name": name}}));
                }
            }
            b"PLoc" if via_udp => {
                let (Some(port), Some(kind), Some(name), Some(state)) =
                    (r.u16(), r.ucs1(), r.ucs1(), r.ucs1())
                else {
                    return;
                };
                cx.state(
                    json!({"peer": {"type": kind, "name": name, "state": state, "port": port}}),
                );
                if self.port.is_none() && port != 0 {
                    self.port = Some(port);
                    cx.log(
                        Level::Info,
                        format!("{name} listens for CITP on TCP port {port} (its PLoc)"),
                    );
                    if !self.watching {
                        self.udp_open = false;
                        cx.udp_close(UDP);
                    }
                    self.connect(cx);
                }
            }
            _ => {}
        }
    }

    fn on_msex(&mut self, cx: &mut Cx, body: &[u8]) {
        let mut r = Reader::new(body);
        let (Some(major), Some(minor), Some(cookie)) = (r.u8(), r.u8(), r.cookie()) else {
            return;
        };
        let v = (major, minor);
        let rest = &body[6..];
        match &cookie {
            b"SInf" => self.on_sinf(cx, v, rest),
            b"LSta" => self.on_lsta(cx, v, rest),
            b"Nack" => self.on_nack(cx, rest),
            b"ELIn" => self.on_elin(cx, v, rest),
            b"ELUp" => self.on_elup(cx, v, rest),
            b"MEIn" | b"EEIn" | b"GLEI" => self.on_elements(cx, v, &cookie, rest),
            b"ELTh" | b"EThn" => self.on_thumb(cx, v, &cookie, rest),
            b"VSrc" => self.on_vsrc(cx, rest),
            b"StFr" => self.on_frame(cx, v, rest),
            _ => {}
        }
    }

    fn on_sinf(&mut self, cx: &mut Cx, v: (u8, u8), body: &[u8]) {
        let mut r = Reader::new(body);
        let mut server = Map::new();
        if v >= (1, 2) {
            let Some(uuid) = r.bytes(36) else { return };
            let uuid = String::from_utf8_lossy(uuid).to_string();
            let (Some(product), Some(a), Some(b), Some(c)) = (r.ucs2(), r.u8(), r.u8(), r.u8())
            else {
                return;
            };
            let Some(n) = r.u8() else { return };
            let mut versions = Vec::new();
            for _ in 0..n {
                let Some(x) = r.u16() else { return };
                let [major, minor] = x.to_be_bytes();
                versions.push(format!("{major}.{minor}"));
            }
            let Some(types) = r.u16() else { return };
            let formats = |r: &mut Reader| -> Option<Vec<String>> {
                let n = r.u8()?;
                (0..n)
                    .map(|_| r.cookie().map(|c| format_name(&c)))
                    .collect()
            };
            let (Some(thumbs), Some(streams)) = (formats(&mut r), formats(&mut r)) else {
                return;
            };
            self.uuid = Some(uuid.clone());
            self.library_types = Some(types);
            server.insert("uuid".into(), json!(uuid));
            server.insert("product".into(), json!(product));
            server.insert("version".into(), json!(format!("{a}.{b}.{c}")));
            server.insert("msex_versions".into(), json!(versions.join(",")));
            let names: Vec<&str> = (1..=9u8)
                .filter(|t| types & (1 << (t - 1)) != 0)
                .filter_map(type_name)
                .collect();
            server.insert("library_types".into(), json!(names.join(",")));
            server.insert("thumbnail_formats".into(), json!(thumbs.join(",")));
            server.insert("stream_formats".into(), json!(streams.join(",")));
        } else {
            let (Some(product), Some(a), Some(b)) = (r.ucs2(), r.u8(), r.u8()) else {
                return;
            };
            server.insert("product".into(), json!(product));
            server.insert("version".into(), json!(format!("{a}.{b}")));
            server.insert(
                "msex_versions".into(),
                json!(format!("{major}.{minor}", major = v.0, minor = v.1)),
            );
        }
        let Some(n) = r.u8() else { return };
        let mut layers = Map::new();
        for i in 0..n {
            let Some(source) = r.ucs1() else { return };
            layers.insert(
                (u16::from(i) + 1).to_string(),
                json!({"dmx_source": source}),
            );
        }
        server.insert("layer_count".into(), json!(n));
        cx.state(json!({"server": server, "layers": layers}));
        if v >= (1, 2) {
            cx.cancel_timer(SINF_WAIT);
            self.ready(cx, v);
        } else if self.version.is_none() {
            // A 1.2 server may answer CInf with a 1.2 SInf yet.
            cx.set_timer(SINF_WAIT, SINF_GRACE);
        }
    }

    fn on_lsta(&mut self, cx: &mut Cx, v: (u8, u8), body: &[u8]) {
        let mut r = Reader::new(body);
        let Some(n) = r.u8() else { return };
        let mut changed = Map::new();
        for _ in 0..n {
            let (Some(layer), Some(output), Some(t)) = (r.u8(), r.u8(), r.u8()) else {
                return;
            };
            // MSEX 1.2 gives the library's type and identifier; before, the
            // third byte is the media library's number.
            let (library_type, library) = if v >= (1, 2) {
                let Some(id) = r.lib() else { return };
                (type_name(t).unwrap_or("unknown"), id.key())
            } else {
                ("media", t.to_string())
            };
            let (Some(number), Some(name), Some(position), Some(length), Some(fps), Some(flags)) =
                (r.u8(), r.ucs2(), r.u32(), r.u32(), r.u8(), r.u32())
            else {
                return;
            };
            let value = json!({
                "output": u16::from(output) + 1,
                "library_type": library_type,
                "library": library,
                "media_number": number,
                "media_name": name,
                "position": position,
                "length": length,
                "fps": fps,
                "flags": flags,
                "playing": flags & 0x01 != 0,
                "reverse": flags & 0x02 != 0,
                "looping": flags & 0x04 != 0,
                "bouncing": flags & 0x08 != 0,
                "random": flags & 0x10 != 0,
                "paused": flags & 0x20 != 0,
            });
            if self.layers.get(&layer) != Some(&value) {
                self.layers.insert(layer, value.clone());
                changed.insert((u16::from(layer) + 1).to_string(), value);
            }
        }
        if !changed.is_empty() {
            cx.state(json!({"layers": changed}));
        }
    }

    fn on_nack(&mut self, cx: &mut Cx, body: &[u8]) {
        let Some(cookie) = Reader::new(body).cookie() else {
            return;
        };
        let refused = self
            .in_flight
            .as_ref()
            .is_some_and(|(r, _, _)| r.ask.cookie() == &cookie);
        if !refused {
            return;
        }
        if let Some(r) = self.answered(cx) {
            let name = String::from_utf8_lossy(&cookie).to_string();
            if let Some(id) = r.command {
                cx.complete(
                    id,
                    Err(CommandError::DeviceError {
                        code: Some(name.clone()),
                        message: format!("the media server does not support {name} (Nack)"),
                    }),
                );
            }
        }
        self.pump(cx);
    }

    fn on_elin(&mut self, cx: &mut Cx, v: (u8, u8), body: &[u8]) {
        let mut r = Reader::new(body);
        let Some(t) = r.u8() else { return };
        let Some(n) = r.count(v) else { return };
        let mut listed: Vec<(LibId, Lib, Value)> = Vec::new();
        for _ in 0..n {
            let id = if v >= (1, 1) {
                let Some(id) = r.lib() else { return };
                id
            } else {
                let Some(number) = r.u8() else { return };
                LibId {
                    level: 1,
                    path: [number, 0, 0],
                }
            };
            let serial = if v >= (1, 2) { r.u32() } else { None };
            let (Some(min), Some(max), Some(name)) = (r.u8(), r.u8(), r.ucs2()) else {
                return;
            };
            let libraries = if v >= (1, 1) {
                let Some(c) = r.count(v) else { return };
                c
            } else {
                0
            };
            let Some(elements) = r.count(v) else { return };
            let mut info = json!({
                "name": name,
                "dmx_min": min,
                "dmx_max": max,
                "library_count": libraries,
                "element_count": elements,
            });
            if let Some(serial) = serial {
                info["serial"] = json!(serial);
            }
            listed.push((
                id,
                Lib {
                    libraries,
                    elements,
                },
                info,
            ));
        }
        // The parent listed: the request's, or the first library's.
        let asked = match self.in_flight.as_ref() {
            Some((
                Request {
                    ask: Ask::Libraries { t: at, parent },
                    ..
                },
                _,
                _,
            )) if *at == t => Some(*parent),
            _ => None,
        };
        let parent = asked.or_else(|| listed.first().map(|(id, _, _)| id.parent()));
        let walk = match asked {
            Some(_) => self.answered(cx).is_some_and(|r| r.walk),
            None => false,
        };
        let Some(t_name) = type_name(t) else {
            self.pump(cx);
            return;
        };
        // Libraries under the parent that it no longer lists are gone.
        let mut removed = Map::new();
        if let Some(parent) = parent {
            let keep: BTreeSet<LibId> = listed.iter().map(|(id, _, _)| *id).collect();
            let gone: Vec<(u8, LibId)> = self
                .libs
                .keys()
                .filter(|(lt, id)| {
                    *lt == t
                        && id.level == parent.level + 1
                        && id.under(parent)
                        && !keep.contains(id)
                })
                .copied()
                .collect();
            for (_, id) in gone {
                // It and its sub-libraries, each keyed by its own path.
                for sub in self.libs.keys().filter(|(lt, k)| *lt == t && k.under(id)) {
                    removed.insert(sub.1.key(), Value::Null);
                }
                self.libs.retain(|(lt, k), _| !(*lt == t && k.under(id)));
            }
        }
        let mut patch = Map::new();
        for (id, lib, info) in &listed {
            self.libs.insert((t, *id), *lib);
            patch.insert(id.key(), info.clone());
        }
        if !removed.is_empty() {
            cx.state(json!({"libraries": {t_name: removed}}));
        }
        if !patch.is_empty() {
            cx.state(json!({"libraries": {t_name: patch}}));
        }
        if walk {
            for (id, lib, _) in listed {
                if lib.libraries > 0 && id.level < 3 && self.version >= Some((1, 1)) {
                    self.enqueue(
                        cx,
                        Request {
                            ask: Ask::Libraries { t, parent: id },
                            thumb: None,
                            command: None,
                            walk: true,
                        },
                    );
                }
                if lib.elements > 0 {
                    self.enqueue(
                        cx,
                        Request {
                            ask: Ask::Elements { t, lib: id },
                            thumb: None,
                            command: None,
                            walk: true,
                        },
                    );
                } else {
                    cx.state(json!({"libraries": {t_name: {id.key(): {"elements": null}}}}));
                }
            }
        }
        self.pump(cx);
    }

    fn on_elup(&mut self, cx: &mut Cx, v: (u8, u8), body: &[u8]) {
        let mut r = Reader::new(body);
        let Some(t) = r.u8() else { return };
        let lib = if v >= (1, 1) {
            let Some(id) = r.lib() else { return };
            id
        } else {
            let Some(n) = r.u8() else { return };
            LibId {
                level: 1,
                path: [n, 0, 0],
            }
        };
        let Some(flags) = r.u8() else { return };
        if !self.monitor || type_name(t).is_none() {
            return;
        }
        if flags & (0x01 | 0x02 | 0x10) != 0 && lib.level > 0 {
            self.enqueue(
                cx,
                Request {
                    ask: Ask::Elements { t, lib },
                    thumb: None,
                    command: None,
                    walk: true,
                },
            );
        }
        if flags & (0x04 | 0x08 | 0x20) != 0 && lib.level < 3 {
            let parent = if self.version >= Some((1, 1)) {
                lib
            } else {
                LibId::ROOT
            };
            self.enqueue(
                cx,
                Request {
                    ask: Ask::Libraries { t, parent },
                    thumb: None,
                    command: None,
                    walk: true,
                },
            );
        }
    }

    fn on_elements(&mut self, cx: &mut Cx, v: (u8, u8), cookie: &[u8; 4], body: &[u8]) {
        let mut r = Reader::new(body);
        let declared = if cookie == b"GLEI" && v >= (1, 2) {
            let Some(t) = r.u8() else { return };
            Some(t)
        } else {
            None
        };
        let lib = if v >= (1, 1) {
            let Some(id) = r.lib() else { return };
            id
        } else {
            let Some(n) = r.u8() else { return };
            LibId {
                level: 1,
                path: [n, 0, 0],
            }
        };
        let asked = match self.in_flight.as_ref() {
            Some((
                Request {
                    ask: Ask::Elements { t, lib: l },
                    ..
                },
                _,
                _,
            )) if *l == lib => Some(*t),
            _ => None,
        };
        let t = match (cookie, declared) {
            (b"MEIn", _) => Some(1),
            (b"EEIn", _) => Some(2),
            (_, Some(t)) => Some(t),
            _ => asked,
        };
        let Some(n) = r.count(v) else { return };
        let mut elements = Map::new();
        for _ in 0..n {
            let Some(number) = r.u8() else { return };
            let serial = if v >= (1, 2) { r.u32() } else { None };
            let (Some(min), Some(max), Some(name)) = (r.u8(), r.u8(), r.ucs2()) else {
                return;
            };
            let mut e = json!({"name": name, "dmx_min": min, "dmx_max": max});
            if let Some(serial) = serial {
                e["serial"] = json!(serial);
            }
            match cookie {
                b"MEIn" => {
                    let (Some(ts), Some(w), Some(h), Some(len), Some(fps)) =
                        (r.u64(), r.u16(), r.u16(), r.u32(), r.u8())
                    else {
                        return;
                    };
                    e["version"] = json!(ts);
                    e["width"] = json!(w);
                    e["height"] = json!(h);
                    e["length"] = json!(len);
                    e["fps"] = json!(fps);
                }
                b"EEIn" => {
                    let Some(count) = r.u8() else { return };
                    let mut names = Map::new();
                    for i in 0..count {
                        let Some(p) = r.ucs2() else { return };
                        names.insert(i.to_string(), json!(p));
                    }
                    e["parameters"] = Value::Object(names);
                }
                _ => {
                    let Some(ts) = r.u64() else { return };
                    e["version"] = json!(ts);
                }
            }
            elements.insert(number.to_string(), e);
        }
        if asked.is_some() {
            self.answered(cx);
        }
        if let Some(t_name) = t.and_then(type_name) {
            let key = lib.key();
            cx.state(json!({"libraries": {t_name: {&key: {"elements": null}}}}));
            if !elements.is_empty() {
                cx.state(json!({"libraries": {t_name: {&key: {"elements": elements}}}}));
            }
        }
        self.pump(cx);
    }

    fn on_thumb(&mut self, cx: &mut Cx, v: (u8, u8), cookie: &[u8; 4], body: &[u8]) {
        let mut r = Reader::new(body);
        let Some(t) = r.u8() else { return };
        let lib = if v >= (1, 1) {
            let Some(id) = r.lib() else { return };
            id
        } else {
            let Some(n) = r.u8() else { return };
            LibId {
                level: 1,
                path: [n, 0, 0],
            }
        };
        let element = if cookie == b"EThn" {
            let Some(e) = r.u8() else { return };
            Some(e)
        } else {
            None
        };
        let (Some(format), Some(w), Some(h), Some(size)) = (r.cookie(), r.u16(), r.u16(), r.u16())
        else {
            return;
        };
        let Some(data) = r.bytes(usize::from(size)) else {
            return;
        };
        let matches = self
            .in_flight
            .as_ref()
            .is_some_and(|(q, _, _)| match &q.ask {
                Ask::LibraryThumb { t: qt, lib: ql } => cookie == b"ELTh" && *qt == t && *ql == lib,
                Ask::ElementThumb {
                    t: qt,
                    lib: ql,
                    element: qe,
                } => cookie == b"EThn" && *qt == t && *ql == lib && Some(*qe) == element,
                _ => false,
            });
        if !matches {
            return;
        }
        if let Some(q) = self.answered(cx) {
            if let Some(id) = q.command {
                let mut value = json!({
                    "library_type": type_name(t).unwrap_or("unknown"),
                    "library": lib.key(),
                    "format": format_name(&format),
                    "width": w,
                    "height": h,
                    "data": base64::engine::general_purpose::STANDARD.encode(data),
                });
                if let Some(e) = element {
                    value["element"] = json!(e);
                }
                cx.complete(id, Ok(Outcome::Value { value }));
            }
        }
        self.pump(cx);
    }

    fn on_vsrc(&mut self, cx: &mut Cx, body: &[u8]) {
        let mut r = Reader::new(body);
        let Some(n) = r.u16() else { return };
        let mut list = Vec::new();
        let mut patch = Map::new();
        let mut sources = Vec::new();
        for _ in 0..n {
            let (Some(id), Some(name), Some(output), Some(layer), Some(flags), Some(w), Some(h)) =
                (r.u16(), r.ucs2(), r.u8(), r.u8(), r.u16(), r.u16(), r.u16())
            else {
                return;
            };
            // 0xFF: not tied to an output, or to a layer; kept as 0.
            let one = |b: u8| if b == 0xFF { 0 } else { u16::from(b) + 1 };
            let entry = json!({
                "name": name,
                "output": one(output),
                "layer": one(layer),
                "without_effects": flags & 1 != 0,
                "width": w,
                "height": h,
            });
            let mut listed = entry.clone();
            listed["id"] = json!(id);
            list.push(listed);
            patch.insert(id.to_string(), entry);
            sources.push(Source { id });
        }
        self.sources = sources;
        cx.state(json!({"video_sources": null}));
        if !patch.is_empty() {
            cx.state(json!({"video_sources": patch}));
        }
        if self
            .in_flight
            .as_ref()
            .is_some_and(|(q, _, _)| q.ask == Ask::Sources)
        {
            if let Some(q) = self.answered(cx) {
                if let Some(id) = q.command {
                    cx.complete(id, Ok(Outcome::Value { value: json!(list) }));
                }
            }
        }
        if self.watching {
            self.request_stream(cx);
        }
        self.pump(cx);
    }

    /// The source the live stream shows: the setting's, or the first listed.
    fn stream_id(&self) -> Option<u16> {
        self.stream_source
            .or_else(|| self.sources.first().map(|s| s.id))
    }

    fn request_stream(&mut self, cx: &mut Cx) {
        if !self.connected {
            return;
        }
        let Some(version) = self.version else { return };
        let Some(source) = self.stream_id() else {
            // Which sources there are is read first.
            self.enqueue(
                cx,
                Request {
                    ask: Ask::Sources,
                    thumb: None,
                    command: None,
                    walk: false,
                },
            );
            return;
        };
        let index = self.next_index();
        let bytes = rqst(version, source, self.stream_size, self.stream_fps, index);
        self.send(cx, bytes);
        cx.set_timer(RENEW, STREAM_RENEW);
    }

    fn on_frame(&mut self, cx: &mut Cx, v: (u8, u8), body: &[u8]) {
        if !self.watching {
            return;
        }
        let mut r = Reader::new(body);
        if v >= (1, 2) {
            let Some(uuid) = r.bytes(36) else { return };
            if let Some(ours) = &self.uuid {
                if ours.as_bytes() != uuid {
                    return;
                }
            }
        }
        let (Some(source), Some(format), Some(_w), Some(_h), Some(size)) =
            (r.u16(), r.cookie(), r.u16(), r.u16(), r.u16())
        else {
            return;
        };
        if Some(source) != self.stream_id() {
            return;
        }
        let Some(data) = r.bytes(usize::from(size)) else {
            return;
        };
        match &format {
            b"JPEG" => cx.frame(LIVE, "jpeg", data.to_vec()),
            b"fJPG" => {
                let mut f = Reader::new(data);
                let (Some(index), Some(count), Some(_part), Some(offset)) =
                    (f.u32(), f.u16(), f.u16(), f.u32())
                else {
                    return;
                };
                let piece = data[12..].to_vec();
                let start = !matches!(&self.fragments, Some((i, _, _)) if *i == index);
                if start {
                    self.fragments = Some((index, count, BTreeMap::new()));
                }
                let Some((_, count, pieces)) = self.fragments.as_mut() else {
                    return;
                };
                pieces.insert(offset, piece);
                if pieces.len() >= usize::from(*count) {
                    let jpeg: Vec<u8> = pieces.values().flatten().copied().collect();
                    self.fragments = None;
                    cx.frame(LIVE, "jpeg", jpeg);
                }
            }
            other => {
                if !self.warned_format {
                    self.warned_format = true;
                    cx.log(
                        Level::Warning,
                        format!(
                            "the media server streams {} frames; only JPEG is published",
                            format_name(other)
                        ),
                    );
                }
            }
        }
    }

    fn on_tcp_data(&mut self, cx: &mut Cx, data: &[u8]) {
        self.buf.extend_from_slice(data);
        cx.set_timer(SILENCE, QUIET);
        cx.alive();
        loop {
            match frame(&self.buf) {
                Framed::Incomplete => break,
                Framed::Skip(n) => {
                    self.buf.drain(..n);
                }
                Framed::Message {
                    layer,
                    part,
                    body,
                    used,
                    ..
                } => {
                    self.buf.drain(..used);
                    let (count, index) = part;
                    if count > 1 {
                        self.parts.insert(index, body);
                        if self.parts.len() < usize::from(count) {
                            continue;
                        }
                        let whole: Vec<u8> = std::mem::take(&mut self.parts)
                            .into_values()
                            .flatten()
                            .collect();
                        self.on_message(cx, layer, &whole, false);
                    } else {
                        self.on_message(cx, layer, &body, false);
                    }
                }
            }
        }
    }

    fn library_param(params: &Params) -> Result<(u8, LibId), CommandError> {
        let invalid = |message: String| CommandError::InvalidParams { message };
        let t = params
            .get("library_type")
            .and_then(Value::as_str)
            .and_then(type_number)
            .ok_or_else(|| invalid("library_type is not a library type".into()))?;
        let text = params
            .get("library")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let lib = LibId::parse(text).ok_or_else(|| {
            invalid(format!(
                "library '{text}' is not one to three numbers 0 to 255 joined by -, such as 1 or 1-4"
            ))
        })?;
        Ok((t, lib))
    }

    fn thumb_param(params: &Params) -> Result<Thumb, CommandError> {
        let format = params
            .get("format")
            .and_then(Value::as_str)
            .and_then(format_cookie)
            .ok_or_else(|| CommandError::InvalidParams {
                message: "format is not jpeg, png or rgb8".into(),
            })?;
        let int = |n: &str, d: u64| params.get(n).and_then(Value::as_u64).unwrap_or(d);
        Ok(Thumb {
            format,
            width: int("width", 128).min(u64::from(u16::MAX)) as u16,
            height: int("height", 72).min(u64::from(u16::MAX)) as u16,
            preserve_aspect: params
                .get("preserve_aspect")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        })
    }
}

impl Module for Citp {
    fn start(&mut self, cx: &mut Cx) {
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let request = |ask: Ask, thumb: Option<Thumb>| Request {
            ask,
            thumb,
            command: Some(id),
            walk: false,
        };
        match name {
            "refresh_libraries" => {
                self.walks.push(id);
                self.walk(cx);
                self.walk_done(cx);
            }
            "read_library" => {
                let (t, lib) = match Citp::library_param(params) {
                    Ok(p) => p,
                    Err(e) => return cx.complete(id, Err(e)),
                };
                self.walks.push(id);
                if lib.level < 3 && self.version >= Some((1, 1)) {
                    self.enqueue(
                        cx,
                        Request {
                            ask: Ask::Libraries { t, parent: lib },
                            thumb: None,
                            command: None,
                            walk: true,
                        },
                    );
                }
                self.enqueue(
                    cx,
                    Request {
                        ask: Ask::Elements { t, lib },
                        thumb: None,
                        command: None,
                        walk: false,
                    },
                );
            }
            "get_video_sources" => self.enqueue(cx, request(Ask::Sources, None)),
            "get_library_thumbnail" | "get_element_thumbnail" => {
                let (t, lib) = match Citp::library_param(params) {
                    Ok(p) => p,
                    Err(e) => return cx.complete(id, Err(e)),
                };
                let thumb = match Citp::thumb_param(params) {
                    Ok(t) => t,
                    Err(e) => return cx.complete(id, Err(e)),
                };
                let ask = if name == "get_library_thumbnail" {
                    Ask::LibraryThumb { t, lib }
                } else {
                    let element = params.get("element").and_then(Value::as_u64).unwrap_or(0);
                    Ask::ElementThumb {
                        t,
                        lib,
                        element: element.min(255) as u8,
                    }
                };
                self.enqueue(cx, request(ask, Some(thumb)));
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        if socket != UDP || from.ip() != self.host {
            return;
        }
        if let Framed::Message { layer, body, .. } = frame(data) {
            self.on_message(cx, layer, &body, true);
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        if socket == UDP {
            self.udp_open = false;
            cx.log(
                Level::Warning,
                format!("the CITP multicast port {MULTICAST_PORT} could not be used: {message}"),
            );
        }
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        if socket != TCP {
            return;
        }
        match input {
            TcpInput::Connected => {
                self.connected = true;
                self.connecting = false;
                self.retry_after = RETRY_MIN;
                self.layers.clear();
                cx.connection(Connection::Connected);
                cx.set_timer(SILENCE, QUIET);
                let name = self.client_name.clone();
                self.send(cx, pnam(&name));
                self.send(cx, cinf());
            }
            TcpInput::Data(data) => self.on_tcp_data(cx, &data),
            TcpInput::Closed { reason } => self.dropped(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.connect(cx),
            SILENCE => {
                cx.tcp_close(TCP);
                self.dropped(
                    cx,
                    format!("nothing from the media server for {} s", QUIET / 1000),
                );
            }
            REPLY => {
                if let Some((r, _, _)) = self.in_flight.take() {
                    if let Some(id) = r.command {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                }
                self.pump(cx);
            }
            SINF_WAIT if self.version.is_none() && self.connected => self.ready(cx, (1, 0)),
            RENEW if self.watching => self.request_stream(cx),
            _ => {}
        }
    }

    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        if stream != LIVE {
            return;
        }
        self.watching = watching;
        if watching {
            self.open_udp(cx);
            self.request_stream(cx);
        } else {
            cx.cancel_timer(RENEW);
            self.fragments = None;
            if self.port.is_some() && self.udp_open {
                self.udp_open = false;
                cx.udp_close(UDP);
            }
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(TCP);
        if self.udp_open {
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

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));

    fn module(monitor: bool, port: Option<u16>, settings: Value) -> Citp {
        let spec = Catalog::source_tree().devices["citp-msex"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        Citp::new(OpenContext {
            host: HOST,
            host_name: None,
            port,
            model: "media-server".into(),
            channels: None,
            settings,
            monitor,
        })
        .unwrap()
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    /// The MSEX cookies of the messages sent.
    fn cookies(actions: &[Action]) -> Vec<String> {
        sent(actions)
            .iter()
            .map(|m| match &m[16..20] {
                b"MSEX" => String::from_utf8_lossy(&m[22..26]).to_string(),
                _ => String::from_utf8_lossy(&m[20..24]).to_string(),
            })
            .collect()
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

    fn completed(actions: &[Action]) -> Vec<(CommandId, CommandResult)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Complete { id, result } => Some((*id, result.clone())),
                _ => None,
            })
            .collect()
    }

    fn ucs2(text: &str) -> Vec<u8> {
        let mut b: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        b.extend_from_slice(&[0, 0]);
        b
    }

    fn sinf12() -> Vec<u8> {
        let mut b = b"550e8400-e29b-41d4-a716-446655440000".to_vec();
        b.extend(ucs2("Media Master"));
        b.extend([6, 2, 1]);
        b.push(3);
        for v in [0x0100u16, 0x0101, 0x0102] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&0b11u16.to_le_bytes()); // media and effects
        b.push(2);
        b.extend_from_slice(b"JPEGRGB8");
        b.push(1);
        b.extend_from_slice(b"JPEG");
        b.push(2);
        b.extend(ucs1("ArtNet/0/0/1"));
        b.extend(ucs1("ArtNet/0/0/41"));
        msex((1, 2), b"SInf", &b, 0)
    }

    fn lsta12(position: u32, flags: u32) -> Vec<u8> {
        // One layer: layer 0, output 0, media (1), library {1, 2}, media 3.
        let mut b = vec![1u8, 0, 0, 1, 1, 2, 0, 0, 3];
        b.extend(ucs2("Clouds.mov"));
        b.extend_from_slice(&position.to_le_bytes());
        b.extend_from_slice(&250u32.to_le_bytes());
        b.push(25);
        b.extend_from_slice(&flags.to_le_bytes());
        msex((1, 2), b"LSta", &b, 0)
    }

    fn elin12(parent_level: u8, entries: &[([u8; 4], &str, u16, u16)]) -> Vec<u8> {
        let _ = parent_level;
        let mut b = vec![1u8];
        b.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        for (id, name, libs, elements) in entries {
            b.extend_from_slice(id);
            b.extend_from_slice(&7u32.to_le_bytes());
            b.extend([0, 255]);
            b.extend(ucs2(name));
            b.extend_from_slice(&libs.to_le_bytes());
            b.extend_from_slice(&elements.to_le_bytes());
        }
        msex((1, 2), b"ELIn", &b, 0)
    }

    fn mein12(lib: [u8; 4], names: &[&str]) -> Vec<u8> {
        let mut b = lib.to_vec();
        b.extend_from_slice(&(names.len() as u16).to_le_bytes());
        for (i, n) in names.iter().enumerate() {
            b.push(i as u8);
            b.extend_from_slice(&3u32.to_le_bytes());
            b.extend([i as u8, i as u8]);
            b.extend(ucs2(n));
            b.extend_from_slice(&1_700_000_000u64.to_le_bytes());
            b.extend_from_slice(&1920u16.to_le_bytes());
            b.extend_from_slice(&1080u16.to_le_bytes());
            b.extend_from_slice(&250u32.to_le_bytes());
            b.push(25);
        }
        msex((1, 2), b"MEIn", &b, 0)
    }

    fn connected(m: &mut Citp) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, TCP, TcpInput::Connected);
        cx.take()
    }

    #[test]
    fn the_header_is_laid_out_as_the_specification_gives_it() {
        let m = msex((1, 2), b"GVSr", &[], 7);
        assert_eq!(&m[..4], b"CITP");
        assert_eq!(&m[4..6], &[1, 0]);
        assert_eq!(u16::from_le_bytes([m[6], m[7]]), 7);
        assert_eq!(u32::from_le_bytes([m[8], m[9], m[10], m[11]]), 26);
        assert_eq!(&m[12..16], &[1, 0, 0, 0]);
        assert_eq!(&m[16..20], b"MSEX");
        assert_eq!(&m[20..22], &[1, 2]);
        assert_eq!(&m[22..26], b"GVSr");
        // CInf names 1.0, 1.1 and 1.2, each MSB major, LSB minor.
        let c = cinf();
        assert_eq!(&c[26..], &[3, 0, 1, 1, 1, 2, 1]);
        let p = pnam("Desk");
        assert_eq!(&p[16..24], b"PINFPNam");
        assert_eq!(&p[24..], b"Desk\0");
    }

    #[test]
    fn requests_follow_each_version() {
        let lib = LibId::parse("1-4").unwrap();
        assert_eq!(lib.bytes(), [2, 1, 4, 0]);
        assert_eq!(lib.parent().bytes(), [1, 1, 0, 0]);
        assert_eq!(
            &geli((1, 2), 1, LibId::ROOT, 1)[26..],
            &[1, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(&geli((1, 1), 1, lib, 1)[26..], &[1, 2, 1, 4, 0, 0]);
        assert_eq!(&geli((1, 0), 2, LibId::ROOT, 1)[26..], &[2, 0]);
        assert_eq!(&gein((1, 2), 1, lib, 1)[26..], &[1, 2, 1, 4, 0, 0, 0]);
        assert_eq!(
            &gein((1, 0), 1, LibId::parse("3").unwrap(), 1)[26..],
            &[1, 3, 0]
        );
        let th = Thumb {
            format: *b"JPEG",
            width: 128,
            height: 72,
            preserve_aspect: true,
        };
        assert_eq!(
            &geth((1, 2), 1, lib, 5, th, 1)[26..],
            &[b'J', b'P', b'E', b'G', 128, 0, 72, 0, 1, 1, 2, 1, 4, 0, 1, 0, 5]
        );
        assert_eq!(
            &gelt((1, 1), 1, lib, th, 1)[26..],
            &[b'J', b'P', b'E', b'G', 128, 0, 72, 0, 1, 1, 1, 2, 1, 4, 0]
        );
        assert_eq!(
            &rqst((1, 2), 3, (640, 360), 10, 1)[26..],
            &[3, 0, b'J', b'P', b'E', b'G', 128, 2, 104, 1, 10, 5]
        );
    }

    #[test]
    fn greets_reads_the_server_and_walks_the_libraries() {
        let mut m = module(true, Some(4811), json!({}));
        let actions = connected(&mut m);
        assert_eq!(cookies(&actions), ["PNam", "CInf"]);
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, TCP, TcpInput::Data(sinf12()));
        let actions = cx.take();
        let st = state(&actions);
        assert_eq!(st["server"]["product"], "Media Master");
        assert_eq!(st["server"]["version"], "6.2.1");
        assert_eq!(st["server"]["uuid"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(st["server"]["msex_versions"], "1.0,1.1,1.2");
        assert_eq!(st["server"]["library_types"], "media,effects");
        assert_eq!(st["server"]["thumbnail_formats"], "jpeg,rgb8");
        assert_eq!(st["server"]["msex_version"], "1.2");
        assert_eq!(st["layers"]["2"]["dmx_source"], "ArtNet/0/0/41");
        // The media libraries first, one request at a time.
        assert_eq!(cookies(&actions), ["GELI"]);

        let mut cx = Cx::new(30);
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Data(elin12(
                0,
                &[
                    ([1, 0, 0, 0], "Images", 1, 2),
                    ([1, 1, 0, 0], "Empty", 0, 0),
                ],
            )),
        );
        let actions = cx.take();
        assert!(actions.contains(&Action::RoundTrip(20)));
        let st = state(&actions);
        assert_eq!(st["libraries"]["media"]["0"]["name"], "Images");
        assert_eq!(st["libraries"]["media"]["0"]["element_count"], 2);
        assert_eq!(st["libraries"]["media"]["0"]["serial"], 7);
        // The effects' libraries come next; Images' sub-library and
        // elements are queued behind.
        assert_eq!(cookies(&actions), ["GELI"]);
        assert_eq!(&sent(&actions)[0][26..], &[2, 0, 0, 0, 0, 0, 0]);
        let mut cx = Cx::new(40);
        let mut empty = vec![2u8];
        empty.extend_from_slice(&0u16.to_le_bytes());
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Data(msex((1, 2), b"ELIn", &empty, 0)),
        );
        assert_eq!(cookies(&cx.take()), ["GVSr"]);
        let mut cx = Cx::new(50);
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Data(msex((1, 2), b"VSrc", &[0, 0], 0)),
        );
        let actions = cx.take();
        assert_eq!(cookies(&actions), ["GELI"]);
        assert_eq!(&sent(&actions)[0][26..], &[1, 1, 0, 0, 0, 0, 0]);
        let mut cx = Cx::new(60);
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Data(elin12(1, &[([2, 0, 0, 0], "Stills", 0, 1)])),
        );
        let actions = cx.take();
        assert_eq!(cookies(&actions), ["GEIn"]);
        assert_eq!(&sent(&actions)[0][26..], &[1, 1, 0, 0, 0, 0, 0]);
        let mut cx = Cx::new(70);
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Data(mein12([1, 0, 0, 0], &["Primo.gif", "Secundo.gif"])),
        );
        let actions = cx.take();
        let st = state(&actions);
        let e = &st["libraries"]["media"]["0"]["elements"]["1"];
        assert_eq!(e["name"], "Secundo.gif");
        assert_eq!(e["width"], 1920);
        assert_eq!(e["length"], 250);
        assert_eq!(e["version"], 1_700_000_000u64);
        assert_eq!(cookies(&actions), ["GEIn"]);
        assert_eq!(&sent(&actions)[0][26..], &[1, 2, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn a_library_the_server_stops_listing_leaves_state() {
        let mut m = module(true, Some(4811), json!({}));
        connected(&mut m);
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, TCP, TcpInput::Data(sinf12()));
        let mut cx = Cx::new(20);
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Data(elin12(
                0,
                &[([1, 0, 0, 0], "A", 0, 0), ([1, 1, 0, 0], "B", 0, 0)],
            )),
        );
        // An unsolicited list of the root without B.
        let mut cx = Cx::new(30);
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Data(elin12(0, &[([1, 0, 0, 0], "A", 0, 0)])),
        );
        let mut st = json!({"libraries": {"media": {"0": {"name": "A"}, "1": {"name": "B"}}}});
        for a in cx.take() {
            if let Action::State(p) = a {
                merge_patch(&mut st, &p);
            }
        }
        assert!(st["libraries"]["media"].get("1").is_none(), "{st}");
    }

    #[test]
    fn layer_status_is_kept_and_repeats_report_nothing() {
        let mut m = module(true, Some(4811), json!({}));
        connected(&mut m);
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, TCP, TcpInput::Data(lsta12(100, 0x05)));
        let st = state(&cx.take());
        let l = &st["layers"]["1"];
        assert_eq!(l["output"], 1);
        assert_eq!(l["library_type"], "media");
        assert_eq!(l["library"], "2");
        assert_eq!(l["media_number"], 3);
        assert_eq!(l["media_name"], "Clouds.mov");
        assert_eq!(l["position"], 100);
        assert_eq!(l["length"], 250);
        assert_eq!(l["fps"], 25);
        assert_eq!(l["playing"], true);
        assert_eq!(l["looping"], true);
        assert_eq!(l["paused"], false);
        let mut cx = Cx::new(20);
        m.tcp(&mut cx, TCP, TcpInput::Data(lsta12(100, 0x05)));
        assert_eq!(state(&cx.take()), json!({}));
        let mut cx = Cx::new(30);
        m.tcp(&mut cx, TCP, TcpInput::Data(lsta12(101, 0x21)));
        let st = state(&cx.take());
        assert_eq!(st["layers"]["1"]["position"], 101);
        assert_eq!(st["layers"]["1"]["paused"], true);
    }

    #[test]
    fn a_1_0_server_gets_1_1_requests_after_a_grace() {
        let mut m = module(true, Some(4811), json!({}));
        connected(&mut m);
        let mut b = ucs2("Old Server");
        b.extend([2, 3, 1]);
        b.extend(ucs1("ArtNet/0/0/1"));
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, TCP, TcpInput::Data(msex((1, 0), b"SInf", &b, 0)));
        let actions = cx.take();
        assert_eq!(state(&actions)["server"]["version"], "2.3");
        assert!(cookies(&actions).is_empty());
        let mut cx = Cx::new(1010);
        m.timer(&mut cx, SINF_WAIT);
        let actions = cx.take();
        assert_eq!(state(&actions)["server"]["msex_version"], "1.1");
        let s = sent(&actions);
        assert_eq!(&s[0][20..26], &[1, 1, b'G', b'E', b'L', b'I']);
        assert_eq!(&s[0][26..], &[1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn opened_for_commands_only_reads_nothing_but_keeps_layer_status() {
        let mut m = module(false, Some(4811), json!({}));
        let actions = connected(&mut m);
        assert_eq!(cookies(&actions), ["PNam", "CInf"]);
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, TCP, TcpInput::Data(sinf12()));
        let actions = cx.take();
        assert!(cookies(&actions).is_empty(), "{:?}", cookies(&actions));
        let mut cx = Cx::new(20);
        m.tcp(&mut cx, TCP, TcpInput::Data(lsta12(5, 1)));
        assert_eq!(state(&cx.take())["layers"]["1"]["position"], 5);
        // Commands still work.
        let mut cx = Cx::new(30);
        m.command(&mut cx, 9, "get_video_sources", &Params::new());
        assert_eq!(cookies(&cx.take()), ["GVSr"]);
        let mut body = 1u16.to_le_bytes().to_vec();
        body.extend_from_slice(&4u16.to_le_bytes());
        body.extend(ucs2("Output 1"));
        body.extend([0, 0xFF]);
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&1920u16.to_le_bytes());
        body.extend_from_slice(&1080u16.to_le_bytes());
        let mut cx = Cx::new(45);
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Data(msex((1, 2), b"VSrc", &body, 0)),
        );
        let actions = cx.take();
        assert!(actions.contains(&Action::RoundTrip(15)));
        assert_eq!(state(&actions)["video_sources"]["4"]["name"], "Output 1");
        assert_eq!(state(&actions)["video_sources"]["4"]["output"], 1);
        assert_eq!(state(&actions)["video_sources"]["4"]["layer"], 0);
        let done = completed(&actions);
        assert_eq!(done.len(), 1);
        let Ok(Outcome::Value { value }) = &done[0].1 else {
            panic!("{done:?}")
        };
        assert_eq!(value[0]["id"], 4);
    }

    #[test]
    fn an_element_thumbnail_is_returned_and_a_nack_fails_the_command() {
        let mut m = module(true, Some(4811), json!({}));
        connected(&mut m);
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, TCP, TcpInput::Data(sinf12()));
        let params = json!({"library_type": "media", "library": "0", "element": 1, "format": "jpeg",
                            "width": 64, "height": 36, "preserve_aspect": true});
        let mut cx = Cx::new(20);
        m.command(
            &mut cx,
            3,
            "get_element_thumbnail",
            params.as_object().unwrap(),
        );
        // Behind the GELI in flight.
        assert!(cookies(&cx.take()).is_empty());
        let mut cx = Cx::new(30);
        m.tcp(&mut cx, TCP, TcpInput::Data(elin12(0, &[])));
        assert_eq!(cookies(&cx.take()), ["GETh"]);
        let mut b = vec![1u8, 1, 0, 0, 0, 1];
        b.extend_from_slice(b"JPEG");
        b.extend_from_slice(&64u16.to_le_bytes());
        b.extend_from_slice(&36u16.to_le_bytes());
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend([0xFF, 0xD8, 0xFF]);
        let mut cx = Cx::new(40);
        m.tcp(&mut cx, TCP, TcpInput::Data(msex((1, 2), b"EThn", &b, 0)));
        let done = completed(&cx.take());
        let Ok(Outcome::Value { value }) = &done[0].1 else {
            panic!("{done:?}")
        };
        assert_eq!(value["format"], "jpeg");
        assert_eq!(value["element"], 1);
        assert_eq!(value["data"], "/9j/");
        // A library thumbnail the server does not offer.
        let params = json!({"library_type": "media", "library": "0", "format": "png"});
        let mut cx = Cx::new(50);
        m.command(
            &mut cx,
            4,
            "get_library_thumbnail",
            params.as_object().unwrap(),
        );
        // The effects' libraries were asked for meanwhile; then GELT goes.
        let mut cx = Cx::new(55);
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Data(msex((1, 2), b"ELIn", &[2, 0, 0], 0)),
        );
        assert_eq!(cookies(&cx.take()), ["GELT"]);
        let mut cx = Cx::new(60);
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Data(msex((1, 2), b"Nack", b"GELT", 0)),
        );
        let done = completed(&cx.take());
        assert!(
            matches!(&done[0], (4, Err(CommandError::DeviceError { .. }))),
            "{done:?}"
        );
    }

    #[test]
    fn a_server_without_a_port_is_found_by_its_ploc() {
        let mut m = module(true, None, json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let actions = cx.take();
        assert!(actions.contains(&Action::UdpJoin {
            socket: UDP,
            group: GROUPS[0]
        }));
        assert!(!actions.iter().any(|a| matches!(a, Action::TcpOpen { .. })));
        let mut body = b"PLoc".to_vec();
        body.extend_from_slice(&4811u16.to_le_bytes());
        body.extend(ucs1("MediaServer"));
        body.extend(ucs1("Media Master"));
        body.extend(ucs1("Running"));
        let ploc = citp(b"PINF", &body, 0);
        // Another host's PLoc is not this server's.
        let mut cx = Cx::new(10);
        m.datagram(&mut cx, UDP, "10.0.0.10:4809".parse().unwrap(), &ploc);
        assert!(cx.take().is_empty());
        let mut cx = Cx::new(20);
        m.datagram(&mut cx, UDP, SocketAddr::new(HOST, 4809), &ploc);
        let actions = cx.take();
        assert_eq!(state(&actions)["peer"]["state"], "Running");
        assert!(actions.contains(&Action::TcpOpen {
            socket: TCP,
            to: SocketAddr::new(HOST, 4811)
        }));
    }

    #[test]
    fn the_live_stream_requests_frames_and_publishes_jpeg() {
        let mut m = module(true, Some(4811), json!({"stream_source": 2}));
        connected(&mut m);
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, TCP, TcpInput::Data(sinf12()));
        let mut cx = Cx::new(20);
        m.stream_watch(&mut cx, LIVE, true);
        let actions = cx.take();
        assert!(actions.iter().any(|a| matches!(a, Action::UdpOpen { .. })));
        assert_eq!(cookies(&actions), ["RqSt"]);
        // A frame in fragments, multicast with this server's UUID.
        let jpeg = [0xFFu8, 0xD8, 0x01, 0x02, 0xFF, 0xD9];
        let fragment = |offset: usize, part: &[u8], index: u16| {
            let mut b = b"550e8400-e29b-41d4-a716-446655440000".to_vec();
            b.extend_from_slice(&2u16.to_le_bytes());
            b.extend_from_slice(b"fJPG");
            b.extend_from_slice(&640u16.to_le_bytes());
            b.extend_from_slice(&360u16.to_le_bytes());
            let mut data = 9u32.to_le_bytes().to_vec();
            data.extend_from_slice(&2u16.to_le_bytes());
            data.extend_from_slice(&index.to_le_bytes());
            data.extend_from_slice(&(offset as u32).to_le_bytes());
            data.extend_from_slice(part);
            b.extend_from_slice(&(data.len() as u16).to_le_bytes());
            b.extend(data);
            msex((1, 2), b"StFr", &b, 0)
        };
        let from = SocketAddr::new(HOST, 4809);
        let mut cx = Cx::new(30);
        m.datagram(&mut cx, UDP, from, &fragment(3, &jpeg[3..], 1));
        assert!(cx.take().is_empty());
        let mut cx = Cx::new(31);
        m.datagram(&mut cx, UDP, from, &fragment(0, &jpeg[..3], 0));
        assert!(cx.take().contains(&Action::Frame {
            stream: LIVE,
            format: "jpeg",
            data: jpeg.to_vec()
        }));
        let mut cx = Cx::new(40);
        m.stream_watch(&mut cx, LIVE, false);
        assert!(cx.take().contains(&Action::UdpClose { socket: UDP }));
    }

    #[test]
    fn a_silent_server_is_connected_again() {
        let mut m = module(true, Some(4811), json!({}));
        connected(&mut m);
        let mut cx = Cx::new(QUIET);
        m.timer(&mut cx, SILENCE);
        let actions = cx.take();
        assert!(actions.contains(&Action::TcpClose { socket: TCP }));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        assert!(actions.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
    }

    #[test]
    fn messages_split_across_reads_and_fragments_are_joined() {
        let mut m = module(true, Some(4811), json!({}));
        connected(&mut m);
        let whole = lsta12(7, 1);
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, TCP, TcpInput::Data(whole[..10].to_vec()));
        assert_eq!(state(&cx.take()), json!({}));
        let mut cx = Cx::new(11);
        let mut rest = whole[10..].to_vec();
        rest.extend_from_slice(b"junk");
        m.tcp(&mut cx, TCP, TcpInput::Data(rest));
        assert_eq!(state(&cx.take())["layers"]["1"]["position"], 7);
        // Two parts of one message.
        let body = &whole[HEADER..];
        let (a, b) = body.split_at(5);
        let part = |p: u16, bytes: &[u8]| {
            let mut m = citp(b"MSEX", bytes, 0);
            m[12..14].copy_from_slice(&2u16.to_le_bytes());
            m[14..16].copy_from_slice(&p.to_le_bytes());
            m
        };
        let mut cx = Cx::new(12);
        let mut data = part(0, a);
        data.extend(part(1, b));
        m.layers.clear();
        m.tcp(&mut cx, TCP, TcpInput::Data(data));
        assert_eq!(state(&cx.take())["layers"]["1"]["position"], 7);
    }
}
