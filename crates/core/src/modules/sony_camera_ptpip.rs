//! PTP-IP framing (ISO 15740 / CIPA DC-005) as Sony's Camera Control PTP 3
//! uses it: two TCP connections to the camera, one for commands and one for
//! events, each carrying length-prefixed packets.
//!
//! Every packet starts with a little-endian `u32` length that counts the whole
//! packet, then a `u32` packet type. Strings are UTF-16LE ending in a NUL
//! code unit.

/// The PTP-IP protocol version an initiator announces: 1.0.
pub(crate) const PROTOCOL_VERSION: u32 = 0x0001_0000;

/// A packet larger than this is treated as a broken stream. Downloads ask
/// for 4 MiB at a time, but a captured image comes in one GetObject whose
/// data a camera may send as a single packet the size of a RAW file.
const MAX_PACKET: usize = 256 * 1024 * 1024;

const INIT_COMMAND_REQUEST: u32 = 1;
const INIT_COMMAND_ACK: u32 = 2;
const INIT_EVENT_REQUEST: u32 = 3;
const INIT_EVENT_ACK: u32 = 4;
const INIT_FAIL: u32 = 5;
const OPERATION_REQUEST: u32 = 6;
const OPERATION_RESPONSE: u32 = 7;
const EVENT: u32 = 8;
const START_DATA: u32 = 9;
const DATA: u32 = 10;
const CANCEL: u32 = 11;
const END_DATA: u32 = 12;
const PROBE_REQUEST: u32 = 13;
const PROBE_RESPONSE: u32 = 14;

/// The data phase an operation request announces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DataPhase {
    /// No data, or data from the camera to the initiator.
    NoneOrIn = 1,
    /// Data from the initiator to the camera.
    Out = 2,
}

/// Why the camera refused an Init request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailReason {
    /// The camera does not accept this initiator (unpaired, or refused).
    Rejected,
    /// Another initiator holds the camera.
    Busy,
    Other(u32),
}

impl FailReason {
    fn from_u32(v: u32) -> FailReason {
        match v {
            1 => FailReason::Rejected,
            2 => FailReason::Busy,
            other => FailReason::Other(other),
        }
    }

