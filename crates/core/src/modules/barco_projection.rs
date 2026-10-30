//! Barco's binary Barco Projection Protocol for its older projectors: the
//! HDX series, the PGWU-61B family, and the LCD/DLP projectors of the RS-232
//! catalog (BarcoData, BarcoGraphics, BarcoReality, BarcoVision, Galaxy,
//! Helios, ELM, SLM, DP100, XLM), over TCP 43680 or a serial port.
//!
//! Wire format from Barco's "HDX series/Apollo Command Catalog" R5905447/03
//! §2, the "PGWU-61B Command Catalog" R5905746/03 §2 and "RS232 Commands for
//! LCD and DLP projectors" R5975236/09 §1; section numbers below are the HDX
//! catalog's.
//!
//! - A message is 0xFE, the device address, the command bytes, a checksum
//!   (the address plus the command bytes, modulo 256) and 0xFF. Inside, 0x80,
//!   0xFE and 0xFF are sent as 0x80 then 0x00, 0x7E or 0x7F (§2.1). Over
//!   Ethernet the address is 0; on a serial port it is the projector's, and
//!   the `address` setting gives it. The address is escaped like the rest.
//! - Every request is answered first by an acknowledgement, 0x00 0x06, or a
//!   refusal, 0x00 0x15; a read is then answered by a response that repeats
//!   the request's command bytes (and its data, except where the table says
//!   otherwise) before the values (§2.1). One request is outstanding at a
//!   time (§2.2); a reply is waited for 5 s, half the 10 s the catalog
//!   suggests at most, since a projector answers at once.
//! - Strings are C (NUL-ended) or Pascal (length first), words big-endian,
//!   negative numbers two's complement. Each command's layout comes from the
//!   table (`barco_projection_table.rs`), one entry per command and projector
//!   family, since the three catalogs give the same command bytes different
//!   ranges and meanings. The spec's model chooses the family.
//! - The projector never sends unasked (§2.2), so state is polled: status,
//!   source, shutter and freeze every `poll_ms`, every other readable setting
//!   every `refresh_ms` and on connecting, identity and network once on
//!   connecting. A read refused, or unanswered twice, is not polled again
//!   until reconnecting or `refresh`, since no family answers every read.
//! - A setting changed by a command is read back after its acknowledgement.
//! - The link is TCP (the default) or a serial port of the host. The device
//!   is connected once it answers; three requests in a row unanswered report
//!   it disconnected. A link that fails is opened again after 1 s, doubling
//!   to 30 s.
//!
//! Opened for commands only (`monitor` false), nothing is read beyond what
//! commands ask, and no setting is read back; the status (HDX, older models)
//! or lamp status (PGWU) is read after 10 s without traffic as the liveness
//! check, the protocol having no other sign of the projector. What replies
//! carry still reaches state. Every answered request reports its round trip.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    json,
    value::{Map, Value},
};

use super::barco_projection_table::DEFS;
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

/// TCP 0xAAA0 (§2.2).
pub(crate) const DEFAULT_PORT: u16 = 43680;

const SOCKET: Key = "bpp";
const REPLY_TIMER: Key = "reply";
const FAST_TIMER: Key = "poll-fast";
const SLOW_TIMER: Key = "poll-slow";
const PROBE_TIMER: Key = "probe";
const RETRY_TIMER: Key = "retry";

const START: u8 = 0xFE;
const STOP: u8 = 0xFF;
const ESCAPE: u8 = 0x80;
const ACK: [u8; 2] = [0x00, 0x06];
const NACK: [u8; 2] = [0x00, 0x15];

const REPLY_TIMEOUT: Millis = 5_000;
const MISSES: u32 = 3;
/// Unanswered reads before a read is no longer polled.
const READ_MISSES: u32 = 2;
const PROBE_EVERY: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

// ── The command table's types ───────────────────────────────────────────

/// The projector families, each with its own catalog.
pub(crate) const HDX: u8 = 1;
pub(crate) const PGWU: u8 = 2;
pub(crate) const LEGACY: u8 = 4;
pub(crate) const DP100: u8 = 8;
pub(crate) const XLM: u8 = 16;

/// When a read is polled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Poll {
    No,
    /// Every `poll_ms`.
    Fast,
    /// Every `refresh_ms` and on connecting.
    Slow,
    /// On connecting.
    Once,
}

/// How a field of a message is laid out.
#[derive(Debug)]
pub(crate) enum Kind {
    U8,
    I8,
    U16,
    I16,
    U32,
    /// IEEE-754 single precision, big-endian.
    F32,
    /// A byte, 0 or 1.
    Bool,
    /// A word, 0 or 1.
    BoolWord,
    /// NUL-ended text.
    CStr,
    /// Length-first text.
    PStr,
    /// Text of a fixed length, trailing NULs and spaces trimmed.
    Text(usize),
    /// Bytes of a fixed length, kept as hex.
    HexBytes(usize),
    Ip4,
    Mac,
    /// The rest as an unsigned big-endian number.
    UVar,
    /// The rest, given and kept as hex.
    Hex,
    /// The rest as NUL-ended texts.
    CStrList,
    /// An unsigned word holding the value times this.
    Scaled16(u32),
    /// An unsigned double word holding the value times this.
    ScaledU32(u32),
    /// An integer byte, then the fraction in 256ths.
    Fixed8,
    /// A byte with a name for each value.
    Map(&'static [(&'static str, u8)]),
    /// A word with a name for each value.
    Word(&'static [(&'static str, u8)]),
    /// The bytes sent for each value of a parameter (requests only).
    Pick(&'static [(&'static str, &'static [u8])]),
    /// A byte whose named bits become booleans.
    Bits8(&'static [(u8, &'static str)]),
    /// A byte of which one bit names one of two values.
    BitMap(u8, &'static str, &'static str),
    /// One byte with a name, or a NUL-ended text, by the reply's length.
    ByteOrCStr(&'static [(&'static str, u8)]),
    /// Constant bytes.
    Lit(&'static [u8]),
    /// The installation byte: orientation above, automatic detection below.
    Orientation,
    /// Two bytes of 2-bit red, green and blue shifts.
    Convergence,
    /// The soft edge status bits from four parameters.
    SoftEdgeBits,
    /// The infrared receiver bits from three parameters.
    IrBits,
    /// A pattern name, or an empty text for `none`.
    NameOrEmpty,
    /// The older models' internal pattern.
    LegacyPattern,
    /// Lamp history: serial number texts, each with a little-endian word of hours.
    LampHistory,
    /// Diagnosis 3, data indexes 0 to 87.
    Diagnosis,
    /// Two bytes: major and minor version.
    VersionWord,
    /// A length byte, then that many bytes given as hex.
    LenHex,
    /// Programmable blanking shapes from a JSON list.
    Shapes,
    /// The 2-line LCD's backlight, text and cursor blocks.
    LcdDump,
}

#[derive(Debug)]
pub(crate) struct Field {
    pub name: &'static str,
    pub kind: Kind,
    /// Left out of a request when its parameter is; present in a reply only
    /// when bytes remain.
    pub opt: bool,
    /// The value used when the parameter is left out.
    pub default: Option<&'static str>,
}

/// One command of one or more families.
#[derive(Debug)]
pub(crate) struct Def {
    /// The spec's command.
    pub name: &'static str,
    pub fams: u8,
    /// Applies only when this parameter has this value (`*`: is given).
    pub when: Option<(&'static str, &'static str)>,
    pub head: &'static [u8],
    pub args: &'static [Field],
    /// The response after the acknowledgement; none for a write.
    pub reply: Option<&'static [Field]>,
    /// How many of the request's bytes the response repeats; all if none.
    pub echo: Option<usize>,
    /// Where the response goes in state; `{param}` and `{param|default}`
    /// name a parameter's value.
    pub state: Option<&'static str>,
    pub poll: Poll,
    /// The read that gets back what this command changed.
    pub then: Option<&'static str>,
}

