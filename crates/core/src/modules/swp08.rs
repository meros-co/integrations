//! Probel SW-P-08, the General Remote Control Protocol, over TCP.
//!
//! Wire format from Snell's SW-P-08 Issue 30 (2013), section numbers below
//! being that document's; Pro-Bel's SW-P-88 Issue 1 (2001) has the same
//! framing and worked examples.
//!
//! - A message is a command byte and its data, framed as DLE STX, the data,
//!   a byte count of the data, an 8-bit two's complement checksum of data and
//!   count, then DLE ETX. Every 10h between STX and ETX is doubled (§2.2.1).
//! - Every frame is answered DLE ACK, or DLE NAK when its packaging is wrong.
//!   A data reply follows the ACK; the receiver of a frame acknowledges it
//!   within 1 s (ideally 10 ms), else the sender re-sends it, up to five
//!   times (§2.2.2). The module ACKs each good frame at once, NAKs bad ones,
//!   and re-sends its own message on NAK or after 1 s without an ACK, five
//!   times at most. Five sends without an ACK drop the connection.
//! - One message is outstanding at a time. A command whose answer is a data
//!   message (TALLY for an interrogate, CONNECTED for a connect, PROTECT
//!   CONNECTED for a protect, a name response, ...) waits for that message,
//!   matched by its command byte and address, for 3 s.
//! - The general commands pack the matrix and level in one byte (4 bits each)
//!   and destination and source numbers in 10 bits with a multiplier byte
//!   (§3.1.2). Larger numbers need the extended commands (§3.3), chosen per
//!   message by the `command_set` setting; replies are read in either form.
//! - CONNECTED and PROTECT CONNECTED / DIS-CONNECTED are broadcast to every
//!   client on every change (§3.2.3, §3.2.6, §3.2.7) and keep state current.
//!   On connecting the module reads the configured matrix's levels: tally
//!   dump, protect dump and names. Dumps and name tables come in as many
//!   messages as needed, with no end marker, and are applied as they arrive.
//! - Numbers are 1-based in commands and state, 0-based on the wire (§9),
//!   except the System 2 matrix and level, which the wire carries from 1 (§8).
//!
//! Opened for commands only (`monitor` false), nothing is read on connecting
//! and NAMES UPDATED reads nothing again; the only message sent unasked is a
//! DUAL CONTROLLER STATUS REQUEST every 10 s when idle, whose DLE ACK is the
//! liveness check (any controller acknowledges a well-framed message, §2.2.2).
//! What the controller pushes, and what replies carry, still reaches state.

use std::collections::{HashSet, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::{DeviceSpec, Params};
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

/// SW-P-08 over IP has no fixed port (§11.4); 2008 is the common choice.
pub(crate) const DEFAULT_PORT: u16 = 2008;
const SOCKET: Key = "swp08";

const ACK_TIMER: Key = "ack";
const REPLY_TIMER: Key = "reply";
const PROBE_TIMER: Key = "probe";
const RETRY_TIMER: Key = "retry";

const DLE: u8 = 0x10;
const STX: u8 = 0x02;
const ETX: u8 = 0x03;
const ACK: u8 = 0x06;
const NAK: u8 = 0x15;

/// §2.2.2: the notional ACK timeout, and the recommended five tries.
const ACK_TIMEOUT: Millis = 1_000;
const ATTEMPTS: u32 = 5;
/// How long a command waits for its data reply after the ACK.
const REPLY_TIMEOUT: Millis = 3_000;
/// The liveness check's cadence while nothing else is sent.
const PROBE_EVERY: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// A frame's data is at most 255 bytes (§2.2.1), doubled at worst.
const MAX_FRAME: usize = 1_024;

/// Models whose controllers have only the general commands (Issue 30 §8).
const GENERAL_ONLY: &[&str] = &[
    "probel-system2",
    "probel-system3-8400",
    "probel-system3-6000",
    "probel-freeway",
];

// ── Framing ─────────────────────────────────────────────────────────────────

/// Frame one message (command byte first) for the wire (§2.2.1).
pub(crate) fn frame(data: &[u8]) -> Vec<u8> {
    let btc = data.len() as u8;
    let sum = data
        .iter()
        .fold(u32::from(btc), |a, &b| a.wrapping_add(u32::from(b)));
    let chk = (sum as u8).wrapping_neg();
    let mut out = vec![DLE, STX];
    for &b in data.iter().chain([btc, chk].iter()) {
        out.push(b);
        if b == DLE {
            out.push(DLE);
        }
    }
    out.extend([DLE, ETX]);
    out
}

#[derive(Debug, Clone, PartialEq)]
enum Inbound {
    Ack,
    Nak,
    /// A frame whose count and checksum are right: the message, command first.
    Message(Vec<u8>),
    /// A frame whose packaging is wrong.
    Bad,
}

/// Splits the byte stream into acknowledgements and frames.
#[derive(Debug, Default)]
struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    fn feed(&mut self, data: &[u8]) -> Vec<Inbound> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            // Bytes outside a frame mean nothing.
            match self.buf.iter().position(|&b| b == DLE) {
                Some(0) => {}
                Some(i) => {
                    self.buf.drain(..i);
                }
                None => {
                    self.buf.clear();
                    return out;
                }
            }
            if self.buf.len() < 2 {
                return out;
            }
            match self.buf[1] {
                ACK => {
                    out.push(Inbound::Ack);
                    self.buf.drain(..2);
                }
                NAK => {
                    out.push(Inbound::Nak);
                    self.buf.drain(..2);
                }
                STX => match self.take_frame(&mut out) {
                    Some(consumed) => {
                        self.buf.drain(..consumed);
                    }
                    None => {
                        if self.buf.len() > MAX_FRAME {
                            out.push(Inbound::Bad);
                            self.buf.clear();
                        }
                        return out;
                    }
                },
                _ => {
                    self.buf.drain(..1);
                }
            }
        }
    }

    /// A whole frame from the start of the buffer, or None while it is
    /// incomplete; returns how many bytes it used. Acknowledgements embedded
    /// in a frame (§2.2.2) are reported as they are met.
    fn take_frame(&self, out: &mut Vec<Inbound>) -> Option<usize> {
        let b = &self.buf;
        let mut content = Vec::new();
        let mut embedded = Vec::new();
        let mut i = 2;
        while i < b.len() {
            if b[i] != DLE {
                content.push(b[i]);
                i += 1;
                continue;
            }
            let next = *b.get(i + 1)?;
            match next {
                DLE => content.push(DLE),
                ACK => embedded.push(Inbound::Ack),
                NAK => embedded.push(Inbound::Nak),
                ETX => {
                    out.extend(embedded);
                    out.push(check(content));
                    return Some(i + 2);
                }
                // A new frame starts: the one before it is broken.
                STX => {
                    out.extend(embedded);
                    out.push(Inbound::Bad);
                    return Some(i);
                }
                _ => {
                    out.extend(embedded);
                    out.push(Inbound::Bad);
                    return Some(i + 2);
                }
            }
            i += 2;
        }
        None
    }
}

/// Check a frame's byte count and checksum (§2.2.1).
fn check(mut content: Vec<u8>) -> Inbound {
    if content.len() < 3 {
        return Inbound::Bad;
    }
    let chk = content.pop().unwrap();
    let btc = content.pop().unwrap();
    let sum = content
        .iter()
        .fold(u32::from(btc) + u32::from(chk), |a, &b| a + u32::from(b));
    if usize::from(btc) == content.len() && sum & 0xFF == 0 {
        Inbound::Message(content)
    } else {
        Inbound::Bad
    }
}

// ── Messages ────────────────────────────────────────────────────────────────

fn word(hi: u8, lo: u8) -> u32 {
    (u32::from(hi) << 8) | u32::from(lo)
}

/// A 10-bit number from the multiplier's 3 bits and a 7-bit byte (§3.1.2).
fn ten(mult: u8, low: u8) -> u32 {
    (u32::from(mult & 7) << 7) | u32::from(low & 0x7F)
}

fn protect_name(code: u8) -> Option<&'static str> {
    Some(match code {
        0 => "none",
        1 => "panel",
        2 => "panel_override",
        3 => "remote",
        _ => return None,
    })
}

/// Characters per name for a name length code (§3.1.18, §3.1.22).
fn name_chars(code: u8) -> Option<usize> {
    [4, 8, 12, 16].get(usize::from(code)).copied()
}

fn length_code(text: &str) -> Option<u8> {
    match text {
        "4" => Some(0),
        "8" => Some(1),
        "12" => Some(2),
        "16" => Some(3),
        _ => None,
    }
}

fn name_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches([' ', '\0'])
        .to_string()
}

/// Names packed at a fixed width after a header.
fn names(code: u8, count: u8, body: &[u8]) -> Vec<String> {
    let Some(width) = name_chars(code) else {
        return Vec::new();
    };
    body.chunks_exact(width)
        .take(usize::from(count))
        .map(name_text)
        .collect()
}

