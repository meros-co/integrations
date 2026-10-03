//! OCP.1 framing and the AES70 base datatypes, as AES70-3 puts them on a
//! TCP stream.
//!
//! - A PDU is the sync byte 0x3B and a 9-byte header: protocol version
//!   (u16, 1), PDU size (u32, every byte after the sync byte), PDU type (u8)
//!   and message count (u16). The messages follow, all of the header's type.
//! - Types: 0 command (no response wanted), 1 command (response required),
//!   2 notification, 3 response, 4 keep-alive, 5 notification type 2
//!   (AES70-2023).
//! - A command is its size (u32, itself included), handle (u32), target
//!   object number (u32), method id (level u16, index u16), parameter count
//!   (u8) and the parameters. A response is its size, the handle it answers,
//!   a status (u8), parameter count and parameters. A notification is its
//!   size, target, method, parameter count, the subscriber's context (a
//!   blob) and the event: emitter object number, event id and event data.
//!   A keep-alive's body is the heartbeat time, u16 seconds or u32
//!   milliseconds, told apart by length.
//! - Everything is big-endian. A string is its length in Unicode scalar
//!   values (u16) and the UTF-8; a blob its byte length (u16) and the bytes;
//!   a list its count (u16) and the items; a class id its field count (u16)
//!   and the fields (u16 each).
//!
//! Learned from tschiemer/ocac (MIT; `ocp1.h`, `docs/occ_classes.md`),
//! PADL/SwiftOCA (Apache-2.0; `OCF/Messages`, `OCP.1`) and the OCA
//! Alliance's OCAMicro (`OCP.1/Messages`); no code was copied.

use std::fmt;

pub(crate) const SYNC: u8 = 0x3B;
pub(crate) const PROTOCOL_VERSION: u16 = 1;
/// Sync byte and header.
pub(crate) const PDU_HEADER: usize = 10;
/// A PDU larger than this is taken for garbage and the stream resynchronised.
pub(crate) const MAX_PDU: usize = 16 * 1024 * 1024;

pub(crate) const TYPE_COMMAND: u8 = 0;
pub(crate) const TYPE_COMMAND_RRQ: u8 = 1;
pub(crate) const TYPE_NOTIFICATION: u8 = 2;
pub(crate) const TYPE_RESPONSE: u8 = 3;
pub(crate) const TYPE_KEEPALIVE: u8 = 4;
pub(crate) const TYPE_NOTIFICATION2: u8 = 5;

/// A method, property or event id: definition level and index, written
/// "level.index".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Id(pub u16, pub u16);

impl Id {
    /// "3.1" to `Id(3, 1)`.
    pub(crate) fn parse(text: &str) -> Option<Id> {
        let (level, index) = text.split_once('.')?;
        Some(Id(level.parse().ok()?, index.parse().ok()?))
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.0, self.1)
    }
}

