//! A small MQTT 3.1.1 subscriber codec (OASIS MQTT Version 3.1.1, §2 and §3)
//! for the IS-07 event transports, apart from the `generic-mqtt` client
//! (`mqtt.rs`) so neither integration builds the other:
//! what a subscriber needs, and no more. It connects with a clean session,
//! subscribes and unsubscribes at QoS 0, reads PUBLISH packets of any QoS
//! (answering QoS 1 with PUBACK and QoS 2 with PUBREC, then PUBCOMP to the
//! broker's PUBREL), and keeps the connection with PINGREQ.
//!
//! Used by the IS-07 event transports of `nmos-node` (AMWA IS-07 v1.0.1,
//! "Transport - MQTT"), which only listens to state messages a broker relays.

/// Packet types (§2.2.1).
const CONNECT: u8 = 1;
const CONNACK: u8 = 2;
const PUBLISH: u8 = 3;
const PUBACK: u8 = 4;
const PUBREC: u8 = 5;
const PUBREL: u8 = 6;
const PUBCOMP: u8 = 7;
const SUBSCRIBE: u8 = 8;
const SUBACK: u8 = 9;
const UNSUBSCRIBE: u8 = 10;
const UNSUBACK: u8 = 11;
const PINGREQ: u8 = 12;
const PINGRESP: u8 = 13;
const DISCONNECT: u8 = 14;

/// The largest remaining length a packet can declare (§2.2.3).
const MAX_REMAINING: usize = 268_435_455;

/// What the broker sent, as far as a subscriber cares.
#[derive(Debug, PartialEq)]
pub(crate) enum Packet {
    /// CONNACK: 0 is accepted, anything else the broker's refusal (§3.2.2.3).
    ConnAck {
        code: u8,
    },
    Publish {
        topic: String,
        payload: Vec<u8>,
    },
    SubAck,
    UnsubAck,
    PingResp,
    /// Anything else, already answered where the protocol asks for it.
    Other,
}

fn remaining_length(mut n: usize, out: &mut Vec<u8>) {
    loop {
        let mut byte = (n % 128) as u8;
        n /= 128;
        if n > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if n == 0 {
            break;
        }
    }
}

fn utf8(s: &str, out: &mut Vec<u8>) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(u16::MAX as usize);
    out.extend_from_slice(&(len as u16).to_be_bytes());
    out.extend_from_slice(&bytes[..len]);
}

fn packet(kind: u8, flags: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![(kind << 4) | flags];
    remaining_length(body.len(), &mut out);
    out.extend_from_slice(body);
    out
}

/// CONNECT with a clean session (§3.1): the client id, the keep-alive in
/// seconds, and a user name and password when given.
pub(crate) fn connect(client_id: &str, keep_alive_s: u16, login: Option<(&str, &str)>) -> Vec<u8> {
    let mut body = Vec::new();
    utf8("MQTT", &mut body);
    body.push(4); // protocol level 3.1.1
    let mut flags = 0x02; // clean session
    if login.is_some() {
        flags |= 0xc0; // user name and password
    }
    body.push(flags);
    body.extend_from_slice(&keep_alive_s.to_be_bytes());
    utf8(client_id, &mut body);
    if let Some((user, password)) = login {
        utf8(user, &mut body);
        utf8(password, &mut body);
    }
    packet(CONNECT, 0, &body)
}

/// SUBSCRIBE to topics at QoS 0 (§3.8).
pub(crate) fn subscribe(id: u16, topics: &[String]) -> Vec<u8> {
    let mut body = id.to_be_bytes().to_vec();
    for t in topics {
        utf8(t, &mut body);
        body.push(0);
    }
    packet(SUBSCRIBE, 0x02, &body)
}

/// UNSUBSCRIBE from topics (§3.10).
pub(crate) fn unsubscribe(id: u16, topics: &[String]) -> Vec<u8> {
    let mut body = id.to_be_bytes().to_vec();
    for t in topics {
        utf8(t, &mut body);
    }
    packet(UNSUBSCRIBE, 0x02, &body)
}

pub(crate) fn pingreq() -> Vec<u8> {
    packet(PINGREQ, 0, &[])
}

pub(crate) fn disconnect() -> Vec<u8> {
    packet(DISCONNECT, 0, &[])
}

/// Bytes from the broker, split into packets.
#[derive(Debug, Default)]
pub(crate) struct Reader {
    buf: Vec<u8>,
}

