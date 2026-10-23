//! The Basic Encoding Rules subset SNMP uses (RFC 3417 §8, X.690), and
//! SNMP's value types (RFC 2578 §7.1 and RFC 3416 §3).
//!
//! Only definite lengths and the primitive types SNMP declares are handled:
//! INTEGER, OCTET STRING, NULL, OBJECT IDENTIFIER and SEQUENCE, the
//! application types IpAddress, Counter32, Gauge32 (Unsigned32), TimeTicks,
//! Opaque and Counter64, and the context-tagged exceptions noSuchObject,
//! noSuchInstance and endOfMibView. Lengths are encoded in the shortest
//! form, as RFC 3417 §8 asks; any definite form is accepted.

use std::fmt;

use serde_json::{json, Value as Json};

pub(crate) const INTEGER: u8 = 0x02;
pub(crate) const OCTET_STRING: u8 = 0x04;
pub(crate) const NULL: u8 = 0x05;
pub(crate) const OBJECT_IDENTIFIER: u8 = 0x06;
pub(crate) const SEQUENCE: u8 = 0x30;
const IP_ADDRESS: u8 = 0x40;
const COUNTER32: u8 = 0x41;
const GAUGE32: u8 = 0x42;
const TIMETICKS: u8 = 0x43;
const OPAQUE: u8 = 0x44;
const COUNTER64: u8 = 0x46;
const NO_SUCH_OBJECT: u8 = 0x80;
const NO_SUCH_INSTANCE: u8 = 0x81;
const END_OF_MIB_VIEW: u8 = 0x82;

/// An OBJECT IDENTIFIER: at least two sub-identifiers, each 0 to 2^32-1,
/// at most 128 of them (RFC 2578 §3.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Oid(pub(crate) Vec<u32>);

impl Oid {
    /// Dotted decimal, with or without a leading dot: `1.3.6.1.2.1.1.3.0`.
    pub(crate) fn parse(text: &str) -> Result<Oid, String> {
        let t = text.trim();
        let t = t.strip_prefix('.').unwrap_or(t);
        let arcs: Vec<u32> = t
            .split('.')
            .map(|a| a.parse::<u32>())
            .collect::<Result<_, _>>()
            .map_err(|_| {
                format!("'{text}' is not an OID in dotted decimal, such as 1.3.6.1.2.1.1.3.0")
            })?;
        let oid = Oid(arcs);
        oid.check().map_err(|e| format!("'{text}': {e}"))?;
        Ok(oid)
    }

    fn check(&self) -> Result<(), String> {
        let a = &self.0;
        if a.len() < 2 || a.len() > 128 {
            return Err("an OID has 2 to 128 sub-identifiers".into());
        }
        if a[0] > 2 || (a[0] < 2 && a[1] >= 40) || (a[0] == 2 && a[1] > u32::MAX - 80) {
            return Err("the first two sub-identifiers are out of range".into());
        }
        Ok(())
    }

    pub(crate) fn starts_with(&self, root: &Oid) -> bool {
        self.0.starts_with(&root.0)
    }

    /// This OID with sub-identifiers added.
    pub(crate) fn child(&self, arcs: &[u32]) -> Oid {
        let mut v = self.0.clone();
        v.extend_from_slice(arcs);
        Oid(v)
    }

    fn encode_content(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let first = u64::from(self.0[0]) * 40 + u64::from(self.0[1]);
        let mut arcs = vec![first];
        arcs.extend(self.0[2..].iter().map(|a| u64::from(*a)));
        for arc in arcs {
            let mut bytes = vec![(arc & 0x7F) as u8];
            let mut rest = arc >> 7;
            while rest > 0 {
                bytes.push(0x80 | (rest & 0x7F) as u8);
                rest >>= 7;
            }
            bytes.reverse();
            out.extend(bytes);
        }
        out
    }

