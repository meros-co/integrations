//! Blackmagic ATEM switchers over their UDP protocol on port 9910.
//!
//! Written from protocol documentation and MIT-licensed references, never from
//! GPL code: see the spec's sources. The shape of the protocol:
//!
//! - Every datagram is one packet with a 12-byte header: 5 flag bits and an
//!   11-bit length, then the session id, the id being acknowledged, the id the
//!   peer wants retransmitted from, two unused bytes, and the sender's own
//!   15-bit packet id. Big-endian throughout.
//! - The client opens with a fixed hello; the switcher answers with the new
//!   session flag; the client acknowledges, and from then on takes the session
//!   id from every packet it receives.
//! - The switcher then sends its whole state, from `_ver` to `InCm`, and keeps
//!   sending updates. Every packet it marks reliable must be acknowledged, and
//!   packets are accepted strictly in order: a gap is left for the switcher to
//!   resend.
//! - Commands go the other way in reliable packets, resent until acknowledged.
//!   A packet's payload is a run of commands, each an 8-byte header (length
//!   including the header, two unused bytes, a four-letter name) and a body.

use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};
use crate::session::merge_patch;

const PORT: u16 = 9910;
const SOCKET: Key = "atem";

const FLAG_RELIABLE: u8 = 0x01;
const FLAG_SYN: u8 = 0x02;
const FLAG_RETRANSMIT_REQUEST: u8 = 0x08;
const FLAG_ACK: u8 = 0x10;

const HEADER: usize = 12;
/// Packet ids are 15 bits: the switcher wraps from 32767 to 0.
const ID_MODULO: u16 = 1 << 15;
/// The largest datagram the client builds, as ATEM Software Control does.
const MAX_PACKET: usize = 1416;