impl Reader {
    /// Take bytes; returns each whole packet read, and the answers the
    /// protocol asks of the client for them (PUBACK, PUBREC, PUBCOMP), or
    /// `Err` for a stream that is not MQTT.
    pub(crate) fn push(&mut self, data: &[u8]) -> Result<(Vec<Packet>, Vec<Vec<u8>>), String> {
        self.buf.extend_from_slice(data);
        let mut packets = Vec::new();
        let mut answers = Vec::new();
        loop {
            let Some(&first) = self.buf.first() else {
                break;
            };
            // The remaining length: at most four bytes.
            let mut len = 0usize;
            let mut mult = 1usize;
            let mut at = 1;
            let complete = loop {
                let Some(&b) = self.buf.get(at) else {
                    break false;
                };
                len += (b & 0x7f) as usize * mult;
                at += 1;
                if b & 0x80 == 0 {
                    break true;
                }
                mult *= 128;
                if at > 4 {
                    return Err("mqtt: malformed remaining length".into());
                }
            };
            if !complete {
                break;
            }
            if len > MAX_REMAINING {
                return Err("mqtt: packet too long".into());
            }
            if self.buf.len() < at + len {
                break;
            }
            let body: Vec<u8> = self.buf[at..at + len].to_vec();
            self.buf.drain(..at + len);
            let kind = first >> 4;
            let flags = first & 0x0f;
            packets.push(match kind {
                CONNACK if body.len() >= 2 => Packet::ConnAck { code: body[1] },
                PUBLISH => {
                    let qos = (flags >> 1) & 0x03;
                    let tlen = u16::from_be_bytes([
                        *body.first().ok_or("mqtt: short PUBLISH")?,
                        *body.get(1).ok_or("mqtt: short PUBLISH")?,
                    ]) as usize;
                    let topic = body.get(2..2 + tlen).ok_or("mqtt: short PUBLISH")?;
                    let topic = String::from_utf8_lossy(topic).into_owned();
                    let mut rest = 2 + tlen;
                    if qos > 0 {
                        let id = body.get(rest..rest + 2).ok_or("mqtt: short PUBLISH")?;
                        let reply = if qos == 1 { PUBACK } else { PUBREC };
                        answers.push(packet(reply, if reply == PUBREL { 2 } else { 0 }, id));
                        rest += 2;
                    }
                    Packet::Publish {
                        topic,
                        payload: body[rest..].to_vec(),
                    }
                }
                PUBREL if body.len() >= 2 => {
                    answers.push(packet(PUBCOMP, 0, &body[..2]));
                    Packet::Other
                }
                SUBACK => Packet::SubAck,
                UNSUBACK => Packet::UnsubAck,
                PINGRESP => Packet::PingResp,
                _ => Packet::Other,
            });
        }
        Ok((packets, answers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_is_byte_exact() {
        // MQTT 3.1.1 §3.1: "MQTT", level 4, clean session, keep-alive 30 s.
        assert_eq!(
            connect("c1", 30, None),
            [0x10, 14, 0, 4, b'M', b'Q', b'T', b'T', 4, 0x02, 0, 30, 0, 2, b'c', b'1']
        );
        let with_login = connect("c", 60, Some(("u", "p")));
        assert_eq!(with_login[9], 0xc2);
        assert!(with_login.ends_with(&[0, 1, b'u', 0, 1, b'p']));
    }

    #[test]
    fn subscribe_and_unsubscribe_name_their_topics() {
        assert_eq!(
            subscribe(7, &["a/b".into()]),
            [0x82, 8, 0, 7, 0, 3, b'a', b'/', b'b', 0]
        );
        assert_eq!(unsubscribe(8, &["a".into()]), [0xa2, 5, 0, 8, 0, 1, b'a']);
        assert_eq!(pingreq(), [0xc0, 0]);
        assert_eq!(disconnect(), [0xe0, 0]);
    }

    #[test]
    fn packets_are_read_across_reads_and_answered() {
        let mut r = Reader::default();
        assert_eq!(r.push(&[0x20, 2, 0]).unwrap(), (vec![], vec![]));
        let (p, a) = r.push(&[0]).unwrap();
        assert_eq!(p, [Packet::ConnAck { code: 0 }]);
        assert!(a.is_empty());
        // A retained QoS 1 publish: answered with PUBACK.
        let mut publish = vec![0x33, 0];
        let mut body = vec![0, 3, b't', b'/', b'x', 0, 9];
        body.extend_from_slice(b"{\"a\":1}");
        publish[1] = body.len() as u8;
        publish.extend_from_slice(&body);
        let (p, a) = r.push(&publish).unwrap();
        assert_eq!(
            p,
            [Packet::Publish {
                topic: "t/x".into(),
                payload: b"{\"a\":1}".to_vec()
            }]
        );
        assert_eq!(a, [vec![0x40, 2, 0, 9]]);
        // QoS 2: PUBREC, then PUBCOMP for the broker's PUBREL.
        let (_, a) = r.push(&[0x34, 5, 0, 1, b't', 0, 3]).unwrap();
        assert_eq!(a, [vec![0x50, 2, 0, 3]]);
        let (_, a) = r.push(&[0x62, 2, 0, 3]).unwrap();
        assert_eq!(a, [vec![0x70, 2, 0, 3]]);
        let (p, _) = r.push(&[0x90, 3, 0, 1, 0, 0xd0, 0]).unwrap();
        assert_eq!(p, [Packet::SubAck, Packet::PingResp]);
        assert!(r.push(&[0x30, 0xff, 0xff, 0xff, 0xff, 0x7f]).is_err());
    }
}