/// A class id as text: its fields joined by dots ("1.1.1.5").
pub(crate) fn class_text(class: &[u16]) -> String {
    class
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn unhex(text: &str) -> Option<Vec<u8>> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

// --- Writing ---------------------------------------------------------------

/// Big-endian AES70 values into a byte buffer.
#[derive(Debug, Default, Clone)]
pub(crate) struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub(crate) fn new() -> Writer {
        Writer::default()
    }

    pub(crate) fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }
    pub(crate) fn u16(&mut self, v: u16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub(crate) fn u32(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub(crate) fn u64(&mut self, v: u64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub(crate) fn i8(&mut self, v: i8) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub(crate) fn i16(&mut self, v: i16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub(crate) fn i32(&mut self, v: i32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub(crate) fn i64(&mut self, v: i64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub(crate) fn f32(&mut self, v: f32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub(crate) fn f64(&mut self, v: f64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub(crate) fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(u8::from(v))
    }
    pub(crate) fn id(&mut self, id: Id) -> &mut Self {
        self.u16(id.0).u16(id.1)
    }
    /// An OcaString: the count of Unicode scalar values, then the UTF-8.
    pub(crate) fn string(&mut self, s: &str) -> Result<&mut Self, String> {
        let count = u16::try_from(s.chars().count())
            .map_err(|_| "a string longer than 65535 characters".to_string())?;
        self.u16(count);
        self.buf.extend_from_slice(s.as_bytes());
        Ok(self)
    }
    /// An OcaBlob: the byte count, then the bytes.
    pub(crate) fn blob(&mut self, b: &[u8]) -> Result<&mut Self, String> {
        let count =
            u16::try_from(b.len()).map_err(|_| "a blob longer than 65535 bytes".to_string())?;
        self.u16(count);
        self.buf.extend_from_slice(b);
        Ok(self)
    }
    #[cfg(test)]
    pub(crate) fn class_id(&mut self, class: &[u16]) -> &mut Self {
        self.u16(class.len() as u16);
        for f in class {
            self.u16(*f);
        }
        self
    }
    pub(crate) fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(b);
        self
    }
}

// --- Reading ---------------------------------------------------------------

/// Big-endian AES70 values out of a byte slice.
#[derive(Debug, Clone)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

pub(crate) type Decoded<T> = Result<T, String>;

macro_rules! read_int {
    ($name:ident, $t:ty) => {
        pub(crate) fn $name(&mut self) -> Decoded<$t> {
            let b = self.take(std::mem::size_of::<$t>())?;
            Ok(<$t>::from_be_bytes(b.try_into().expect("sized")))
        }
    };
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, at: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len() - self.at
    }

    pub(crate) fn rest(&mut self) -> &'a [u8] {
        let rest = &self.data[self.at..];
        self.at = self.data.len();
        rest
    }

    pub(crate) fn take(&mut self, n: usize) -> Decoded<&'a [u8]> {
        if self.remaining() < n {
            return Err(format!(
                "{n} bytes wanted at offset {}, {} left",
                self.at,
                self.remaining()
            ));
        }
        let b = &self.data[self.at..self.at + n];
        self.at += n;
        Ok(b)
    }

    read_int!(u8, u8);
    read_int!(u16, u16);
    read_int!(u32, u32);
    read_int!(u64, u64);
    read_int!(i8, i8);
    read_int!(i16, i16);
    read_int!(i32, i32);
    read_int!(i64, i64);
    read_int!(f32, f32);
    read_int!(f64, f64);

    pub(crate) fn bool(&mut self) -> Decoded<bool> {
        Ok(self.u8()? != 0)
    }

    pub(crate) fn id(&mut self) -> Decoded<Id> {
        Ok(Id(self.u16()?, self.u16()?))
    }

    /// An OcaString. Its length counts Unicode scalar values, so the bytes
    /// are walked one character at a time.
    pub(crate) fn string(&mut self) -> Decoded<String> {
        let count = self.u16()? as usize;
        let start = self.at;
        let mut end = start;
        for _ in 0..count {
            let lead = *self
                .data
                .get(end)
                .ok_or_else(|| format!("a string of {count} characters runs past the end"))?;
            end += match lead {
                0x00..=0x7F => 1,
                0xC0..=0xDF => 2,
                0xE0..=0xEF => 3,
                0xF0..=0xF7 => 4,
                _ => return Err(format!("byte {lead:#04x} does not start a UTF-8 character")),
            };
        }
        if end > self.data.len() {
            return Err(format!("a string of {count} characters runs past the end"));
        }
        let text = std::str::from_utf8(&self.data[start..end])
            .map_err(|e| format!("a string is not UTF-8: {e}"))?
            .to_string();
        self.at = end;
        Ok(text)
    }

    pub(crate) fn blob(&mut self) -> Decoded<&'a [u8]> {
        let n = self.u16()? as usize;
        self.take(n)
    }

    pub(crate) fn class_id(&mut self) -> Decoded<Vec<u16>> {
        let n = self.u16()?;
        (0..n).map(|_| self.u16()).collect()
    }
}

// --- Messages --------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Command {
    pub handle: u32,
    pub target: u32,
    pub method: Id,
    pub param_count: u8,
    pub params: Vec<u8>,
}