/// The client's hello: new-session flag, length 20, a client-chosen session
/// id, and the fixed payload 01 00 00 00 00 00 00 00.
const HELLO_PACKET: [u8; 20] = [
    0x10, 0x14, 0x53, 0xab, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3a, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

const HELLO_EVERY: Millis = 1_000;
const HELLO_EVERY_SLOW: Millis = 5_000;
const HELLO_ATTEMPTS_FAST: u32 = 5;
/// The switcher sends traffic continuously; this much silence is a lost
/// connection.
const SILENCE_TIMEOUT: Millis = 5_000;
const RESEND_AFTER: Millis = 60;
const RESEND_ATTEMPTS: u32 = 10;

const HELLO: Key = "hello";
const SILENCE: Key = "silence";
const RESEND: Key = "resend";

/// Protocol 2.29 (firmware 8.0.1): DskS and DDsA change layout.
const V2_29: u32 = 0x0002_001D;
/// Protocol 2.30 (firmware 8.1.1): `_top` gains a multiviewer byte; streaming
/// and recording commands exist.
const V2_30: u32 = 0x0002_001E;

#[derive(Debug, PartialEq)]
enum Phase {
    /// Hello sent, waiting for the switcher's answer.
    Hello,
    /// Session open, initial state arriving.
    Loading,
    /// Initial state complete (`InCm`).
    Ready,
}

struct Sent {
    id: u16,
    packet: Vec<u8>,
    sent_at: Millis,
    attempts: u32,
    command: CommandId,
}

/// What commands are checked against: the switcher's own description of
/// itself, from the initial state.
#[derive(Default)]
struct Topology {
    mes: u8,
    auxes: u8,
    dsks: u8,
    usks: BTreeMap<u8, u8>,
    macros: u8,
    macros_used: BTreeMap<u16, bool>,
    sources: BTreeMap<u16, ()>,
    encoder: bool,
}

pub(crate) struct Atem {
    device: SocketAddr,
    phase: Phase,
    connected: bool,
    session: u16,
    /// Last reliable packet id accepted from the switcher.
    last_received: u16,
    next_id: u16,
    in_flight: VecDeque<Sent>,
    hello_attempts: u32,
    version: u32,
    topology: Topology,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

/// A NUL-terminated or NUL-padded string field. Bytes after the NUL are
/// arbitrary on the wire.
fn text(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// `a` is at or before `b` in 15-bit sequence space.
fn at_or_before(a: u16, b: u16) -> bool {
    (b.wrapping_sub(a) % ID_MODULO) < ID_MODULO / 2
}

fn header(flags: u8, length: usize, session: u16, ack: u16, id: u16) -> Vec<u8> {
    let first = ((flags as u16) << 11) | (length as u16 & 0x07FF);
    let mut h = Vec::with_capacity(length);
    h.extend_from_slice(&first.to_be_bytes());
    h.extend_from_slice(&session.to_be_bytes());
    h.extend_from_slice(&ack.to_be_bytes());
    h.extend_from_slice(&[0, 0, 0, 0]);
    h.extend_from_slice(&id.to_be_bytes());
    h
}

/// One command: length (header included), two zero bytes, name, body.
fn command(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut c = Vec::with_capacity(8 + body.len());
    c.extend_from_slice(&((8 + body.len()) as u16).to_be_bytes());
    c.extend_from_slice(&[0, 0]);
    c.extend_from_slice(name);
    c.extend_from_slice(body);
    c
}

fn source_kind(port: u8) -> &'static str {
    match port {
        0 => "external",
        1 => "black",
        2 => "color_bars",
        3 => "color",
        4 => "media_player_fill",
        5 => "media_player_key",
        6 => "supersource",
        7 => "external_direct",
        128 => "me_output",
        129 => "aux",
        130 => "mask",
        131 => "multiviewer",
        132 => "audio_monitor",
        _ => "unknown",
    }
}

fn transition_style(style: u8) -> &'static str {
    match style {
        0 => "mix",
        1 => "dip",
        2 => "wipe",
        3 => "dve",
        4 => "sting",
        _ => "unknown",
    }
}

fn int(params: &Params, name: &str) -> i64 {
    params.get(name).and_then(Value::as_i64).unwrap_or(0)
}

fn flag(params: &Params, name: &str) -> bool {
    params.get(name).and_then(Value::as_bool).unwrap_or(true)
}

fn invalid(message: String) -> CommandError {
    CommandError::InvalidParams { message }
}

impl Atem {
    pub(crate) fn new(ctx: OpenContext) -> Atem {
        Atem::for_device(SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)))
    }

    fn for_device(device: SocketAddr) -> Atem {
        Atem {
            device,
            phase: Phase::Hello,
            connected: false,
            session: 0,
            last_received: 0,
            next_id: 1,
            in_flight: VecDeque::new(),
            hello_attempts: 0,
            version: 0,
            topology: Topology::default(),
        }
    }

    /// A fresh socket (the switcher may hold a slot for the old port) and a
    /// fresh session.
    fn connect(&mut self, cx: &mut Cx) {
        self.phase = Phase::Hello;
        self.session = 0;
        self.last_received = 0;
        self.next_id = 1;
        self.version = 0;
        self.topology = Topology::default();
        cx.udp_open(SOCKET, Bind::Ephemeral);
        self.hello(cx);
    }

    fn hello(&mut self, cx: &mut Cx) {
        cx.udp_send(SOCKET, self.device, HELLO_PACKET.to_vec());
        self.hello_attempts += 1;
        let every = if self.hello_attempts < HELLO_ATTEMPTS_FAST {
            HELLO_EVERY
        } else {
            HELLO_EVERY_SLOW
        };
        cx.set_timer(HELLO, every);
    }

    fn ack(&self, cx: &mut Cx, id: u16) {
        cx.udp_send(
            SOCKET,
            self.device,
            header(FLAG_ACK, HEADER, self.session, id, 0),
        );
    }

    fn lost(&mut self, cx: &mut Cx, reason: &str) {
        for sent in self.in_flight.drain(..) {
            cx.complete(
                sent.command,
                Err(CommandError::Transport {
                    message: reason.into(),
                }),
            );
        }
        cx.cancel_timer(RESEND);
        cx.cancel_timer(SILENCE);
        if self.connected || self.phase != Phase::Hello {
            cx.connection(Connection::Disconnected {
                reason: reason.into(),
            });
        }
        self.connected = false;
        self.hello_attempts = 0;
        self.connect(cx);
    }

    fn send_command(&mut self, cx: &mut Cx, id: CommandId, name: &[u8; 4], body: &[u8]) {
        let payload = command(name, body);
        let packet_id = self.next_id;
        self.next_id = (self.next_id + 1) % ID_MODULO;
        let mut packet = header(
            FLAG_RELIABLE,
            HEADER + payload.len(),
            self.session,
            0,
            packet_id,
        );
        packet.extend_from_slice(&payload);
        debug_assert!(packet.len() <= MAX_PACKET);
        cx.udp_send(SOCKET, self.device, packet.clone());
        self.in_flight.push_back(Sent {
            id: packet_id,
            packet,
            sent_at: cx.now(),
            attempts: 1,
            command: id,
        });
        cx.set_timer(RESEND, RESEND_AFTER);
    }

    fn acknowledged(&mut self, cx: &mut Cx, ack: u16) {
        while let Some(sent) = self.in_flight.front() {
            if !at_or_before(sent.id, ack) {
                break;
            }
            let sent = self.in_flight.pop_front().unwrap();
            cx.complete(sent.command, Ok(Outcome::Ack));
        }
        if self.in_flight.is_empty() {
            cx.cancel_timer(RESEND);
        }
    }

    fn retransmit_from(&mut self, cx: &mut Cx, from: u16) {
        let from = from % ID_MODULO;
        let Some(start) = self.in_flight.iter().position(|s| s.id == from) else {
            self.lost(cx, "the switcher asked for a packet no longer held");
            return;
        };
        let now = cx.now();
        for sent in self.in_flight.iter_mut().skip(start) {
            cx.udp_send(SOCKET, self.device, sent.packet.clone());
            sent.sent_at = now;
        }
    }

    fn receive(&mut self, cx: &mut Cx, data: &[u8]) {
        if data.len() < HEADER {
            return;
        }
        let flags = data[0] >> 3;
        let length = (u16_at(data, 0) & 0x07FF) as usize;
        if length != data.len() {
            return;
        }
        let session = u16_at(data, 2);
        let remote_id = u16_at(data, 10);

        if flags & FLAG_SYN != 0 {
            if self.phase != Phase::Hello {
                return;
            }
            // Byte 12 is the connection status: 2 accepted, 4 asked to start
            // again (OpenSwitcher). Sofie proceeds regardless; so does this.
            if data.get(HEADER) == Some(&0x04) {
                cx.log(
                    Level::Warning,
                    "the switcher asked to restart the connection",
                );
            }
            cx.cancel_timer(HELLO);
            self.session = session;
            self.last_received = remote_id;
            self.phase = Phase::Loading;
            self.ack(cx, remote_id);
            cx.set_timer(SILENCE, SILENCE_TIMEOUT);
            return;
        }
        if self.phase == Phase::Hello {
            return;
        }
        // The switcher assigns a new session id after the handshake; follow
        // whatever it uses.
        self.session = session;
        cx.set_timer(SILENCE, SILENCE_TIMEOUT);
        cx.alive();

        if flags & FLAG_ACK != 0 {
            self.acknowledged(cx, u16_at(data, 4));
        }
        if flags & FLAG_RETRANSMIT_REQUEST != 0 {
            self.retransmit_from(cx, u16_at(data, 6));
        }
        if flags & FLAG_RELIABLE != 0 {
            let expected = (self.last_received + 1) % ID_MODULO;
            if remote_id == expected {
                self.last_received = remote_id;
                self.ack(cx, remote_id);
                self.commands(cx, &data[HEADER..]);
            } else if at_or_before(remote_id, self.last_received) {
                // Already have it: our acknowledgement was lost.
                self.ack(cx, self.last_received);
            }
            // Ahead of a gap: dropped, and the switcher resends in order.
        }
    }

    fn commands(&mut self, cx: &mut Cx, mut payload: &[u8]) {
        let mut patch = json!({});
        while payload.len() >= 8 {
            let length = u16_at(payload, 0) as usize;
            if length < 8 || length > payload.len() {
                break;
            }
            let name: [u8; 4] = payload[4..8].try_into().unwrap();
            let body = &payload[8..length];
            if let Some(p) = self.decode(cx, &name, body) {
                merge_patch(&mut patch, &p);
            }
            payload = &payload[length..];
        }
        if patch.as_object().is_some_and(|m| !m.is_empty()) {
            cx.state(patch);
        }
    }

    /// One command's effect on the state, if it has one. Bodies shorter than
    /// the layout are ignored.
    fn decode(&mut self, cx: &mut Cx, name: &[u8; 4], b: &[u8]) -> Option<Value> {
        let need = |n: usize| (b.len() >= n).then_some(());
        match name {
            b"_ver" => {
                need(4)?;
                self.version = ((u16_at(b, 0) as u32) << 16) | u16_at(b, 2) as u32;
                // A new dump: forget the previous switcher's description.
                self.topology = Topology::default();
                Some(json!({"device": {
                    "protocol_version": format!("{}.{}", u16_at(b, 0), u16_at(b, 2)),
                }}))
            }
            b"_pin" => {
                need(41)?;
                Some(json!({"device": {"product": text(&b[..40]), "model_id": b[40]}}))
            }
            b"_top" => {
                // Before 2.30 there is no multiviewer byte at offset 6.
                let shift = if self.version >= V2_30 { 1 } else { 0 };
                need(12 + shift)?;
                self.topology.mes = b[0];
                self.topology.dsks = b[2];
                self.topology.auxes = b[3];
                Some(json!({"topology": {
                    "mes": b[0],
                    "sources": b[1],
                    "dsks": b[2],
                    "auxes": b[3],
                    "media_players": b[5],
                    "supersources": b[10 + shift],
                }}))
            }
            b"_MeC" => {
                need(2)?;
                self.topology.usks.insert(b[0], b[1]);
                Some(json!({"mes": {(b[0] as u32 + 1).to_string(): {"keyers": b[1]}}}))
            }
            b"_MAC" => {
                need(1)?;
                self.topology.macros = b[0];
                Some(json!({"topology": {"macros": b[0]}}))
            }
            b"InPr" => {
                need(33)?;
                let id = u16_at(b, 0);
                self.topology.sources.insert(id, ());
                Some(json!({"sources": {id.to_string(): {
                    "long_name": text(&b[2..22]),
                    "short_name": text(&b[22..26]),
                    "kind": source_kind(b[32]),
                }}}))
            }
            b"PrgI" => {
                need(4)?;
                Some(json!({"mes": {(b[0] as u32 + 1).to_string(): {"program": u16_at(b, 2)}}}))
            }
            b"PrvI" => {
                need(4)?;
                Some(json!({"mes": {(b[0] as u32 + 1).to_string(): {"preview": u16_at(b, 2)}}}))
            }
            b"TrSS" => {
                need(2)?;
                Some(json!({"mes": {(b[0] as u32 + 1).to_string(): {
                    "transition": {"style": transition_style(b[1])},
                }}}))
            }
            b"TrPs" => {
                need(6)?;
                Some(
                    json!({"mes": {(b[0] as u32 + 1).to_string(): {"transition": {
                        "in_progress": b[1] != 0,
                        "position": u16_at(b, 4) as f64 / 10_000.0,
                    }}}}),
                )
            }
            b"FtbS" => {
                need(3)?;
                Some(
                    json!({"mes": {(b[0] as u32 + 1).to_string(): {"fade_to_black": {
                        "black": b[1] != 0,
                        "in_progress": b[2] != 0,
                    }}}}),
                )
            }
            b"KeOn" => {
                need(3)?;
                Some(json!({"mes": {(b[0] as u32 + 1).to_string(): {
                    "usk": {(b[1] as u32 + 1).to_string(): {"on_air": b[2] != 0}},
                }}}))
            }
            b"AuxS" => {
                need(4)?;
                Some(json!({"auxes": {(b[0] as u32 + 1).to_string(): {"source": u16_at(b, 2)}}}))
            }
            b"DskS" => {
                need(3)?;
                Some(json!({"dsks": {(b[0] as u32 + 1).to_string(): {
                    "on_air": b[1] != 0,
                    "in_transition": b[2] != 0,
                }}}))
            }
            b"DskP" => {
                need(2)?;
                Some(json!({"dsks": {(b[0] as u32 + 1).to_string(): {"tie": b[1] != 0}}}))
            }
            b"TlSr" => {
                need(2)?;
                let count = u16_at(b, 0) as usize;
                let mut tally = Map::new();
                for i in 0..count {
                    let at = 2 + 3 * i;
                    if at + 3 > b.len() {
                        break;
                    }
                    let flags = b[at + 2];
                    tally.insert(
                        u16_at(b, at).to_string(),
                        json!({"program": flags & 1 != 0, "preview": flags & 2 != 0}),
                    );
                }
                Some(json!({"tally": tally}))
            }
            b"MPrp" => {
                need(8)?;
                let index = u16_at(b, 0);
                let used = b[2] != 0;
                self.topology.macros_used.insert(index, used);
                let name_len = u16_at(b, 4) as usize;
                let slot = (index as u32 + 1).to_string();
                if !used {
                    return Some(json!({"macros": {slot: null}}));
                }
                let name = b.get(8..8 + name_len).map(String::from_utf8_lossy)?;
                Some(json!({"macros": {slot: {"name": name}}}))
            }
            b"MRPr" => {
                need(4)?;
                let running = b[0] & 1 != 0;
                let index = u16_at(b, 2);
                Some(json!({"macro_running": if running && index != 0xFFFF {
                    json!(index as u32 + 1)
                } else {
                    Value::Null
                }}))
            }
            b"StRS" => {
                need(2)?;
                self.topology.encoder = true;
                let status = u16_at(b, 0);
                let state = if status & 32 != 0 {
                    "stopping"
                } else if status & 4 != 0 {
                    "streaming"
                } else if status & 2 != 0 {
                    "connecting"
                } else {
                    "idle"
                };
                Some(json!({"streaming": {"state": state}}))
            }
            b"RTMS" => {
                need(2)?;
                self.topology.encoder = true;
                let status = u16_at(b, 0);
                let error = if status & 32768 != 0 {
                    "unknown"
                } else if status & 4 != 0 {
                    "media_full"
                } else if status & 8 != 0 {
                    "media_error"
                } else if status & 16 != 0 {
                    "media_unformatted"
                } else if status & 32 != 0 {
                    "dropping_frames"
                } else if status & 2 != 0 {
                    "none"
                } else {
                    "no_media"
                };
                Some(json!({"recording": {"active": status & 1 != 0, "error": error}}))
            }
            b"InCm" => {
                if self.phase == Phase::Loading {
                    self.phase = Phase::Ready;
                    self.connected = true;
                    self.hello_attempts = 0;
                    cx.connection(Connection::Connected);
                }
                None
            }
            _ => None,
        }
    }

    fn check_me(&self, params: &Params) -> Result<u8, CommandError> {
        let me = int(params, "me").max(1);
        if me > self.topology.mes as i64 {
            return Err(invalid(format!(
                "M/E {me} does not exist; the switcher has {}",
                self.topology.mes
            )));
        }
        Ok((me - 1) as u8)
    }

    fn check_source(&self, params: &Params) -> Result<u16, CommandError> {
        let source = int(params, "source");
        if !self.topology.sources.contains_key(&(source as u16)) {
            return Err(invalid(format!("the switcher has no source {source}")));
        }
        Ok(source as u16)
    }

    fn check_index(&self, params: &Params, name: &str, count: u8) -> Result<u8, CommandError> {
        let n = int(params, name);
        if n < 1 || n > count as i64 {
            return Err(invalid(format!(
                "{name} {n} does not exist; the switcher has {count}"
            )));
        }
        Ok((n - 1) as u8)
    }

    /// A command's name and body, checked against the switcher's topology.
    fn build(&self, name: &str, params: &Params) -> Result<([u8; 4], Vec<u8>), CommandError> {
        let unsupported = || CommandError::UnsupportedForModel {
            command: name.into(),
            model: "this switcher (no encoder, or protocol before 2.30)".into(),
        };
        Ok(match name {
            "cut" => (*b"DCut", vec![self.check_me(params)?, 0, 0, 0]),
            "auto" => (*b"DAut", vec![self.check_me(params)?, 0, 0, 0]),
            "fade_to_black" => (*b"FtbA", vec![self.check_me(params)?, 0, 0, 0]),
            "set_program" | "set_preview" => {
                let me = self.check_me(params)?;
                let [hi, lo] = self.check_source(params)?.to_be_bytes();
                let name = if name == "set_program" {
                    *b"CPgI"
                } else {
                    *b"CPvI"
                };
                (name, vec![me, 0, hi, lo])
            }
            "set_transition_style" => {
                let me = self.check_me(params)?;
                let style = match params.get("style").and_then(Value::as_str) {
                    Some("mix") => 0,
                    Some("dip") => 1,
                    Some("wipe") => 2,
                    Some("dve") => 3,
                    Some("sting") => 4,
                    other => return Err(invalid(format!("unknown style {other:?}"))),
                };
                // Mask bit 0: set the style only.
                (*b"CTTp", vec![0x01, me, style, 0])
            }
            "set_transition_position" => {
                let me = self.check_me(params)?;
                let position = params
                    .get("position")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let [hi, lo] = ((position.clamp(0.0, 1.0) * 10_000.0).round() as u16).to_be_bytes();
                (*b"CTPs", vec![me, 0, hi, lo])
            }
            "set_aux" => {
                let aux = self.check_index(params, "aux", self.topology.auxes)?;
                let [hi, lo] = self.check_source(params)?.to_be_bytes();
                (*b"CAuS", vec![0x01, aux, hi, lo])
            }
            "dsk_auto" => {
                let dsk = self.check_index(params, "dsk", self.topology.dsks)?;
                if self.version >= V2_29 {
                    // Mask 0: toggle, without a direction.
                    (*b"DDsA", vec![0, dsk, 0, 0])
                } else {
                    (*b"DDsA", vec![dsk, 0, 0, 0])
                }
            }
            "dsk_on_air" => {
                let dsk = self.check_index(params, "dsk", self.topology.dsks)?;
                (*b"CDsL", vec![dsk, flag(params, "on_air") as u8, 0, 0])
            }
            "usk_on_air" => {
                let me = self.check_me(params)?;
                let keyers = self.topology.usks.get(&me).copied().unwrap_or(0);
                let keyer = self.check_index(params, "keyer", keyers)?;
                (*b"CKOn", vec![me, keyer, flag(params, "on_air") as u8, 0])
            }
            "run_macro" => {
                let slot = self.check_index(params, "macro", self.topology.macros)?;
                if self.topology.macros_used.get(&(slot as u16)) != Some(&true) {
                    return Err(invalid(format!("macro {} is empty", slot as u16 + 1)));
                }
                let [hi, lo] = (slot as u16).to_be_bytes();
                (*b"MAct", vec![hi, lo, 0, 0])
            }
            "stop_macro" => (*b"MAct", vec![0xFF, 0xFF, 1, 0]),
            "start_streaming" | "stop_streaming" => {
                if self.version < V2_30 || !self.topology.encoder {
                    return Err(unsupported());
                }
                (*b"StrR", vec![(name == "start_streaming") as u8, 0, 0, 0])
            }
            "start_recording" | "stop_recording" => {
                if self.version < V2_30 || !self.topology.encoder {
                    return Err(unsupported());
                }
                (*b"RcTM", vec![(name == "start_recording") as u8, 0, 0, 0])
            }
            other => {
                return Err(CommandError::UnknownCommand {
                    command: other.into(),
                })
            }
        })
    }
}