/// The family a model belongs to.
pub(crate) fn family(model: &str) -> Option<u8> {
    match model {
        m if m.starts_with("hdx-") => Some(HDX),
        "pgwu-61b" | "pgwx-61b" | "pgxg-61b" => Some(PGWU),
        "dp100" => Some(DP100),
        "xlm" => Some(XLM),
        "barcodata" | "barcographics" | "barcoreality" | "barcovision" | "galaxy" | "helios"
        | "elm" | "slm" => Some(LEGACY),
        _ => None,
    }
}

// ── Framing ─────────────────────────────────────────────────────────────

fn escape(out: &mut Vec<u8>, b: u8) {
    match b {
        ESCAPE => out.extend_from_slice(&[ESCAPE, 0x00]),
        START => out.extend_from_slice(&[ESCAPE, 0x7E]),
        STOP => out.extend_from_slice(&[ESCAPE, 0x7F]),
        b => out.push(b),
    }
}

/// Frame a message for an address.
pub(crate) fn frame(address: u8, body: &[u8]) -> Vec<u8> {
    let sum = body.iter().fold(address, |a, &b| a.wrapping_add(b));
    let mut out = vec![START];
    escape(&mut out, address);
    for &b in body {
        escape(&mut out, b);
    }
    escape(&mut out, sum);
    out.push(STOP);
    out
}

/// Splits the byte stream into (address, command bytes) messages.
#[derive(Debug, Default)]
struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    fn feed(&mut self, bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            let Some(start) = self.buf.iter().position(|&b| b == START) else {
                self.buf.clear();
                break;
            };
            self.buf.drain(..start);
            let Some(end) = self.buf.iter().position(|&b| b == STOP) else {
                break;
            };
            // A start inside means the message before it was cut short.
            if let Some(again) = self.buf[1..end].iter().rposition(|&b| b == START) {
                self.buf.drain(..=again);
                continue;
            }
            let raw: Vec<u8> = self.buf[1..end].to_vec();
            self.buf.drain(..=end);
            let mut plain = Vec::with_capacity(raw.len());
            let mut it = raw.into_iter();
            let mut bad = false;
            while let Some(b) = it.next() {
                if b == ESCAPE {
                    match it.next() {
                        Some(0x00) => plain.push(ESCAPE),
                        Some(0x7E) => plain.push(START),
                        Some(0x7F) => plain.push(STOP),
                        _ => bad = true,
                    }
                } else {
                    plain.push(b);
                }
            }
            if bad || plain.len() < 2 {
                continue;
            }
            let check = plain.pop().expect("checksum");
            let sum = plain.iter().fold(0u8, |a, &b| a.wrapping_add(b));
            if sum != check {
                continue;
            }
            let address = plain.remove(0);
            out.push((address, plain));
        }
        out
    }
}

// ── Encoding and decoding ───────────────────────────────────────────────

fn bad(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

/// A parameter's value as the table names it.
fn label(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn int(v: &Value, name: &str) -> Result<i64, CommandError> {
    v.as_i64()
        .or_else(|| v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))
        .ok_or_else(|| bad(format!("{name} is not a whole number")))
}

fn ranged(v: &Value, name: &str, min: i64, max: i64) -> Result<i64, CommandError> {
    let n = int(v, name)?;
    if (min..=max).contains(&n) {
        Ok(n)
    } else {
        Err(bad(format!(
            "{name} {n} is out of range for this model ({min} to {max})"
        )))
    }
}

fn hex_bytes(text: &str, name: &str) -> Result<Vec<u8>, CommandError> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if !text.len().is_multiple_of(2) {
        return Err(bad(format!("{name} is not whole bytes of hex")));
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| bad(format!("{name} is not hex")))
        })
        .collect()
}

fn hex_text(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn text_bytes(s: &str) -> Vec<u8> {
    // ANSI (Latin-1); other characters cannot be sent.
    s.chars()
        .map(|c| if (c as u32) < 256 { c as u8 } else { b'?' })
        .collect()
}

fn bytes_text(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

fn param<'a>(params: &'a Params, name: &str) -> Option<&'a Value> {
    params.get(name).filter(|v| !v.is_null())
}

