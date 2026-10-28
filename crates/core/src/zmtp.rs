//! ZeroMQ's wire protocol, ZMTP 3.0 (RFC 23/ZMTP, rfc.zeromq.org/spec/23)
//! with the NULL security mechanism (RFC 23 §"The NULL Security
//! Mechanism"), for the two socket types a client of a ZeroMQ bus needs: SUB,
//! which subscribes to topic prefixes and receives the messages published
//! under them, and PUB, which publishes messages.
//!
//! The codec is sans-IO: the engine writes what it returns on a TCP
//! connection and feeds what arrives to [`Reader::feed`]. A connection
//! starts with a 64-byte greeting each way, then a READY command naming the
//! socket type, then message frames. This side announces version 3.0, so a
//! SUB subscribes with a message whose first byte is 1 followed by the
//! prefix (RFC 23 §"The SUB and XSUB socket types"), which ZMTP 3.1 peers
//! accept too. A PING command (ZMTP 3.1 heartbeats, RFC 37) is answered
//! with PONG. Nothing is capped: a frame is buffered as it arrives, however
//! long the peer says it is.

/// A socket type this side opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SocketType {
    Sub,
    Pub,
}

impl SocketType {
    fn name(self) -> &'static str {
        match self {
            SocketType::Sub => "SUB",
            SocketType::Pub => "PUB",
        }
    }

    /// The peer socket types this one may talk to (RFC 23, "SUB" and "PUB").
    fn accepts(self, peer: &str) -> bool {
        match self {
            SocketType::Sub => matches!(peer, "PUB" | "XPUB"),
            SocketType::Pub => matches!(peer, "SUB" | "XSUB"),
        }
    }
}

const FLAG_MORE: u8 = 0x01;
const FLAG_LONG: u8 = 0x02;
const FLAG_COMMAND: u8 = 0x04;

/// This side's greeting: signature, version 3.0, the NULL mechanism, not a
/// server, and filler.
pub(crate) fn greeting() -> Vec<u8> {
    let mut g = vec![0u8; 64];
    g[0] = 0xFF;
    g[9] = 0x7F;
    g[10] = 3;
    g[11] = 0;
    g[12..16].copy_from_slice(b"NULL");
    g
}

fn frame(flags: u8, body: &[u8], out: &mut Vec<u8>) {
    if body.len() > 255 {
        out.push(flags | FLAG_LONG);
        out.extend_from_slice(&(body.len() as u64).to_be_bytes());
    } else {
        out.push(flags);
        out.push(body.len() as u8);
    }
    out.extend_from_slice(body);
}

fn command(name: &str, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(1 + name.len() + data.len());
    body.push(name.len() as u8);
    body.extend_from_slice(name.as_bytes());
    body.extend_from_slice(data);
    let mut out = Vec::new();
    frame(FLAG_COMMAND, &body, &mut out);
    out
}

/// The READY command naming this side's socket type.
pub(crate) fn ready(socket: SocketType) -> Vec<u8> {
    let mut data = Vec::new();
    let (name, value) = ("Socket-Type", socket.name());
    data.push(name.len() as u8);
    data.extend_from_slice(name.as_bytes());
    data.extend_from_slice(&(value.len() as u32).to_be_bytes());
    data.extend_from_slice(value.as_bytes());
    command("READY", &data)
}

/// A message of one or more frames, each but the last flagged MORE.
pub(crate) fn message(frames: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    let last = frames.len().saturating_sub(1);
    for (i, f) in frames.iter().enumerate() {
        frame(if i < last { FLAG_MORE } else { 0 }, f, &mut out);
    }
    if frames.is_empty() {
        frame(0, &[], &mut out);
    }
    out
}

/// A SUB socket's subscription to messages whose first frame starts with
/// `prefix`.
pub(crate) fn subscribe(prefix: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(prefix.len() + 1);
    body.push(1);
    body.extend_from_slice(prefix);
    message(&[body])
}

/// What arrived on a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// The peer's READY, with the socket type it names: the connection is
    /// up and messages may go.
    Ready { peer: String },
    /// A whole message, its frames in order.
    Message(Vec<Vec<u8>>),
    /// Bytes to send back (PONG for a PING).
    Reply(Vec<u8>),
    /// The connection cannot go on: a bad greeting, another mechanism, a
    /// socket type that does not pair with this one, or the peer's ERROR.
    Failed(String),
}