impl Module for Atem {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.phase != Phase::Ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.build(name, params) {
            Ok((name, body)) => self.send_command(cx, id, &name, &body),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        if from.ip() != self.device.ip() {
            return;
        }
        self.receive(cx, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        self.lost(cx, &format!("socket error: {message}"));
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            HELLO if self.phase == Phase::Hello => self.hello(cx),
            SILENCE => self.lost(cx, "no data from the switcher for 5 s"),
            RESEND => {
                let now = cx.now();
                let mut give_up = false;
                for sent in self.in_flight.iter_mut() {
                    if now.saturating_sub(sent.sent_at) >= RESEND_AFTER {
                        if sent.attempts >= RESEND_ATTEMPTS {
                            give_up = true;
                            break;
                        }
                        let mut packet = sent.packet.clone();
                        // Mark it a retransmission.
                        packet[0] |= 0x04 << 3;
                        cx.udp_send(SOCKET, self.device, packet);
                        sent.sent_at = now;
                        sent.attempts += 1;
                    }
                }
                if give_up {
                    self.lost(cx, "the switcher stopped acknowledging commands");
                } else if !self.in_flight.is_empty() {
                    cx.set_timer(RESEND, RESEND_AFTER);
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.udp_close(SOCKET);
    }
}

/// The source address a module compares against, for tests.
#[cfg(test)]
impl Atem {
    fn host(&self) -> std::net::IpAddr {
        self.device.ip()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));

    fn atem() -> Atem {
        Atem::for_device(SocketAddr::new(HOST, PORT))
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    fn feed(m: &mut Atem, now: Millis, packet: Vec<u8>) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.datagram(&mut cx, SOCKET, SocketAddr::new(m.host(), PORT), &packet);
        cx.take()
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

    /// A switcher packet: reliable, with the given id and commands.
    fn reliable(session: u16, id: u16, commands: &[Vec<u8>]) -> Vec<u8> {
        let payload: Vec<u8> = commands.concat();
        let mut p = header(FLAG_RELIABLE, HEADER + payload.len(), session, 0, id);
        p.extend_from_slice(&payload);
        p
    }

    /// A switcher command with non-zero bytes where the header's unused bytes
    /// are, as real switchers send.
    fn cmd(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut c = command(name, body);
        c[2] = 0xff;
        c[3] = 0xff;
        c
    }

    fn syn_reply() -> Vec<u8> {
        let mut p = header(FLAG_SYN, HEADER + 8, 0x53ab, 0, 0);
        p.extend_from_slice(&[0x02, 0, 0, 0, 0, 0, 0, 0]);
        p
    }

    fn input(id: u16, long: &str, short: &str, port: u8) -> Vec<u8> {
        let mut b = vec![0u8; 36];
        b[0..2].copy_from_slice(&id.to_be_bytes());
        b[2..2 + long.len()].copy_from_slice(long.as_bytes());
        b[22..22 + short.len()].copy_from_slice(short.as_bytes());
        b[32] = port;
        cmd(b"InPr", &b)
    }

    /// The initial dump of a small 1 M/E switcher on protocol 2.30.
    fn dump() -> Vec<Vec<u8>> {
        let mut pin = vec![0u8; 44];
        pin[..13].copy_from_slice(b"ATEM Mini Pro");
        pin[40] = 14;
        let mut top = vec![0u8; 24];
        top[0] = 1; // M/Es
        top[1] = 3; // sources
        top[2] = 1; // DSKs
        top[3] = 1; // auxes
        top[11] = 0; // SuperSources (offset 11 from 2.30)
        let mut mprp = vec![0, 0, 1, 0, 0, 4, 0, 0];
        mprp.extend_from_slice(b"Open");
        vec![
            cmd(b"_ver", &[0, 2, 0, 30]),
            cmd(b"_pin", &pin),
            cmd(b"_top", &top),
            cmd(b"_MeC", &[0, 1, 0x72, 0x70]),
            cmd(b"_MAC", &[100, 0, 0, 0]),
            input(0, "Black", "BLK", 1),
            input(1, "Camera 1", "CAM1", 0),
            input(2, "Camera 2", "CAM2", 0),
            cmd(b"PrgI", &[0, 0x18, 0, 1]),
            cmd(b"PrvI", &[0, 0, 0, 2, 0, 0, 0, 0]),
            cmd(b"TrSS", &[0, 0, 1, 0, 1, 0, 0, 0]),
            cmd(b"TlSr", &[0, 3, 0, 0, 0, 0, 1, 1, 0, 2, 2, 0]),
            cmd(b"MPrp", &mprp),
            cmd(b"StRS", &[0, 1, 0, 0]),
            cmd(b"InCm", &[1, 0x50, 0x72, 0x70]),
        ]
    }

    /// Hello, the switcher's answer, and the dump in one packet.
    fn ready() -> (Atem, Vec<Action>) {
        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        feed(&mut m, 10, syn_reply());
        let a = feed(&mut m, 20, reliable(0x8001, 1, &dump()));
        (m, a)
    }

    #[test]
    fn opens_with_the_documented_hello() {
        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Ephemeral
        }));
        assert_eq!(
            sent(&a),
            [vec![
                0x10, 0x14, 0x53, 0xab, 0, 0, 0, 0, 0, 0x3a, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0
            ]]
        );
        // Unanswered, the hello repeats.
        let mut cx = Cx::new(1_000);
        m.timer(&mut cx, HELLO);
        assert_eq!(sent(&cx.take()).len(), 1);
    }

