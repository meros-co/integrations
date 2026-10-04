//! TSL UMD tally, in both directions, from TSL's "TSL UMD Protocols" document
//! (V3.1, V4.0 and V5.0).
//!
//! - V3.1: `0x80 + address`, a control byte (tally bits 0-3, brightness bits
//!   4-5), then 16 ASCII characters. Over UDP, one packet per datagram.
//! - V4.0: V3.1, then a checksum (two's complement of the sum of the V3.1
//!   bytes, modulo 128), a VBC byte (minor version in bits 4-6, XDATA count in
//!   bits 0-3) and two XDATA bytes of 2-bit colour tallies (LH, text, RH) for
//!   the left and right displays.
//! - V5.0: little-endian PBC (byte count of what follows), VER, FLAGS (bit 0
//!   UTF-16LE text, bit 1 screen control), SCREEN, then display messages:
//!   INDEX, CONTROL (RH tally bits 0-1, text 2-3, LH 4-5, brightness 6-7,
//!   bit 15 control data) and LENGTH-prefixed text. Over TCP, each packet is
//!   preceded by DLE STX (0xFE 0x02) and a DLE in the packet is doubled.
//!
//! Opening for commands only changes nothing here. The listener only receives,
//! which is its whole purpose, and asks nothing of the switcher; the sender
//! sends only what commands ask. TSL is one-way, so no request is answered and
//! no latency is measured.

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const SOCKET: Key = "tsl";
const DLE: u8 = 0xFE;
const STX: u8 = 0x02;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const RETRY: Key = "retry";

const COLOURS: [&str; 4] = ["off", "red", "green", "amber"];

fn colour(bits: u16) -> &'static str {
    COLOURS[(bits & 3) as usize]
}

fn colour_bits(name: &str) -> u8 {
    COLOURS.iter().position(|c| *c == name).unwrap_or(0) as u8
}

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

// ── Decoding ─────────────────────────────────────────────────────────────