/// What a message from the controller is, in operator numbering (from 1).
#[derive(Debug, Clone, PartialEq)]
enum Msg {
    /// TALLY (3, 131) or CONNECTED (4, 132).
    Route {
        connected: bool,
        m: u32,
        l: u32,
        d: u32,
        s: u32,
        bad: Option<bool>,
    },
    /// Byte or word tally dump (22, 23, 151).
    TallyDump {
        m: u32,
        l: u32,
        first: u32,
        sources: Vec<u32>,
    },
    /// PROTECT TALLY (11, 139), CONNECTED (13, 141), DIS-CONNECTED (15, 143).
    Protect {
        m: u32,
        l: u32,
        d: u32,
        state: &'static str,
        device: u32,
    },
    ProtectDump {
        m: u32,
        l: u32,
        first: u32,
        entries: Vec<(&'static str, u32)>,
    },
    DeviceName {
        device: u32,
        name: String,
    },
    SourceNames {
        m: u32,
        l: u32,
        first: u32,
        names: Vec<String>,
    },
    DestNames {
        m: u32,
        first: u32,
        names: Vec<String>,
    },
    SourceAssocNames {
        m: u32,
        first: u32,
        names: Vec<String>,
    },
    UmdLabels {
        m: u32,
        first: u32,
        names: Vec<String>,
    },
    TieLine {
        m: u32,
        d: u32,
        sources: Vec<Value>,
    },
    SalvoAck {
        salvo: u32,
    },
    GoDone {
        status: u8,
        salvo: u32,
    },
    SalvoTally {
        salvo: u32,
        index: u32,
        validity: u8,
        entry: Value,
    },
    DualStatus {
        active_card: &'static str,
        active: bool,
        idle_ok: bool,
    },
    Implementation {
        transmitted: Vec<u8>,
        received: Vec<u8>,
    },
    Invalid {
        command: u8,
    },
    NamesUpdated,
}

/// Wire numbering to operator numbering.
#[derive(Debug, Clone, Copy)]
struct Numbering {
    /// Added to a wire matrix or level: 1, or 0 for System 2 (§8).
    ml: u32,
}

impl Numbering {
    fn decode(&self, data: &[u8]) -> Option<Msg> {
        let (&cmd, b) = data.split_first()?;
        let ml = |byte: u8| {
            (
                u32::from(byte >> 4) + self.ml,
                u32::from(byte & 0x0F) + self.ml,
            )
        };
        let m1 = |v: u8| u32::from(v) + self.ml;
        let need = |n: usize| (b.len() >= n).then_some(());
        Some(match cmd {
            3 | 4 => {
                need(4)?;
                let (m, l) = ml(b[0]);
                Msg::Route {
                    connected: cmd == 4,
                    m,
                    l,
                    d: ten(b[1] >> 4, b[2]) + 1,
                    s: ten(b[1], b[3]) + 1,
                    bad: Some(b[1] & 0x08 != 0),
                }
            }
            131 | 132 => {
                need(6)?;
                Msg::Route {
                    connected: cmd == 132,
                    m: m1(b[0]),
                    l: m1(b[1]),
                    d: word(b[2], b[3]) + 1,
                    s: word(b[4], b[5]) + 1,
                    bad: None,
                }
            }
            22 => {
                need(3)?;
                let (m, l) = ml(b[0]);
                Msg::TallyDump {
                    m,
                    l,
                    first: u32::from(b[2]) + 1,
                    sources: b[3..]
                        .iter()
                        .take(usize::from(b[1]))
                        .map(|&s| u32::from(s) + 1)
                        .collect(),
                }
            }
            23 => {
                need(4)?;
                let (m, l) = ml(b[0]);
                Msg::TallyDump {
                    m,
                    l,
                    first: word(b[2], b[3]) + 1,
                    sources: b[4..]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .take(usize::from(b[1]))
                        .map(|w| word(w[0], w[1]) + 1)
                        .collect(),
                }
            }
            151 => {
                need(5)?;
                Msg::TallyDump {
                    m: m1(b[0]),
                    l: m1(b[1]),
                    first: word(b[3], b[4]) + 1,
                    sources: b[5..]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .take(usize::from(b[2]))
                        .map(|w| word(w[0], w[1]) + 1)
                        .collect(),
                }
            }
            11 | 13 | 15 => {
                need(5)?;
                let (m, l) = ml(b[0]);
                Msg::Protect {
                    m,
                    l,
                    d: ten(b[2] >> 4, b[3]) + 1,
                    state: protect_name(b[1])?,
                    device: ten(b[2], b[4]),
                }
            }
            139 | 141 | 143 => {
                need(7)?;
                Msg::Protect {
                    m: m1(b[0]),
                    l: m1(b[1]),
                    state: protect_name(b[2])?,
                    d: word(b[3], b[4]) + 1,
                    device: word(b[5], b[6]),
                }
            }
            20 | 148 => {
                let (m, l, at) = if cmd == 20 {
                    need(4)?;
                    let (m, l) = ml(b[0]);
                    (m, l, 1)
                } else {
                    need(5)?;
                    (m1(b[0]), m1(b[1]), 2)
                };
                let count = usize::from(b[at]);
                Msg::ProtectDump {
                    m,
                    l,
                    first: word(b[at + 1], b[at + 2]) + 1,
                    entries: b[at + 3..]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .take(count)
                        .filter_map(|w| {
                            let v = word(w[0], w[1]);
                            Some((protect_name(((v >> 12) & 7) as u8)?, v & 0x3FF))
                        })
                        .collect(),
                }
            }
            18 => {
                need(2)?;
                Msg::DeviceName {
                    device: ten(b[0], b[1]),
                    name: name_text(&b[2..b.len().min(10)]),
                }
            }
            106 | 116 => {
                need(5)?;
                let (m, l) = ml(b[0]);
                let first = word(b[2], b[3]) + 1;
                let list = names(b[1], b[4], &b[5..]);
                if cmd == 106 {
                    Msg::SourceNames {
                        m,
                        l,
                        first,
                        names: list,
                    }
                } else {
                    Msg::SourceAssocNames {
                        m,
                        first,
                        names: list,
                    }
                }
            }
            234 => {
                need(6)?;
                Msg::SourceNames {
                    m: m1(b[0]),
                    l: m1(b[1]),
                    first: word(b[3], b[4]) + 1,
                    names: names(b[2], b[5], &b[6..]),
                }
            }
            107 | 108 => {
                need(5)?;
                let m = u32::from(b[0] >> 4) + self.ml;
                let first = word(b[2], b[3]) + 1;
                let list = names(b[1], b[4], &b[5..]);
                if cmd == 107 {
                    Msg::DestNames {
                        m,
                        first,
                        names: list,
                    }
                } else {
                    Msg::UmdLabels {
                        m,
                        first,
                        names: list,
                    }
                }
            }
            235 | 236 => {
                need(6)?;
                let m = m1(b[0]);
                let first = word(b[3], b[4]) + 1;
                let list = names(b[2], b[5], &b[6..]);
                if cmd == 235 {
                    Msg::DestNames {
                        m,
                        first,
                        names: list,
                    }
                } else {
                    Msg::UmdLabels {
                        m,
                        first,
                        names: list,
                    }
                }
            }
            113 => {
                need(4)?;
                Msg::TieLine {
                    m: m1(b[0]),
                    d: word(b[1], b[2]) + 1,
                    sources: b[4..]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .take(usize::from(b[3]))
                        .map(|e| {
                            json!({
                                "matrix": m1(e[0]),
                                "level": m1(e[1]),
                                "source": word(e[2], e[3]) + 1,
                            })
                        })
                        .collect(),
                }
            }
            122 => {
                need(5)?;
                Msg::SalvoAck {
                    salvo: u32::from(b[4] & 0x7F) + 1,
                }
            }
            250 => {
                need(7)?;
                Msg::SalvoAck {
                    salvo: u32::from(b[6] & 0x7F) + 1,
                }
            }
            123 => {
                need(2)?;
                Msg::GoDone {
                    status: b[0],
                    salvo: u32::from(b[1] & 0x7F) + 1,
                }
            }
            125 => {
                need(7)?;
                let (m, l) = ml(b[0]);
                Msg::SalvoTally {
                    salvo: u32::from(b[4] & 0x7F) + 1,
                    index: u32::from(b[5]),
                    validity: b[6],
                    entry: json!({
                        "matrix": m,
                        "level": l,
                        "destination": ten(b[1] >> 4, b[2]) + 1,
                        "source": ten(b[1], b[3]) + 1,
                    }),
                }
            }
            253 => {
                need(10)?;
                Msg::SalvoTally {
                    salvo: u32::from(b[6] & 0x7F) + 1,
                    index: word(b[7], b[8]),
                    validity: b[9],
                    entry: json!({
                        "matrix": m1(b[0]),
                        "level": m1(b[1]),
                        "destination": word(b[2], b[3]) + 1,
                        "source": word(b[4], b[5]) + 1,
                    }),
                }
            }
            9 => {
                need(1)?;
                Msg::DualStatus {
                    active_card: if b[0] & 1 == 0 { "master" } else { "slave" },
                    active: b[0] & 2 != 0,
                    idle_ok: b.get(1).is_none_or(|&i| i == 0),
                }
            }
            98 => {
                need(2)?;
                let outs = usize::from(b[0]);
                let ins = usize::from(b[1]);
                let list = &b[2..];
                if list.len() < outs + ins {
                    return None;
                }
                Msg::Implementation {
                    transmitted: list[..outs].to_vec(),
                    received: list[outs..outs + ins].to_vec(),
                }
            }
            99 => Msg::Invalid {
                command: *b.first()?,
            },
            30 => Msg::NamesUpdated,
            _ => return None,
        })
    }
}

/// Set `value` at a path of keys under `root`.
fn put(root: &mut Map<String, Value>, path: &[String], value: Value) {
    let (last, parents) = path.split_last().expect("a path");
    let mut node = root;
    for key in parents {
        node = node
            .entry(key.clone())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("objects all the way down");
    }
    node.insert(last.clone(), value);
}

fn dest_path(m: u32, l: u32, d: u32, field: &str) -> Vec<String> {
    vec![
        "matrices".into(),
        m.to_string(),
        "levels".into(),
        l.to_string(),
        "destinations".into(),
        d.to_string(),
        field.into(),
    ]
}

/// The state a message carries.
fn state_patch(msg: &Msg) -> Option<Value> {
    let mut root = Map::new();
    let names_under = |root: &mut Map<String, Value>,
                       prefix: &[String],
                       first: u32,
                       list: &[String],
                       leaf: Option<&str>| {
        for (i, name) in list.iter().enumerate() {
            let mut path = prefix.to_vec();
            path.push((first + i as u32).to_string());
            if let Some(leaf) = leaf {
                path.push(leaf.into());
            }
            put(root, &path, json!(name));
        }
    };
    match msg {
        Msg::Route {
            m, l, d, s, bad, ..
        } => {
            put(&mut root, &dest_path(*m, *l, *d, "source"), json!(s));
            if let Some(bad) = bad {
                put(&mut root, &dest_path(*m, *l, *d, "source_bad"), json!(bad));
            }
        }
        Msg::TallyDump {
            m,
            l,
            first,
            sources,
        } => {
            for (i, s) in sources.iter().enumerate() {
                put(
                    &mut root,
                    &dest_path(*m, *l, first + i as u32, "source"),
                    json!(s),
                );
            }
        }
        Msg::Protect {
            m,
            l,
            d,
            state,
            device,
        } => {
            put(&mut root, &dest_path(*m, *l, *d, "protect"), json!(state));
            put(
                &mut root,
                &dest_path(*m, *l, *d, "protect_device"),
                json!(device),
            );
        }
        Msg::ProtectDump {
            m,
            l,
            first,
            entries,
        } => {
            for (i, (state, device)) in entries.iter().enumerate() {
                let d = first + i as u32;
                put(&mut root, &dest_path(*m, *l, d, "protect"), json!(state));
                put(
                    &mut root,
                    &dest_path(*m, *l, d, "protect_device"),
                    json!(device),
                );
            }
        }
        Msg::DeviceName { device, name } => {
            put(
                &mut root,
                &["protect_devices".into(), device.to_string(), "name".into()],
                json!(name),
            );
        }
        Msg::SourceNames { m, l, first, names } => names_under(
            &mut root,
            &[
                "matrices".into(),
                m.to_string(),
                "levels".into(),
                l.to_string(),
                "sources".into(),
            ],
            *first,
            names,
            Some("name"),
        ),
        Msg::DestNames { m, first, names } => names_under(
            &mut root,
            &["matrices".into(), m.to_string(), "destinations".into()],
            *first,
            names,
            Some("name"),
        ),
        Msg::SourceAssocNames { m, first, names } => names_under(
            &mut root,
            &[
                "matrices".into(),
                m.to_string(),
                "source_associations".into(),
            ],
            *first,
            names,
            Some("name"),
        ),
        Msg::UmdLabels { m, first, names } => names_under(
            &mut root,
            &["matrices".into(), m.to_string(), "umd_labels".into()],
            *first,
            names,
            None,
        ),
        Msg::DualStatus {
            active_card,
            active,
            idle_ok,
        } => {
            root.insert(
                "controller".into(),
                json!({"active_card": active_card, "active": active, "idle_ok": idle_ok}),
            );
        }
        Msg::Implementation {
            transmitted,
            received,
        } => {
            root.insert(
                "implementation".into(),
                json!({"received": received, "transmitted": transmitted}),
            );
        }
        _ => return None,
    }
    (!root.is_empty()).then_some(Value::Object(root))
}

// ── Requests ────────────────────────────────────────────────────────────────

/// The data message a request waits for, in operator numbering.
#[derive(Debug, Clone, PartialEq)]
enum Expect {
    /// The DLE ACK is all.
    Ack,
    /// The DLE ACK is all, and the controller never says more.
    Unverified,
    Tally {
        m: u32,
        l: u32,
        d: u32,
    },
    Connected {
        m: u32,
        l: u32,
        d: u32,
        s: u32,
    },
    Protect {
        m: u32,
        l: u32,
        d: u32,
        want: Want,
    },
    DeviceName {
        device: u32,
    },
    SourceName {
        m: u32,
        l: u32,
        s: u32,
    },
    DestName {
        m: u32,
        d: u32,
    },
    SourceAssocName {
        m: u32,
        n: u32,
    },
    UmdLabel {
        m: u32,
        s: u32,
    },
    TieLine {
        m: u32,
        d: u32,
    },
    SalvoAck {
        salvo: u32,
    },
    GoDone {
        salvo: u32,
        fire: bool,
    },
    SalvoTally {
        salvo: u32,
        index: u32,
    },
    DualStatus,
    Implementation,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Want {
    Protected { device: u32 },
    Unprotected,
    Read,
}

#[derive(Debug, Clone)]
struct Job {
    command: Option<CommandId>,
    /// The message, command byte first.
    data: Vec<u8>,
    expect: Expect,
    /// Sends so far.
    attempts: u32,
    /// Salvo entries read so far, for `get_salvo`.
    entries: Vec<Value>,
    /// get_salvo in the extended form.
    extended: bool,
}

impl Job {
    fn new(data: Vec<u8>, expect: Expect) -> Job {
        Job {
            command: None,
            data,
            expect,
            attempts: 0,
            entries: Vec::new(),
            extended: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage {
    AwaitAck,
    AwaitReply,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CommandSet {
    Auto,
    General,
    Extended,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Connecting,
    Ready,
}

pub(crate) struct Swp08 {
    device: SocketAddr,
    model: String,
    supports: HashSet<String>,
    numbering: Numbering,
    extended_model: bool,
    command_set: CommandSet,
    matrix: u32,
    levels: u32,
    name_length: u8,
    read_names: bool,
    read_protects: bool,
    protect_device: u32,
    monitor: bool,
    phase: Phase,
    connected: bool,
    deframer: Deframer,
    commands: VecDeque<Job>,
    polls: VecDeque<Job>,
    in_flight: Option<(Job, Stage)>,
    sent_at: Millis,
    retry_after: Millis,
}

fn int(p: &Params, key: &str) -> Result<u32, CommandError> {
    p.get(key)
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn text<'a>(p: &'a Params, key: &str) -> Result<&'a str, CommandError> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn device_error(message: impl Into<String>) -> CommandError {
    CommandError::DeviceError {
        code: None,
        message: message.into(),
    }
}

fn hi(v: u32) -> u8 {
    (v >> 8) as u8
}

fn lo(v: u32) -> u8 {
    v as u8
}

/// The general form's multiplier byte (§3.1.2).
fn mult(d: u32, s: u32) -> u8 {
    (((d >> 7) & 7) << 4) as u8 | ((s >> 7) & 7) as u8
}

impl Swp08 {
    pub(crate) fn new(spec: &DeviceSpec, ctx: OpenContext) -> Result<Swp08, String> {
        let model = spec
            .models
            .iter()
            .find(|m| m.id == ctx.model)
            .ok_or_else(|| format!("no SW-P-08 model '{}'", ctx.model))?;
        let s = &ctx.settings;
        let num = |key: &str, default: u64| s.get(key).and_then(Value::as_u64).unwrap_or(default);
        let flag = |key: &str| s.get(key).and_then(Value::as_bool).unwrap_or(true);
        let command_set = match s.get("command_set").and_then(Value::as_str) {
            None | Some("auto") => CommandSet::Auto,
            Some("general") => CommandSet::General,
            Some("extended") => CommandSet::Extended,
            Some(other) => return Err(format!("unknown command_set '{other}'")),
        };
        let extended_model = !GENERAL_ONLY.contains(&model.id.as_str());
        if command_set == CommandSet::Extended && !extended_model {
            return Err(format!(
                "model '{}' has only the general commands; command_set cannot be extended",
                model.id
            ));
        }
        let name_length = match s.get("name_length").and_then(Value::as_str) {
            None => 1,
            Some(t) => match length_code(t) {
                Some(code) if code < 3 => code,
                _ => return Err(format!("name_length '{t}' is not 4, 8 or 12")),
            },
        };
        let mut m = Swp08 {
            device: SocketAddr::new(ctx.host, ctx.port.unwrap_or(DEFAULT_PORT)),
            model: model.id.clone(),
            supports: model.supports.iter().cloned().collect(),
            numbering: Numbering {
                ml: if model.id == "probel-system2" { 0 } else { 1 },
            },
            extended_model,
            command_set,
            matrix: num("matrix", 1) as u32,
            levels: num("levels", 1) as u32,
            name_length,
            read_names: flag("read_names"),
            read_protects: flag("read_protects"),
            protect_device: num("protect_device", 0) as u32,
            monitor: ctx.monitor,
            phase: Phase::Closed,
            connected: false,
            deframer: Deframer::default(),
            commands: VecDeque::new(),
            polls: VecDeque::new(),
            in_flight: None,
            sent_at: 0,
            retry_after: RETRY_MIN,
        };
        m.matrix = m.matrix.max(1);
        m.levels = m.levels.max(1);
        Ok(m)
    }

    // ── Numbering and the choice of command set ─────────────────────────────

    /// A matrix or level number on the wire.
    fn ml_wire(&self, what: &str, v: u32) -> Result<u32, CommandError> {
        v.checked_sub(self.numbering.ml)
            .filter(|w| *w <= 255)
            .ok_or_else(|| invalid(format!("{what} {v} is out of range for this model")))
    }

    /// Whether to send the extended form: needed for a matrix or level above
    /// 15 or a number above 1023 on the wire (§3.3).
    fn extended(&self, m: u32, l: u32, numbers: &[u32]) -> Result<bool, CommandError> {
        let needed = m > 15 || l > 15 || numbers.iter().any(|&n| n > 1023);
        match self.command_set {
            CommandSet::Extended => Ok(true),
            CommandSet::General | CommandSet::Auto if needed && !self.extended_model => Err(
                invalid(format!(
                    "model '{}' has only the general commands: matrix and level up to {}, numbers up to 1024",
                    self.model,
                    16 + self.numbering.ml - 1
                )),
            ),
            CommandSet::General if needed => Err(invalid(
                "command_set is general, whose commands carry matrix and level up to 16 and numbers up to 1024",
            )),
            _ => Ok(needed),
        }
    }

    /// Only the general form exists: check the numbers fit it.
    fn general_only(&self, m: u32, l: u32, numbers: &[u32]) -> Result<(), CommandError> {
        if m > 15 || l > 15 || numbers.iter().any(|&n| n > 1023) {
            Err(invalid(
                "this command has only the general form: matrix and level up to 16, numbers up to 1024",
            ))
        } else {
            Ok(())
        }
    }

    // ── Building requests ───────────────────────────────────────────────────

    fn tally_dump(&self, m: u32, l: u32) -> Result<Job, CommandError> {
        let (mw, lw) = (self.ml_wire("matrix", m)?, self.ml_wire("level", l)?);
        let data = if self.extended(mw, lw, &[])? {
            vec![149, mw as u8, lw as u8]
        } else {
            vec![21, (mw << 4 | lw) as u8]
        };
        Ok(Job::new(data, Expect::Ack))
    }

    fn protect_dump(&self, m: u32, l: u32) -> Result<Job, CommandError> {
        let (mw, lw) = (self.ml_wire("matrix", m)?, self.ml_wire("level", l)?);
        let data = if self.extended(mw, lw, &[])? {
            vec![147, mw as u8, lw as u8, 0, 0]
        } else {
            vec![19, (mw << 4 | lw) as u8, 0, 0]
        };
        Ok(Job::new(data, Expect::Ack))
    }

    fn source_names(&self, m: u32, l: u32) -> Result<Job, CommandError> {
        let (mw, lw) = (self.ml_wire("matrix", m)?, self.ml_wire("level", l)?);
        let data = if self.extended(mw, lw, &[])? {
            vec![228, mw as u8, lw as u8, self.name_length]
        } else {
            vec![100, (mw << 4 | lw) as u8, self.name_length]
        };
        Ok(Job::new(data, Expect::Ack))
    }

    fn dest_names(&self, m: u32) -> Result<Job, CommandError> {
        let mw = self.ml_wire("matrix", m)?;
        let data = if self.extended(mw, 0, &[])? {
            vec![230, mw as u8, self.name_length]
        } else {
            vec![102, (mw << 4) as u8, self.name_length]
        };
        Ok(Job::new(data, Expect::Ack))
    }

    fn command_job(&self, name: &str, p: &Params) -> Result<Job, CommandError> {
        let get = |key: &str| int(p, key);
        let opt = |key: &str| p.get(key).and_then(Value::as_u64).map(|v| v as u32);
        let ml = || -> Result<(u32, u32, u32, u32), CommandError> {
            let m = opt("matrix").unwrap_or(1);
            let l = opt("level").unwrap_or(1);
            Ok((m, l, self.ml_wire("matrix", m)?, self.ml_wire("level", l)?))
        };
        let job = match name {
            "connect" | "salvo_add" => {
                let (m, l, mw, lw) = ml()?;
                let (d, s) = (get("destination")?, get("source")?);
                let (dw, sw) = (d - 1, s - 1);
                let ext = self.extended(mw, lw, &[dw, sw])?;
                let mut data = if ext {
                    vec![
                        if name == "connect" { 130 } else { 248 },
                        mw as u8,
                        lw as u8,
                        hi(dw),
                        lo(dw),
                        hi(sw),
                        lo(sw),
                    ]
                } else {
                    vec![
                        if name == "connect" { 2 } else { 120 },
                        (mw << 4 | lw) as u8,
                        mult(dw, sw),
                        (dw & 0x7F) as u8,
                        (sw & 0x7F) as u8,
                    ]
                };
                if name == "connect" {
                    Job::new(data, Expect::Connected { m, l, d, s })
                } else {
                    let salvo = get("salvo")?;
                    data.push((salvo - 1) as u8);
                    Job::new(data, Expect::SalvoAck { salvo })
                }
            }
            "interrogate" => {
                let (m, l, mw, lw) = ml()?;
                let d = get("destination")?;
                let dw = d - 1;
                let data = if self.extended(mw, lw, &[dw])? {
                    vec![129, mw as u8, lw as u8, hi(dw), lo(dw)]
                } else {
                    vec![1, (mw << 4 | lw) as u8, mult(dw, 0), (dw & 0x7F) as u8]
                };
                Job::new(data, Expect::Tally { m, l, d })
            }
            "get_tally_dump" => {
                let (m, l, _, _) = ml()?;
                self.tally_dump(m, l)?
            }
            "get_protect_dump" => {
                let (m, l, _, _) = ml()?;
                self.protect_dump(m, l)?
            }
            "protect" | "unprotect" | "get_protect" => {
                let (m, l, mw, lw) = ml()?;
                let d = get("destination")?;
                let dw = d - 1;
                let dev = self.protect_device;
                let ext = self.extended(mw, lw, &[dw, dev])?;
                let data = match (name, ext) {
                    ("get_protect", true) => vec![138, mw as u8, lw as u8, hi(dw), lo(dw)],
                    ("get_protect", false) => {
                        vec![10, (mw << 4 | lw) as u8, mult(dw, 0), (dw & 0x7F) as u8]
                    }
                    (_, true) => vec![
                        if name == "protect" { 140 } else { 142 },
                        mw as u8,
                        lw as u8,
                        hi(dw),
                        lo(dw),
                        hi(dev),
                        lo(dev),
                    ],
                    (_, false) => vec![
                        if name == "protect" { 12 } else { 14 },
                        (mw << 4 | lw) as u8,
                        mult(dw, dev),
                        (dw & 0x7F) as u8,
                        (dev & 0x7F) as u8,
                    ],
                };
                let want = match name {
                    "protect" => Want::Protected { device: dev },
                    "unprotect" => Want::Unprotected,
                    _ => Want::Read,
                };
                Job::new(data, Expect::Protect { m, l, d, want })
            }
            "master_protect" => {
                let (m, l, mw, lw) = ml()?;
                let d = get("destination")?;
                let (dw, dev) = (d - 1, self.protect_device);
                let data = vec![29, mw as u8, lw as u8, hi(dw), lo(dw), hi(dev), lo(dev)];
                Job::new(
                    data,
                    Expect::Protect {
                        m,
                        l,
                        d,
                        want: Want::Protected { device: dev },
                    },
                )
            }
            "get_protect_device_name" => {
                let device = get("device")?;
                Job::new(
                    vec![17, (device >> 7) as u8 & 7, (device & 0x7F) as u8],
                    Expect::DeviceName { device },
                )
            }
            "get_source_names" => {
                let (m, l, _, _) = ml()?;
                self.source_names(m, l)?
            }
            "get_source_name" => {
                let (m, l, mw, lw) = ml()?;
                let s = get("source")?;
                let sw = s - 1;
                let data = if self.extended(mw, lw, &[])? {
                    vec![229, mw as u8, lw as u8, self.name_length, hi(sw), lo(sw)]
                } else {
                    vec![101, (mw << 4 | lw) as u8, self.name_length, hi(sw), lo(sw)]
                };
                Job::new(data, Expect::SourceName { m, l, s })
            }
            "get_destination_names" => self.dest_names(opt("matrix").unwrap_or(1))?,
            "get_destination_name" => {
                let m = opt("matrix").unwrap_or(1);
                let mw = self.ml_wire("matrix", m)?;
                let d = get("destination")?;
                let dw = d - 1;
                let data = if self.extended(mw, 0, &[])? {
                    vec![231, mw as u8, self.name_length, hi(dw), lo(dw)]
                } else {
                    vec![103, (mw << 4) as u8, self.name_length, hi(dw), lo(dw)]
                };
                Job::new(data, Expect::DestName { m, d })
            }
            "get_source_association_names" | "get_source_association_name" => {
                let m = opt("matrix").unwrap_or(1);
                let mw = self.ml_wire("matrix", m)?;
                self.general_only(mw, 0, &[])?;
                if name == "get_source_association_names" {
                    Job::new(vec![114, (mw << 4) as u8, self.name_length], Expect::Ack)
                } else {
                    let n = get("source_association")?;
                    let nw = n - 1;
                    Job::new(
                        vec![115, (mw << 4) as u8, self.name_length, hi(nw), lo(nw)],
                        Expect::SourceAssocName { m, n },
                    )
                }
            }
            "get_umd_labels" | "get_umd_label" => {
                let m = opt("matrix").unwrap_or(1);
                let mw = self.ml_wire("matrix", m)?;
                let code = length_code(text(p, "length").unwrap_or("8"))
                    .ok_or_else(|| invalid("length is 4, 8, 12 or 16"))?;
                let ext = self.extended(mw, 0, &[])?;
                if name == "get_umd_labels" {
                    let data = if ext {
                        vec![232, mw as u8, code]
                    } else {
                        vec![104, (mw << 4) as u8, code]
                    };
                    Job::new(data, Expect::Ack)
                } else {
                    let s = get("source")?;
                    let sw = s - 1;
                    let data = if ext {
                        vec![233, mw as u8, code, hi(sw), lo(sw)]
                    } else {
                        vec![105, (mw << 4) as u8, code, hi(sw), lo(sw)]
                    };
                    Job::new(data, Expect::UmdLabel { m, s })
                }
            }
            "update_name" => {
                let kind = match text(p, "kind")? {
                    "source" => 0,
                    "source_association" => 1,
                    "destination_association" => 2,
                    "umd_label" => 3,
                    other => return Err(invalid(format!("unknown kind '{other}'"))),
                };
                let (_, _, mw, lw) = ml()?;
                if mw > 19 || lw > 15 {
                    return Err(invalid("update_name takes matrix 1-20 and level 1-16"));
                }
                let code = length_code(text(p, "length")?)
                    .ok_or_else(|| invalid("length is 4, 8, 12 or 16"))?;
                let width = name_chars(code).expect("a known code");
                let label = text(p, "name")?;
                if !label.is_ascii() || label.len() > width {
                    return Err(invalid(format!(
                        "name must be ASCII of at most {width} characters"
                    )));
                }
                let n = get("number")? - 1;
                let mut data = vec![117, kind, code, mw as u8, lw as u8, hi(n), lo(n), 1];
                data.extend(format!("{label:<width$}").bytes());
                Job::new(data, Expect::Unverified)
            }
            "salvo_fire" | "salvo_clear" => {
                let salvo = get("salvo")?;
                let fire = name == "salvo_fire";
                Job::new(
                    vec![121, if fire { 0 } else { 1 }, (salvo - 1) as u8],
                    Expect::GoDone { salvo, fire },
                )
            }
            "get_salvo" => {
                let salvo = get("salvo")?;
                let ext = self.command_set == CommandSet::Extended;
                let mut job = self.salvo_read(salvo, 0, ext);
                job.extended = ext;
                job
            }
            "connect_tie_line" => {
                let dm = self.ml_wire("matrix", opt("destination_matrix").unwrap_or(1))?;
                let sm = self.ml_wire("matrix", opt("source_matrix").unwrap_or(1))?;
                let da = get("destination_association")? - 1;
                let sa = get("source_association")? - 1;
                let (dl, sl) = (get("destination_levels")?, get("source_levels")?);
                Job::new(
                    vec![
                        111,
                        dm as u8,
                        hi(da),
                        lo(da),
                        hi(dl),
                        lo(dl),
                        sm as u8,
                        hi(sa),
                        lo(sa),
                        hi(sl),
                        lo(sl),
                    ],
                    Expect::Unverified,
                )
            }
            "get_tie_line" => {
                let m = opt("matrix").unwrap_or(1);
                let mw = self.ml_wire("matrix", m)?;
                let d = get("destination_association")?;
                let dw = d - 1;
                Job::new(
                    vec![112, mw as u8, hi(dw), lo(dw)],
                    Expect::TieLine { m, d },
                )
            }
            "get_controller_status" => Job::new(vec![8], Expect::DualStatus),
            "get_implementation" => Job::new(vec![97], Expect::Implementation),
            "clear_protects" => {
                let (_, _, mw, lw) = ml()?;
                let (mb, lb) = match text(p, "scope").unwrap_or("level") {
                    "level" => (mw as u8, lw as u8),
                    "matrix" => (mw as u8, 0xFF),
                    "all" => (0xFF, 0xFF),
                    other => return Err(invalid(format!("unknown scope '{other}'"))),
                };
                Job::new(vec![7, 2, mb, lb], Expect::Ack)
            }
            "reset" => {
                let kind = match text(p, "kind")? {
                    "hard" => 0,
                    "soft" => 1,
                    other => return Err(invalid(format!("unknown kind '{other}'"))),
                };
                Job::new(vec![7, kind], Expect::Ack)
            }
            _ => {
                return Err(CommandError::UnknownCommand {
                    command: name.into(),
                })
            }
        };
        Ok(job)
    }

    /// One step of reading a salvo group (§3.1.31, §3.4.17).
    fn salvo_read(&self, salvo: u32, index: u32, extended: bool) -> Job {
        let s = (salvo - 1) as u8;
        let data = if extended {
            vec![252, s, hi(index), lo(index)]
        } else {
            vec![124, s, index as u8]
        };
        let mut job = Job::new(data, Expect::SalvoTally { salvo, index });
        job.extended = extended;
        job
    }

    /// Reads that keep state complete: on connecting, on refresh, and the
    /// parts named after a salvo or a names change.
    fn queue_reads(&mut self, routes: bool, protects: bool, names: bool) {
        let m = self.matrix;
        let mut jobs: Vec<Result<Job, CommandError>> = Vec::new();
        for l in 1..=self.levels {
            if routes && self.supports.contains("get_tally_dump") {
                jobs.push(self.tally_dump(m, l));
            }
            if protects && self.read_protects && self.supports.contains("get_protect_dump") {
                jobs.push(self.protect_dump(m, l));
            }
            if names && self.read_names && self.supports.contains("get_source_names") {
                jobs.push(self.source_names(m, l));
            }
        }
        if names && self.read_names && self.supports.contains("get_destination_names") {
            jobs.push(self.dest_names(m));
        }
        for job in jobs.into_iter().flatten() {
            self.polls.push_back(job);
        }
    }

    // ── The link ────────────────────────────────────────────────────────────

    fn send_in_flight(&mut self, cx: &mut Cx) {
        if let Some((job, stage)) = self.in_flight.as_mut() {
            job.attempts += 1;
            *stage = Stage::AwaitAck;
            cx.tcp_send(SOCKET, frame(&job.data));
            cx.set_timer(ACK_TIMER, ACK_TIMEOUT);
            self.sent_at = cx.now();
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Ready || self.in_flight.is_some() {
            return;
        }
        let Some(job) = self.commands.pop_front().or_else(|| self.polls.pop_front()) else {
            return;
        };
        self.in_flight = Some((job, Stage::AwaitAck));
        self.send_in_flight(cx);
    }

    /// The in-flight request is finished, with this result for its command.
    fn finish(&mut self, cx: &mut Cx, result: Result<Outcome, CommandError>) {
        cx.cancel_timer(ACK_TIMER);
        cx.cancel_timer(REPLY_TIMER);
        if let Some((job, _)) = self.in_flight.take() {
            if let Some(id) = job.command {
                cx.complete(id, result);
            } else if let Err(e) = result {
                cx.log(
                    Level::Debug,
                    format!("SW-P-08: request {} failed: {e}", job.data[0]),
                );
            }
        }
        self.pump(cx);
    }

    fn set_connected(&mut self, cx: &mut Cx) {
        if !self.connected {
            self.connected = true;
            self.retry_after = RETRY_MIN;
            cx.connection(Connection::Connected);
        }
    }

    /// The controller cannot be reached or stopped acknowledging: fail what
    /// is waiting and reconnect with backoff.
    fn lost(&mut self, cx: &mut Cx, reason: String) {
        let fail = |cx: &mut Cx, job: Job| {
            if let Some(id) = job.command {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        };
        if let Some((job, _)) = self.in_flight.take() {
            fail(cx, job);
        }
        for job in self.commands.drain(..) {
            fail(cx, job);
        }
        self.polls.clear();
        cx.cancel_timer(ACK_TIMER);
        cx.cancel_timer(REPLY_TIMER);
        if self.phase != Phase::Closed {
            cx.tcp_close(SOCKET);
        }
        self.phase = Phase::Closed;
        self.deframer = Deframer::default();
        self.connected = false;
        cx.connection(Connection::Disconnected {
            reason: reason.clone(),
        });
        cx.log(Level::Debug, format!("SW-P-08: {reason}"));
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn on_ack(&mut self, cx: &mut Cx) {
        let Some((job, stage)) = self.in_flight.as_mut() else {
            return;
        };
        if *stage != Stage::AwaitAck {
            return;
        }
        cx.cancel_timer(ACK_TIMER);
        match job.expect {
            Expect::Ack => {
                cx.round_trip(cx.now() - self.sent_at);
                self.finish(cx, Ok(Outcome::Ack));
            }
            Expect::Unverified => {
                cx.round_trip(cx.now() - self.sent_at);
                self.finish(cx, Ok(Outcome::Unverified));
            }
            _ => {
                *stage = Stage::AwaitReply;
                cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
            }
        }
    }

    fn on_nak(&mut self, cx: &mut Cx) {
        let Some((job, stage)) = self.in_flight.as_ref() else {
            return;
        };
        if *stage != Stage::AwaitAck {
            return;
        }
        // A poll the controller refuses is most likely a command it does not
        // implement: drop it rather than repeat it.
        if job.command.is_none() || job.attempts >= ATTEMPTS {
            let cmd = job.data[0];
            self.finish(
                cx,
                Err(device_error(format!(
                    "the controller refused message {cmd} with NAK"
                ))),
            );
        } else {
            self.send_in_flight(cx);
        }
    }

    /// A message from the controller: apply it to state, then see whether
    /// it answers the request in flight.
    fn on_message(&mut self, cx: &mut Cx, data: &[u8]) {
        let Some(msg) = self.numbering.decode(data) else {
            cx.log(
                Level::Debug,
                format!(
                    "SW-P-08: ignored message {}",
                    data.first().copied().unwrap_or(0)
                ),
            );
            return;
        };
        if let Some(patch) = state_patch(&msg) {
            cx.state(patch);
        }
        if msg == Msg::NamesUpdated && self.monitor {
            self.queue_reads(false, false, true);
        }
        if let Msg::Invalid { command } = msg {
            if let Some((job, Stage::AwaitReply)) = &self.in_flight {
                if job.data[0] == command {
                    cx.round_trip(cx.now() - self.sent_at);
                    self.finish(
                        cx,
                        Err(CommandError::DeviceError {
                            code: Some(command.to_string()),
                            message: format!("the controller reports message {command} invalid"),
                        }),
                    );
                }
            }
            return;
        }
        let Some((job, Stage::AwaitReply)) = &self.in_flight else {
            return;
        };
        let fire = matches!(job.expect, Expect::GoDone { fire: true, .. });
        let Some(result) = answer(&job.expect, &msg) else {
            return;
        };
        cx.round_trip(cx.now() - self.sent_at);
        match result {
            Answer::Done(result) => {
                if fire && result.is_ok() && self.monitor {
                    // No CONNECTED follows a salvo (§3.1.30).
                    self.queue_reads(true, false, false);
                }
                self.finish(cx, result);
            }
            Answer::SalvoEntry { entry, more } => {
                let (mut job, _) = self.in_flight.take().expect("checked above");
                cx.cancel_timer(REPLY_TIMER);
                if let Some(entry) = entry {
                    job.entries.push(entry);
                }
                let Expect::SalvoTally { salvo, index } = job.expect else {
                    unreachable!("a salvo answer");
                };
                if more {
                    let mut next = self.salvo_read(salvo, index + 1, job.extended);
                    next.command = job.command;
                    next.entries = job.entries;
                    self.commands.push_front(next);
                    self.pump(cx);
                } else {
                    let list = Value::Array(job.entries);
                    if let Some(id) = job.command {
                        cx.complete(id, Ok(Outcome::Value { value: list }));
                    }
                    self.pump(cx);
                }
            }
        }
    }

    fn on_data(&mut self, cx: &mut Cx, data: &[u8]) {
        cx.alive();
        self.set_connected(cx);
        for inbound in self.deframer.feed(data) {
            match inbound {
                Inbound::Ack => self.on_ack(cx),
                Inbound::Nak => self.on_nak(cx),
                Inbound::Bad => cx.tcp_send(SOCKET, vec![DLE, NAK]),
                Inbound::Message(message) => {
                    // §2.2.2: acknowledge at once, then act on it.
                    cx.tcp_send(SOCKET, vec![DLE, ACK]);
                    self.on_message(cx, &message);
                }
            }
            if self.phase != Phase::Ready {
                return;
            }
        }
    }
}

enum Answer {
    Done(Result<Outcome, CommandError>),
    SalvoEntry { entry: Option<Value>, more: bool },
}

/// Whether `msg` answers a request waiting for `expect`, and how.
fn answer(expect: &Expect, msg: &Msg) -> Option<Answer> {
    let value = |v: Value| Answer::Done(Ok(Outcome::Value { value: v }));
    Some(match (expect, msg) {
        (
            Expect::Tally { m, l, d },
            Msg::Route {
                connected: false,
                m: rm,
                l: rl,
                d: rd,
                s,
                ..
            },
        ) if (m, l, d) == (rm, rl, rd) => value(json!(s)),
        (
            Expect::Connected { m, l, d, s },
            Msg::Route {
                connected: true,
                m: rm,
                l: rl,
                d: rd,
                s: rs,
                ..
            },
        ) if (m, l, d) == (rm, rl, rd) => Answer::Done(if s == rs {
            Ok(Outcome::Ack)
        } else {
            Err(device_error(format!(
                "destination {d} reports source {rs}, not {s}"
            )))
        }),
        (
            Expect::Protect { m, l, d, want },
            Msg::Protect {
                m: rm,
                l: rl,
                d: rd,
                state,
                device,
            },
        ) if (m, l, d) == (rm, rl, rd) => Answer::Done(match want {
            Want::Read => Ok(Outcome::Value {
                value: json!({"state": state, "device": device}),
            }),
            Want::Protected { device: ours } if *state == "remote" && device == ours => {
                Ok(Outcome::Ack)
            }
            Want::Unprotected if *state == "none" => Ok(Outcome::Ack),
            _ => Err(device_error(format!(
                "destination {d} is {state} (device {device})"
            ))),
        }),
        (Expect::DeviceName { device }, Msg::DeviceName { device: rd, name }) if device == rd => {
            value(json!(name))
        }
        (
            Expect::SourceName { m, l, s },
            Msg::SourceNames {
                m: rm,
                l: rl,
                first,
                names,
            },
        ) if (m, l, s) == (rm, rl, first) => value(json!(names.first()?)),
        (
            Expect::DestName { m, d },
            Msg::DestNames {
                m: rm,
                first,
                names,
            },
        ) if (m, d) == (rm, first) => value(json!(names.first()?)),
        (
            Expect::SourceAssocName { m, n },
            Msg::SourceAssocNames {
                m: rm,
                first,
                names,
            },
        ) if (m, n) == (rm, first) => value(json!(names.first()?)),
        (
            Expect::UmdLabel { m, s },
            Msg::UmdLabels {
                m: rm,
                first,
                names,
            },
        ) if (m, s) == (rm, first) => value(json!(names.first()?)),
        (
            Expect::TieLine { m, d },
            Msg::TieLine {
                m: rm,
                d: rd,
                sources,
            },
        ) if (m, d) == (rm, rd) => value(Value::Array(sources.clone())),
        (Expect::SalvoAck { salvo }, Msg::SalvoAck { salvo: rs }) if salvo == rs => {
            Answer::Done(Ok(Outcome::Ack))
        }
        (Expect::GoDone { salvo, fire }, Msg::GoDone { status, salvo: rs }) if salvo == rs => {
            Answer::Done(match (fire, status) {
                (true, 0) | (false, 1 | 2) => Ok(Outcome::Ack),
                (true, 2) => Err(device_error(format!("salvo {salvo} holds no routes"))),
                _ => Err(device_error(format!(
                    "salvo {salvo}: unexpected GO DONE status {status}"
                ))),
            })
        }
        (
            Expect::SalvoTally { salvo, index },
            Msg::SalvoTally {
                salvo: rs,
                index: ri,
                validity,
                entry,
            },
        ) if salvo == rs && (index & 0xFF) == (ri & 0xFF) => match validity {
            0 => Answer::SalvoEntry {
                entry: Some(entry.clone()),
                more: true,
            },
            1 => Answer::SalvoEntry {
                entry: Some(entry.clone()),
                more: false,
            },
            _ => Answer::SalvoEntry {
                entry: None,
                more: false,
            },
        },
        (
            Expect::DualStatus,
            Msg::DualStatus {
                active_card,
                active,
                idle_ok,
            },
        ) => value(json!({"active_card": active_card, "active": active, "idle_ok": idle_ok})),
        (
            Expect::Implementation,
            Msg::Implementation {
                transmitted,
                received,
            },
        ) => value(json!({"received": received, "transmitted": transmitted})),
        _ => return None,
    })
}

impl Module for Swp08 {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.phase = Phase::Connecting;
        cx.tcp_open(SOCKET, self.device);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.phase == Phase::Closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        if name == "refresh" {
            // Everything, once, even when opened for commands only.
            self.queue_reads(true, true, true);
            cx.complete(id, Ok(Outcome::Ack));
            self.pump(cx);
            return;
        }
        match self.command_job(name, params) {
            Ok(mut job) => {
                job.command = Some(id);
                self.commands.push_back(job);
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                if self.phase != Phase::Connecting {
                    return;
                }
                self.phase = Phase::Ready;
                // The first message's ACK shows the controller is there.
                self.polls.push_back(Job::new(vec![8], Expect::Ack));
                if self.monitor {
                    if self.supports.contains("get_implementation") {
                        self.polls
                            .push_back(Job::new(vec![97], Expect::Implementation));
                    }
                    self.queue_reads(true, true, true);
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
                self.pump(cx);
            }
            TcpInput::Data(data) => {
                if self.phase == Phase::Ready {
                    self.on_data(cx, &data);
                }
            }
            TcpInput::Closed { reason } => {
                if self.phase != Phase::Closed {
                    // Already closed: lost() sends no close for it.
                    self.phase = Phase::Closed;
                    self.lost(cx, reason);
                }
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => {
                if self.phase == Phase::Closed {
                    self.phase = Phase::Connecting;
                    cx.connection(Connection::Connecting);
                    cx.tcp_open(SOCKET, self.device);
                }
            }
            PROBE_TIMER => {
                if self.phase == Phase::Ready {
                    if self.in_flight.is_none() && self.commands.is_empty() && self.polls.is_empty()
                    {
                        self.polls.push_back(Job::new(vec![8], Expect::Ack));
                        self.pump(cx);
                    }
                    cx.set_timer(PROBE_TIMER, PROBE_EVERY);
                }
            }
            ACK_TIMER => {
                let Some((job, Stage::AwaitAck)) = &self.in_flight else {
                    return;
                };
                if job.attempts >= ATTEMPTS {
                    self.lost(cx, format!("no acknowledgement after {ATTEMPTS} sends"));
                } else {
                    self.send_in_flight(cx);
                }
            }
            REPLY_TIMER => {
                if matches!(self.in_flight, Some((_, Stage::AwaitReply))) {
                    self.finish(cx, Err(CommandError::Timeout));
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Closed {
            cx.tcp_close(SOCKET);
        }
        self.phase = Phase::Closed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn spec() -> DeviceSpec {
        Catalog::source_tree().devices["probel-swp08"].clone()
    }

    fn module(model: &str, monitor: bool, settings: Value) -> Swp08 {
        let spec = spec();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        Swp08::new(
            &spec,
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 8)),
                port: None,
                model: model.into(),
                channels: None,
                settings,
                monitor,
            },
        )
        .unwrap()
    }

    fn unhex(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
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

    fn state(actions: &[Action]) -> Value {
        let mut merged = json!({});
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(&mut merged, p);
            }
        }
        merged
    }

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn feed_at(m: &mut Swp08, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn feed(m: &mut Swp08, bytes: &[u8]) -> Vec<Action> {
        feed_at(m, 0, bytes)
    }

    fn ack(m: &mut Swp08) -> Vec<Action> {
        feed(m, &[DLE, ACK])
    }

    fn run(m: &mut Swp08, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn timer(m: &mut Swp08, key: Key) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.timer(&mut cx, key);
        cx.take()
    }

    /// Started and connected; returns what was sent on connecting.
    fn connected(m: &mut Swp08) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    /// Acknowledge every request until the module is idle.
    fn drain(m: &mut Swp08) {
        while m.in_flight.is_some() {
            if matches!(m.in_flight, Some((_, Stage::AwaitReply))) {
                timer(m, REPLY_TIMER);
            } else {
                ack(m);
            }
        }
    }

    fn idle(model: &str) -> Swp08 {
        let mut m = module(model, false, json!({}));
        connected(&mut m);
        drain(&mut m);
        m
    }

    #[test]
    fn framing_matches_the_documented_examples() {
        // SW-P-88 Issue 1 §9.2 and §9.3; Issue 30 §9.
        assert_eq!(
            frame(&unhex("01 11 00 01")),
            unhex("10 02 01 11 00 01 04 E9 10 03")
        );
        assert_eq!(
            frame(&unhex("02 11 00 02 00")),
            unhex("10 02 02 11 00 02 00 05 E6 10 03")
        );
        assert_eq!(frame(&[97]), unhex("10 02 61 01 9E 10 03"));
        let mut d = Deframer::default();
        assert_eq!(
            d.feed(&unhex("10 06 10 02 03 11 01 01 19 05 CC 10 03")),
            vec![Inbound::Ack, Inbound::Message(unhex("03 11 01 01 19"))]
        );
    }

    #[test]
    fn dle_is_doubled_and_undoubled() {
        // Destination 17 is wire 16 (10h).
        let data = [2, 0x00, 0x00, 0x10, 0x05];
        let wire = frame(&data);
        assert_eq!(wire, unhex("10 02 02 00 00 10 10 05 05 E4 10 03"));
        let mut d = Deframer::default();
        // Split anywhere, including between the two DLEs.
        let mut got = d.feed(&wire[..6]);
        got.extend(d.feed(&wire[6..]));
        assert_eq!(got, vec![Inbound::Message(data.to_vec())]);
        // A bad checksum is reported, and garbage between frames is skipped.
        assert_eq!(
            d.feed(&unhex("FF 00 10 02 03 11 01 01 19 05 CD 10 03")),
            vec![Inbound::Bad]
        );
    }

    #[test]
    fn interrogate_and_connect_are_the_documented_bytes() {
        let mut m = idle("generic");
        // Wire matrix 1, level 1, destination 1: operator 2, 2, 2.
        let a = run(
            &mut m,
            1,
            "interrogate",
            json!({"matrix": 2, "level": 2, "destination": 2}),
        );
        assert_eq!(sent(&a), vec![unhex("10 02 01 11 00 01 04 E9 10 03")]);
        assert!(ack(&mut m)
            .iter()
            .all(|a| !matches!(a, Action::Complete { .. })));
        // The tally reply: source 153 on the wire, 154 here. It is ACKed.
        let a = feed(&mut m, &unhex("10 02 03 11 01 01 19 05 CC 10 03"));
        assert_eq!(sent(&a), vec![vec![DLE, ACK]]);
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(154) }))
        );
        assert_eq!(
            state(&a)["matrices"]["2"]["levels"]["2"]["destinations"]["2"],
            json!({"source": 154, "source_bad": false})
        );

        let a = run(
            &mut m,
            2,
            "connect",
            json!({"matrix": 2, "level": 2, "destination": 3, "source": 1}),
        );
        assert_eq!(sent(&a), vec![unhex("10 02 02 11 00 02 00 05 E6 10 03")]);
        ack(&mut m);
        let a = feed(&mut m, &unhex("10 02 04 11 00 02 00 05 E4 10 03"));
        assert_eq!(completed(&a, 2), Some(Ok(Outcome::Ack)));
        assert_eq!(
            state(&a)["matrices"]["2"]["levels"]["2"]["destinations"]["3"]["source"],
            json!(1)
        );
    }

    #[test]
    fn large_numbers_use_the_extended_commands() {
        let mut m = idle("generic");
        let a = run(
            &mut m,
            1,
            "connect",
            json!({"destination": 2000, "source": 1500}),
        );
        // 130, matrix 0, level 0, dest 1999 = 07CF, source 1499 = 05DB.
        assert_eq!(sent(&a), vec![frame(&unhex("82 00 00 07 CF 05 DB"))]);
        ack(&mut m);
        let a = feed(&mut m, &frame(&unhex("84 00 00 07 CF 05 DB 00")));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        // A level above 16 too.
        let a = run(
            &mut m,
            2,
            "interrogate",
            json!({"level": 17, "destination": 1}),
        );
        assert_eq!(sent(&a), vec![frame(&unhex("81 00 10 00 00"))]);
        // A model without the extended set refuses before sending.
        let mut f = idle("probel-freeway");
        let a = run(
            &mut f,
            3,
            "connect",
            json!({"destination": 1025, "source": 1}),
        );
        assert!(sent(&a).is_empty());
        assert!(matches!(
            completed(&a, 3),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        // command_set extended uses them for small numbers as well.
        let mut e = module("generic", false, json!({"command_set": "extended"}));
        connected(&mut e);
        drain(&mut e);
        let a = run(&mut e, 4, "interrogate", json!({"destination": 5}));
        assert_eq!(sent(&a), vec![frame(&unhex("81 00 00 00 04"))]);
    }

    #[test]
    fn system2_sends_matrix_and_level_as_given() {
        let mut m = idle("probel-system2");
        let a = run(
            &mut m,
            1,
            "interrogate",
            json!({"matrix": 1, "level": 1, "destination": 2}),
        );
        assert_eq!(sent(&a), vec![unhex("10 02 01 11 00 01 04 E9 10 03")]);
    }

    #[test]
    fn a_nak_or_a_missing_ack_resends_five_times_then_reconnects() {
        let mut m = idle("generic");
        let a = run(&mut m, 1, "get_tally_dump", json!({}));
        let first = sent(&a);
        assert_eq!(first, vec![frame(&[21, 0x00])]);
        // NAK: sent again.
        assert_eq!(sent(&feed(&mut m, &[DLE, NAK])), first);
        // No ACK within 1 s: sent again, up to five sends in all.
        for _ in 0..3 {
            assert_eq!(sent(&timer(&mut m, ACK_TIMER)), first);
        }
        let a = timer(&mut m, ACK_TIMER);
        assert!(sent(&a).is_empty());
        assert!(matches!(
            completed(&a, 1),
            Some(Err(CommandError::Transport { .. }))
        ));
        assert!(a.contains(&Action::TcpClose { socket: SOCKET }));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY_TIMER,
            after: RETRY_MIN
        }));
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        // A command while disconnected fails at once; the retry reconnects.
        assert_eq!(
            completed(&run(&mut m, 2, "interrogate", json!({"destination": 1})), 2),
            Some(Err(CommandError::NotConnected))
        );
        let a = timer(&mut m, RETRY_TIMER);
        assert!(a.iter().any(|a| matches!(a, Action::TcpOpen { .. })));
    }

    #[test]
    fn a_bad_frame_is_refused_with_nak() {
        let mut m = idle("generic");
        let a = feed(&mut m, &unhex("10 02 03 11 01 01 19 05 CD 10 03"));
        assert_eq!(sent(&a), vec![vec![DLE, NAK]]);
        assert_eq!(state(&a), json!({}));
    }

    #[test]
    fn monitored_it_reads_the_configured_levels_on_connecting() {
        let mut m = module(
            "generic",
            true,
            json!({"matrix": 1, "levels": 2, "name_length": "12"}),
        );
        let mut frames = sent(&connected(&mut m));
        while m.in_flight.is_some() {
            frames.extend(sent(&ack(&mut m)));
            if matches!(m.in_flight, Some((_, Stage::AwaitReply))) {
                frames.extend(sent(&timer(&mut m, REPLY_TIMER)));
            }
        }
        let expected: Vec<Vec<u8>> = [
            &[8][..],
            &[97],
            &[21, 0x00],
            &[19, 0x00, 0, 0],
            &[100, 0x00, 2],
            &[21, 0x01],
            &[19, 0x01, 0, 0],
            &[100, 0x01, 2],
            &[102, 0x00, 2],
        ]
        .iter()
        .map(|d| frame(d))
        .collect();
        assert_eq!(frames, expected);
    }

    #[test]
    fn opened_for_commands_only_it_sends_only_the_liveness_check() {
        let mut m = module("generic", false, json!({"levels": 4}));
        let a = connected(&mut m);
        assert_eq!(sent(&a), vec![frame(&[8])]);
        let a = ack(&mut m);
        assert!(sent(&a).is_empty());
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // Every 10 s while idle, the same request again.
        assert_eq!(sent(&timer(&mut m, PROBE_TIMER)), vec![frame(&[8])]);
        ack(&mut m);
        // Commands still work, and pushes still reach state.
        let a = run(&mut m, 1, "get_source_names", json!({"level": 3}));
        assert_eq!(sent(&a), vec![frame(&[100, 0x02, 1])]);
        assert_eq!(completed(&ack(&mut m), 1), Some(Ok(Outcome::Ack)));
        let a = feed(&mut m, &frame(&unhex("04 02 00 05 07")));
        assert_eq!(
            state(&a)["matrices"]["1"]["levels"]["3"]["destinations"]["6"]["source"],
            json!(8)
        );
        // NAMES UPDATED reads nothing again.
        assert_eq!(
            sent(&feed(&mut m, &frame(&[30, 0, 0, 0, 1, 0, 0, 0, 0x0F]))),
            vec![vec![DLE, ACK]]
        );
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let mut m = idle("generic");
        let mut cx = Cx::new(1_000);
        m.command(
            &mut cx,
            1,
            "interrogate",
            json!({"destination": 1}).as_object().unwrap(),
        );
        let a = feed_at(&mut m, 1_020, &[DLE, ACK]);
        assert!(!a.iter().any(|a| matches!(a, Action::RoundTrip(_))));
        let a = feed_at(&mut m, 1_045, &frame(&unhex("03 00 00 00 04")));
        assert!(a.contains(&Action::RoundTrip(45)));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(5) }))
        );
        let mut cx = Cx::new(2_000);
        m.command(&mut cx, 2, "get_tally_dump", json!({}).as_object().unwrap());
        let a = feed_at(&mut m, 2_007, &[DLE, ACK]);
        assert!(a.contains(&Action::RoundTrip(7)));
    }