    #[test]
    fn acknowledges_the_handshake_and_adopts_the_new_session() {
        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let a = feed(&mut m, 10, syn_reply());
        assert_eq!(
            sent(&a),
            [vec![0x80, 0x0c, 0x53, 0xab, 0, 0, 0, 0, 0, 0, 0, 0]]
        );

        // The first reliable packet carries the switcher's new session id,
        // which the acknowledgement echoes.
        let a = feed(
            &mut m,
            20,
            reliable(0x8001, 1, &[cmd(b"_ver", &[0, 2, 0, 30])]),
        );
        assert_eq!(
            sent(&a),
            [vec![0x80, 0x0c, 0x80, 0x01, 0, 1, 0, 0, 0, 0, 0, 0]]
        );
    }

    #[test]
    fn the_dump_builds_the_state_and_ends_in_connected() {
        let (_, a) = ready();
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let s = state(&a);
        assert_eq!(s["device"]["product"], "ATEM Mini Pro");
        assert_eq!(s["device"]["model_id"], 14);
        assert_eq!(s["device"]["protocol_version"], "2.30");
        assert_eq!(s["topology"]["mes"], 1);
        assert_eq!(s["topology"]["auxes"], 1);
        assert_eq!(
            s["sources"]["1"],
            json!({"long_name": "Camera 1", "short_name": "CAM1", "kind": "external"})
        );
        assert_eq!(s["mes"]["1"]["program"], 1);
        assert_eq!(s["mes"]["1"]["preview"], 2);
        assert_eq!(s["mes"]["1"]["keyers"], 1);
        assert_eq!(s["mes"]["1"]["transition"]["style"], "mix");
        assert_eq!(s["tally"]["1"], json!({"program": true, "preview": false}));
        assert_eq!(s["tally"]["2"], json!({"program": false, "preview": true}));
        assert_eq!(s["macros"]["1"]["name"], "Open");
        assert_eq!(s["streaming"]["state"], "idle");
    }