/// One display's update, as decoded.
#[derive(Debug, Default, PartialEq)]
struct Update {
    screen: u16,
    index: u16,
    text: Option<String>,
    brightness: u8,
    /// LH, text, RH colour tallies.
    tally: Option<[&'static str; 3]>,
    /// V4.0's right display.
    right: Option<[&'static str; 3]>,
    /// V3.1 / V4.0 on/off tallies 1-4.
    lamps: Option<[bool; 4]>,
}

/// A datagram, decoded: the version and its display updates. A V5.0 packet
/// begins with its own byte count less two. A V3.1 or V4.0 datagram is exactly
/// 18 or 22 bytes and begins with 0x80 or more, so read as a byte count it
/// would claim at least 128 bytes, never the 16 or 20 that follow: the two
/// cannot be confused. V5.0 is checked first, because a long V5.0 packet can
/// also begin with 0x80 or more.
fn decode(data: &[u8]) -> Option<(&'static str, Vec<Update>)> {
    if data.len() >= 6 && le16(data, 0) as usize == data.len() - 2 {
        return decode_v5(data);
    }
    if (data.len() == 18 || data.len() == 22) && data[0] >= 0x80 {
        return decode_v3(data);
    }
    None
}

fn decode_v3(data: &[u8]) -> Option<(&'static str, Vec<Update>)> {
    let control = data[1];
    // Bit 6 set: command data, not display data; not decoded.
    if control & 0x40 != 0 {
        return None;
    }
    let text: String = data[2..18]
        .iter()
        .map(|&b| {
            if (0x20..=0x7e).contains(&b) {
                b as char
            } else {
                ' '
            }
        })
        .collect();
    let mut update = Update {
        index: (data[0] - 0x80) as u16,
        text: Some(text.trim_end().to_string()),
        brightness: (control >> 4) & 3,
        lamps: Some([0, 1, 2, 3].map(|i| control & (1 << i) != 0)),
        ..Update::default()
    };
    let mut version = "3.1";
    // V4.0: checksum, VBC with minor version 0 and two XDATA bytes.
    if data.len() >= 22 && data[19] & 0x70 == 0 && data[19] & 0x0f == 2 {
        let sum: u32 = data[..18].iter().map(|&b| b as u32).sum();
        let checksum = ((0x100 - (sum & 0xff)) & 0x7f) as u8;
        if data[18] == checksum {
            version = "4.0";
            let xl = data[20] as u16;
            let xr = data[21] as u16;
            update.tally = Some([colour(xl >> 4), colour(xl >> 2), colour(xl)]);
            update.right = Some([colour(xr >> 4), colour(xr >> 2), colour(xr)]);
        }
    }
    Some((version, vec![update]))
}

fn decode_v5(data: &[u8]) -> Option<(&'static str, Vec<Update>)> {
    let flags = data[3];
    let unicode = flags & 1 != 0;
    // Screen control: not defined in V5.0.
    if flags & 2 != 0 {
        return Some(("5.0", Vec::new()));
    }
    let screen = le16(data, 4);
    let mut at = 6;
    let mut out = Vec::new();
    while at + 4 <= data.len() {
        let index = le16(data, at);
        let control = le16(data, at + 2);
        at += 4;
        if control & 0x8000 != 0 {
            // Control data: not defined in V5.0, and its length is unknown,
            // so nothing after it can be read.
            break;
        }
        if at + 2 > data.len() {
            break;
        }
        let length = le16(data, at) as usize;
        at += 2;
        let Some(bytes) = data.get(at..at + length) else {
            break;
        };
        at += length;
        let text = if unicode {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        } else {
            String::from_utf8_lossy(bytes).into_owned()
        };
        out.push(Update {
            screen,
            index,
            text: Some(text),
            brightness: ((control >> 6) & 3) as u8,
            tally: Some([colour(control >> 4), colour(control >> 2), colour(control)]),
            ..Update::default()
        });
    }
    Some(("5.0", out))
}

// ── Encoding ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum Version {
    V31,
    V40,
    V50Udp,
    V50Tcp,
}

struct Display {
    screen: u16,
    index: u16,
    text: String,
    brightness: u8,
    lh: u8,
    text_tally: u8,
    rh: u8,
    lamps: u8,
}

fn encode_v3(d: &Display, v4: bool) -> Vec<u8> {
    let mut p = Vec::with_capacity(22);
    p.push(0x80 + d.index as u8);
    p.push((d.lamps & 0x0f) | ((d.brightness & 3) << 4));
    let mut chars: Vec<u8> = d
        .text
        .chars()
        .map(|c| {
            if (' '..='~').contains(&c) {
                c as u8
            } else {
                b'?'
            }
        })
        .take(16)
        .collect();
    chars.resize(16, b' ');
    p.extend_from_slice(&chars);
    if v4 {
        let sum: u32 = p.iter().map(|&b| b as u32).sum();
        p.push(((0x100 - (sum & 0xff)) & 0x7f) as u8);
        p.push(0x02); // minor version 0, two XDATA bytes
        p.push((d.lh << 4) | (d.text_tally << 2) | d.rh);
        p.push(0); // the right display's tallies: off
    }
    p
}

fn encode_v5(d: &Display, unicode: bool) -> Vec<u8> {
    let text: Vec<u8> = if unicode {
        d.text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    } else {
        d.text
            .chars()
            .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
            .collect()
    };
    let control: u16 = (d.rh as u16)
        | ((d.text_tally as u16) << 2)
        | ((d.lh as u16) << 4)
        | ((d.brightness as u16 & 3) << 6);
    let mut body = Vec::new();
    body.push(0); // VER: minor version 0
    body.push(unicode as u8); // FLAGS
    body.extend_from_slice(&d.screen.to_le_bytes());
    body.extend_from_slice(&d.index.to_le_bytes());
    body.extend_from_slice(&control.to_le_bytes());
    body.extend_from_slice(&(text.len() as u16).to_le_bytes());
    body.extend_from_slice(&text);
    let mut p = (body.len() as u16).to_le_bytes().to_vec();
    p.extend_from_slice(&body);
    p
}

/// DLE STX before the packet; every DLE in it doubled. Byte counts are not
/// affected by the stuffing.
fn wrap_stream(packet: &[u8]) -> Vec<u8> {
    let mut out = vec![DLE, STX];
    for &b in packet {
        out.push(b);
        if b == DLE {
            out.push(DLE);
        }
    }
    out
}

// ── Receiving ────────────────────────────────────────────────────────────

/// Undoes the V5.0 stream wrapper: waits for DLE STX, un-doubles DLE, and
/// ends each packet by its own byte count. A DLE STX inside a packet starts a
/// new one, which recovers from a truncated packet.
#[derive(Default)]
struct Deframer {
    packet: Option<Vec<u8>>,
    after_dle: bool,
}

impl Deframer {
    fn feed(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for &b in data {
            if self.after_dle {
                self.after_dle = false;
                match b {
                    STX => {
                        self.packet = Some(Vec::new());
                        continue;
                    }
                    DLE => {} // a doubled DLE: one data byte
                    _ => {
                        // Not valid after DLE: drop the packet and resync.
                        self.packet = None;
                        continue;
                    }
                }
            } else if b == DLE {
                self.after_dle = true;
                continue;
            }
            let Some(packet) = self.packet.as_mut() else {
                continue;
            };
            packet.push(b);
            if packet.len() >= 2 && packet.len() == le16(packet, 0) as usize + 2 {
                out.push(self.packet.take().unwrap());
            } else if packet.len() > 2050 {
                // Beyond the 2048-byte maximum: not a packet.
                self.packet = None;
            }
        }
        out
    }
}

/// Splits a TCP stream of V3.1 packets: a header byte (0x80 + address)
/// followed by 17 bytes below 0x80. A byte of 0x80 or more inside a packet
/// starts a new one, which recovers from a truncated packet.
#[derive(Default)]
struct V3Stream {
    packet: Vec<u8>,
}

impl V3Stream {
    fn feed(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for &b in data {
            if b >= 0x80 {
                self.packet = vec![b];
            } else if !self.packet.is_empty() {
                self.packet.push(b);
                if self.packet.len() == 18 {
                    out.push(std::mem::take(&mut self.packet));
                }
            }
        }
        out
    }
}

/// How the listener receives.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Receive {
    Udp,
    /// V5.0 over TCP: the switcher connects to us.
    V5Tcp,
    /// V3.1 over TCP (Ross Carbonite): the switcher connects to us.
    V3Tcp,
}