    fn to_u32(self) -> u32 {
        match self {
            FailReason::Rejected => 1,
            FailReason::Busy => 2,
            FailReason::Other(v) => v,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Packet {
    InitCommandRequest {
        guid: [u8; 16],
        name: String,
        version: u32,
    },
    InitCommandAck {
        connection: u32,
        guid: [u8; 16],
        name: String,
        version: u32,
    },
    InitEventRequest {
        connection: u32,
    },
    InitEventAck,
    InitFail {
        reason: FailReason,
    },
    OperationRequest {
        phase: DataPhase,
        code: u16,
        transaction: u32,
        params: Vec<u32>,
    },
    OperationResponse {
        code: u16,
        transaction: u32,
        params: Vec<u32>,
    },
    Event {
        code: u16,
        transaction: u32,
        params: Vec<u32>,
    },
    StartData {
        transaction: u32,
        total: u64,
    },
    Data {
        transaction: u32,
        payload: Vec<u8>,
    },
    EndData {
        transaction: u32,
        payload: Vec<u8>,
    },
    Cancel {
        transaction: u32,
    },
    ProbeRequest,
    ProbeResponse,
    /// A packet type this module does not use, kept so the stream stays in step.
    Other {
        kind: u32,
    },
}

pub(crate) fn utf16z(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2 + 2);
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out.extend_from_slice(&[0, 0]);
    out
}

/// Reads a NUL-terminated UTF-16LE string; returns it and the bytes used.
fn read_utf16z(bytes: &[u8]) -> Result<(String, usize), String> {
    let mut units = Vec::new();
    let mut at = 0;
    loop {
        let pair = bytes.get(at..at + 2).ok_or("unterminated UTF-16 string")?;
        at += 2;
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    Ok((String::from_utf16_lossy(&units), at))
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, String> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| "packet too short".to_string())
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| "packet too short".to_string())
}

/// Up to five `u32` parameters following a fixed header.
fn params_from(b: &[u8]) -> Vec<u32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .take(5)
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

fn frame(kind: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&((8 + body.len()) as u32).to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(body);
    out
}

impl Packet {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut b = Vec::new();
        let kind = match self {
            Packet::InitCommandRequest {
                guid,
                name,
                version,
            } => {
                b.extend_from_slice(guid);
                b.extend_from_slice(&utf16z(name));
                b.extend_from_slice(&version.to_le_bytes());
                INIT_COMMAND_REQUEST
            }
            Packet::InitCommandAck {
                connection,
                guid,
                name,
                version,
            } => {
                b.extend_from_slice(&connection.to_le_bytes());
                b.extend_from_slice(guid);
                b.extend_from_slice(&utf16z(name));
                b.extend_from_slice(&version.to_le_bytes());
                INIT_COMMAND_ACK
            }
            Packet::InitEventRequest { connection } => {
                b.extend_from_slice(&connection.to_le_bytes());
                INIT_EVENT_REQUEST
            }
            Packet::InitEventAck => INIT_EVENT_ACK,
            Packet::InitFail { reason } => {
                b.extend_from_slice(&reason.to_u32().to_le_bytes());
                INIT_FAIL
            }
            Packet::OperationRequest {
                phase,
                code,
                transaction,
                params,
            } => {
                b.extend_from_slice(&(*phase as u32).to_le_bytes());
                b.extend_from_slice(&code.to_le_bytes());
                b.extend_from_slice(&transaction.to_le_bytes());
                for p in params.iter().take(5) {
                    b.extend_from_slice(&p.to_le_bytes());
                }
                OPERATION_REQUEST
            }
            Packet::OperationResponse {
                code,
                transaction,
                params,
            } => {
                b.extend_from_slice(&code.to_le_bytes());
                b.extend_from_slice(&transaction.to_le_bytes());
                for p in params.iter().take(5) {
                    b.extend_from_slice(&p.to_le_bytes());
                }
                OPERATION_RESPONSE
            }
            Packet::Event {
                code,
                transaction,
                params,
            } => {
                b.extend_from_slice(&code.to_le_bytes());
                b.extend_from_slice(&transaction.to_le_bytes());
                for p in params.iter().take(3) {
                    b.extend_from_slice(&p.to_le_bytes());
                }
                EVENT
            }
            Packet::StartData { transaction, total } => {
                b.extend_from_slice(&transaction.to_le_bytes());
                b.extend_from_slice(&total.to_le_bytes());
                START_DATA
            }
            Packet::Data {
                transaction,
                payload,
            } => {
                b.extend_from_slice(&transaction.to_le_bytes());
                b.extend_from_slice(payload);
                DATA
            }
            Packet::EndData {
                transaction,
                payload,
            } => {
                b.extend_from_slice(&transaction.to_le_bytes());
                b.extend_from_slice(payload);
                END_DATA
            }
            Packet::Cancel { transaction } => {
                b.extend_from_slice(&transaction.to_le_bytes());
                CANCEL
            }
            Packet::ProbeRequest => PROBE_REQUEST,
            Packet::ProbeResponse => PROBE_RESPONSE,
            Packet::Other { kind } => *kind,
        };
        frame(kind, &b)
    }