    #[test]
    fn a_gap_is_left_for_the_switcher_to_resend() {
        let (mut m, _) = ready();
        // Packet 3 before 2: dropped, and nothing acknowledged past 1.
        let a = feed(
            &mut m,
            30,
            reliable(0x8001, 3, &[cmd(b"PrgI", &[0, 0, 0, 2])]),
        );
        assert!(sent(&a).is_empty());
        assert_eq!(state(&a), json!({}));
        // Packet 1 again: already applied, acknowledged again.
        let a = feed(&mut m, 31, reliable(0x8001, 1, &[]));
        assert_eq!(sent(&a), [header(FLAG_ACK, HEADER, 0x8001, 1, 0)]);
        // Packet 2 in order is applied.
        let a = feed(
            &mut m,
            32,
            reliable(0x8001, 2, &[cmd(b"PrgI", &[0, 0, 0, 2])]),
        );
        assert_eq!(state(&a)["mes"]["1"]["program"], 2);
    }

    #[test]
    fn packet_ids_wrap_at_15_bits() {
        let (mut m, _) = ready();
        m.last_received = ID_MODULO - 1;
        let a = feed(
            &mut m,
            30,
            reliable(0x8001, 0, &[cmd(b"PrgI", &[0, 0, 0, 2])]),
        );
        assert_eq!(state(&a)["mes"]["1"]["program"], 2);
        assert_eq!(m.last_received, 0);
    }