pub(crate) struct Listener {
    port: u16,
    receive: Receive,
    deframer: Deframer,
    v3: V3Stream,
    /// Displays seen per screen, for V5.0 broadcasts.
    known: BTreeMap<u16, BTreeSet<u16>>,
}

impl Listener {
    pub(crate) fn new(port: u16, model: &str) -> Listener {
        Listener {
            port,
            receive: match model {
                "tsl-umd-5-tcp" => Receive::V5Tcp,
                "tsl-umd-3-tcp" => Receive::V3Tcp,
                _ => Receive::Udp,
            },
            deframer: Deframer::default(),
            v3: V3Stream::default(),
            known: BTreeMap::new(),
        }
    }

    fn receive(&mut self, cx: &mut Cx, data: &[u8]) {
        cx.alive();
        match decode(data) {
            Some((version, updates)) => {
                let patch = self.patch(version, updates);
                cx.state(patch);
            }
            None => cx.log(
                Level::Debug,
                format!("{} bytes that are not a TSL UMD packet", data.len()),
            ),
        }
    }

    fn patch(&mut self, version: &str, updates: Vec<Update>) -> Value {
        let mut screens = Map::new();
        for u in updates {
            let targets: Vec<(u16, u16)> = match (u.screen, u.index) {
                (0xFFFF, 0xFFFF) => self
                    .known
                    .iter()
                    .flat_map(|(s, set)| set.iter().map(move |i| (*s, *i)))
                    .collect(),
                (0xFFFF, i) => self.known.keys().map(|s| (*s, i)).collect(),
                (s, 0xFFFF) => self
                    .known
                    .get(&s)
                    .map(|set| set.iter().map(|i| (s, *i)).collect())
                    .unwrap_or_default(),
                (s, i) => {
                    self.known.entry(s).or_default().insert(i);
                    vec![(s, i)]
                }
            };
            let mut display = json!({"brightness": u.brightness});
            if let Some(t) = &u.text {
                display["text"] = json!(t);
            }
            if let Some([lh, text, rh]) = u.tally {
                display["tally"] = json!({"lh": lh, "text": text, "rh": rh});
            }
            if let Some([lh, text, rh]) = u.right {
                display["right_display"] = json!({"lh": lh, "text": text, "rh": rh});
            }
            if let Some(lamps) = u.lamps {
                display["lamps"] =
                    json!({"1": lamps[0], "2": lamps[1], "3": lamps[2], "4": lamps[3]});
            }
            for (s, i) in targets {
                let screen = screens
                    .entry(s.to_string())
                    .or_insert_with(|| json!({"displays": {}}));
                screen["displays"][i.to_string()] = display.clone();
            }
        }
        json!({"protocol": version, "screens": screens})
    }
}