    fn decode_content(bytes: &[u8]) -> Result<Oid, String> {
        let mut arcs = Vec::new();
        let mut value: u64 = 0;
        let mut started = false;
        for (i, b) in bytes.iter().enumerate() {
            if !started && *b == 0x80 {
                return Err("an OID sub-identifier has a leading 0x80".into());
            }
            started = true;
            value = (value << 7) | u64::from(b & 0x7F);
            if value > u64::from(u32::MAX) + 80 {
                return Err("an OID sub-identifier is too large".into());
            }
            if b & 0x80 == 0 {
                if arcs.is_empty() {
                    let (x, y) = match value {
                        0..=39 => (0, value),
                        40..=79 => (1, value - 40),
                        _ => (2, value - 80),
                    };
                    arcs.push(x as u32);
                    arcs.push(u32::try_from(y).map_err(|_| "an OID sub-identifier is too large")?);
                } else {
                    arcs.push(
                        u32::try_from(value).map_err(|_| "an OID sub-identifier is too large")?,
                    );
                }
                value = 0;
                started = false;
            } else if i == bytes.len() - 1 {
                return Err("an OID ends inside a sub-identifier".into());
            }
        }
        if arcs.len() > 128 {
            return Err("an OID has more than 128 sub-identifiers".into());
        }
        if arcs.is_empty() {
            return Err("an empty OID".into());
        }
        Ok(Oid(arcs))
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.0.iter().map(u32::to_string).collect();
        f.write_str(&parts.join("."))
    }
}

/// One value of a variable binding.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Integer(i64),
    OctetString(Vec<u8>),
    Null,
    Oid(Oid),
    IpAddress([u8; 4]),
    Counter32(u32),
    Gauge32(u32),
    TimeTicks(u32),
    Opaque(Vec<u8>),
    Counter64(u64),
    NoSuchObject,
    NoSuchInstance,
    EndOfMibView,
}

/// The types a SET can carry, as the `set` command names them.
pub(crate) const SET_TYPES: [&str; 10] = [
    "integer",
    "string",
    "hex",
    "oid",
    "ip_address",
    "counter32",
    "gauge32",
    "timeticks",
    "counter64",
    "opaque",
];