    #[test]
    fn dumps_and_names_become_state() {
        let mut m = idle("generic");
        // Word dump (23): matrix 0 level 1, 3 tallies from destination 0.
        let a = feed(&mut m, &frame(&unhex("17 01 03 00 00 00 05 00 06 04 00")));
        let dests = &state(&a)["matrices"]["1"]["levels"]["2"]["destinations"];
        assert_eq!(dests["1"]["source"], json!(6));
        assert_eq!(dests["3"]["source"], json!(1025));
        // Byte dump (22) and extended dump (151).
        let a = feed(&mut m, &frame(&unhex("16 00 02 0A 01 02")));
        assert_eq!(
            state(&a)["matrices"]["1"]["levels"]["1"]["destinations"]["12"]["source"],
            json!(3)
        );
        let a = feed(&mut m, &frame(&unhex("97 02 11 01 04 00 00 09")));
        assert_eq!(
            state(&a)["matrices"]["3"]["levels"]["18"]["destinations"]["1025"]["source"],
            json!(10)
        );
        // Protect dump (20): first destination 4, OEM device 7 then none.
        let a = feed(&mut m, &frame(&unhex("14 00 02 00 04 30 07 00 00")));
        let dests = &state(&a)["matrices"]["1"]["levels"]["1"]["destinations"];
        assert_eq!(
            dests["5"],
            json!({"protect": "remote", "protect_device": 7})
        );
        assert_eq!(dests["6"], json!({"protect": "none", "protect_device": 0}));
        // Source names (106), 4 characters, from source 1.
        let a = feed(&mut m, &frame(b"\x6a\x00\x00\x00\x00\x02CAM1VTR "));
        let sources = &state(&a)["matrices"]["1"]["levels"]["1"]["sources"];
        assert_eq!(sources["1"]["name"], json!("CAM1"));
        assert_eq!(sources["2"]["name"], json!("VTR"));
        // Destination names (107) and UMD labels (108), matrix 1.
        let a = feed(&mut m, &frame(b"\x6b\x10\x01\x00\x09\x01MON 1   "));
        assert_eq!(
            state(&a)["matrices"]["2"]["destinations"]["10"]["name"],
            json!("MON 1")
        );
        let a = feed(&mut m, &frame(b"\x6c\x00\x00\x00\x00\x01CAM"));
        assert_eq!(state(&a), json!({}), "a short label is not read");
        // A broadcast protect change (13): device 130 through the multiplier.
        let a = feed(&mut m, &frame(&unhex("0D 00 01 01 05 02")));
        assert_eq!(
            state(&a)["matrices"]["1"]["levels"]["1"]["destinations"]["6"],
            json!({"protect": "panel", "protect_device": 130})
        );
        // Dual controller status.
        let a = feed(&mut m, &frame(&[9, 0b11, 1]));
        assert_eq!(
            state(&a)["controller"],
            json!({"active_card": "slave", "active": true, "idle_ok": false})
        );
    }