    /// Decodes one packet's body, given its type.
    pub(crate) fn decode(kind: u32, b: &[u8]) -> Result<Packet, String> {
        Ok(match kind {
            INIT_COMMAND_REQUEST => {
                let guid: [u8; 16] = b.get(..16).ok_or("packet too short")?.try_into().unwrap();
                let (name, used) = read_utf16z(&b[16..])?;
                let version = u32_at(b, 16 + used)?;
                Packet::InitCommandRequest {
                    guid,
                    name,
                    version,
                }
            }
            INIT_COMMAND_ACK => {
                let connection = u32_at(b, 0)?;
                let guid: [u8; 16] = b.get(4..20).ok_or("packet too short")?.try_into().unwrap();
                let (name, used) = read_utf16z(&b[20..])?;
                // Some responders leave the version out; it carries nothing
                // this module acts on.
                let version = u32_at(b, 20 + used).unwrap_or(0);
                Packet::InitCommandAck {
                    connection,
                    guid,
                    name,
                    version,
                }
            }
            INIT_EVENT_REQUEST => Packet::InitEventRequest {
                connection: u32_at(b, 0)?,
            },
            INIT_EVENT_ACK => Packet::InitEventAck,
            INIT_FAIL => Packet::InitFail {
                reason: FailReason::from_u32(u32_at(b, 0).unwrap_or(0)),
            },
            OPERATION_REQUEST => {
                let phase = match u32_at(b, 0)? {
                    2 => DataPhase::Out,
                    _ => DataPhase::NoneOrIn,
                };
                Packet::OperationRequest {
                    phase,
                    code: u16_at(b, 4)?,
                    transaction: u32_at(b, 6)?,
                    params: params_from(&b[10..]),
                }
            }
            OPERATION_RESPONSE => Packet::OperationResponse {
                code: u16_at(b, 0)?,
                transaction: u32_at(b, 2)?,
                params: params_from(&b[6..]),
            },
            EVENT => Packet::Event {
                code: u16_at(b, 0)?,
                transaction: u32_at(b, 2)?,
                params: params_from(&b[6..]),
            },
            START_DATA => {
                let transaction = u32_at(b, 0)?;
                let lo = u32_at(b, 4)? as u64;
                let hi = u32_at(b, 8)? as u64;
                Packet::StartData {
                    transaction,
                    total: lo | (hi << 32),
                }
            }
            DATA => Packet::Data {
                transaction: u32_at(b, 0)?,
                payload: b[4..].to_vec(),
            },
            END_DATA => Packet::EndData {
                transaction: u32_at(b, 0)?,
                payload: b[4..].to_vec(),
            },
            CANCEL => Packet::Cancel {
                transaction: u32_at(b, 0)?,
            },
            PROBE_REQUEST => Packet::ProbeRequest,
            PROBE_RESPONSE => Packet::ProbeResponse,
            other => Packet::Other { kind: other },
        })
    }
}

/// Splits a TCP byte stream into packets.
#[derive(Default)]
pub(crate) struct Framer {
    buffer: Vec<u8>,
}

impl Framer {
    /// Every whole packet now available. An error means the stream cannot be
    /// trusted any more and the connection should be dropped.
    pub(crate) fn feed(&mut self, data: &[u8]) -> Result<Vec<Packet>, String> {
        self.buffer.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            if self.buffer.len() < 8 {
                break;
            }
            let length = u32_at(&self.buffer, 0)? as usize;
            if !(8..=MAX_PACKET).contains(&length) {
                return Err(format!("PTP-IP packet length {length} is not valid"));
            }
            if self.buffer.len() < length {
                break;
            }
            let kind = u32_at(&self.buffer, 4)?;
            let packet = Packet::decode(kind, &self.buffer[8..length])
                .map_err(|e| format!("malformed PTP-IP packet type {kind}: {e}"))?;
            self.buffer.drain(..length);
            out.push(packet);
        }
        Ok(out)
    }
}