impl Command {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u32((4 + 4 + 4 + 4 + 1 + self.params.len()) as u32)
            .u32(self.handle)
            .u32(self.target)
            .id(self.method)
            .u8(self.param_count)
            .bytes(&self.params);
        w.buf
    }

    fn decode(r: &mut Reader) -> Decoded<Command> {
        Ok(Command {
            handle: r.u32()?,
            target: r.u32()?,
            method: r.id()?,
            param_count: r.u8()?,
            params: r.rest().to_vec(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Response {
    pub handle: u32,
    pub status: u8,
    pub param_count: u8,
    pub params: Vec<u8>,
}

impl Response {
    #[cfg(test)]
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u32((4 + 4 + 1 + 1 + self.params.len()) as u32)
            .u32(self.handle)
            .u8(self.status)
            .u8(self.param_count)
            .bytes(&self.params);
        w.buf
    }

    fn decode(r: &mut Reader) -> Decoded<Response> {
        Ok(Response {
            handle: r.u32()?,
            status: r.u8()?,
            param_count: r.u8()?,
            params: r.rest().to_vec(),
        })
    }
}

/// An event as it arrives: who emitted it, which event, and its data.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Event {
    pub emitter: u32,
    pub event: Id,
    pub data: Vec<u8>,
}

/// A notification (PDU type 2): the subscriber method it is addressed to,
/// the subscriber's context and the event.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Notification {
    pub target: u32,
    pub method: Id,
    pub param_count: u8,
    pub context: Vec<u8>,
    /// Absent when the notification carries only the context.
    pub event: Option<Event>,
}

impl Notification {
    #[cfg(test)]
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut body = Writer::new();
        body.u32(self.target)
            .id(self.method)
            .u8(self.param_count)
            .blob(&self.context)
            .expect("a short context");
        if let Some(e) = &self.event {
            body.u32(e.emitter).id(e.event).bytes(&e.data);
        }
        let mut w = Writer::new();
        w.u32((4 + body.buf.len()) as u32).bytes(&body.buf);
        w.buf
    }

    fn decode(r: &mut Reader) -> Decoded<Notification> {
        let target = r.u32()?;
        let method = r.id()?;
        let param_count = r.u8()?;
        let context = r.blob()?.to_vec();
        let event = if r.remaining() >= 8 {
            Some(Event {
                emitter: r.u32()?,
                event: r.id()?,
                data: r.rest().to_vec(),
            })
        } else {
            None
        };
        Ok(Notification {
            target,
            method,
            param_count,
            context,
            event,
        })
    }
}

/// A notification of type 2 (AES70-2023): the event, whether it is an event
/// (0) or an exception (1), and its data.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Notification2 {
    pub event: Event,
    pub kind: u8,
}

impl Notification2 {
    fn decode(r: &mut Reader) -> Decoded<Notification2> {
        let emitter = r.u32()?;
        let event = r.id()?;
        let kind = r.u8()?;
        Ok(Notification2 {
            event: Event {
                emitter,
                event,
                data: r.rest().to_vec(),
            },
            kind,
        })
    }
}

/// The heartbeat time a keep-alive carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeepAlive {
    Seconds(u16),
    Millis(u32),
}

impl KeepAlive {
    /// The form for an interval: whole seconds when it is one, else
    /// milliseconds (the second form, AES70-2018).
    pub(crate) fn for_interval(ms: u64) -> KeepAlive {
        if ms.is_multiple_of(1000) && ms / 1000 <= u64::from(u16::MAX) && ms > 0 {
            KeepAlive::Seconds((ms / 1000) as u16)
        } else {
            KeepAlive::Millis(ms.clamp(1, u64::from(u32::MAX)) as u32)
        }
    }

    #[cfg(test)]
    pub(crate) fn millis(self) -> u64 {
        match self {
            KeepAlive::Seconds(s) => u64::from(s) * 1000,
            KeepAlive::Millis(ms) => u64::from(ms),
        }
    }