impl Value {
    /// The type's name in state and command results.
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Value::Integer(_) => "integer",
            Value::OctetString(_) => "octet_string",
            Value::Null => "null",
            Value::Oid(_) => "oid",
            Value::IpAddress(_) => "ip_address",
            Value::Counter32(_) => "counter32",
            Value::Gauge32(_) => "gauge32",
            Value::TimeTicks(_) => "timeticks",
            Value::Opaque(_) => "opaque",
            Value::Counter64(_) => "counter64",
            Value::NoSuchObject => "no_such_object",
            Value::NoSuchInstance => "no_such_instance",
            Value::EndOfMibView => "end_of_mib_view",
        }
    }

    /// One of the exceptions a v2c or v3 agent answers in place of a value.
    pub(crate) fn is_exception(&self) -> bool {
        matches!(
            self,
            Value::NoSuchObject | Value::NoSuchInstance | Value::EndOfMibView
        )
    }

    /// A value of a type named in [`SET_TYPES`], from text: `string` is
    /// UTF-8, `hex` and `opaque` are hex digit pairs (separators allowed),
    /// `oid` dotted decimal, `ip_address` dotted quad, the rest decimal.
    pub(crate) fn parse(kind: &str, text: &str) -> Result<Value, String> {
        let t = text.trim();
        let unsigned = |max: u64| -> Result<u64, String> {
            t.parse::<u64>()
                .ok()
                .filter(|v| *v <= max)
                .ok_or_else(|| format!("'{text}' is not a {kind} (0 to {max})"))
        };
        Ok(match kind {
            "integer" => Value::Integer(
                t.parse::<i32>()
                    .map_err(|_| {
                        format!("'{text}' is not an integer from -2147483648 to 2147483647")
                    })?
                    .into(),
            ),
            "string" => Value::OctetString(text.as_bytes().to_vec()),
            "hex" => Value::OctetString(parse_hex(t)?),
            "opaque" => Value::Opaque(parse_hex(t)?),
            "oid" => Value::Oid(Oid::parse(t)?),
            "ip_address" => Value::IpAddress(
                t.parse::<std::net::Ipv4Addr>()
                    .map_err(|_| format!("'{text}' is not an IPv4 address"))?
                    .octets(),
            ),
            "counter32" => Value::Counter32(unsigned(u64::from(u32::MAX))? as u32),
            "gauge32" => Value::Gauge32(unsigned(u64::from(u32::MAX))? as u32),
            "timeticks" => Value::TimeTicks(unsigned(u64::from(u32::MAX))? as u32),
            "counter64" => Value::Counter64(unsigned(u64::MAX)?),
            "null" => Value::Null,
            other => return Err(format!("'{other}' is not a value type")),
        })
    }

    /// The value as JSON: numbers as numbers, an OCTET STRING as text
    /// (invalid UTF-8 shown as U+FFFD; `hex` has its bytes), an OID and an
    /// IpAddress in dotted form, an exception as null.
    pub(crate) fn to_json(&self) -> Json {
        match self {
            Value::Integer(v) => json!(v),
            Value::OctetString(b) => json!(String::from_utf8_lossy(b)),
            Value::Null => Json::Null,
            Value::Oid(o) => json!(o.to_string()),
            Value::IpAddress(a) => json!(std::net::Ipv4Addr::from(*a).to_string()),
            Value::Counter32(v) | Value::Gauge32(v) | Value::TimeTicks(v) => json!(v),
            Value::Opaque(b) => json!(hex(b)),
            Value::Counter64(v) => json!(v),
            Value::NoSuchObject | Value::NoSuchInstance | Value::EndOfMibView => Json::Null,
        }
    }

    /// The bytes of an OCTET STRING or Opaque, for binary values.
    pub(crate) fn bytes(&self) -> Option<&[u8]> {
        match self {
            Value::OctetString(b) | Value::Opaque(b) => Some(b),
            _ => None,
        }
    }

    pub(crate) fn encode(&self, out: &mut Vec<u8>) {
        match self {
            Value::Integer(v) => tlv(out, INTEGER, &integer_content(*v)),
            Value::OctetString(b) => tlv(out, OCTET_STRING, b),
            Value::Null => tlv(out, NULL, &[]),
            Value::Oid(o) => tlv(out, OBJECT_IDENTIFIER, &o.encode_content()),
            Value::IpAddress(a) => tlv(out, IP_ADDRESS, a),
            Value::Counter32(v) => tlv(out, COUNTER32, &unsigned_content(u64::from(*v))),
            Value::Gauge32(v) => tlv(out, GAUGE32, &unsigned_content(u64::from(*v))),
            Value::TimeTicks(v) => tlv(out, TIMETICKS, &unsigned_content(u64::from(*v))),
            Value::Opaque(b) => tlv(out, OPAQUE, b),
            Value::Counter64(v) => tlv(out, COUNTER64, &unsigned_content(*v)),
            Value::NoSuchObject => tlv(out, NO_SUCH_OBJECT, &[]),
            Value::NoSuchInstance => tlv(out, NO_SUCH_INSTANCE, &[]),
            Value::EndOfMibView => tlv(out, END_OF_MIB_VIEW, &[]),
        }
    }

    pub(crate) fn decode(tag: u8, content: &[u8]) -> Result<Value, String> {
        let unsigned32 = |c: &[u8]| -> Result<u32, String> {
            u32::try_from(decode_unsigned(c)?).map_err(|_| "a 32-bit value is too large".into())
        };
        Ok(match tag {
            INTEGER => Value::Integer(decode_integer(content)?),
            OCTET_STRING => Value::OctetString(content.to_vec()),
            NULL => Value::Null,
            OBJECT_IDENTIFIER => Value::Oid(Oid::decode_content(content)?),
            IP_ADDRESS => Value::IpAddress(
                content
                    .try_into()
                    .map_err(|_| "an IpAddress is not 4 bytes".to_string())?,
            ),
            COUNTER32 => Value::Counter32(unsigned32(content)?),
            GAUGE32 => Value::Gauge32(unsigned32(content)?),
            TIMETICKS => Value::TimeTicks(unsigned32(content)?),
            OPAQUE => Value::Opaque(content.to_vec()),
            COUNTER64 => Value::Counter64(decode_unsigned(content)?),
            NO_SUCH_OBJECT => Value::NoSuchObject,
            NO_SUCH_INSTANCE => Value::NoSuchInstance,
            END_OF_MIB_VIEW => Value::EndOfMibView,
            other => return Err(format!("unknown value type 0x{other:02x}")),
        })
    }
}