    #[test]
    fn protect_succeeds_only_as_this_device() {
        let mut m = module("generic", false, json!({"protect_device": 9}));
        connected(&mut m);
        drain(&mut m);
        let a = run(&mut m, 1, "protect", json!({"destination": 3}));
        assert_eq!(sent(&a), vec![frame(&[12, 0x00, 0x00, 2, 9])]);
        ack(&mut m);
        let a = feed(&mut m, &frame(&[13, 0x00, 3, 0x00, 2, 9]));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        run(&mut m, 2, "protect", json!({"destination": 4}));
        ack(&mut m);
        let a = feed(&mut m, &frame(&[13, 0x00, 1, 0x00, 3, 2]));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
        run(&mut m, 3, "get_protect", json!({"destination": 4}));
        ack(&mut m);
        let a = feed(&mut m, &frame(&[11, 0x00, 2, 0x00, 3, 2]));
        assert_eq!(
            completed(&a, 3),
            Some(Ok(Outcome::Value {
                value: json!({"state": "panel_override", "device": 2})
            }))
        );
        // A connect nobody answers times out, and the next request goes.
        run(&mut m, 4, "connect", json!({"destination": 4, "source": 1}));
        ack(&mut m);
        run(&mut m, 5, "interrogate", json!({"destination": 4}));
        let a = timer(&mut m, REPLY_TIMER);
        assert_eq!(completed(&a, 4), Some(Err(CommandError::Timeout)));
        assert_eq!(sent(&a), vec![frame(&[1, 0x00, 0x00, 3])]);
    }