    pub(crate) fn pdu(self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            KeepAlive::Seconds(s) => w.u16(s),
            KeepAlive::Millis(ms) => w.u32(ms),
        };
        pdu(TYPE_KEEPALIVE, &[w.buf])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Message {
    /// A command the device sent (type 0 or 1); a controller has none to
    /// answer.
    Command(Command),
    Response(Response),
    Notification(Notification),
    Notification2(Notification2),
    KeepAlive(KeepAlive),
}

/// One PDU of messages already encoded, each starting with its own size.
pub(crate) fn pdu(kind: u8, messages: &[Vec<u8>]) -> Vec<u8> {
    let body: usize = messages.iter().map(Vec::len).sum();
    let mut w = Writer::new();
    w.u8(SYNC)
        .u16(PROTOCOL_VERSION)
        .u32((PDU_HEADER - 1 + body) as u32)
        .u8(kind)
        .u16(messages.len() as u16);
    for m in messages {
        w.bytes(m);
    }
    w.buf
}

/// The messages of one complete PDU (sync byte included).
pub(crate) fn parse_pdu(bytes: &[u8]) -> Decoded<(u8, Vec<Message>)> {
    let mut r = Reader::new(bytes);
    if r.u8()? != SYNC {
        return Err("no sync byte".into());
    }
    let version = r.u16()?;
    if version < 1 {
        return Err(format!("protocol version {version}"));
    }
    let size = r.u32()? as usize;
    if size + 1 != bytes.len() {
        return Err(format!(
            "the header says {} bytes, the PDU has {}",
            size + 1,
            bytes.len()
        ));
    }
    let kind = r.u8()?;
    let count = r.u16()?;
    if kind == TYPE_KEEPALIVE {
        // The heartbeat time is the rest of the PDU; its length says which
        // unit it is in.
        let body = r.rest();
        let ka = match body.len() {
            2 => KeepAlive::Seconds(u16::from_be_bytes([body[0], body[1]])),
            4 => KeepAlive::Millis(u32::from_be_bytes([body[0], body[1], body[2], body[3]])),
            n => return Err(format!("a keep-alive of {n} bytes")),
        };
        return Ok((kind, vec![Message::KeepAlive(ka)]));
    }
    let mut messages = Vec::with_capacity(count as usize);
    for i in 0..count {
        let mut peek = r.clone();
        let size = peek.u32()? as usize;
        if size < 4 {
            return Err(format!("message {i} declares {size} bytes"));
        }
        let message = r
            .take(size)
            .map_err(|e| format!("message {i} of {count}: {e}"))?;
        let mut m = Reader::new(&message[4..]);
        messages.push(match kind {
            TYPE_COMMAND | TYPE_COMMAND_RRQ => Message::Command(Command::decode(&mut m)?),
            TYPE_RESPONSE => Message::Response(Response::decode(&mut m)?),
            TYPE_NOTIFICATION => Message::Notification(Notification::decode(&mut m)?),
            TYPE_NOTIFICATION2 => Message::Notification2(Notification2::decode(&mut m)?),
            other => return Err(format!("PDU type {other}")),
        });
    }
    Ok((kind, messages))
}

/// Splits the device's byte stream into PDUs.
#[derive(Debug, Default)]
pub(crate) struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    /// Complete PDUs, and why any bytes were discarded. Bytes before a sync
    /// byte are dropped, and a header that cannot be one makes the deframer
    /// look for the next sync byte.
    pub(crate) fn feed(&mut self, data: &[u8]) -> (Vec<Vec<u8>>, Vec<String>) {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        let mut discarded = Vec::new();
        let mut at = 0;
        loop {
            match self.buf[at..].iter().position(|&b| b == SYNC) {
                Some(0) => {}
                Some(skip) => {
                    discarded.push(format!("{skip} bytes before a sync byte"));
                    at += skip;
                }
                None => {
                    if at < self.buf.len() {
                        discarded.push(format!("{} bytes with no sync byte", self.buf.len() - at));
                    }
                    at = self.buf.len();
                    break;
                }
            }
            if self.buf.len() - at < PDU_HEADER {
                break;
            }
            let h = &self.buf[at..];
            let version = u16::from_be_bytes([h[1], h[2]]);
            let size = u32::from_be_bytes([h[3], h[4], h[5], h[6]]) as usize;
            let kind = h[7];
            if version == 0
                || !(PDU_HEADER - 1..=MAX_PDU).contains(&size)
                || kind > TYPE_NOTIFICATION2
            {
                discarded.push(format!(
                    "a header that is not OCP.1 (version {version}, size {size}, type {kind})"
                ));
                at += 1;
                continue;
            }
            if self.buf.len() - at < size + 1 {
                break;
            }
            out.push(self.buf[at..at + size + 1].to_vec());
            at += size + 1;
        }
        self.buf.drain(..at);
        (out, discarded)
    }
}