/// Lowercase hex, no separators.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex digit pairs, optionally separated by spaces, `:`, `-`, `.` or `,`,
/// optionally starting 0x.
pub(crate) fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    let t = text.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    let digits: String = t
        .chars()
        .filter(|c| !matches!(c, ' ' | ':' | '-' | '.' | ','))
        .collect();
    if !digits.len().is_multiple_of(2) || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("'{text}' is not bytes in hex, such as 00 1a ff"));
    }
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// Append a tag, its length and its content.
pub(crate) fn tlv(out: &mut Vec<u8>, tag: u8, content: &[u8]) {
    out.push(tag);
    let len = content.len();
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes = (len as u64).to_be_bytes();
        let skip = bytes.iter().take_while(|b| **b == 0).count();
        out.push(0x80 | (8 - skip) as u8);
        out.extend_from_slice(&bytes[skip..]);
    }
    out.extend_from_slice(content);
}

/// A tag, length and content built by `f`.
pub(crate) fn wrap(out: &mut Vec<u8>, tag: u8, f: impl FnOnce(&mut Vec<u8>)) {
    let mut content = Vec::new();
    f(&mut content);
    tlv(out, tag, &content);
}

pub(crate) fn integer(out: &mut Vec<u8>, v: i64) {
    tlv(out, INTEGER, &integer_content(v));
}

pub(crate) fn octets(out: &mut Vec<u8>, b: &[u8]) {
    tlv(out, OCTET_STRING, b);
}

pub(crate) fn oid(out: &mut Vec<u8>, o: &Oid) {
    tlv(out, OBJECT_IDENTIFIER, &o.encode_content());
}

/// Two's complement in the fewest bytes.
fn integer_content(v: i64) -> Vec<u8> {
    let bytes = v.to_be_bytes();
    let mut start = 0;
    while start < 7 {
        let (b, next) = (bytes[start], bytes[start + 1]);
        if (b == 0x00 && next & 0x80 == 0) || (b == 0xFF && next & 0x80 != 0) {
            start += 1;
        } else {
            break;
        }
    }
    bytes[start..].to_vec()
}

/// An unsigned value as a non-negative INTEGER: a leading zero byte when
/// the top bit is set.
fn unsigned_content(v: u64) -> Vec<u8> {
    let bytes = v.to_be_bytes();
    let skip = bytes.iter().take_while(|b| **b == 0).count().min(7);
    let mut out = Vec::new();
    if bytes[skip] & 0x80 != 0 {
        out.push(0);
    }
    out.extend_from_slice(&bytes[skip..]);
    out
}

fn decode_integer(c: &[u8]) -> Result<i64, String> {
    if c.is_empty() || c.len() > 8 {
        return Err("an INTEGER of 0 or more than 8 bytes".into());
    }
    let mut v: i64 = if c[0] & 0x80 != 0 { -1 } else { 0 };
    for b in c {
        v = (v << 8) | i64::from(*b);
    }
    Ok(v)
}