impl Module for Listener {
    fn start(&mut self, cx: &mut Cx) {
        if self.receive != Receive::Udp {
            cx.tcp_listen(SOCKET, self.port);
            cx.connection(Connection::Disconnected {
                reason: "waiting for the switcher to connect".into(),
            });
        } else {
            cx.udp_open(SOCKET, Bind::Shared(self.port));
            cx.connection(Connection::Unmonitored);
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.deframer = Deframer::default();
                self.v3 = V3Stream::default();
                cx.connection(Connection::Connected);
            }
            TcpInput::Data(data) => {
                let packets = match self.receive {
                    Receive::V3Tcp => self.v3.feed(&data),
                    _ => self.deframer.feed(&data),
                };
                for packet in packets {
                    self.receive(cx, &packet);
                }
            }
            // Still listening: the switcher reconnects when it can.
            TcpInput::Closed { reason } => cx.connection(Connection::Disconnected {
                reason: format!("the switcher's connection closed: {reason}"),
            }),
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        cx.log(Level::Warning, message.to_string());
        cx.connection(Connection::Disconnected {
            reason: message.to_string(),
        });
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, _params: &Params) {
        cx.complete(
            id,
            Err(CommandError::UnknownCommand {
                command: name.into(),
            }),
        );
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, _from: SocketAddr, data: &[u8]) {
        self.receive(cx, data);
    }

    fn timer(&mut self, _cx: &mut Cx, _key: Key) {}
}

// ── Sending ──────────────────────────────────────────────────────────────

pub(crate) struct Sender {
    device: SocketAddr,
    version: Version,
    unicode: bool,
    connected: bool,
    retry_after: Millis,
}

impl Sender {
    pub(crate) fn new(ctx: &OpenContext, port: u16) -> Sender {
        let version = match ctx.model.as_str() {
            "tsl-3-1" => Version::V31,
            "tsl-4-0" => Version::V40,
            "tsl-5-0-tcp" => Version::V50Tcp,
            _ => Version::V50Udp,
        };
        Sender {
            device: SocketAddr::new(ctx.host, port),
            version,
            unicode: ctx
                .settings
                .get("unicode")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            connected: false,
            retry_after: RETRY_MIN,
        }
    }

    fn packet(&self, d: &Display) -> Vec<u8> {
        match self.version {
            Version::V31 => encode_v3(d, false),
            Version::V40 => encode_v3(d, true),
            Version::V50Udp => encode_v5(d, self.unicode),
            Version::V50Tcp => wrap_stream(&encode_v5(d, self.unicode)),
        }
    }
}