/// Reads a peer's greeting, commands and messages from the bytes of one
/// connection.
#[derive(Debug)]
pub(crate) struct Reader {
    socket: SocketType,
    buffer: Vec<u8>,
    greeted: bool,
    ready: bool,
    parts: Vec<Vec<u8>>,
    failed: bool,
}

impl Reader {
    pub(crate) fn new(socket: SocketType) -> Reader {
        Reader {
            socket,
            buffer: Vec::new(),
            greeted: false,
            ready: false,
            parts: Vec::new(),
            failed: false,
        }
    }

    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<Event> {
        let mut events = Vec::new();
        if self.failed {
            return events;
        }
        self.buffer.extend_from_slice(bytes);
        if !self.greeted {
            if self.buffer.len() < 64 {
                // A signature that is already wrong need not wait for more.
                if !self.buffer.is_empty() && self.buffer[0] != 0xFF {
                    return self.fail("not a ZMTP peer: bad signature".into());
                }
                return events;
            }
            let g: Vec<u8> = self.buffer.drain(..64).collect();
            if g[0] != 0xFF || g[9] & 0x01 != 0x01 {
                return self.fail("not a ZMTP peer: bad signature".into());
            }
            if g[10] < 3 {
                return self.fail(format!(
                    "ZMTP {}.{} is not supported (3.0 or later)",
                    g[10], g[11]
                ));
            }
            let mechanism: Vec<u8> = g[12..32].iter().copied().take_while(|b| *b != 0).collect();
            if mechanism != b"NULL" {
                return self.fail(format!(
                    "the peer asks for the {} security mechanism; only NULL is supported",
                    String::from_utf8_lossy(&mechanism)
                ));
            }
            self.greeted = true;
        }
        while let Some((flags, body, used)) = next_frame(&self.buffer) {
            self.buffer.drain(..used);
            if flags & FLAG_COMMAND != 0 {
                match self.command(&body) {
                    Some(Event::Failed(reason)) => return self.fail(reason),
                    Some(e) => events.push(e),
                    None => {}
                }
                continue;
            }
            if !self.ready {
                return self.fail("a message arrived before the peer's READY".into());
            }
            self.parts.push(body);
            if flags & FLAG_MORE == 0 {
                events.push(Event::Message(std::mem::take(&mut self.parts)));
            }
        }
        events
    }

    fn fail(&mut self, reason: String) -> Vec<Event> {
        self.failed = true;
        self.buffer.clear();
        vec![Event::Failed(reason)]
    }

    fn command(&mut self, body: &[u8]) -> Option<Event> {
        let n = *body.first()? as usize;
        let name = body.get(1..1 + n)?;
        let data = &body[1 + n..];
        match name {
            b"READY" => {
                let peer = property(data, "Socket-Type").unwrap_or_default();
                if !self.socket.accepts(&peer) {
                    return Some(Event::Failed(format!(
                        "a {} socket cannot talk to the peer's {}",
                        self.socket.name(),
                        if peer.is_empty() {
                            "unnamed socket"
                        } else {
                            &peer
                        }
                    )));
                }
                self.ready = true;
                Some(Event::Ready { peer })
            }
            b"ERROR" => {
                let size = *data.first().unwrap_or(&0) as usize;
                let reason = data.get(1..1 + size).unwrap_or(&[]);
                Some(Event::Failed(format!(
                    "the peer refused the connection: {}",
                    String::from_utf8_lossy(reason)
                )))
            }
            b"PING" => {
                // A two-byte TTL, then the context the PONG carries back.
                let context = data.get(2..).unwrap_or(&[]);
                Some(Event::Reply(command("PONG", context)))
            }
            _ => None,
        }
    }
}

/// One whole frame at the start of `buffer`: its flags, body and the bytes it
/// took.
fn next_frame(buffer: &[u8]) -> Option<(u8, Vec<u8>, usize)> {
    let flags = *buffer.first()?;
    let (size, head) = if flags & FLAG_LONG != 0 {
        let b: [u8; 8] = buffer.get(1..9)?.try_into().ok()?;
        (u64::from_be_bytes(b), 9usize)
    } else {
        (*buffer.get(1)? as u64, 2usize)
    };
    let end = head.checked_add(usize::try_from(size).ok()?)?;
    let body = buffer.get(head..end)?.to_vec();
    Some((flags, body, end))
}