/// PropertyChanged event data: the property, its new value as bytes, and the
/// change type (1 current value, 2 minimum, 3 maximum, 4 item added, 5 item
/// changed, 6 item deleted).
pub(crate) fn property_changed(data: &[u8]) -> Decoded<(Id, &[u8], u8)> {
    if data.len() < 5 {
        return Err(format!("PropertyChanged data of {} bytes", data.len()));
    }
    let mut r = Reader::new(data);
    let id = r.id()?;
    let value = &data[4..data.len() - 1];
    Ok((id, value, data[data.len() - 1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_pdu_has_the_documented_layout() {
        // GetMembers (3.5) on the root block (100), handle 7.
        let cmd = Command {
            handle: 7,
            target: 100,
            method: Id(3, 5),
            param_count: 0,
            params: vec![],
        };
        let bytes = pdu(TYPE_COMMAND_RRQ, &[cmd.encode()]);
        assert_eq!(
            hex(&bytes),
            concat!(
                "3b",       // sync
                "0001",     // protocol version 1
                "0000001a", // 26 bytes after the sync byte: 9 header + 17
                "01",       // command, response required
                "0001",     // one message
                "00000011", // command size 17, itself included
                "00000007", // handle
                "00000064", // target 100
                "00030005", // method 3.5
                "00",       // no parameters
            )
        );
        let (kind, messages) = parse_pdu(&bytes).unwrap();
        assert_eq!(kind, TYPE_COMMAND_RRQ);
        assert_eq!(messages, vec![Message::Command(cmd)]);
    }

    #[test]
    fn several_messages_share_a_pdu() {
        let a = Response {
            handle: 1,
            status: 0,
            param_count: 1,
            params: vec![0x3f, 0x80, 0, 0],
        };
        let b = Response {
            handle: 2,
            status: 5,
            param_count: 0,
            params: vec![],
        };
        let bytes = pdu(TYPE_RESPONSE, &[a.encode(), b.encode()]);
        assert_eq!(bytes[8..10], [0, 2]);
        let (_, messages) = parse_pdu(&bytes).unwrap();
        assert_eq!(messages, vec![Message::Response(a), Message::Response(b)]);
    }

    #[test]
    fn keepalives_in_seconds_and_milliseconds() {
        assert_eq!(
            hex(&KeepAlive::for_interval(5000).pdu()),
            "3b00010000000b0400010005"
        );
        assert_eq!(
            hex(&KeepAlive::for_interval(1500).pdu()),
            "3b00010000000d040001000005dc"
        );
        let (_, m) = parse_pdu(&KeepAlive::Seconds(3).pdu()).unwrap();
        assert_eq!(m, vec![Message::KeepAlive(KeepAlive::Seconds(3))]);
        let (_, m) = parse_pdu(&KeepAlive::Millis(250).pdu()).unwrap();
        assert_eq!(m, vec![Message::KeepAlive(KeepAlive::Millis(250))]);
        assert_eq!(KeepAlive::Millis(250).millis(), 250);
        assert_eq!(KeepAlive::Seconds(2).millis(), 2000);
    }

    #[test]
    fn notifications_carry_context_and_event() {
        let n = Notification {
            target: 4096,
            method: Id(1, 1),
            param_count: 2,
            context: vec![],
            event: Some(Event {
                emitter: 10001,
                event: Id(1, 1),
                data: vec![0, 4, 0, 1, 0xc1, 0x20, 0, 0, 1],
            }),
        };
        let bytes = pdu(TYPE_NOTIFICATION, &[n.encode()]);
        let (kind, m) = parse_pdu(&bytes).unwrap();
        assert_eq!(kind, TYPE_NOTIFICATION);
        assert_eq!(m, vec![Message::Notification(n.clone())]);
        let event = n.event.unwrap();
        let (id, value, change) = property_changed(&event.data).unwrap();
        assert_eq!(id, Id(4, 1));
        assert_eq!(f32::from_be_bytes(value.try_into().unwrap()), -10.0);
        assert_eq!(change, 1);
    }

    #[test]
    fn notification2_is_read() {
        let mut w = Writer::new();
        w.u32(0) // size, patched below
            .u32(42)
            .id(Id(1, 1))
            .u8(0)
            .bytes(&[0, 4, 0, 1, 1, 1]);
        let n = w.buf.len() as u32;
        w.buf[..4].copy_from_slice(&n.to_be_bytes());
        let (_, m) = parse_pdu(&pdu(TYPE_NOTIFICATION2, &[w.buf])).unwrap();
        assert_eq!(
            m,
            vec![Message::Notification2(Notification2 {
                event: Event {
                    emitter: 42,
                    event: Id(1, 1),
                    data: vec![0, 4, 0, 1, 1, 1]
                },
                kind: 0
            })]
        );
    }

    #[test]
    fn strings_count_characters_not_bytes() {
        let mut w = Writer::new();
        w.string("Gain é∑").unwrap();
        // 7 characters, 10 bytes of UTF-8.
        assert_eq!(&w.buf[..2], &[0, 7]);
        assert_eq!(w.buf.len(), 2 + 10);
        w.u8(0xAA);
        let mut r = Reader::new(&w.buf);
        assert_eq!(r.string().unwrap(), "Gain é∑");
        assert_eq!(r.u8().unwrap(), 0xAA);
        // Four-byte characters count once.
        let mut w = Writer::new();
        w.string("🎚x").unwrap();
        assert_eq!(&w.buf[..2], &[0, 2]);
        assert_eq!(Reader::new(&w.buf).string().unwrap(), "🎚x");
        // Truncated.
        assert!(Reader::new(&[0, 3, b'a']).string().is_err());
    }

    #[test]
    fn base_types_round_trip() {
        let mut w = Writer::new();
        w.u8(1)
            .u16(0xBEEF)
            .u32(100)
            .u64(u64::MAX)
            .i8(-2)
            .i16(-300)
            .i32(-70000)
            .i64(i64::MIN)
            .f32(-3.5)
            .f64(1e-9)
            .bool(true)
            .class_id(&[1, 1, 1, 5]);
        w.blob(&[9, 8, 7]).unwrap();
        let mut r = Reader::new(&w.buf);
        assert_eq!(r.u8().unwrap(), 1);
        assert_eq!(r.u16().unwrap(), 0xBEEF);
        assert_eq!(r.u32().unwrap(), 100);
        assert_eq!(r.u64().unwrap(), u64::MAX);
        assert_eq!(r.i8().unwrap(), -2);
        assert_eq!(r.i16().unwrap(), -300);
        assert_eq!(r.i32().unwrap(), -70000);
        assert_eq!(r.i64().unwrap(), i64::MIN);
        assert_eq!(r.f32().unwrap(), -3.5);
        assert_eq!(r.f64().unwrap(), 1e-9);
        assert!(r.bool().unwrap());
        assert_eq!(r.class_id().unwrap(), vec![1, 1, 1, 5]);
        assert_eq!(r.blob().unwrap(), &[9, 8, 7]);
        assert_eq!(r.remaining(), 0);
        assert!(r.u8().is_err());
        // A class id is its field count and the fields.
        assert_eq!(
            hex(&Writer::new().class_id(&[1, 1, 3]).buf),
            "0003000100010003"
        );
    }

    #[test]
    fn ids_and_hex() {
        assert_eq!(Id::parse("3.17"), Some(Id(3, 17)));
        assert_eq!(Id::parse("3"), None);
        assert_eq!(Id::parse("a.1"), None);
        assert_eq!(Id(4, 1).to_string(), "4.1");
        assert_eq!(class_text(&[1, 1, 1, 5]), "1.1.1.5");
        assert_eq!(unhex("00 0aFF"), Some(vec![0, 10, 255]));
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
        assert_eq!(hex(&[0, 10, 255]), "000aff");
    }

    #[test]
    fn the_deframer_splits_joins_and_resynchronises() {
        let a = KeepAlive::Seconds(1).pdu();
        let b = pdu(
            TYPE_RESPONSE,
            &[Response {
                handle: 9,
                status: 0,
                param_count: 0,
                params: vec![],
            }
            .encode()],
        );
        let mut stream = vec![0xFF, 0x00];
        stream.extend_from_slice(&a);
        stream.extend_from_slice(&b);
        let mut d = Deframer::default();
        // Fed a byte at a time, every PDU still comes out whole.
        let mut got = Vec::new();
        let mut dropped = Vec::new();
        for byte in &stream {
            let (pdus, why) = d.feed(&[*byte]);
            got.extend(pdus);
            dropped.extend(why);
        }
        assert_eq!(got, vec![a.clone(), b.clone()]);
        assert_eq!(dropped.len(), 2);
        // Two PDUs in one read.
        let mut both = a.clone();
        both.extend_from_slice(&b);
        assert_eq!(d.feed(&both).0, vec![a.clone(), b]);
        // A sync byte that starts no PDU is skipped.
        let mut bad = vec![SYNC, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        bad.extend_from_slice(&a);
        let (pdus, why) = d.feed(&bad);
        assert_eq!(pdus, vec![a]);
        assert!(!why.is_empty());
    }

    #[test]
    fn malformed_pdus_are_errors() {
        assert!(parse_pdu(&[SYNC, 0, 1, 0, 0, 0, 9, 3, 0, 1]).is_err());
        // A message that declares more bytes than the PDU has.
        let mut p = pdu(TYPE_RESPONSE, &[vec![0, 0, 0, 40, 0, 0, 0, 1, 0, 0]]);
        assert!(parse_pdu(&p).is_err());
        // A size field that disagrees with the PDU's length.
        p.push(0);
        assert!(parse_pdu(&p).is_err());
        assert!(property_changed(&[0, 1, 0]).is_err());
    }
}