impl Module for Sender {
    fn start(&mut self, cx: &mut Cx) {
        if self.version == Version::V50Tcp {
            cx.connection(Connection::Connecting);
            cx.tcp_open(SOCKET, self.device);
        } else {
            cx.udp_open(SOCKET, Bind::Ephemeral);
            cx.connection(Connection::Unmonitored);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if name != "set_display" {
            cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            );
            return;
        }
        let int = |k: &str| params.get(k).and_then(Value::as_i64).unwrap_or(0);
        let text = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or("");
        let index = int("index");
        if matches!(self.version, Version::V31 | Version::V40) && index > 126 {
            cx.complete(
                id,
                Err(CommandError::InvalidParams {
                    message: "V3.1 and V4.0 display addresses are 0-126".into(),
                }),
            );
            return;
        }
        let display = Display {
            screen: int("screen") as u16,
            index: index as u16,
            text: text("text").to_string(),
            brightness: int("brightness") as u8,
            lh: colour_bits(text("lh")),
            text_tally: colour_bits(text("text_tally")),
            rh: colour_bits(text("rh")),
            lamps: int("lamps") as u8,
        };
        let packet = self.packet(&display);
        match self.version {
            Version::V50Tcp => {
                if !self.connected {
                    cx.complete(id, Err(CommandError::NotConnected));
                    return;
                }
                cx.tcp_send(SOCKET, packet);
            }
            _ => cx.udp_send(SOCKET, self.device, packet),
        }
        // TSL is one-way: nothing confirms a display applied it.
        cx.complete(id, Ok(Outcome::Unverified));
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.connected = true;
                self.retry_after = RETRY_MIN;
                cx.connection(Connection::Connected);
            }
            // Displays send nothing back.
            TcpInput::Data(_) => cx.alive(),
            TcpInput::Closed { reason } => {
                self.connected = false;
                cx.connection(Connection::Disconnected { reason });
                cx.set_timer(RETRY, self.retry_after);
                self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == RETRY {
            cx.connection(Connection::Connecting);
            cx.tcp_open(SOCKET, self.device);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    fn display(text: &str) -> Display {
        Display {
            screen: 0,
            index: 3,
            text: text.into(),
            brightness: 3,
            lh: 1,
            text_tally: 2,
            rh: 3,
            lamps: 0b0101,
        }
    }

    #[test]
    fn v3_1_layout() {
        let p = encode_v3(&display("CAM 1"), false);
        assert_eq!(p.len(), 18);
        assert_eq!(p[0], 0x83);
        // Tallies 1 and 3 on, full brightness.
        assert_eq!(p[1], 0b0011_0101);
        assert_eq!(&p[2..18], b"CAM 1           ");
    }

    #[test]
    fn v4_0_checksum_and_colours() {
        let p = encode_v3(&display("CAM 1"), true);
        assert_eq!(p.len(), 22);
        let sum: u32 = p[..18].iter().map(|&b| b as u32).sum();
        // Adding the checksum makes the sum 0 modulo 128.
        assert_eq!((sum + p[18] as u32) % 128, 0);
        assert_eq!(p[19], 0x02);
        // LH red, text green, RH amber.
        assert_eq!(p[20], 0b01_10_11);
        assert_eq!(p[21], 0);
    }

    #[test]
    fn v5_0_layout_is_little_endian() {
        let mut d = display("Lead");
        d.index = 0x0102;
        d.screen = 1;
        let p = encode_v5(&d, false);
        assert_eq!(
            p,
            [
                14,
                0, // PBC: 14 bytes follow
                0,
                0, // VER, FLAGS
                1,
                0, // SCREEN
                0x02,
                0x01, // INDEX
                0b11_01_10_11,
                0, // CONTROL: brightness 3, LH red, text green, RH amber
                4,
                0, // LENGTH
                b'L',
                b'e',
                b'a',
                b'd',
            ]
        );
        let u = encode_v5(&display("é"), true);
        assert_eq!(u[3], 1, "FLAGS bit 0: UTF-16LE");
        assert_eq!(&u[12..], &[0xe9, 0x00]);
    }

    #[test]
    fn tcp_wrapper_stuffs_dle() {
        assert_eq!(wrap_stream(&[1, 0xFE, 2]), [0xFE, 0x02, 1, 0xFE, 0xFE, 2]);
    }

    #[test]
    fn every_version_decodes_what_it_encodes() {
        let d = display("CAM 1");
        let (v, u) = decode(&encode_v3(&d, false)).unwrap();
        assert_eq!(v, "3.1");
        assert_eq!(u[0].index, 3);
        assert_eq!(u[0].text.as_deref(), Some("CAM 1"));
        assert_eq!(u[0].lamps, Some([true, false, true, false]));
        assert_eq!(u[0].tally, None);

        let (v, u) = decode(&encode_v3(&d, true)).unwrap();
        assert_eq!(v, "4.0");
        assert_eq!(u[0].tally, Some(["red", "green", "amber"]));
        assert_eq!(u[0].right, Some(["off", "off", "off"]));

        let (v, u) = decode(&encode_v5(&d, false)).unwrap();
        assert_eq!(v, "5.0");
        assert_eq!(u[0].tally, Some(["red", "green", "amber"]));
        assert_eq!(u[0].brightness, 3);
        let (_, u) = decode(&encode_v5(&display("Wörd"), true)).unwrap();
        assert_eq!(u[0].text.as_deref(), Some("Wörd"));
    }

    #[test]
    fn the_stream_wrapper_round_trips_across_reads() {
        let mut d = display("A");
        // An index whose bytes include DLE, so stuffing is exercised.
        d.index = 0x00FE;
        let packet = encode_v5(&d, false);
        let wrapped = [wrap_stream(&packet), wrap_stream(&packet)].concat();
        let mut f = Deframer::default();
        let mut out = Vec::new();
        for chunk in wrapped.chunks(3) {
            out.extend(f.feed(chunk));
        }
        assert_eq!(out, [packet.clone(), packet]);
    }

    #[test]
    fn a_long_v5_packet_is_not_read_as_v3() {
        let mut d = display(&"x".repeat(200));
        d.index = 1;
        let p = encode_v5(&d, false);
        assert!(
            p[0] >= 0x80,
            "the byte count's low byte looks like a V3.1 header"
        );
        let (v, u) = decode(&p).unwrap();
        assert_eq!(v, "5.0");
        assert_eq!(u[0].text.as_deref().map(str::len), Some(200));
    }

    #[test]
    fn v5_packets_carry_several_displays() {
        let mut p = vec![0, 0, 0, 0, 0, 0];
        for (i, t) in [(0u16, "A"), (1, "BB")] {
            p.extend_from_slice(&i.to_le_bytes());
            p.extend_from_slice(&0x0002u16.to_le_bytes()); // RH green
            p.extend_from_slice(&(t.len() as u16).to_le_bytes());
            p.extend_from_slice(t.as_bytes());
        }
        let pbc = (p.len() - 2) as u16;
        p[..2].copy_from_slice(&pbc.to_le_bytes());
        let (_, u) = decode(&p).unwrap();
        assert_eq!(u.len(), 2);
        assert_eq!(u[1].text.as_deref(), Some("BB"));
        assert_eq!(u[1].tally, Some(["off", "off", "green"]));
    }

    #[test]
    fn the_listener_reports_state_and_applies_broadcasts() {
        let mut l = Listener::new(8900, "tsl-umd");
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Shared(8900)
        }));
        assert!(a.contains(&Action::Connection(Connection::Unmonitored)));