fn bool_param(params: &Params, name: &str) -> bool {
    param(params, name)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn encode_value(out: &mut Vec<u8>, kind: &Kind, v: &Value, name: &str) -> Result<(), CommandError> {
    match kind {
        Kind::U8 => out.push(ranged(v, name, 0, 255)? as u8),
        Kind::I8 => out.push(ranged(v, name, -128, 127)? as i8 as u8),
        Kind::U16 => out.extend_from_slice(&(ranged(v, name, 0, 0xFFFF)? as u16).to_be_bytes()),
        Kind::I16 => {
            out.extend_from_slice(&(ranged(v, name, -0x8000, 0x7FFF)? as i16).to_be_bytes())
        }
        Kind::U32 => {
            out.extend_from_slice(&(ranged(v, name, 0, 0xFFFF_FFFF)? as u32).to_be_bytes())
        }
        Kind::F32 => {
            let f = v
                .as_f64()
                .ok_or_else(|| bad(format!("{name} is not a number")))?;
            out.extend_from_slice(&(f as f32).to_be_bytes());
        }
        Kind::Bool => out.push(u8::from(
            v.as_bool()
                .ok_or_else(|| bad(format!("{name} is not on or off")))?,
        )),
        Kind::BoolWord => {
            let on = v
                .as_bool()
                .ok_or_else(|| bad(format!("{name} is not on or off")))?;
            out.extend_from_slice(&[0, u8::from(on)]);
        }
        Kind::CStr => {
            let s = v
                .as_str()
                .ok_or_else(|| bad(format!("{name} is not text")))?;
            out.extend(text_bytes(s));
            out.push(0);
        }
        Kind::PStr => {
            let s = text_bytes(
                v.as_str()
                    .ok_or_else(|| bad(format!("{name} is not text")))?,
            );
            let len =
                u8::try_from(s.len()).map_err(|_| bad(format!("{name} is longer than 255")))?;
            out.push(len);
            out.extend(s);
        }
        Kind::Ip4 => {
            let s = v
                .as_str()
                .ok_or_else(|| bad(format!("{name} is not an address")))?;
            let a: std::net::Ipv4Addr = s
                .parse()
                .map_err(|_| bad(format!("{name} '{s}' is not an IPv4 address")))?;
            out.extend_from_slice(&a.octets());
        }
        Kind::Mac => {
            let s = v
                .as_str()
                .ok_or_else(|| bad(format!("{name} is not a MAC address")))?;
            let b = hex_bytes(&s.replace([':', '-'], ""), name)?;
            if b.len() != 6 {
                return Err(bad(format!("{name} '{s}' is not a MAC address")));
            }
            out.extend(b);
        }
        Kind::Hex => out.extend(hex_bytes(v.as_str().unwrap_or_default(), name)?),
        Kind::LenHex => {
            let b = hex_bytes(v.as_str().unwrap_or_default(), name)?;
            out.push(
                u8::try_from(b.len())
                    .map_err(|_| bad(format!("{name} is longer than 255 bytes")))?,
            );
            out.extend(b);
        }
        Kind::Scaled16(scale) => {
            let f = v
                .as_f64()
                .ok_or_else(|| bad(format!("{name} is not a number")))?;
            let n = (f * f64::from(*scale)).round();
            if !(0.0..=65535.0).contains(&n) {
                return Err(bad(format!("{name} is out of range")));
            }
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        Kind::ScaledU32(scale) => {
            let f = v
                .as_f64()
                .ok_or_else(|| bad(format!("{name} is not a number")))?;
            let n = (f * f64::from(*scale)).round();
            if !(0.0..=4_294_967_295.0).contains(&n) {
                return Err(bad(format!("{name} is out of range")));
            }
            out.extend_from_slice(&(n as u32).to_be_bytes());
        }
        Kind::Map(pairs) | Kind::Word(pairs) => {
            let l = label(v).unwrap_or_default();
            let b = pairs
                .iter()
                .find(|(n, _)| *n == l)
                .map(|(_, b)| *b)
                .ok_or_else(|| bad(format!("{name} '{l}' is not available on this model")))?;
            if matches!(kind, Kind::Word(_)) {
                out.push(0);
            }
            out.push(b);
        }
        Kind::Pick(pairs) => {
            let l = label(v).unwrap_or_default();
            let b = pairs
                .iter()
                .find(|(n, _)| *n == l)
                .map(|(_, b)| *b)
                .ok_or_else(|| bad(format!("{name} '{l}' is not available on this model")))?;
            out.extend_from_slice(b);
        }
        Kind::NameOrEmpty => {
            let s = v.as_str().unwrap_or_default();
            if s != "none" {
                out.extend(text_bytes(s));
            }
            out.push(0);
        }
        other => return Err(bad(format!("{name}: {other:?} cannot be sent"))),
    }
    Ok(())
}

/// The fields made from several parameters.
fn encode_composite(out: &mut Vec<u8>, kind: &Kind, params: &Params) -> Result<(), CommandError> {
    let get = |name: &str| -> Result<i64, CommandError> {
        let v = param(params, name).ok_or_else(|| bad(format!("{name} is required")))?;
        int(v, name)
    };
    match kind {
        Kind::Convergence => {
            for axis in ["horizontal", "vertical"] {
                let mut b = 0u8;
                for (shift, color) in [(0, "red"), (2, "green"), (4, "blue")] {
                    let n = get(&format!("{color}_{axis}"))?;
                    if !(0..=3).contains(&n) {
                        return Err(bad("convergence shifts are 0 to 3"));
                    }
                    b |= (n as u8) << shift;
                }
                out.push(b);
            }
        }
        Kind::SoftEdgeBits => {
            let mut b = 0u8;
            for (bit, name) in [
                (0, "enabled"),
                (1, "white_level_lines"),
                (2, "black_level_lines"),
                (3, "data_doubling"),
            ] {
                if bool_param(params, name) {
                    b |= 1 << bit;
                }
            }
            out.push(b);
        }
        Kind::IrBits => {
            let mut b = 0u8;
            for (bit, name) in [(0, "front"), (1, "rear"), (3, "side")] {
                if bool_param(params, name) {
                    b |= 1 << bit;
                }
            }
            out.push(b);
        }
        Kind::LegacyPattern => encode_legacy_pattern(out, params)?,
        Kind::Shapes => encode_shapes(out, params)?,
        Kind::Lit(b) => out.extend_from_slice(b),
        other => return Err(bad(format!("{other:?} cannot be sent"))),
    }
    Ok(())
}

/// The older models' internal pattern (R5975236 "internal pattern, write").
fn encode_legacy_pattern(out: &mut Vec<u8>, params: &Params) -> Result<(), CommandError> {
    let pattern = param(params, "pattern")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let color = || -> Result<u8, CommandError> {
        match param(params, "color").and_then(Value::as_str) {
            Some("red") => Ok(1),
            Some("green") => Ok(2),
            Some("blue") => Ok(3),
            _ => Err(bad("leveling patterns need a color")),
        }
    };
    let position = || -> Result<u8, CommandError> {
        let v =
            param(params, "position").ok_or_else(|| bad("leveling patterns need a position"))?;
        Ok(ranged(v, "position", 1, 6)? as u8)
    };
    let invertible = !matches!(pattern, "purity" | "file");
    match pattern {
        "convergence_green" => out.push(0x01),
        "convergence_red_green" => out.push(0x02),
        "convergence_blue_green" => out.push(0x03),
        "convergence_red_green_blue" => out.extend_from_slice(&[0x21, 0x20]),
        "hatch" => out.push(0x04),
        "checkerboard" => out.push(0x19),
        "color_bars" => out.push(0x1A),
        "multiburst" => out.push(0x1B),
        "outline" => out.push(0x1C),
        "alphanumeric" => out.push(0x23),
        "page_character" => {
            let c = param(params, "character")
                .and_then(Value::as_str)
                .and_then(|s| s.chars().next())
                .filter(char::is_ascii_alphabetic)
                .ok_or_else(|| bad("the page character pattern needs a letter"))?;
            out.extend_from_slice(&[0x22, c as u8]);
        }
        "purity" => out.push(0x20),
        "leveling_coarse" => out.extend_from_slice(&[0x24, color()?, position()?]),
        "leveling_fine" => {
            let level = param(params, "level")
                .ok_or_else(|| bad("the fine leveling pattern needs a level"))?;
            out.extend_from_slice(&[
                0x25,
                color()?,
                position()?,
                ranged(level, "level", 0, 255)? as u8,
            ]);
        }
        "file" => {
            let f = param(params, "file")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("the file pattern needs a file"))?;
            out.push(0xC0);
            out.extend(text_bytes(f));
            out.push(0);
        }
        other => {
            return Err(bad(format!(
                "pattern '{other}' is not available on this model"
            )))
        }
    }
    if let Some(inverted) = param(params, "inverted").and_then(Value::as_bool) {
        if !invertible {
            return Err(bad("this pattern cannot be inverted"));
        }
        out.push(u8::from(inverted));
    }
    Ok(())
}

/// Programmable blanking shapes (R5975236 "programmable blanking, write").
fn encode_shapes(out: &mut Vec<u8>, params: &Params) -> Result<(), CommandError> {
    let shapes = param(params, "shapes")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("shapes is a list of shapes"))?;
    for s in shapes {
        let kind = s.get("shape").and_then(Value::as_str).unwrap_or_default();
        let hole = s.get("hole").and_then(Value::as_bool).unwrap_or(false);
        let (tag, keys): (u8, &[&str]) = match kind {
            "circle" => (b'C', &["x", "y", "r"]),
            "rectangle" => (b'R', &["x1", "y1", "x2", "y2"]),
            "line" => (b'L', &["y", "x1", "x2"]),
            "triangle" => (b'T', &["x1", "y1", "x2", "y2", "x3", "y3"]),
            other => {
                return Err(bad(format!(
                    "shape '{other}' is not circle, rectangle, line or triangle"
                )))
            }
        };
        out.push(if hole { tag.to_ascii_lowercase() } else { tag });
        for k in keys {
            let v = s
                .get(*k)
                .ok_or_else(|| bad(format!("a {kind} needs {k}")))?;
            out.extend_from_slice(&(ranged(v, k, 0, 0xFFFF)? as u16).to_be_bytes());
        }
    }
    Ok(())
}

/// The request for a command, or why it cannot be sent.
pub(crate) fn encode(def: &Def, params: &Params) -> Result<Vec<u8>, CommandError> {
    let mut out = def.head.to_vec();
    for f in def.args {
        if f.name == "_" {
            encode_composite(&mut out, &f.kind, params)?;
            continue;
        }
        let given = param(params, f.name).cloned();
        let v = match (given, f.default) {
            (Some(v), _) => v,
            (None, Some(d)) => Value::String(d.into()),
            (None, None) if f.opt => continue,
            (None, None) => return Err(bad(format!("{} is required on this model", f.name))),
        };
        encode_value(&mut out, &f.kind, &v, f.name)?;
    }
    Ok(out)
}

fn take<'a>(data: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    if data.len() < n {
        return None;
    }
    let (a, b) = data.split_at(n);
    *data = b;
    Some(a)
}

fn cstr(data: &mut &[u8]) -> String {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    let s = bytes_text(&data[..end]);
    *data = &data[(end + 1).min(data.len())..];
    s
}

fn float(f: f32) -> Value {
    format!("{f}")
        .parse::<f64>()
        .map(Value::from)
        .unwrap_or(Value::Null)
}