/// The packets for an operation with a data-out phase: the request, the
/// announced length, and the payload in one End Data packet.
pub(crate) fn request_with_data(
    code: u16,
    transaction: u32,
    params: &[u32],
    payload: &[u8],
) -> Vec<u8> {
    let mut out = Packet::OperationRequest {
        phase: DataPhase::Out,
        code,
        transaction,
        params: params.to_vec(),
    }
    .encode();
    out.extend(
        Packet::StartData {
            transaction,
            total: payload.len() as u64,
        }
        .encode(),
    );
    out.extend(
        Packet::EndData {
            transaction,
            payload: payload.to_vec(),
        }
        .encode(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_command_request_layout() {
        let guid = [0x11u8; 16];
        let bytes = Packet::InitCommandRequest {
            guid,
            name: "Ab".into(),
            version: PROTOCOL_VERSION,
        }
        .encode();
        let mut expected = vec![34, 0, 0, 0, 1, 0, 0, 0];
        expected.extend_from_slice(&guid);
        expected.extend_from_slice(&[b'A', 0, b'b', 0, 0, 0]);
        expected.extend_from_slice(&[0, 0, 1, 0]);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn operation_request_layout() {
        let bytes = Packet::OperationRequest {
            phase: DataPhase::NoneOrIn,
            code: 0x9201,
            transaction: 7,
            params: vec![1, 0, 0],
        }
        .encode();
        assert_eq!(
            bytes,
            [
                30, 0, 0, 0, 6, 0, 0, 0, // length, type
                1, 0, 0, 0, // data phase
                0x01, 0x92, // code
                7, 0, 0, 0, // transaction
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]
        );
    }

    #[test]
    fn every_packet_round_trips() {
        let packets = vec![
            Packet::InitCommandRequest {
                guid: [3; 16],
                name: "Meros".into(),
                version: PROTOCOL_VERSION,
            },
            Packet::InitCommandAck {
                connection: 9,
                guid: [4; 16],
                name: "ILCE-1".into(),
                version: PROTOCOL_VERSION,
            },
            Packet::InitEventRequest { connection: 9 },
            Packet::InitEventAck,
            Packet::InitFail {
                reason: FailReason::Busy,
            },
            Packet::OperationRequest {
                phase: DataPhase::Out,
                code: 0x9205,
                transaction: 2,
                params: vec![0x5005, 1],
            },
            Packet::OperationResponse {
                code: 0x2001,
                transaction: 2,
                params: vec![0x12C],
            },
            Packet::Event {
                code: 0xC203,
                transaction: 0,
                params: vec![],
            },
            Packet::StartData {
                transaction: 5,
                total: 0x1_0000_0002,
            },
            Packet::Data {
                transaction: 5,
                payload: vec![1, 2],
            },
            Packet::EndData {
                transaction: 5,
                payload: vec![3],
            },
            Packet::Cancel { transaction: 5 },
            Packet::ProbeRequest,
            Packet::ProbeResponse,
        ];
        let mut stream = Vec::new();
        for p in &packets {
            stream.extend(p.encode());
        }
        // Fed one byte at a time, the framer still finds every packet.
        let mut framer = Framer::default();
        let mut out = Vec::new();
        for byte in &stream {
            out.extend(framer.feed(&[*byte]).unwrap());
        }
        assert_eq!(out, packets);
    }

    #[test]
    fn a_bad_length_is_an_error() {
        let mut framer = Framer::default();
        assert!(framer.feed(&[4, 0, 0, 0, 1, 0, 0, 0]).is_err());
    }

    #[test]
    fn data_out_is_request_start_and_end() {
        let bytes = request_with_data(0x9207, 3, &[0xD2C1, 0], &[2, 0]);
        let mut framer = Framer::default();
        let packets = framer.feed(&bytes).unwrap();
        assert_eq!(
            packets,
            [
                Packet::OperationRequest {
                    phase: DataPhase::Out,
                    code: 0x9207,
                    transaction: 3,
                    params: vec![0xD2C1, 0]
                },
                Packet::StartData {
                    transaction: 3,
                    total: 2
                },
                Packet::EndData {
                    transaction: 3,
                    payload: vec![2, 0]
                },
            ]
        );
    }

    #[test]
    fn an_ack_without_version_still_decodes() {
        let mut body = vec![1, 0, 0, 0];
        body.extend_from_slice(&[0u8; 16]);
        body.extend(utf16z("X"));
        let p = Packet::decode(2, &body).unwrap();
        assert!(
            matches!(p, Packet::InitCommandAck { connection: 1, ref name, version: 0, .. } if name == "X")
        );
    }
}