        let from = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 5000);
        let mut cx = Cx::new(10);
        l.datagram(&mut cx, SOCKET, from, &encode_v5(&display("CAM 4"), false));
        let patch = cx
            .take()
            .into_iter()
            .find_map(|a| match a {
                Action::State(p) => Some(p),
                _ => None,
            })
            .unwrap();
        assert_eq!(patch["protocol"], "5.0");
        let d = &patch["screens"]["0"]["displays"]["3"];
        assert_eq!(d["text"], "CAM 4");
        assert_eq!(
            d["tally"],
            json!({"lh": "red", "text": "green", "rh": "amber"})
        );

        // A broadcast reaches every display already seen on the screen.
        let mut b = display("ALL");
        b.index = 0xFFFF;
        let mut cx = Cx::new(20);
        l.datagram(&mut cx, SOCKET, from, &encode_v5(&b, false));
        let patch = cx
            .take()
            .into_iter()
            .find_map(|a| match a {
                Action::State(p) => Some(p),
                _ => None,
            })
            .unwrap();
        assert_eq!(patch["screens"]["0"]["displays"]["3"]["text"], "ALL");
    }

    #[test]
    fn the_sender_encodes_per_model() {
        let ctx = |model: &str| OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)),
            host_name: None,
            port: Some(8900),
            model: model.into(),
            channels: None,
            settings: Params::new(),
            monitor: true,
        };
        let params = json!({"index": 3, "screen": 0, "text": "CAM 1", "brightness": 3,
                            "lh": "red", "text_tally": "green", "rh": "amber", "lamps": 5})
        .as_object()
        .unwrap()
        .clone();

        let mut s = Sender::new(&ctx("tsl-4-0"), 8900);
        let mut cx = Cx::new(0);
        s.start(&mut cx);
        s.command(&mut cx, 1, "set_display", &params);
        let a = cx.take();
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Unverified)
        }));
        let sent = a
            .iter()
            .find_map(|x| match x {
                Action::UdpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(sent, encode_v3(&display("CAM 1"), true));

        let mut big = params.clone();
        big.insert("index".into(), json!(200));
        let mut cx = Cx::new(1);
        s.command(&mut cx, 2, "set_display", &big);
        assert!(matches!(
            &cx.take()[..],
            [Action::Complete {
                result: Err(CommandError::InvalidParams { .. }),
                ..
            }]
        ));

        let mut s = Sender::new(&ctx("tsl-5-0-tcp"), 8900);
        let mut cx = Cx::new(0);
        s.start(&mut cx);
        s.tcp(&mut cx, SOCKET, TcpInput::Connected);
        s.command(&mut cx, 3, "set_display", &params);
        let sent = cx
            .take()
            .into_iter()
            .find_map(|x| match x {
                Action::TcpSend { data, .. } => Some(data),
                _ => None,
            })
            .unwrap();
        assert_eq!(&sent[..2], &[DLE, STX]);
    }

    #[test]
    fn opened_for_commands_only_the_sender_is_unchanged() {
        let ctx = OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)),
            host_name: None,
            port: Some(8900),
            model: "tsl-5-0".into(),
            channels: None,
            settings: Params::new(),
            monitor: false,
        };
        let mut s = Sender::new(&ctx, 8900);
        let mut cx = Cx::new(0);
        s.start(&mut cx);
        let a = cx.take();
        // Nothing is ever asked of a display: only the socket opens.
        assert!(!a.iter().any(|x| matches!(x, Action::UdpSend { .. })));
        let params = json!({"index": 1, "text": "CAM 2"})
            .as_object()
            .unwrap()
            .clone();
        let mut cx = Cx::new(1);
        s.command(&mut cx, 1, "set_display", &params);
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(x, Action::UdpSend { .. })));
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Unverified)
        }));
        // One-way: no reply, so no round trip.
        assert!(!a.iter().any(|x| matches!(x, Action::RoundTrip(_))));
    }

    #[test]
    fn v3_1_packets_over_tcp_are_split_from_the_stream() {
        let mut l = Listener::new(5727, "tsl-umd-3-tcp");
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::TcpListen { port: 5727, .. })));
        let mut cx = Cx::new(1);
        l.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let mut first = vec![0x80 + 3, 0x02];
        first.extend_from_slice(b"CAM 3           ");
        let mut second = vec![0x80 + 4, 0x01];
        second.extend_from_slice(b"CAM 4           ");
        // A truncated packet, then the two, split across reads.
        let mut stream = vec![0x80 + 9, 0x01, b'X'];
        stream.extend(&first);
        stream.extend(&second[..5]);
        l.tcp(&mut cx, SOCKET, TcpInput::Data(stream));
        l.tcp(&mut cx, SOCKET, TcpInput::Data(second[5..].to_vec()));
        let states: Vec<Value> = cx
            .take()
            .into_iter()
            .filter_map(|a| match a {
                Action::State(p) => Some(p),
                _ => None,
            })
            .collect();
        assert_eq!(states.len(), 2);
        assert_eq!(states[0]["screens"]["0"]["displays"]["3"]["text"], "CAM 3");
        assert_eq!(
            states[0]["screens"]["0"]["displays"]["3"]["lamps"]["2"],
            true
        );
        assert_eq!(
            states[1]["screens"]["0"]["displays"]["4"]["lamps"]["1"],
            true
        );
    }
}