    #[test]
    fn commands_are_reliable_packets_completed_by_the_switchers_ack() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(100);
        let params = json!({"me": 1, "source": 2}).as_object().unwrap().clone();
        m.command(&mut cx, 7, "set_program", &params);
        let a = cx.take();
        let packet = &sent(&a)[0];
        // Reliable flag, length 12 + 12, session, id 1, then CPgI.
        assert_eq!(&packet[..12], &header(FLAG_RELIABLE, 24, 0x8001, 0, 1)[..]);
        assert_eq!(
            &packet[12..],
            &[0, 12, 0, 0, b'C', b'P', b'g', b'I', 0, 0, 0, 2]
        );

        let a = feed(&mut m, 110, header(FLAG_ACK, HEADER, 0x8001, 1, 0));
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Ack)
        }));
    }

    #[test]
    fn unacknowledged_commands_are_resent_then_the_connection_is_dropped() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            1,
            "cut",
            &json!({"me": 1}).as_object().unwrap().clone(),
        );
        let first = sent(&cx.take())[0].clone();
        let mut now = 100;
        for _ in 1..RESEND_ATTEMPTS {
            now += RESEND_AFTER;
            let mut cx = Cx::new(now);
            m.timer(&mut cx, RESEND);
            let resent = sent(&cx.take());
            assert_eq!(resent.len(), 1);
            assert_eq!(resent[0][0], first[0] | 0x20, "marked as a retransmission");
            assert_eq!(&resent[0][1..], &first[1..]);
        }
        now += RESEND_AFTER;
        let mut cx = Cx::new(now);
        m.timer(&mut cx, RESEND);
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                id: 1,
                result: Err(CommandError::Transport { .. })
            }
        )));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
    }

    #[test]
    fn a_retransmit_request_resends_from_the_asked_packet() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(100);
        let me = json!({"me": 1}).as_object().unwrap().clone();
        m.command(&mut cx, 1, "cut", &me);
        m.command(&mut cx, 2, "auto", &me);
        cx.take();
        let mut request = header(FLAG_RETRANSMIT_REQUEST, HEADER, 0x8001, 0, 0);
        request[6..8].copy_from_slice(&2u16.to_be_bytes());
        let a = feed(&mut m, 120, request);
        let resent = sent(&a);
        assert_eq!(resent.len(), 1);
        assert_eq!(&resent[0][16..20], b"DAut");
    }

    #[test]
    fn commands_are_checked_against_the_switchers_topology() {
        let (mut m, _) = ready();
        let check = |m: &mut Atem, name: &str, params: Value| {
            let mut cx = Cx::new(100);
            m.command(&mut cx, 1, name, params.as_object().unwrap());
            let a = cx.take();
            assert!(sent(&a).is_empty(), "{name} sent nothing");
            match &a[..] {
                [Action::Complete { result: Err(e), .. }] => e.clone(),
                other => panic!("{other:?}"),
            }
        };
        assert!(matches!(
            check(&mut m, "cut", json!({"me": 2})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            check(&mut m, "set_program", json!({"me": 1, "source": 3010})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            check(&mut m, "set_aux", json!({"aux": 2, "source": 1})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            check(&mut m, "run_macro", json!({"macro": 2})),
            CommandError::InvalidParams { .. }
        ));
        assert!(matches!(
            check(&mut m, "usk_on_air", json!({"me": 1, "keyer": 2})),
            CommandError::InvalidParams { .. }
        ));
    }

    #[test]
    fn version_dependent_layouts() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            1,
            "dsk_auto",
            &json!({"dsk": 1}).as_object().unwrap().clone(),
        );
        // 2.29 and later: mask, DSK index, direction, reserved.
        assert_eq!(
            &sent(&cx.take())[0][12..],
            &[0, 12, 0, 0, b'D', b'D', b's', b'A', 0, 0, 0, 0]
        );

        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        feed(&mut m, 10, syn_reply());
        let mut dump = dump();
        dump[0] = cmd(b"_ver", &[0, 2, 0, 28]);
        // Before 2.30 _top has no multiviewer byte, and there is no encoder.
        dump.retain(|c| &c[4..8] != b"StRS");
        let a = feed(&mut m, 20, reliable(0x8001, 1, &dump));
        assert_eq!(state(&a)["topology"]["mes"], 1);
        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            2,
            "dsk_auto",
            &json!({"dsk": 1}).as_object().unwrap().clone(),
        );
        assert_eq!(&sent(&cx.take())[0][20..], &[0, 0, 0, 0]);
        let mut cx = Cx::new(101);
        m.command(&mut cx, 3, "start_streaming", &Params::new());
        assert!(matches!(
            &cx.take()[..],
            [Action::Complete {
                result: Err(CommandError::UnsupportedForModel { .. }),
                ..
            }]
        ));
    }

    #[test]
    fn command_layouts() {
        let (mut m, _) = ready();
        let body = |m: &mut Atem, name: &str, params: Value| {
            let mut cx = Cx::new(100);
            m.command(&mut cx, 1, name, params.as_object().unwrap());
            sent(&cx.take())[0][12..].to_vec()
        };
        let b = body(
            &mut m,
            "set_transition_position",
            json!({"me": 1, "position": 0.5}),
        );
        assert_eq!(&b[4..], b"CTPs\0\0\x13\x88");
        let b = body(
            &mut m,
            "set_transition_style",
            json!({"me": 1, "style": "wipe"}),
        );
        assert_eq!(&b[4..], b"CTTp\x01\0\x02\0");
        let b = body(&mut m, "set_aux", json!({"aux": 1, "source": 2}));
        assert_eq!(&b[4..], b"CAuS\x01\0\0\x02");
        let b = body(&mut m, "run_macro", json!({"macro": 1}));
        assert_eq!(&b[4..], b"MAct\0\0\0\0");
        let b = body(&mut m, "stop_macro", json!({}));
        assert_eq!(&b[4..], b"MAct\xff\xff\x01\0");
        let b = body(&mut m, "start_streaming", json!({}));
        assert_eq!(&b[4..], b"StrR\x01\0\0\0");
        let b = body(
            &mut m,
            "usk_on_air",
            json!({"me": 1, "keyer": 1, "on_air": true}),
        );
        assert_eq!(&b[4..], b"CKOn\0\0\x01\0");
    }

    #[test]
    fn silence_reconnects_on_a_fresh_socket() {
        let (mut m, _) = ready();
        let mut cx = Cx::new(10_000);
        m.timer(&mut cx, SILENCE);
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Ephemeral
        }));
        assert_eq!(sent(&a), [HELLO_PACKET.to_vec()]);
        // Commands wait for the new session's dump.
        let mut cx = Cx::new(10_001);
        m.command(
            &mut cx,
            5,
            "cut",
            &json!({"me": 1}).as_object().unwrap().clone(),
        );
        assert!(matches!(
            &cx.take()[..],
            [Action::Complete {
                id: 5,
                result: Err(CommandError::NotConnected)
            }]
        ));
    }

    #[test]
    fn datagrams_from_other_hosts_are_ignored() {
        let mut m = atem();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(10);
        m.datagram(
            &mut cx,
            SOCKET,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 99)), PORT),
            &syn_reply(),
        );
        assert!(cx.take().is_empty());
    }
}