fn decode_unsigned(c: &[u8]) -> Result<u64, String> {
    // Agents sometimes send an unsigned value without the leading zero its
    // top bit asks for; it is read as unsigned either way.
    let c = if c.len() == 9 && c[0] == 0 {
        &c[1..]
    } else {
        c
    };
    if c.is_empty() || c.len() > 8 {
        return Err("an unsigned value of 0 or more than 8 bytes".into());
    }
    Ok(c.iter().fold(0u64, |v, b| (v << 8) | u64::from(*b)))
}

/// Reads TLVs from a buffer, keeping positions in the whole message (USM's
/// authentication covers the whole message, so where its parameters lie
/// must be known).
#[derive(Clone)]
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    end: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Reader<'a> {
        Reader {
            buf,
            pos: 0,
            end: buf.len(),
        }
    }

    /// A reader over `start..end` of `buf`, positions kept in `buf`.
    pub(crate) fn within(buf: &'a [u8], start: usize, end: usize) -> Reader<'a> {
        Reader {
            buf,
            pos: start,
            end: end.min(buf.len()),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.pos >= self.end
    }

    /// The next TLV: its tag, where its content starts in the whole
    /// buffer, and its content.
    pub(crate) fn next(&mut self) -> Result<(u8, usize, &'a [u8]), String> {
        let short = || "the message ends early".to_string();
        let tag = *self
            .buf
            .get(self.pos)
            .filter(|_| self.pos < self.end)
            .ok_or_else(short)?;
        let mut at = self.pos + 1;
        let first = *self
            .buf
            .get(at)
            .filter(|_| at < self.end)
            .ok_or_else(short)?;
        at += 1;
        let len = if first < 0x80 {
            usize::from(first)
        } else {
            let n = usize::from(first & 0x7F);
            if n == 0 || n > 4 {
                return Err("an indefinite or oversized length".into());
            }
            if at + n > self.end {
                return Err(short());
            }
            let len = self.buf[at..at + n]
                .iter()
                .fold(0usize, |v, b| (v << 8) | usize::from(*b));
            at += n;
            len
        };
        if at + len > self.end {
            return Err(short());
        }
        self.pos = at + len;
        Ok((tag, at, &self.buf[at..at + len]))
    }

    /// The next TLV, which must have this tag.
    pub(crate) fn expect(&mut self, tag: u8) -> Result<(usize, &'a [u8]), String> {
        let (got, at, content) = self.next()?;
        if got != tag {
            return Err(format!("expected tag 0x{tag:02x}, found 0x{got:02x}"));
        }
        Ok((at, content))
    }

    /// A reader over the content of the next TLV, which must have this tag.
    pub(crate) fn enter(&mut self, tag: u8) -> Result<Reader<'a>, String> {
        let (at, content) = self.expect(tag)?;
        Ok(Reader {
            buf: self.buf,
            pos: at,
            end: at + content.len(),
        })
    }

    pub(crate) fn integer(&mut self) -> Result<i64, String> {
        decode_integer(self.expect(INTEGER)?.1)
    }

    pub(crate) fn octets(&mut self) -> Result<&'a [u8], String> {
        Ok(self.expect(OCTET_STRING)?.1)
    }

    pub(crate) fn oid(&mut self) -> Result<Oid, String> {
        Oid::decode_content(self.expect(OBJECT_IDENTIFIER)?.1)
    }

    pub(crate) fn value(&mut self) -> Result<Value, String> {
        let (tag, _, content) = self.next()?;
        Value::decode(tag, content)
    }

    /// The tag of the TLV the reader is at, without moving past it.
    pub(crate) fn peek_tag(&self) -> Option<u8> {
        (self.pos < self.end).then(|| self.buf[self.pos])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_take_the_fewest_bytes() {
        for (v, bytes) in [
            (0i64, vec![0x00]),
            (127, vec![0x7F]),
            (128, vec![0x00, 0x80]),
            (256, vec![0x01, 0x00]),
            (-1, vec![0xFF]),
            (-128, vec![0x80]),
            (-129, vec![0xFF, 0x7F]),
            (i32::MAX as i64, vec![0x7F, 0xFF, 0xFF, 0xFF]),
        ] {
            assert_eq!(integer_content(v), bytes, "{v}");
            assert_eq!(decode_integer(&bytes).unwrap(), v);
        }
        assert_eq!(
            unsigned_content(0xFFFF_FFFF),
            vec![0, 0xFF, 0xFF, 0xFF, 0xFF]
        );
        assert_eq!(
            decode_unsigned(&[0, 0xFF, 0xFF, 0xFF, 0xFF]).unwrap(),
            0xFFFF_FFFF
        );
        assert_eq!(
            decode_unsigned(&[0xFF, 0xFF, 0xFF, 0xFF]).unwrap(),
            0xFFFF_FFFF
        );
    }

    /// X.690 §8.19.5's example, and sysUpTime.0.
    #[test]
    fn oids_encode_as_x690_shows() {
        let o = Oid::parse("2.999.3").unwrap();
        assert_eq!(o.encode_content(), vec![0x88, 0x37, 0x03]);
        let up = Oid::parse(".1.3.6.1.2.1.1.3.0").unwrap();
        assert_eq!(up.encode_content(), vec![0x2B, 6, 1, 2, 1, 1, 3, 0]);
        assert_eq!(Oid::decode_content(&up.encode_content()).unwrap(), up);
        assert_eq!(Oid::decode_content(&[0x88, 0x37, 0x03]).unwrap(), o);
        let big = Oid::parse("1.3.6.1.4.1.4294967295").unwrap();
        assert_eq!(Oid::decode_content(&big.encode_content()).unwrap(), big);
        assert!(Oid::parse("1").is_err());
        assert!(Oid::parse("1.40").is_err());
        assert!(Oid::parse("1.3.x").is_err());
        assert_eq!(up.to_string(), "1.3.6.1.2.1.1.3.0");
    }

    #[test]
    fn long_lengths_round_trip() {
        let mut out = Vec::new();
        octets(&mut out, &[7u8; 300]);
        assert_eq!(&out[..4], &[0x04, 0x82, 0x01, 0x2C]);
        let mut r = Reader::new(&out);
        assert_eq!(r.octets().unwrap().len(), 300);
        assert!(r.is_empty());
        assert!(Reader::new(&out[..100]).octets().is_err());
    }

    #[test]
    fn values_round_trip_and_parse_from_text() {
        for (kind, text, value) in [
            ("integer", "-5", Value::Integer(-5)),
            ("string", "Rack 3", Value::OctetString(b"Rack 3".to_vec())),
            ("hex", "00 1a:ff", Value::OctetString(vec![0, 0x1a, 0xff])),
            ("oid", "1.3.6.1", Value::Oid(Oid(vec![1, 3, 6, 1]))),
            ("ip_address", "192.0.2.1", Value::IpAddress([192, 0, 2, 1])),
            ("counter32", "4294967295", Value::Counter32(u32::MAX)),
            ("gauge32", "7", Value::Gauge32(7)),
            ("timeticks", "100", Value::TimeTicks(100)),
            (
                "counter64",
                "18446744073709551615",
                Value::Counter64(u64::MAX),
            ),
            ("opaque", "9f78", Value::Opaque(vec![0x9f, 0x78])),
        ] {
            let parsed = Value::parse(kind, text).unwrap();
            assert_eq!(parsed, value, "{kind}");
            let mut out = Vec::new();
            parsed.encode(&mut out);
            let mut r = Reader::new(&out);
            assert_eq!(r.value().unwrap(), value, "{kind}");
        }
        assert!(Value::parse("integer", "2147483648").is_err());
        assert!(Value::parse("counter32", "-1").is_err());
        assert!(Value::parse("hex", "abc").is_err());
        for v in [
            Value::NoSuchObject,
            Value::NoSuchInstance,
            Value::EndOfMibView,
            Value::Null,
        ] {
            let mut out = Vec::new();
            v.encode(&mut out);
            assert_eq!(Reader::new(&out).value().unwrap(), v);
        }
    }
}
