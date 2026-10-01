//! S101, the framing that carries Ember+ over a byte stream, written from
//! Lawo's Ember+ Specification 2.50, chapter "Message Framing".
//!
//! - Variant 1 (the default every provider supports): a frame starts with BOF
//!   0xFE and ends with EOF 0xFF; every byte of 0xF8 or above inside it is
//!   sent as CE 0xFD followed by the byte XOR 0x20. The 16-bit CRC of the data
//!   (initial 0xFFFF, the table in the appendix), inverted, follows the data
//!   and is escaped the same way. Over data and CRC together the CRC is always
//!   0xF0B8.
//! - Variant 2 (non-escaping): 0xF8, a byte giving the number of length bytes
//!   (0 to 7), the payload length most significant byte first, the payload.
//!   This module decodes it but never sends it, since a provider only uses it
//!   after a consumer has probed for it.
//! - Every message begins slot 0x00, message type 0x0E (EmBER), a command:
//!   0x00 EmBER packet, 0x01 keep-alive request, 0x02 keep-alive response, and
//!   version 0x01. An EmBER packet continues with flags (0xC0 single packet,
//!   0x80 first, 0x40 last, 0x00 a middle packet, 0x20 empty), DTD 0x01 (Glow),
//!   the number of application bytes and the application bytes (the Glow DTD
//!   version, minor then major), then the BER payload.

pub(crate) const BOF: u8 = 0xFE;
pub(crate) const EOF: u8 = 0xFF;
pub(crate) const CE: u8 = 0xFD;
pub(crate) const XOR: u8 = 0x20;
/// Bytes at or above this are escaped inside a variant 1 frame; outside a
/// frame it starts a variant 2 frame.
pub(crate) const INVALID: u8 = 0xF8;

pub(crate) const SLOT: u8 = 0x00;
pub(crate) const MESSAGE_EMBER: u8 = 0x0E;
pub(crate) const COMMAND_EMBER: u8 = 0x00;
pub(crate) const COMMAND_KEEPALIVE_REQUEST: u8 = 0x01;
pub(crate) const COMMAND_KEEPALIVE_RESPONSE: u8 = 0x02;
pub(crate) const VERSION: u8 = 0x01;
pub(crate) const DTD_GLOW: u8 = 0x01;

pub(crate) const FLAG_SINGLE: u8 = 0xC0;
pub(crate) const FLAG_FIRST: u8 = 0x80;
pub(crate) const FLAG_LAST: u8 = 0x40;
pub(crate) const FLAG_EMPTY: u8 = 0x20;

/// The Glow DTD version sent in the application bytes, minor then major:
/// 2.50, the version the specification describes.
pub(crate) const GLOW_VERSION: [u8; 2] = [50, 2];

/// "The payload size of an Ember+ packet is limited to 1024 bytes"
/// (Behaviour rules, "Message length"). Longer messages are split.
pub(crate) const MAX_PACKET_PAYLOAD: usize = 1024;

/// A frame (or an assembled message) larger than this is not Ember+.
const MAX_FRAME: usize = 16 * 1024 * 1024;

/// The CRC table of the appendix: CRC-16 with the reflected polynomial
/// 0x8408, built here and checked against the appendix in the tests.
const CRC_TABLE: [u16; 256] = {
    let mut table = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u16;
        let mut bit = 0;
        while bit < 8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0x8408
            } else {
                c >> 1
            };
            bit += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
};

/// "WORD ComputeCRC(WORD crc, BYTE byte)" of the specification.
pub(crate) fn crc_update(crc: u16, byte: u8) -> u16 {
    (crc >> 8) ^ CRC_TABLE[usize::from((crc ^ u16::from(byte)) as u8)]
}

pub(crate) fn crc(data: &[u8]) -> u16 {
    data.iter().fold(0xFFFF, |c, &b| crc_update(c, b))
}

/// The CRC over a frame's data and its CRC bytes when the frame is intact.
pub(crate) const CRC_RESIDUE: u16 = 0xF0B8;

fn push_escaped(out: &mut Vec<u8>, b: u8) {
    if b >= INVALID {
        out.push(CE);
        out.push(b ^ XOR);
    } else {
        out.push(b);
    }
}

/// One variant 1 frame around `data`.
pub(crate) fn frame(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 8);
    out.push(BOF);
    for &b in data {
        push_escaped(&mut out, b);
    }
    let c = !crc(data);
    push_escaped(&mut out, (c & 0xFF) as u8);
    push_escaped(&mut out, (c >> 8) as u8);
    out.push(EOF);
    out
}

pub(crate) fn keepalive_request() -> Vec<u8> {
    frame(&[SLOT, MESSAGE_EMBER, COMMAND_KEEPALIVE_REQUEST, VERSION])
}