/// A READY property's value, by name in any case (RFC 23: property names are
/// case-insensitive).
fn property(mut data: &[u8], wanted: &str) -> Option<String> {
    while let Some(&n) = data.first() {
        let n = n as usize;
        let name = data.get(1..1 + n)?;
        let size: [u8; 4] = data.get(1 + n..5 + n)?.try_into().ok()?;
        let size = u32::from_be_bytes(size) as usize;
        let value = data.get(5 + n..5 + n + size)?;
        if name.eq_ignore_ascii_case(wanted.as_bytes()) {
            return Some(String::from_utf8_lossy(value).into_owned());
        }
        data = &data[5 + n + size..];
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(socket: &str) -> Vec<u8> {
        let mut bytes = greeting();
        let mut data = Vec::new();
        data.push(11);
        data.extend_from_slice(b"Socket-Type");
        data.extend_from_slice(&(socket.len() as u32).to_be_bytes());
        data.extend_from_slice(socket.as_bytes());
        bytes.extend(command("READY", &data));
        bytes
    }

    #[test]
    fn the_greeting_is_version_3_0_with_the_null_mechanism() {
        let g = greeting();
        assert_eq!(g.len(), 64);
        assert_eq!(
            g[..16].to_vec(),
            vec![0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0x7F, 3, 0, b'N', b'U', b'L', b'L']
        );
        assert!(g[16..].iter().all(|b| *b == 0));
    }

    #[test]
    fn ready_names_the_socket_type() {
        assert_eq!(
            ready(SocketType::Sub),
            [
                &[0x04, 25, 5][..],
                b"READY",
                &[11],
                b"Socket-Type",
                &[0, 0, 0, 3],
                b"SUB"
            ]
            .concat()
        );
    }

    #[test]
    fn frames_are_short_or_long_and_flag_more() {
        assert_eq!(
            message(&[b"ab".to_vec(), b"c".to_vec()]),
            vec![0x01, 2, b'a', b'b', 0x00, 1, b'c']
        );
        let long = vec![7u8; 300];
        let m = message(std::slice::from_ref(&long));
        assert_eq!(m[..9].to_vec(), vec![0x02, 0, 0, 0, 0, 0, 0, 1, 44]);
        assert_eq!(m[9..].to_vec(), long);
        assert_eq!(subscribe(b"ab"), vec![0x00, 3, 1, b'a', b'b']);
    }

    #[test]
    fn a_publishers_messages_arrive_whole_in_any_pieces() {
        let mut r = Reader::new(SocketType::Sub);
        let mut bytes = peer("XPUB");
        bytes.extend(message(&[
            b"{\"Type\":\"Update\"}".to_vec(),
            b"{}".to_vec(),
        ]));
        let mut events = Vec::new();
        for b in bytes.chunks(3) {
            events.extend(r.feed(b));
        }
        assert_eq!(
            events,
            vec![
                Event::Ready {
                    peer: "XPUB".into()
                },
                Event::Message(vec![b"{\"Type\":\"Update\"}".to_vec(), b"{}".to_vec()])
            ]
        );
    }

    #[test]
    fn a_ping_is_answered_with_its_context() {
        let mut r = Reader::new(SocketType::Pub);
        let mut bytes = peer("XSUB");
        bytes.extend(command("PING", &[0, 10, b'x', b'y']));
        let events = r.feed(&bytes);
        assert_eq!(events[1], Event::Reply(command("PONG", b"xy")));
    }

    #[test]
    fn a_peer_that_does_not_pair_or_errs_fails_the_connection() {
        let mut r = Reader::new(SocketType::Sub);
        assert!(matches!(r.feed(&peer("SUB"))[0], Event::Failed(_)));
        let mut r = Reader::new(SocketType::Sub);
        let mut bytes = greeting();
        bytes[12..17].copy_from_slice(b"CURVE");
        assert!(matches!(r.feed(&bytes)[0], Event::Failed(ref m) if m.contains("CURVE")));
        let mut r = Reader::new(SocketType::Pub);
        let mut bytes = greeting();
        bytes.extend(command("ERROR", &[4, b'n', b'o', b'p', b'e']));
        assert_eq!(
            r.feed(&bytes),
            vec![Event::Failed(
                "the peer refused the connection: nope".into()
            )]
        );
        let mut r = Reader::new(SocketType::Sub);
        assert!(matches!(r.feed(b"GET / HTTP/1.1")[0], Event::Failed(_)));
    }
}