fn named(pairs: &[(&'static str, u8)], b: u8) -> Value {
    pairs
        .iter()
        .find(|(_, v)| *v == b)
        .map(|(n, _)| json!(n))
        .unwrap_or_else(|| json!(b))
}

fn word(data: &mut &[u8]) -> Option<u16> {
    take(data, 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}

fn decode_value(kind: &Kind, data: &mut &[u8]) -> Option<Value> {
    Some(match kind {
        Kind::U8 => json!(take(data, 1)?[0]),
        Kind::I8 => json!(take(data, 1)?[0] as i8),
        Kind::U16 => json!(word(data)?),
        Kind::I16 => json!(word(data)? as i16),
        Kind::U32 => {
            let b = take(data, 4)?;
            json!(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        }
        Kind::F32 => {
            let b = take(data, 4)?;
            float(f32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        }
        Kind::Bool => json!(take(data, 1)?[0] != 0),
        Kind::BoolWord => json!(word(data)? != 0),
        Kind::CStr => json!(cstr(data)),
        Kind::PStr => {
            let n = take(data, 1)?[0] as usize;
            json!(bytes_text(take(data, n.min(data.len()))?))
        }
        Kind::Text(n) => {
            let b = take(data, *n)?;
            json!(bytes_text(b).trim_end_matches(['\0', ' ']))
        }
        Kind::HexBytes(n) => json!(hex_text(take(data, *n)?)),
        Kind::Ip4 => {
            let b = take(data, 4)?;
            json!(format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]))
        }
        Kind::Mac => {
            let b = take(data, 6)?;
            json!(b
                .iter()
                .map(|x| format!("{x:02x}"))
                .collect::<Vec<_>>()
                .join(":"))
        }
        Kind::UVar => {
            let b = take(data, data.len())?;
            if b.is_empty() || b.len() > 8 {
                return None;
            }
            json!(b.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
        }
        Kind::Hex | Kind::LenHex => json!(hex_text(take(data, data.len())?)),
        Kind::CStrList => {
            let mut list = Vec::new();
            while !data.is_empty() {
                list.push(cstr(data));
            }
            json!(list)
        }
        Kind::Scaled16(s) => float((f64::from(word(data)?) / f64::from(*s)) as f32),
        Kind::ScaledU32(s) => {
            let b = take(data, 4)?;
            let n = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
            json!((f64::from(n) / f64::from(*s) * 10_000.0).round() / 10_000.0)
        }
        Kind::Fixed8 => {
            let b = take(data, 2)?;
            json!(f64::from(b[0]) + (f64::from(b[1]) / 256.0 * 1000.0).round() / 1000.0)
        }
        Kind::Map(pairs) => named(pairs, take(data, 1)?[0]),
        Kind::Word(pairs) => {
            let w = word(data)?;
            match u8::try_from(w) {
                Ok(b) => named(pairs, b),
                Err(_) => json!(w),
            }
        }
        Kind::Bits8(bits) => {
            let b = take(data, 1)?[0];
            let mut m = Map::new();
            for (bit, name) in bits.iter() {
                m.insert((*name).into(), json!(b & (1 << bit) != 0));
            }
            Value::Object(m)
        }
        Kind::BitMap(bit, zero, one) => {
            let b = take(data, 1)?[0];
            json!(if b & (1 << bit) != 0 { one } else { zero })
        }
        Kind::ByteOrCStr(pairs) => {
            if data.len() == 1 {
                named(pairs, take(data, 1)?[0])
            } else {
                json!(cstr(data))
            }
        }
        Kind::Lit(b) => {
            take(data, b.len())?;
            Value::Null
        }
        Kind::Orientation => {
            let b = take(data, 1)?[0];
            let orientation = match b & 0xF0 {
                0x40 => "front_table",
                0x80 => "front_ceiling",
                0x00 => "rear_table",
                0xC0 => "rear_ceiling",
                _ => "unknown",
            };
            let auto = match b & 0x0F {
                0 => "none",
                1 => "auto_front",
                2 => "auto_rear",
                _ => "unknown",
            };
            json!({"orientation": orientation, "auto": auto})
        }
        Kind::Convergence => {
            let b = take(data, 2)?;
            let mut m = Map::new();
            for (i, axis) in ["horizontal", "vertical"].iter().enumerate() {
                for (shift, color) in [(0, "red"), (2, "green"), (4, "blue")] {
                    m.insert(format!("{color}_{axis}"), json!((b[i] >> shift) & 3));
                }
            }
            Value::Object(m)
        }
        Kind::LampHistory => {
            let mut lamps = Vec::new();
            while !data.is_empty() {
                let serial = cstr(data);
                let h = take(data, 2)?;
                lamps.push(
                    json!({"serial_number": serial, "runtime_h": u16::from_le_bytes([h[0], h[1]])}),
                );
            }
            json!(lamps)
        }
        Kind::Diagnosis => decode_diagnosis(data)?,
        Kind::VersionWord => {
            let b = take(data, 2)?;
            json!(format!("{}.{}", b[0], b[1]))
        }
        Kind::LcdDump => decode_lcd(data)?,
        Kind::Pick(_)
        | Kind::SoftEdgeBits
        | Kind::IrBits
        | Kind::NameOrEmpty
        | Kind::LegacyPattern
        | Kind::Shapes => return None,
    })
}

/// Diagnosis 3, indexes 0 to 87 (R5975236 "diagnosis 3, read").
fn decode_diagnosis(data: &mut &[u8]) -> Option<Value> {
    let b = take(data, 88)?;
    let mut m = Map::new();
    let temps = [
        "temp_tec_red_c",
        "temp_tec_green_c",
        "temp_tec_blue_c",
        "temp_ambient_c",
        "temp_lamp_house_c",
        "temp_elcabox_c",
        "temp_smps1_secondary_c",
        "temp_smps2_secondary_c",
        "temp_rack_c",
    ];
    for (i, n) in temps.iter().enumerate() {
        m.insert((*n).into(), json!(b[i] as i8));
    }
    m.insert("tec_status".into(), json!(b[9]));
    let w = |i: usize| u16::from_be_bytes([b[i], b[i + 1]]);
    let volts = [
        "plus_plus_5v",
        "plus_plus_12v",
        "plus_3v3",
        "plus_5v",
        "plus_12v",
        "plus_24v_elek",
        "plus_12v_d320",
        "plus_tec",
        "plus_fan0",
        "plus_fan1",
        "plus_fan2",
        "plus_fan3",
    ];
    for (i, n) in volts.iter().enumerate() {
        m.insert(format!("voltage_{n}_mv"), json!(w(10 + 2 * i)));
    }
    let fans = [
        "elcabox_1",
        "elcabox_2",
        "elcabox_3",
        "elcabox_4",
        "cold_mirror_top",
        "cold_mirror_bottom",
        "light_tube",
        "start_pulse_generator",
        "lamp_top",
        "lamp_anode",
        "lamp_cathode",
        "sealing",
    ];
    for (i, n) in fans.iter().enumerate() {
        m.insert(format!("fan_{n}_rpm"), json!(w(34 + 2 * i)));
    }
    for (i, n) in ["pfc", "smps1_primary", "smps2_primary"].iter().enumerate() {
        m.insert(format!("temp_heatsink_{n}_c"), json!(b[58 + i] as i8));
    }
    for (i, n) in [
        "air_flow",
        "lamp_house",
        "tilt",
        "engine_mount",
        "water_flow",
        "water_level",
    ]
    .iter()
    .enumerate()
    {
        m.insert(format!("switch_{n}"), json!(b[61 + i] != 0));
    }
    let d = |i: usize| u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    m.insert("errors".into(), json!(d(68)));
    for n in 0..4 {
        m.insert(format!("errors_{n}"), json!(d(72 + 4 * n)));
    }
    Some(Value::Object(m))
}

/// The 2-line LCD read (R5975236 "2 line LCD, read"): tagged blocks.
fn decode_lcd(data: &mut &[u8]) -> Option<Value> {
    let mut m = Map::new();
    let mut lines = Vec::new();
    while let Some(&tag) = data.first() {
        *data = &data[1..];
        match tag {
            0x04 => {
                m.insert("backlight_on".into(), json!(take(data, 1)?[0] != 0));
            }
            0x02 => {
                take(data, 2)?;
                lines.push(json!(cstr(data)));
            }
            0x03 => {
                let c = take(data, 4)?;
                m.insert(
                    "cursor".into(),
                    json!({"x": c[0], "y": c[1], "enabled": c[2] != 0, "blink": c[3] != 0}),
                );
            }
            _ => return None,
        }
    }
    m.insert("lines".into(), json!(lines));
    Some(Value::Object(m))
}

/// The value a response carries: its one field, or an object of its fields.
pub(crate) fn decode(fields: &[Field], mut data: &[u8]) -> Option<Value> {
    let mut m = Map::new();
    let mut last = None;
    for f in fields {
        if data.is_empty() && f.opt {
            break;
        }
        let v = decode_value(&f.kind, &mut data)?;
        if f.name.starts_with('_') {
            continue;
        }
        last = Some(v.clone());
        m.insert(f.name.into(), v);
    }
    let visible = fields.iter().filter(|f| !f.name.starts_with('_')).count();
    if visible == 1 {
        last
    } else {
        Some(Value::Object(m))
    }
}

/// A state path with its parameters filled in.
fn state_path(template: &str, params: &Params) -> Option<String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = rest[open..].find('}')? + open;
        let inner = &rest[open + 1..close];
        let (name, default) = match inner.split_once('|') {
            Some((n, d)) => (n, Some(d)),
            None => (inner, None),
        };
        let v = param(params, name)
            .and_then(label)
            .or(default.map(String::from))?;
        out.push_str(&v);
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    Some(out)
}

fn at_path(path: &str, value: Value) -> Value {
    path.rsplit('.').fold(value, |inner, key| {
        let mut m = Map::new();
        m.insert(key.to_string(), inner);
        Value::Object(m)
    })
}

// ── The module ──────────────────────────────────────────────────────────

/// A read kept current.
#[derive(Debug)]
struct Read {
    def: &'static Def,
    params: Params,
}

#[derive(Debug)]
struct Job {
    id: Option<CommandId>,
    def: Option<&'static Def>,
    params: Params,
    body: Vec<u8>,
    /// The polled read this is, if any.
    read: Option<usize>,
    /// For a raw message: whether a response follows the acknowledgement.
    raw_reply: bool,
}

impl Job {
    fn expects_reply(&self) -> bool {
        self.def.map_or(self.raw_reply, |d| d.reply.is_some())
    }

    fn echo(&self) -> &[u8] {
        match self.def.and_then(|d| d.echo) {
            Some(n) => &self.body[..n.min(self.body.len())],
            None if self.def.is_some() => &self.body,
            None => &[],
        }
    }
}

#[derive(Debug)]
struct InFlight {
    job: Job,
    sent_at: Millis,
}

pub(crate) struct BarcoProjection {
    link: Link,
    tcp: bool,
    address: u8,
    fam: u8,
    monitor: bool,
    poll_ms: Millis,
    refresh_ms: Millis,
    reads: Vec<Read>,
    /// Reads not polled again until reconnecting or `refresh`.
    skipped: HashSet<usize>,
    read_misses: HashMap<usize, u32>,
    open: bool,
    closed: bool,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    deframer: Deframer,
    queue: VecDeque<Job>,
    in_flight: Option<InFlight>,
}

/// Every combination of a def's named values that its polling reads.
fn expansions(def: &'static Def) -> Vec<Params> {
    let mut out = vec![Params::new()];
    for f in def.args {
        let names: Vec<&str> = match &f.kind {
            Kind::Map(p) => p.iter().map(|(n, _)| *n).collect(),
            Kind::Pick(p) => p.iter().map(|(n, _)| *n).collect(),
            _ => continue,
        };
        // An optional value is read only when state is kept per value.
        let in_state = def
            .state
            .is_some_and(|s| s.contains(&format!("{{{}", f.name)));
        if f.opt && !in_state {
            continue;
        }
        out = out
            .into_iter()
            .flat_map(|p| {
                names.iter().map(move |n| {
                    let mut p = p.clone();
                    p.insert(f.name.into(), Value::String((*n).into()));
                    p
                })
            })
            .collect();
    }
    out.retain(|p| matches_when(def, p));
    out
}

fn matches_when(def: &Def, params: &Params) -> bool {
    match def.when {
        None => true,
        Some((name, "*")) => param(params, name).is_some(),
        Some((name, value)) => param(params, name).and_then(label).as_deref() == Some(value),
    }
}

/// The table's entry for a command on a family with these parameters.
fn find(fam: u8, name: &str, params: &Params) -> Option<&'static Def> {
    DEFS.iter()
        .find(|d| d.name == name && d.fams & fam != 0 && matches_when(d, params))
}

impl BarcoProjection {
    pub(crate) fn new(ctx: OpenContext) -> Result<BarcoProjection, String> {
        let fam = family(&ctx.model)
            .ok_or_else(|| format!("no projector family for model '{}'", ctx.model))?;
        let s = &ctx.settings;
        let link = Link::from_settings(&ctx, DEFAULT_PORT)?;
        let tcp = matches!(link, Link::Tcp(_));
        let num = |key: &str, default: u64| s.get(key).and_then(Value::as_u64).unwrap_or(default);
        let reads = DEFS
            .iter()
            .filter(|d| d.fams & fam != 0 && d.poll != Poll::No)
            .flat_map(|d| {
                expansions(d)
                    .into_iter()
                    .map(move |params| Read { def: d, params })
            })
            .collect();
        Ok(BarcoProjection {
            link,
            tcp,
            // Over Ethernet the address is always 0 (§2.1).
            address: if tcp {
                0
            } else {
                num("address", 1).min(255) as u8
            },
            fam,
            monitor: ctx.monitor,
            poll_ms: num("poll_ms", 5_000).max(500),
            refresh_ms: num("refresh_ms", 60_000).max(1_000),
            reads,
            skipped: HashSet::new(),
            read_misses: HashMap::new(),
            open: false,
            closed: false,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            deframer: Deframer::default(),
            queue: VecDeque::new(),
            in_flight: None,
        })
    }

    fn read_job(&self, index: usize) -> Option<Job> {
        let r = &self.reads[index];
        let body = encode(r.def, &r.params).ok()?;
        Some(Job {
            id: None,
            def: Some(r.def),
            params: r.params.clone(),
            body,
            read: Some(index),
            raw_reply: false,
        })
    }

    fn is_queued(&self, index: usize) -> bool {
        self.queue.iter().any(|j| j.read == Some(index))
            || self
                .in_flight
                .as_ref()
                .is_some_and(|f| f.job.read == Some(index))
    }

    /// Queue the polled reads of these groups not queued already.
    fn queue_reads(&mut self, groups: &[Poll]) {
        for group in groups {
            for i in 0..self.reads.len() {
                if self.reads[i].def.poll == *group
                    && !self.skipped.contains(&i)
                    && !self.is_queued(i)
                {
                    if let Some(job) = self.read_job(i) {
                        self.queue.push_back(job);
                    }
                }
            }
        }
    }

    /// The read that shows the projector is there.
    fn probe(&self) -> Option<Job> {
        let name = if self.fam == PGWU {
            "get_lamp_status"
        } else {
            "get_projector_status"
        };
        let def = find(self.fam, name, &Params::new())?;
        Some(Job {
            id: None,
            def: Some(def),
            params: Params::new(),
            body: encode(def, &Params::new()).ok()?,
            read: None,
            raw_reply: false,
        })
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || !self.open {
            return;
        }
        let Some(job) = self.queue.pop_front() else {
            return;
        };
        cx.tcp_send(SOCKET, frame(self.address, &job.body));
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        cx.set_timer(PROBE_TIMER, PROBE_EVERY);
        self.in_flight = Some(InFlight {
            job,
            sent_at: cx.now(),
        });
    }

    fn enqueue(&mut self, cx: &mut Cx, job: Job) {
        // Commands and their read-backs go ahead of polling.
        let at = self
            .queue
            .iter()
            .position(|j| j.id.is_none() && j.read.is_some());
        match at {
            Some(at) if job.read.is_none() => self.queue.insert(at, job),
            _ => self.queue.push_back(job),
        }
        self.pump(cx);
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.misses = 0;
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
    }

    /// Read back what a command changed.
    fn read_back(&mut self, cx: &mut Cx, def: &Def, params: &Params) {
        let Some(getter) = def.then else {
            return;
        };
        if !self.monitor {
            return;
        }
        let mut chosen = None;
        for d in DEFS
            .iter()
            .filter(|d| d.name == getter && d.fams & self.fam != 0)
        {
            let mut p = Params::new();
            for f in d.args {
                if let Some(v) = param(params, f.name) {
                    p.insert(f.name.into(), v.clone());
                }
            }
            if !matches_when(d, &p) {
                continue;
            }
            if let Ok(body) = encode(d, &p) {
                chosen = Some((d, p, body));
                break;
            }
        }
        if let Some((d, p, body)) = chosen {
            self.enqueue(
                cx,
                Job {
                    id: None,
                    def: Some(d),
                    params: p,
                    body,
                    read: None,
                    raw_reply: false,
                },
            );
        }
    }

    fn finish(&mut self, cx: &mut Cx, result: Result<Option<Vec<u8>>, CommandError>) {
        let Some(flight) = self.in_flight.take() else {
            return;
        };
        cx.cancel_timer(REPLY_TIMER);
        let job = flight.job;
        if result.is_ok() {
            cx.round_trip(cx.now().saturating_sub(flight.sent_at));
        }
        let outcome = match (result, job.def) {
            (Err(e), _) => {
                if let Some(i) = job.read {
                    self.skipped.insert(i);
                }
                Err(e)
            }
            (Ok(data), None) => Ok(Outcome::Value {
                value: json!({"reply": data.map(|d| hex_text(&d))}),
            }),
            (Ok(None), Some(def)) => {
                self.read_back(cx, def, &job.params);
                Ok(Outcome::Ack)
            }
            (Ok(Some(data)), Some(def)) => {
                let fields = def.reply.unwrap_or(&[]);
                match decode(fields, &data) {
                    Some(value) => {
                        if let Some(i) = job.read {
                            self.read_misses.remove(&i);
                        }
                        if let Some(path) = def.state.and_then(|t| state_path(t, &job.params)) {
                            let kept = match &value {
                                Value::Array(_) => json!(value.to_string()),
                                v => v.clone(),
                            };
                            cx.state(at_path(&path, kept));
                        }
                        Ok(Outcome::Value { value })
                    }
                    None => {
                        cx.log(
                            Level::Warning,
                            format!(
                                "{}: the response {} does not match the catalog",
                                def.name,
                                hex_text(&data)
                            ),
                        );
                        if let Some(i) = job.read {
                            self.skipped.insert(i);
                        }
                        Err(CommandError::DeviceError {
                            code: None,
                            message: "the response does not match the catalog's layout".into(),
                        })
                    }
                }
            }
        };
        if let Some(id) = job.id {
            cx.complete(id, outcome);
        }
    }

    fn received(&mut self, cx: &mut Cx, address: u8, body: Vec<u8>) {
        // On a serial chain, only this projector's messages.
        if !self.tcp && address != self.address {
            return;
        }
        self.heard(cx);
        let Some(flight) = self.in_flight.as_ref() else {
            return;
        };
        let wants_reply = flight.job.expects_reply();
        if body == ACK {
            if wants_reply {
                // The response follows: wait for it afresh.
                cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
            } else {
                self.finish(cx, Ok(None));
            }
        } else if body == NACK {
            self.finish(
                cx,
                Err(CommandError::DeviceError {
                    code: Some("NACK".into()),
                    message:
                        "the projector refused the command (NACK): not valid, or not available now"
                            .into(),
                }),
            );
        } else if wants_reply {
            let echo = flight.job.echo().len();
            if body.starts_with(flight.job.echo()) {
                let data = body[echo..].to_vec();
                self.finish(cx, Ok(Some(data)));
            }
        }
        self.pump(cx);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.open = false;
        self.closed = true;
        self.reachable = false;
        self.deframer = Deframer::default();
        for job in self
            .in_flight
            .take()
            .map(|f| f.job)
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
        for key in [REPLY_TIMER, FAST_TIMER, SLOW_TIMER, PROBE_TIMER] {
            cx.cancel_timer(key);
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn open_link(&mut self, cx: &mut Cx) {
        self.closed = false;
        cx.connection(Connection::Connecting);
        self.link.open(cx, SOCKET);
    }

    fn raw(&mut self, cx: &mut Cx, id: CommandId, params: &Params) {
        let body = match hex_bytes(
            param(params, "hex")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            "hex",
        ) {
            Ok(b) if !b.is_empty() => b,
            Ok(_) => return cx.complete(id, Err(bad("hex is empty"))),
            Err(e) => return cx.complete(id, Err(e)),
        };
        let job = Job {
            id: Some(id),
            def: None,
            params: Params::new(),
            body,
            read: None,
            raw_reply: bool_param(params, "reply"),
        };
        self.enqueue(cx, job);
    }
}

impl Module for BarcoProjection {
    fn start(&mut self, cx: &mut Cx) {
        self.open_link(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match name {
            "refresh" => {
                self.skipped.clear();
                self.read_misses.clear();
                self.queue_reads(&[Poll::Fast, Poll::Slow, Poll::Once]);
                self.pump(cx);
                cx.complete(id, Ok(Outcome::Ack));
                return;
            }
            "send_raw" => return self.raw(cx, id, params),
            _ => {}
        }
        let Some(def) = find(self.fam, name, params) else {
            let known = DEFS.iter().any(|d| d.name == name);
            cx.complete(
                id,
                Err(if known {
                    bad(format!(
                        "{name}: these parameters are not available on this model"
                    ))
                } else {
                    CommandError::UnknownCommand {
                        command: name.into(),
                    }
                }),
            );
            return;
        };
        match encode(def, params) {
            Ok(body) => self.enqueue(
                cx,
                Job {
                    id: Some(id),
                    def: Some(def),
                    params: params.clone(),
                    body,
                    read: None,
                    raw_reply: false,
                },
            ),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.open = true;
                self.retry_after = RETRY_MIN;
                self.deframer = Deframer::default();
                self.skipped.clear();
                self.read_misses.clear();
                if self.monitor {
                    self.queue_reads(&[Poll::Fast, Poll::Once, Poll::Slow]);
                    cx.set_timer(FAST_TIMER, self.poll_ms);
                    cx.set_timer(SLOW_TIMER, self.refresh_ms);
                } else if let Some(probe) = self.probe() {
                    // Nothing to read: one status read shows the projector
                    // is there, and the idle check repeats it.
                    self.queue.push_back(probe);
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
                self.pump(cx);
            }
            TcpInput::Data(bytes) => {
                for (address, body) in self.deframer.feed(&bytes) {
                    self.received(cx, address, body);
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => self.open_link(cx),
            REPLY_TIMER => {
                if let Some(flight) = self.in_flight.take() {
                    if let Some(i) = flight.job.read {
                        let n = self.read_misses.entry(i).or_insert(0);
                        *n += 1;
                        if *n >= READ_MISSES {
                            self.skipped.insert(i);
                        }
                    }
                    if let Some(id) = flight.job.id {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES && self.reachable {
                        self.reachable = false;
                        cx.log(
                            Level::Warning,
                            "the projector answered none of three requests",
                        );
                        cx.connection(Connection::Disconnected {
                            reason: "no reply from the projector".into(),
                        });
                    }
                }
                self.pump(cx);
            }
            FAST_TIMER if self.open && self.monitor => {
                self.queue_reads(&[Poll::Fast]);
                self.pump(cx);
                cx.set_timer(FAST_TIMER, self.poll_ms);
            }
            SLOW_TIMER if self.open && self.monitor => {
                self.queue_reads(&[Poll::Slow]);
                self.pump(cx);
                cx.set_timer(SLOW_TIMER, self.refresh_ms);
            }
            PROBE_TIMER if self.open => {
                if self.in_flight.is_none() && self.queue.is_empty() {
                    if let Some(probe) = self.probe() {
                        self.queue.push_back(probe);
                        self.pump(cx);
                    }
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult, FlowControl, Parity, SerialPort};
    use crate::session::merge_patch;
    use std::collections::BTreeSet;
    use std::net::{IpAddr, Ipv4Addr};

    fn module(model: &str, monitor: bool, settings: Value) -> BarcoProjection {
        let spec = Catalog::source_tree().devices["barco-projection-protocol"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        BarcoProjection::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7)),
            host_name: None,
            port: None,
            model: model.into(),
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

    fn opened(m: &mut BarcoProjection) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    fn feed(m: &mut BarcoProjection, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut BarcoProjection, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn ack(address: u8) -> Vec<u8> {
        frame(address, &ACK)
    }

    #[test]
    fn messages_are_framed_checked_and_escaped() {
        // R5975236 "balance, decrement" to address 1: FE 01 23 0A 2E FF; the
        // acknowledgement FE 01 00 06 07 FF.
        assert_eq!(
            frame(1, &[0x23, 0x0A]),
            [0xFE, 0x01, 0x23, 0x0A, 0x2E, 0xFF]
        );
        assert_eq!(ack(1), [0xFE, 0x01, 0x00, 0x06, 0x07, 0xFF]);
        // 0x80, 0xFE and 0xFF inside, the checksum included, are escaped.
        assert_eq!(
            frame(0, &[0x20, 0x80, 0xFE, 0xE2]),
            [0xFE, 0x00, 0x20, 0x80, 0x00, 0x80, 0x7E, 0xE2, 0x80, 0x00, 0xFF]
        );
        let mut d = Deframer::default();
        let mut bytes = vec![0x00, 0x13];
        bytes.extend(frame(0, &[0x20, 0x80, 0xFE, 0xE2]));
        let mut corrupt = frame(0, &[0x21, 0x02, 0x10]);
        corrupt[3] ^= 1;
        bytes.extend(corrupt);
        bytes.extend(&frame(0, &[0x21, 0x02, 0x10])[..3]);
        assert_eq!(d.feed(&bytes), [(0, vec![0x20, 0x80, 0xFE, 0xE2])]);
        // Split anywhere, a message is still read once whole.
        assert_eq!(
            d.feed(&frame(0, &[0x21, 0x02, 0x10])[3..]),
            [(0, vec![0x21, 0x02, 0x10])]
        );
    }

    #[test]
    fn commands_follow_each_family_s_catalog() {
        let hdx = |name: &str, p: Value| {
            encode(
                find(HDX, name, p.as_object().unwrap()).unwrap(),
                p.as_object().unwrap(),
            )
        };
        let pg = |name: &str, p: Value| {
            encode(
                find(PGWU, name, p.as_object().unwrap()).unwrap(),
                p.as_object().unwrap(),
            )
        };
        let old = |name: &str, p: Value| {
            encode(
                find(LEGACY, name, p.as_object().unwrap()).unwrap(),
                p.as_object().unwrap(),
            )
        };
        assert_eq!(
            hdx("set_brightness", json!({"value": 128})).unwrap(),
            [0x20, 0x02, 0x80]
        );
        assert_eq!(
            hdx("set_source", json!({"source": 3})).unwrap(),
            [0x31, 0x03]
        );
        assert_eq!(
            hdx("set_shutter", json!({"open": true})).unwrap(),
            [0x20, 0x42, 0x01]
        );
        assert_eq!(
            hdx("set_shutter", json!({"open": false, "speed": "fast"})).unwrap(),
            [0x23, 0x42, 0x00]
        );
        assert_eq!(
            old("set_shutter", json!({"open": true})).unwrap(),
            [0x22, 0x42, 0x00]
        );
        assert_eq!(
            hdx("set_freeze", json!({"enabled": true})).unwrap(),
            [0x27, 0x23]
        );
        assert_eq!(
            pg("set_freeze", json!({"enabled": true})).unwrap(),
            [0x20, 0xAC, 0x01]
        );
        assert_eq!(pg("power_on", json!({})).unwrap(), [0x58, 0x03]);
        assert_eq!(hdx("power_off", json!({})).unwrap(), [0x66]);
        assert_eq!(
            hdx("lens_focus", json!({"direction": "far"})).unwrap(),
            [0xF4, 0x83, 0x01]
        );
        assert_eq!(
            pg("lens_focus", json!({"direction": "in"})).unwrap(),
            [0xF4, 0x83, 0x01]
        );
        assert!(pg("lens_focus", json!({"direction": "near"})).is_err());
        assert_eq!(
            hdx(
                "step_geometry",
                json!({"setting": "blanking_top", "direction": "up"})
            )
            .unwrap(),
            [0x22, 0x4C]
        );
        assert_eq!(
            hdx(
                "set_geometry",
                json!({"setting": "blanking_left", "value": 300})
            )
            .unwrap(),
            [0x20, 0x4E, 0x01, 0x2C]
        );
        assert!(hdx(
            "set_geometry",
            json!({"setting": "size_vertical", "value": 3})
        )
        .is_err());
        assert_eq!(
            hdx(
                "set_input_balance",
                json!({"level": "white", "color": "blue", "value": -2})
            )
            .unwrap(),
            [0x20, 0x6F, 0x02, 0xFE]
        );
        assert_eq!(
            hdx("set_color_temperature", json!({"temperature": "6500"})).unwrap(),
            [0x20, 0x45, 0x65]
        );
        assert_eq!(
            old("set_color_temperature", json!({"temperature": "6500"})).unwrap(),
            [0x20, 0x45, 65]
        );
        assert_eq!(
            old("set_color_temperature", json!({"file": "warm"})).unwrap(),
            [0x20, 0x45, b'w', b'a', b'r', b'm', 0]
        );
        assert_eq!(
            hdx("set_warp", json!({"parameter": "rotation", "value": 1.5})).unwrap(),
            [0x20, 0xA1, 0x03, 0x3F, 0xC0, 0x00, 0x00]
        );
        assert_eq!(
            hdx(
                "set_warp_point",
                json!({"x": 2, "y": 3, "shift_x": 0.0, "shift_y": 0.0})
            )
            .unwrap(),
            [0x20, 0xA1, 0x50, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            hdx("set_p7_point", json!({"point": "red_x", "value": 0.64})).unwrap(),
            [0x20, 0x97, 0x00, 0x01, 0x19, 0x00]
        );
        assert_eq!(
            hdx("set_test_pattern", json!({"pattern": "none"})).unwrap(),
            [0x41, 0xC0, 0x00]
        );
        assert_eq!(
            old("set_internal_pattern", json!({"pattern": "leveling_fine", "color": "green", "position": 4, "level": 200, "inverted": true})).unwrap(),
            [0x41, 0x25, 0x02, 0x04, 200, 0x01]
        );
        assert_eq!(
            old("set_network", json!({"mode": "manual", "ip_address": "10.0.0.5", "subnet_mask": "255.0.0.0", "gateway": "10.0.0.1"})).unwrap(),
            [0x11, 0x81, 0, 10, 0, 0, 5, 255, 0, 0, 0, 10, 0, 0, 1]
        );
        assert_eq!(
            old("set_network", json!({"mode": "dhcp"})).unwrap(),
            [0x11, 0x81, 1]
        );
        assert_eq!(
            hdx("set_convergence", json!({"red_horizontal": 1, "green_horizontal": 2, "blue_horizontal": 3, "red_vertical": 0, "green_vertical": 0, "blue_vertical": 1})).unwrap(),
            [0x82, 0, 0, 0, 0x4C, 0b0011_1001, 0b0001_0000]
        );
        assert_eq!(
            old(
                "set_blanking_shapes",
                json!({"shapes": [{"shape": "circle", "hole": true, "x": 10, "y": 20, "r": 5}]})
            )
            .unwrap(),
            [0xE1, b'c', 0, 10, 0, 20, 0, 5]
        );
    }

    #[test]
    fn responses_are_decoded_into_state_by_family() {
        let def = find(HDX, "get_projector_status", &Params::new()).unwrap();
        assert_eq!(
            decode(def.reply.unwrap(), &[0x43]).unwrap(),
            json!({"projector_on": true, "text_on": true, "lamp_on": true})
        );
        let def = find(LEGACY, "get_color_temperature", &Params::new()).unwrap();
        assert_eq!(decode(def.reply.unwrap(), &[54]).unwrap(), json!("5400"));
        assert_eq!(
            decode(def.reply.unwrap(), b"warm\0").unwrap(),
            json!("warm")
        );
        let def = find(HDX, "get_contrast_enhancement", &Params::new()).unwrap();
        assert_eq!(
            decode(def.reply.unwrap(), &[1]).unwrap(),
            json!({"mode": "high"})
        );
        assert_eq!(
            decode(def.reply.unwrap(), &[2, 0, 0, 0x12, 0x34]).unwrap(),
            json!({"mode": "mid", "position": 0x1234})
        );
        let def = find(LEGACY, "get_lamp_history", &Params::new()).unwrap();
        assert_eq!(
            decode(def.reply.unwrap(), b"A1\0\x10\x01B2\0\x05\x00").unwrap(),
            json!([{"serial_number": "A1", "runtime_h": 272}, {"serial_number": "B2", "runtime_h": 5}])
        );
        let def = find(PGWU, "get_about_info", &Params::new()).unwrap();
        let mut about = vec![4];
        about.extend(b"SN12345678");
        about.extend([
            1, 2, 3, 4, 5, 6, 0, 1, 148, 128, 60, 31, 128, 1, 0, 0, 0, 100, 0, 0, 0, 0, 1, 0, 0, 0,
            0x0E, 0x10, 1, 0, 1, 2,
        ]);
        let v = decode(def.reply.unwrap(), &about).unwrap();
        assert_eq!(v["model"], "PGWU-61B");
        assert_eq!(v["serial_number"], "SN12345678");
        assert_eq!(v["pixel_clock_mhz"], 148.5);
        assert_eq!(v["horizontal_refresh_khz"], 31.5);
        assert_eq!(v["lamp_runtime_h"], 100);
        assert_eq!(v["projector_runtime_s"], 3600);
        assert_eq!(v["color_space"], "rgb");
        assert_eq!(
            state_path(
                "warp.areas.{area|all}.{parameter}",
                json!({"parameter": "scale_vertical"}).as_object().unwrap()
            )
            .unwrap(),
            "warp.areas.all.scale_vertical"
        );
    }

    #[test]
    fn the_serial_link_uses_the_address_and_the_catalog_s_line_settings() {
        let mut m = module(
            "galaxy",
            false,
            json!({"link": "serial", "serial_port": "/dev/ttyUSB0", "address": 5}),
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let ports: Vec<SerialPort> = cx
            .take()
            .into_iter()
            .filter_map(|a| match a {
                Action::SerialOpen { port, .. } => Some(port),
                _ => None,
            })
            .collect();
        assert_eq!(
            ports,
            [SerialPort {
                path: "/dev/ttyUSB0".into(),
                baud_rate: 9600,
                data_bits: 8,
                parity: Parity::None,
                stop_bits: 1,
                flow_control: FlowControl::None,
            }]
        );
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        // Commands only: the status read is the liveness check, to address 5.
        assert_eq!(sent(&cx.take()), [frame(5, &[0x67])]);
        // Another projector's answer on the chain is not this one's.
        let a = feed(&mut m, 1, &frame(6, &[0x67, 0x01]));
        assert!(state(&a).as_object().unwrap().is_empty());
        let a = feed(&mut m, 2, &[ack(5), frame(5, &[0x67, 0x01])].concat());
        assert_eq!(state(&a)["status"]["projector_on"], true);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
    }

    #[test]
    fn opened_for_commands_only_it_reads_nothing_but_the_liveness_check() {
        let mut m = module("hdx-w20-flex", false, json!({}));
        let a = opened(&mut m);
        assert_eq!(sent(&a), [frame(0, &[0x67])]);
        let a = feed(&mut m, 4, &[ack(0), frame(0, &[0x67, 0x01])].concat());
        assert!(a.contains(&Action::RoundTrip(4)));
        assert!(sent(&a).is_empty());

        let a = run(&mut m, 1, "set_brightness", json!({"value": 200}));
        assert_eq!(sent(&a), [frame(0, &[0x20, 0x02, 200])]);
        let a = feed(&mut m, 9, &ack(0));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert!(a.contains(&Action::RoundTrip(9)));
        // Commands only: the setting is not read back.
        assert!(sent(&a).is_empty());

        let a = run(&mut m, 2, "get_brightness", json!({}));
        assert_eq!(sent(&a), [frame(0, &[0x21, 0x02])]);
        let a = feed(&mut m, 10, &ack(0));
        assert_eq!(completed(&a, 2), None);
        let a = feed(&mut m, 11, &frame(0, &[0x21, 0x02, 200]));
        assert_eq!(
            completed(&a, 2),
            Some(Ok(Outcome::Value { value: json!(200) }))
        );
        assert_eq!(state(&a), json!({"picture": {"brightness": 200}}));
    }

    #[test]
    fn monitored_it_polls_reads_back_and_drops_refused_reads() {
        let mut m = module("hdx-w14", true, json!({}));
        let a = opened(&mut m);
        let first = sent(&a);
        assert_eq!(first.len(), 1);
        // The fast group first: the status.
        assert_eq!(first[0], frame(0, &[0x67]));
        let refused = m.reads.len();
        assert!(refused > 100, "{refused} reads");
        // A refused read is not polled again.
        let a = feed(&mut m, 1, &frame(0, &NACK));
        assert_eq!(m.skipped.len(), 1);
        assert_eq!(sent(&a).len(), 1);
        // A command goes ahead of the queued reads, and is read back.
        run(&mut m, 5, "set_source", json!({"source": 2}));
        feed(&mut m, 2, &frame(0, &NACK));
        let a = feed(&mut m, 3, &ack(0));
        assert_eq!(completed(&a, 5), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a), [frame(0, &[0x32])]);
        let a = feed(&mut m, 4, &[ack(0), frame(0, &[0x32, 0x02])].concat());
        assert_eq!(state(&a), json!({"source": {"number": 2}}));
    }

    #[test]
    fn unanswered_requests_report_the_projector_gone() {
        let mut m = module("pgwu-61b", false, json!({}));
        let a = opened(&mut m);
        assert_eq!(sent(&a), [frame(0, &[0x67, 0x40])]);
        feed(&mut m, 1, &[ack(0), frame(0, &[0x67, 0x40, 0x01])].concat());
        let mut gone = Vec::new();
        for id in 1..=3 {
            run(&mut m, id, "get_lamp_on", json!({"lamp": "1"}));
            let mut cx = Cx::new(0);
            m.timer(&mut cx, REPLY_TIMER);
            gone = cx.take();
            assert_eq!(completed(&gone, id), Some(Err(CommandError::Timeout)));
        }
        assert!(gone
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
    }

    #[test]
    fn raw_messages_return_the_response() {
        let mut m = module("xlm", false, json!({}));
        opened(&mut m);
        feed(&mut m, 1, &[ack(0), frame(0, &[0x67, 0x00])].concat());
        let a = run(&mut m, 1, "send_raw", json!({"hex": "871b", "reply": true}));
        assert_eq!(sent(&a), [frame(0, &[0x87, 0x1B])]);
        let a = feed(&mut m, 2, &[ack(0), frame(0, &[0x87, 0x1B, 0x04])].concat());
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value {
                value: json!({"reply": "871b04"})
            }))
        );
    }

    /// The spec and the table describe the same commands, and each model
    /// supports exactly the commands its family's catalog has.
    #[test]
    fn the_spec_matches_the_table() {
        let spec = Catalog::source_tree().devices["barco-projection-protocol"].clone();
        let module_commands = ["refresh", "send_raw"];
        for d in DEFS {
            let c = spec
                .commands
                .get(d.name)
                .unwrap_or_else(|| panic!("{} is not in the spec", d.name));
            for f in d.args.iter().filter(|f| f.name != "_") {
                let p = c
                    .params
                    .get(f.name)
                    .unwrap_or_else(|| panic!("{}: no param {}", d.name, f.name));
                if let (Kind::Map(pairs), Some(values)) = (&f.kind, &p.values) {
                    for (n, _) in pairs.iter() {
                        assert!(
                            values.iter().any(|v| v == n),
                            "{}: {} not a value of {}",
                            d.name,
                            n,
                            f.name
                        );
                    }
                }
            }
        }
        for name in spec.commands.keys() {
            assert!(
                module_commands.contains(&name.as_str()) || DEFS.iter().any(|d| d.name == name),
                "{name} has no table entry"
            );
        }
        for model in &spec.models {
            let fam = family(&model.id).unwrap_or_else(|| panic!("no family for {}", model.id));
            let mut want: BTreeSet<&str> = DEFS
                .iter()
                .filter(|d| d.fams & fam != 0)
                .map(|d| d.name)
                .collect();
            want.extend(module_commands);
            let have: BTreeSet<&str> = model.supports.iter().map(String::as_str).collect();
            assert_eq!(have, want, "{}", model.id);
        }
    }
}