pub(crate) fn keepalive_response() -> Vec<u8> {
    frame(&[SLOT, MESSAGE_EMBER, COMMAND_KEEPALIVE_RESPONSE, VERSION])
}

fn ember_header(flags: u8) -> Vec<u8> {
    let mut h = vec![
        SLOT,
        MESSAGE_EMBER,
        COMMAND_EMBER,
        VERSION,
        flags,
        DTD_GLOW,
        GLOW_VERSION.len() as u8,
    ];
    h.extend_from_slice(&GLOW_VERSION);
    h
}

/// A BER message as framed EmBER packets: one single packet, or first,
/// middle and last packets of at most [`MAX_PACKET_PAYLOAD`] bytes each,
/// every one with the full header.
pub(crate) fn ember_frames(payload: &[u8]) -> Vec<u8> {
    let chunks: Vec<&[u8]> = if payload.is_empty() {
        vec![payload]
    } else {
        payload.chunks(MAX_PACKET_PAYLOAD).collect()
    };
    let last = chunks.len() - 1;
    let mut out = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let flags = match (i == 0, i == last) {
            (true, true) => FLAG_SINGLE,
            (true, false) => FLAG_FIRST,
            (false, true) => FLAG_LAST,
            (false, false) => 0x00,
        };
        let mut data = ember_header(flags);
        data.extend_from_slice(chunk);
        out.extend(frame(&data));
    }
    out
}

/// A message, from a frame's data.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Message {
    Ember { flags: u8, payload: Vec<u8> },
    KeepAliveRequest,
    KeepAliveResponse,
}

pub(crate) fn parse_message(data: &[u8]) -> Result<Message, String> {
    let [slot, message, command, rest @ ..] = data else {
        return Err(format!("a message of {} bytes", data.len()));
    };
    if *message != MESSAGE_EMBER {
        return Err(format!(
            "message type 0x{message:02X} in slot {slot}, not EmBER"
        ));
    }
    match *command {
        COMMAND_KEEPALIVE_REQUEST => Ok(Message::KeepAliveRequest),
        COMMAND_KEEPALIVE_RESPONSE => Ok(Message::KeepAliveResponse),
        COMMAND_EMBER => {
            let [_version, flags, dtd, app_len, rest @ ..] = rest else {
                return Err("an EmBER packet without its header".into());
            };
            if *dtd != DTD_GLOW {
                return Err(format!("DTD {dtd}, not Glow"));
            }
            let app_len = usize::from(*app_len);
            if rest.len() < app_len {
                return Err("an EmBER packet shorter than its application bytes".into());
            }
            Ok(Message::Ember {
                flags: *flags,
                payload: rest[app_len..].to_vec(),
            })
        }
        other => Err(format!("unknown S101 command 0x{other:02X}")),
    }
}

/// Joins multi-packet EmBER messages.
#[derive(Debug, Default)]
pub(crate) struct Assembler {
    partial: Option<Vec<u8>>,
}

impl Assembler {
    /// The complete BER message, once its last packet arrives. A message's
    /// packets that arrive out of order are dropped, with the reason.
    pub(crate) fn push(&mut self, flags: u8, payload: Vec<u8>) -> Result<Option<Vec<u8>>, String> {
        let flags = flags & 0xE0;
        if flags & FLAG_EMPTY != 0 && payload.is_empty() {
            return Ok(None);
        }
        match flags & FLAG_SINGLE {
            FLAG_SINGLE => {
                // A single packet ends any message left unfinished.
                self.partial = None;
                Ok(Some(payload))
            }
            FLAG_FIRST => {
                let dropped = self.partial.replace(payload).is_some();
                if dropped {
                    return Err("a new multi-packet message began before the last ended; the partial message was dropped".into());
                }
                Ok(None)
            }
            _ => {
                let Some(mut buf) = self.partial.take() else {
                    return Err(
                        "a packet of a multi-packet message without its first packet".into(),
                    );
                };
                buf.extend_from_slice(&payload);
                if buf.len() > MAX_FRAME {
                    return Err(format!("a multi-packet message over {MAX_FRAME} bytes"));
                }
                if flags & FLAG_LAST != 0 {
                    Ok(Some(buf))
                } else {
                    self.partial = Some(buf);
                    Ok(None)
                }
            }
        }
    }
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Outside,
    Escaped {
        escape: bool,
    },
    /// Variant 2: waiting for the count of length bytes.
    LengthCount,
    Length {
        remaining: usize,
        length: u64,
    },
    Payload {
        remaining: usize,
    },
}

/// Splits a byte stream into frames' data, checking each variant 1 CRC.
#[derive(Debug, Default)]
pub(crate) struct Decoder {
    state: State,
    buf: Vec<u8>,
}