    #[test]
    fn a_salvo_is_read_index_by_index() {
        let mut m = idle("generic");
        let a = run(&mut m, 1, "get_salvo", json!({"salvo": 3}));
        assert_eq!(sent(&a), vec![frame(&[124, 2, 0])]);
        ack(&mut m);
        let a = feed(&mut m, &frame(&[125, 0x00, 0x00, 4, 1, 2, 0, 0]));
        assert_eq!(sent(&a), vec![vec![DLE, ACK], frame(&[124, 2, 1])]);
        ack(&mut m);
        let a = feed(&mut m, &frame(&[125, 0x01, 0x00, 5, 2, 2, 1, 1]));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!([
                    {"matrix": 1, "level": 1, "destination": 5, "source": 2},
                    {"matrix": 1, "level": 2, "destination": 6, "source": 3}
                ])
            }))
        );
        // Firing an empty salvo fails; clearing it does not.
        run(&mut m, 2, "salvo_fire", json!({"salvo": 3}));
        ack(&mut m);
        let a = feed(&mut m, &frame(&[123, 2, 2]));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));
        run(&mut m, 3, "salvo_clear", json!({"salvo": 3}));
        ack(&mut m);
        assert_eq!(
            completed(&feed(&mut m, &frame(&[123, 2, 2])), 3),
            Some(Ok(Outcome::Ack))
        );
    }

    #[test]
    fn update_name_pads_and_is_unverified() {
        let mut m = idle("generic");
        let a = run(
            &mut m,
            1,
            "update_name",
            json!({"kind": "source", "number": 3, "length": "8", "name": "CAM 3"}),
        );
        assert_eq!(
            sent(&a),
            vec![frame(b"\x75\x00\x01\x00\x00\x00\x02\x01CAM 3   ")]
        );
        assert_eq!(completed(&ack(&mut m), 1), Some(Ok(Outcome::Unverified)));
        let a = run(
            &mut m,
            2,
            "update_name",
            json!({"kind": "source", "number": 3, "length": "4", "name": "CAMERA"}),
        );
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
    }
}