impl Decoder {
    /// The data of every complete frame, and why anything was discarded.
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> (Vec<Vec<u8>>, Vec<String>) {
        let mut frames = Vec::new();
        let mut errors = Vec::new();
        for &b in bytes {
            match &mut self.state {
                State::Outside => match b {
                    BOF => {
                        self.buf.clear();
                        self.state = State::Escaped { escape: false };
                    }
                    INVALID => self.state = State::LengthCount,
                    _ => {}
                },
                State::Escaped { escape } => {
                    if b == BOF {
                        // "A BOF byte always indicates the start of a new frame."
                        if !self.buf.is_empty() {
                            errors.push("a frame without its EOF was discarded".into());
                        }
                        self.buf.clear();
                        *escape = false;
                    } else if b == EOF {
                        let data = std::mem::take(&mut self.buf);
                        self.state = State::Outside;
                        if data.len() < 2 {
                            errors.push("a frame too short to hold its CRC".into());
                        } else if crc(&data) != CRC_RESIDUE {
                            errors.push("a frame with a bad CRC was discarded".into());
                        } else {
                            frames.push(data[..data.len() - 2].to_vec());
                        }
                    } else if *escape {
                        *escape = false;
                        self.buf.push(b ^ XOR);
                    } else if b == CE {
                        *escape = true;
                    } else if b >= INVALID {
                        errors.push(format!(
                            "an unescaped 0x{b:02X} inside a frame; the frame was discarded"
                        ));
                        self.buf.clear();
                        self.state = State::Outside;
                    } else {
                        self.buf.push(b);
                    }
                    if self.buf.len() > MAX_FRAME {
                        errors.push(format!("a frame over {MAX_FRAME} bytes was discarded"));
                        self.buf.clear();
                        self.state = State::Outside;
                    }
                }
                State::LengthCount => {
                    let count = usize::from(b & 0x07);
                    self.buf.clear();
                    self.state = if count == 0 {
                        // A payload of no bytes.
                        frames.push(Vec::new());
                        State::Outside
                    } else {
                        State::Length {
                            remaining: count,
                            length: 0,
                        }
                    };
                }
                State::Length { remaining, length } => {
                    *length = (*length << 8) | u64::from(b);
                    *remaining -= 1;
                    if *remaining == 0 {
                        let length = *length;
                        if length == 0 {
                            frames.push(Vec::new());
                            self.state = State::Outside;
                        } else if length > MAX_FRAME as u64 {
                            errors.push(format!("a variant 2 frame of {length} bytes is over {MAX_FRAME}; the stream cannot be resynchronised until the next frame"));
                            self.state = State::Outside;
                        } else {
                            self.state = State::Payload {
                                remaining: length as usize,
                            };
                        }
                    }
                }
                State::Payload { remaining } => {
                    self.buf.push(b);
                    *remaining -= 1;
                    if *remaining == 0 {
                        frames.push(std::mem::take(&mut self.buf));
                        self.state = State::Outside;
                    }
                }
            }
        }
        (frames, errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Appendix, "S101 CRC Table": the first two rows and the last.
    #[test]
    fn crc_table_matches_the_appendix() {
        assert_eq!(
            CRC_TABLE[..16],
            [
                0x0000, 0x1189, 0x2312, 0x329b, 0x4624, 0x57ad, 0x6536, 0x74bf, 0x8c48, 0x9dc1,
                0xaf5a, 0xbed3, 0xca6c, 0xdbe5, 0xe97e, 0xf8f7
            ]
        );
        assert_eq!(
            CRC_TABLE[248..],
            [0x7bc7, 0x6a4e, 0x58d5, 0x495c, 0x3de3, 0x2c6a, 0x1ef1, 0x0f78]
        );
        assert_eq!(CRC_TABLE[128], 0x8408);
    }

    // "Variant 1: Escaping": "The data 0xFF, 0x00, 0xF9, 0x01 would encode to
    // 0xFE, 0xFD, 0xDF, 0x00, 0xFD, 0xD9, 0x01, 0x95, 0x83, 0xFF".
    #[test]
    fn the_escaping_example_encodes_and_decodes() {
        let data = [0xFF, 0x00, 0xF9, 0x01];
        let wire = [0xFE, 0xFD, 0xDF, 0x00, 0xFD, 0xD9, 0x01, 0x95, 0x83, 0xFF];
        assert_eq!(frame(&data), wire);
        // "The result of a decoded CRC must always be 0xF0B8."
        assert_eq!(crc(&[0xFF, 0x00, 0xF9, 0x01, 0x95, 0x83]), CRC_RESIDUE);
        let mut d = Decoder::default();
        let (frames, errors) = d.feed(&wire);
        assert_eq!(frames, vec![data.to_vec()]);
        assert!(errors.is_empty());
    }

    #[test]
    fn crc_bytes_are_escaped_and_corruption_is_caught() {
        // Find data whose CRC has a byte of 0xF8 or above, so it is escaped.
        let data = (0u8..=255)
            .map(|b| vec![b, 0x10])
            .find(|d| {
                let c = !crc(d);
                (c & 0xFF) as u8 >= INVALID || (c >> 8) as u8 >= INVALID
            })
            .unwrap();
        let wire = frame(&data);
        assert!(wire.contains(&CE));
        let mut d = Decoder::default();
        assert_eq!(d.feed(&wire).0, vec![data.clone()]);
        let mut bad = wire.clone();
        bad[1] ^= 0x01;
        let (frames, errors) = d.feed(&bad);
        assert!(frames.is_empty());
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn frames_split_across_reads_and_garbage_between_them() {
        let a = frame(&[1, 2, 3]);
        let b = frame(&[0xFE, 0xFF, 0xFD]);
        let mut stream = vec![0x00, 0x42];
        stream.extend(&a);
        stream.extend([0x10]);
        stream.extend(&b);
        let mut d = Decoder::default();
        let mut got = Vec::new();
        for chunk in stream.chunks(3) {
            got.extend(d.feed(chunk).0);
        }
        assert_eq!(got, vec![vec![1, 2, 3], vec![0xFE, 0xFF, 0xFD]]);
        // A BOF inside a frame restarts it.
        let mut restarted = vec![BOF, 1, 2];
        restarted.extend(&a);
        let (frames, errors) = d.feed(&restarted);
        assert_eq!(frames, vec![vec![1, 2, 3]]);
        assert_eq!(errors.len(), 1);
    }

    // "Variant 2: Non-Escaping": 0xF8, 0x04, 0x00, 0x00, 0x00, 0x0A, then ten
    // payload bytes.
    #[test]
    fn variant_two_frames_decode() {
        let mut wire = vec![0xF8, 0x04, 0x00, 0x00, 0x00, 0x0A];
        let payload: Vec<u8> = (0xF6..=0xFF).collect();
        wire.extend(&payload);
        wire.extend(frame(&[9]));
        let mut d = Decoder::default();
        assert_eq!(d.feed(&wire).0, vec![payload, vec![9]]);
    }

    // "S101 Messages": keep-alive request and response carry slot, message
    // type, command and version only.
    #[test]
    fn keepalives() {
        let mut d = Decoder::default();
        let (frames, _) = d.feed(&keepalive_request());
        assert_eq!(frames, vec![vec![0x00, 0x0E, 0x01, 0x01]]);
        assert_eq!(parse_message(&frames[0]), Ok(Message::KeepAliveRequest));
        let (frames, _) = d.feed(&keepalive_response());
        assert_eq!(parse_message(&frames[0]), Ok(Message::KeepAliveResponse));
    }

    // "Usage": the header of a single packet message.
    #[test]
    fn ember_packet_header_and_multi_packet_messages() {
        let mut d = Decoder::default();
        let (frames, _) = d.feed(&ember_frames(&[0x60, 0x00]));
        assert_eq!(
            frames[0],
            vec![0x00, 0x0E, 0x00, 0x01, 0xC0, 0x01, 0x02, 50, 0x02, 0x60, 0x00]
        );
        // The specification's own example header, DTD 2.5.
        let example = [0x00, 0x0E, 0x00, 0x01, 0xC0, 0x01, 0x02, 0x05, 0x02, 0xAA];
        assert_eq!(
            parse_message(&example),
            Ok(Message::Ember {
                flags: 0xC0,
                payload: vec![0xAA]
            })
        );

        let big: Vec<u8> = (0..2500u32).map(|i| (i % 251) as u8).collect();
        let (frames, errors) = d.feed(&ember_frames(&big));
        assert!(errors.is_empty());
        assert_eq!(frames.len(), 3);
        let mut asm = Assembler::default();
        let mut out = None;
        let mut flags_seen = Vec::new();
        for f in frames {
            let Message::Ember { flags, payload } = parse_message(&f).unwrap() else {
                panic!()
            };
            assert!(payload.len() <= MAX_PACKET_PAYLOAD);
            flags_seen.push(flags);
            if let Some(m) = asm.push(flags, payload).unwrap() {
                out = Some(m);
            }
        }
        assert_eq!(flags_seen, vec![FLAG_FIRST, 0x00, FLAG_LAST]);
        assert_eq!(out, Some(big));
        // A middle packet without its first is refused.
        assert!(asm.push(0x00, vec![1]).is_err());
        assert_eq!(asm.push(FLAG_EMPTY, vec![]), Ok(None));
    }
}
