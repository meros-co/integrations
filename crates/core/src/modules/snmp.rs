//! SNMP for native modules: the messages of SNMPv1 (RFC 1157), SNMPv2c
//! (RFC 1901, RFC 3416) and SNMPv3 (RFC 3412 with the User-based Security
//! Model, RFC 3414; snmp_usm.rs), a manager that sends requests to one
//! agent ([`Client`]), and a receiver for the traps and informs agents send
//! ([`Receiver`]).
//!
//! Both are protocol logic only, as every module is: they ask the session
//! for sockets, datagrams and timers through [`Cx`], so any module can embed
//! them (a generic SNMP integration, a PDU or UPS, a network switch, an
//! audio router's SNMP agent). No MIB files are read: objects are named by
//! OID, and values carry their SNMP type.
//!
//! The client:
//! - matches each response to its request by request-id (v1, v2c) or msgID
//!   (v3), resends after `timeout` up to `retries` times, and fails the
//!   request with [`SnmpError::Timeout`] after the last;
//! - for v3, first discovers the agent's snmpEngineID, engineBoots and
//!   engineTime (RFC 3414 §4) with an empty, unauthenticated request, holds
//!   requests until it knows them, keeps the engine's time running locally,
//!   and resends once on a notInTimeWindow report (RFC 3414 §3.2 step 7);
//! - walks a subtree with GETNEXT or GETBULK until the agent answers past
//!   it, with endOfMibView, or (v1) noSuchName, and refuses an agent that
//!   answers an OID not after the one asked for (RFC 3416 §4.2.2 and §4.2.3
//!   make the order lexicographic).

use std::collections::{BTreeMap, VecDeque};
use std::net::{IpAddr, SocketAddr};

use super::snmp_ber::{self as ber, Oid, Reader, Value};
use super::snmp_usm::{Auth, Privacy};
use crate::module::{Bind, Cx, Key, Millis};

const GET: u8 = 0xA0;
const GET_NEXT: u8 = 0xA1;
const RESPONSE: u8 = 0xA2;
const SET: u8 = 0xA3;
const TRAP_V1: u8 = 0xA4;
const GET_BULK: u8 = 0xA5;
const INFORM: u8 = 0xA6;
const TRAP_V2: u8 = 0xA7;
const REPORT: u8 = 0xA8;

/// msgFlags (RFC 3412 §6.4).
const FLAG_AUTH: u8 = 0x01;
const FLAG_PRIV: u8 = 0x02;
const FLAG_REPORTABLE: u8 = 0x04;

/// The USM security model (RFC 3411 §5, SnmpSecurityModel).
const USM: i64 = 3;

/// The largest message this end accepts (msgMaxSize): a UDP datagram's
/// payload over IPv4.
const MAX_SIZE: i64 = 65_507;

/// sysUpTime.0 and snmpTrapOID.0 (RFC 3418), the first two variable
/// bindings of every v2 notification (RFC 3416 §4.2.6).
fn sys_up_time() -> Oid {
    Oid(vec![1, 3, 6, 1, 2, 1, 1, 3, 0])
}
fn snmp_trap_oid() -> Oid {
    Oid(vec![1, 3, 6, 1, 6, 3, 1, 1, 4, 1, 0])
}

/// usmStats counters (RFC 3414 §5), reported in place of a response.
const USM_STATS: [(u32, &str); 6] = [
    (1, "unsupportedSecLevels"),
    (2, "notInTimeWindows"),
    (3, "unknownUserNames"),
    (4, "unknownEngineIDs"),
    (5, "wrongDigests"),
    (6, "decryptionErrors"),
];
const USM_STATS_PREFIX: [u32; 9] = [1, 3, 6, 1, 6, 3, 15, 1, 1];

/// Names of other counters an agent reports (RFC 3412 §5, RFC 3413 §4).
fn report_name(oid: &Oid) -> String {
    let a = &oid.0;
    if a.len() == 11 && a[..9] == USM_STATS_PREFIX {
        if let Some((_, name)) = USM_STATS.iter().find(|(n, _)| *n == a[9]) {
            return format!("usmStats{}{}", name[..1].to_uppercase(), &name[1..]);
        }
    }
    let known: [(&[u32], &str); 4] = [
        (
            &[1, 3, 6, 1, 6, 3, 11, 2, 1, 1, 0],
            "snmpUnknownSecurityModels",
        ),
        (&[1, 3, 6, 1, 6, 3, 11, 2, 1, 2, 0], "snmpInvalidMsgs"),
        (
            &[1, 3, 6, 1, 6, 3, 11, 2, 1, 3, 0],
            "snmpUnknownPDUHandlers",
        ),
        (&[1, 3, 6, 1, 6, 3, 12, 1, 5, 0], "snmpUnknownContexts"),
    ];
    for (k, name) in known {
        if a == k {
            return name.into();
        }
    }
    if *a == [1, 3, 6, 1, 6, 3, 12, 1, 4, 0] {
        return "snmpUnavailableContexts".into();
    }
    oid.to_string()
}

/// The error-status names of RFC 3416 §3.
pub(crate) fn status_name(status: i64) -> String {
    const NAMES: [&str; 19] = [
        "noError",
        "tooBig",
        "noSuchName",
        "badValue",
        "readOnly",
        "genErr",
        "noAccess",
        "wrongType",
        "wrongLength",
        "wrongEncoding",
        "wrongValue",
        "noCreation",
        "inconsistentValue",
        "resourceUnavailable",
        "commitFailed",
        "undoFailed",
        "authorizationError",
        "notWritable",
        "inconsistentName",
    ];
    usize::try_from(status)
        .ok()
        .and_then(|i| NAMES.get(i))
        .map_or_else(|| format!("error {status}"), |n| (*n).to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Version {
    V1,
    V2c,
    V3,
}

impl Version {
    fn number(self) -> i64 {
        match self {
            Version::V1 => 0,
            Version::V2c => 1,
            Version::V3 => 3,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Version::V1 => "v1",
            Version::V2c => "v2c",
            Version::V3 => "v3",
        }
    }
}

/// One variable binding.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VarBind {
    pub(crate) oid: Oid,
    pub(crate) value: Value,
}

/// A PDU of any kind but SNMPv1's trap. For GETBULK, `error_status` and
/// `error_index` are non-repeaters and max-repetitions (RFC 3416 §3).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pdu {
    pub(crate) tag: u8,
    pub(crate) request_id: i32,
    pub(crate) error_status: i64,
    pub(crate) error_index: i64,
    pub(crate) varbinds: Vec<VarBind>,
}

/// SNMPv1's Trap-PDU (RFC 1157 §4.1.6).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TrapV1 {
    pub(crate) enterprise: Oid,
    pub(crate) agent: [u8; 4],
    pub(crate) generic: i64,
    pub(crate) specific: i64,
    pub(crate) timestamp: u32,
    pub(crate) varbinds: Vec<VarBind>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AnyPdu {
    Pdu(Pdu),
    TrapV1(TrapV1),
}

fn encode_varbinds(out: &mut Vec<u8>, varbinds: &[VarBind]) {
    ber::wrap(out, ber::SEQUENCE, |out| {
        for vb in varbinds {
            ber::wrap(out, ber::SEQUENCE, |out| {
                ber::oid(out, &vb.oid);
                vb.value.encode(out);
            });
        }
    });
}

impl Pdu {
    pub(crate) fn encode(&self, out: &mut Vec<u8>) {
        ber::wrap(out, self.tag, |out| {
            ber::integer(out, i64::from(self.request_id));
            ber::integer(out, self.error_status);
            ber::integer(out, self.error_index);
            encode_varbinds(out, &self.varbinds);
        });
    }
}

fn decode_varbinds(r: &mut Reader) -> Result<Vec<VarBind>, String> {
    let mut list = r.enter(ber::SEQUENCE)?;
    let mut out = Vec::new();
    while !list.is_empty() {
        let mut vb = list.enter(ber::SEQUENCE)?;
        let oid = vb.oid()?;
        let value = vb.value()?;
        out.push(VarBind { oid, value });
    }
    Ok(out)
}

fn decode_pdu(r: &mut Reader) -> Result<AnyPdu, String> {
    let (tag, _, content) = r.next()?;
    let mut c = Reader::new(content);
    match tag {
        TRAP_V1 => {
            let enterprise = c.oid()?;
            let (atag, _, addr) = c.next()?;
            if atag != 0x40 || addr.len() != 4 {
                return Err("a v1 trap's agent-addr is not an IpAddress".into());
            }
            let generic = c.integer()?;
            let specific = c.integer()?;
            let timestamp = match c.value()? {
                Value::TimeTicks(t) => t,
                _ => return Err("a v1 trap's time-stamp is not TimeTicks".into()),
            };
            let varbinds = decode_varbinds(&mut c)?;
            Ok(AnyPdu::TrapV1(TrapV1 {
                enterprise,
                agent: addr.try_into().expect("4 bytes"),
                generic,
                specific,
                timestamp,
                varbinds,
            }))
        }
        GET | GET_NEXT | RESPONSE | SET | GET_BULK | INFORM | TRAP_V2 | REPORT => {
            let request_id = c.integer()?;
            let error_status = c.integer()?;
            let error_index = c.integer()?;
            let varbinds = decode_varbinds(&mut c)?;
            Ok(AnyPdu::Pdu(Pdu {
                tag,
                request_id: request_id as i32,
                error_status,
                error_index,
                varbinds,
            }))
        }
        other => Err(format!("unknown PDU type 0x{other:02x}")),
    }
}

/// msgSecurityParameters of the USM (RFC 3414 §2.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct UsmParams {
    pub(crate) engine_id: Vec<u8>,
    pub(crate) boots: u32,
    pub(crate) time: u32,
    pub(crate) user: Vec<u8>,
    pub(crate) auth: Vec<u8>,
    pub(crate) privacy: Vec<u8>,
}

/// A message as received.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Message {
    Community {
        version: Version,
        community: Vec<u8>,
        pdu: AnyPdu,
    },
    V3(V3Message),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct V3Message {
    pub(crate) msg_id: i32,
    pub(crate) max_size: i64,
    pub(crate) flags: u8,
    pub(crate) usm: UsmParams,
    /// Where msgAuthenticationParameters' content lies in the message.
    pub(crate) auth_at: usize,
    /// The scopedPDU: in the clear, or encrypted.
    pub(crate) scoped: Scoped,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Scoped {
    Plain(Vec<u8>),
    Encrypted(Vec<u8>),
}

/// A scopedPDU's parts (RFC 3412 §6).
pub(crate) struct ScopedPdu {
    pub(crate) context_engine_id: Vec<u8>,
    pub(crate) context_name: Vec<u8>,
    pub(crate) pdu: AnyPdu,
}

pub(crate) fn decode_scoped(bytes: &[u8]) -> Result<ScopedPdu, String> {
    let mut r = Reader::new(bytes);
    let mut s = r.enter(ber::SEQUENCE)?;
    let context_engine_id = s.octets()?.to_vec();
    let context_name = s.octets()?.to_vec();
    let pdu = decode_pdu(&mut s)?;
    Ok(ScopedPdu {
        context_engine_id,
        context_name,
        pdu,
    })
}

fn encode_scoped(context_engine_id: &[u8], context_name: &[u8], pdu: &Pdu) -> Vec<u8> {
    let mut out = Vec::new();
    ber::wrap(&mut out, ber::SEQUENCE, |out| {
        ber::octets(out, context_engine_id);
        ber::octets(out, context_name);
        pdu.encode(out);
    });
    out
}

pub(crate) fn decode(data: &[u8]) -> Result<Message, String> {
    let mut r = Reader::new(data);
    let mut m = r.enter(ber::SEQUENCE)?;
    let version = match m.integer()? {
        0 => Version::V1,
        1 => Version::V2c,
        3 => Version::V3,
        other => return Err(format!("unknown SNMP version {other}")),
    };
    if version != Version::V3 {
        let community = m.octets()?.to_vec();
        let pdu = decode_pdu(&mut m)?;
        return Ok(Message::Community {
            version,
            community,
            pdu,
        });
    }
    let mut header = m.enter(ber::SEQUENCE)?;
    let msg_id = header.integer()?;
    let max_size = header.integer()?;
    let flags = *header.octets()?.first().ok_or("empty msgFlags")?;
    let model = header.integer()?;
    if model != USM {
        return Err(format!("security model {model} is not the USM"));
    }
    let (params_at, params) = m.expect(ber::OCTET_STRING)?;
    // The USM parameters are a SEQUENCE inside the OCTET STRING; read it
    // within the whole message so positions are the message's.
    let mut sp = Reader::within(data, params_at, params_at + params.len());
    let mut u = sp.enter(ber::SEQUENCE)?;
    let engine_id = u.octets()?.to_vec();
    let boots = u.integer()?.clamp(0, i64::from(u32::MAX)) as u32;
    let time = u.integer()?.clamp(0, i64::from(u32::MAX)) as u32;
    let user = u.octets()?.to_vec();
    let (auth_at, auth) = u.expect(ber::OCTET_STRING)?;
    let privacy = u.octets()?.to_vec();
    let scoped = match m.peek_tag() {
        Some(ber::OCTET_STRING) => Scoped::Encrypted(m.octets()?.to_vec()),
        Some(ber::SEQUENCE) => {
            let (_, content) = m.expect(ber::SEQUENCE)?;
            let mut whole = Vec::new();
            ber::tlv(&mut whole, ber::SEQUENCE, content);
            Scoped::Plain(whole)
        }
        _ => return Err("no scopedPDU".into()),
    };
    Ok(Message::V3(V3Message {
        msg_id: msg_id as i32,
        max_size,
        flags,
        usm: UsmParams {
            engine_id,
            boots,
            time,
            user,
            auth: auth.to_vec(),
            privacy,
        },
        auth_at,
        scoped,
    }))
}

/// How many bytes a tag and a length of `len` take.
fn header_len(len: usize) -> usize {
    if len < 0x80 {
        2
    } else {
        2 + (usize::BITS - len.leading_zeros()).div_ceil(8) as usize
    }
}

/// A v1 or v2c message.
pub(crate) fn encode_community(version: Version, community: &[u8], pdu: &Pdu) -> Vec<u8> {
    let mut out = Vec::new();
    ber::wrap(&mut out, ber::SEQUENCE, |out| {
        ber::integer(out, version.number());
        ber::octets(out, community);
        pdu.encode(out);
    });
    out
}

/// A v1 Trap-PDU message, for tests and agents built on this layer.
#[cfg(test)]
pub(crate) fn encode_trap_v1(community: &[u8], trap: &TrapV1) -> Vec<u8> {
    let mut out = Vec::new();
    ber::wrap(&mut out, ber::SEQUENCE, |out| {
        ber::integer(out, 0);
        ber::octets(out, community);
        ber::wrap(out, TRAP_V1, |out| {
            ber::oid(out, &trap.enterprise);
            ber::tlv(out, 0x40, &trap.agent);
            ber::integer(out, trap.generic);
            ber::integer(out, trap.specific);
            Value::TimeTicks(trap.timestamp).encode(out);
            encode_varbinds(out, &trap.varbinds);
        });
    });
    out
}

/// A user's keys, localized to one engine.
#[derive(Clone)]
pub(crate) struct Keys {
    auth: Option<(Auth, Vec<u8>)>,
    privacy: Option<(Privacy, Vec<u8>)>,
}

/// An SNMPv3 user (RFC 3414 §2.1) and its password-derived keys, which
/// are worked out once (each takes a 1 MiB hash) and then localized to
/// each engine met. Never printed: the keys are as good as the passwords.
#[derive(Clone)]
pub(crate) struct User {
    pub(crate) name: Vec<u8>,
    auth: Option<(Auth, Vec<u8>)>,
    privacy: Option<(Privacy, Vec<u8>)>,
    localized: BTreeMap<Vec<u8>, Keys>,
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("name", &String::from_utf8_lossy(&self.name))
            .field("auth", &self.auth.as_ref().map(|a| a.0))
            .field("privacy", &self.privacy.as_ref().map(|p| p.0))
            .finish_non_exhaustive()
    }
}

/// The security level a user is used at (RFC 3411 §3.4.3).
// The names are RFC 3411's.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    NoAuthNoPriv,
    AuthNoPriv,
    AuthPriv,
}

impl User {
    /// A user at a level: authentication with `auth` from AuthNoPriv up,
    /// privacy with `privacy` at AuthPriv. RFC 3414 §11.2 asks for
    /// passwords of at least 8 characters, so shorter ones are refused.
    pub(crate) fn new(
        name: &str,
        level: Level,
        auth: Option<(Auth, &str)>,
        privacy: Option<(Privacy, &str)>,
    ) -> Result<User, String> {
        let short = |what: &str| {
            format!("the {what} password must be at least 8 characters (RFC 3414 §11.2)")
        };
        let auth = if level == Level::NoAuthNoPriv {
            None
        } else {
            let (proto, password) =
                auth.ok_or("an authentication protocol and password are needed")?;
            if password.chars().count() < 8 {
                return Err(short("authentication"));
            }
            Some((proto, proto.password_key(password.as_bytes())))
        };
        let privacy = if level == Level::AuthPriv {
            let (proto, password) = privacy.ok_or("a privacy protocol and password are needed")?;
            if password.chars().count() < 8 {
                return Err(short("privacy"));
            }
            let auth_proto = auth.as_ref().expect("auth at AuthPriv").0;
            Some((proto, auth_proto.password_key(password.as_bytes())))
        } else {
            None
        };
        Ok(User {
            name: name.as_bytes().to_vec(),
            auth,
            privacy,
            localized: BTreeMap::new(),
        })
    }

    pub(crate) fn level(&self) -> Level {
        match (&self.auth, &self.privacy) {
            (None, _) => Level::NoAuthNoPriv,
            (Some(_), None) => Level::AuthNoPriv,
            (Some(_), Some(_)) => Level::AuthPriv,
        }
    }

    fn flags(&self) -> u8 {
        match self.level() {
            Level::NoAuthNoPriv => 0,
            Level::AuthNoPriv => FLAG_AUTH,
            Level::AuthPriv => FLAG_AUTH | FLAG_PRIV,
        }
    }

    fn keys(&mut self, engine_id: &[u8]) -> Keys {
        if let Some(k) = self.localized.get(engine_id) {
            return k.clone();
        }
        let auth = self
            .auth
            .as_ref()
            .map(|(proto, ku)| (*proto, proto.localize(ku, engine_id)));
        let privacy = match (&self.privacy, &self.auth) {
            (Some((proto, ku)), Some((auth, _))) => {
                Some((*proto, proto.key(*auth, &auth.localize(ku, engine_id))))
            }
            _ => None,
        };
        let keys = Keys { auth, privacy };
        // Agents met by one integration are few; an engine that keeps
        // changing its ID would grow this, so it is kept small.
        if self.localized.len() >= 64 {
            self.localized.clear();
        }
        self.localized.insert(engine_id.to_vec(), keys.clone());
        keys
    }
}

/// Build a v3 message for `user` (or unauthenticated, for discovery and
/// reports, when `user` is `None` or `flags` asks for no authentication).
#[allow(clippy::too_many_arguments)]
fn encode_v3(
    msg_id: i32,
    flags: u8,
    engine: (&[u8], u32, u32),
    user_name: &[u8],
    keys: Option<&Keys>,
    salt: u64,
    scoped: &[u8],
) -> Vec<u8> {
    let (engine_id, boots, time) = engine;
    let auth = keys
        .and_then(|k| k.auth.as_ref())
        .filter(|_| flags & FLAG_AUTH != 0);
    let privacy = keys
        .and_then(|k| k.privacy.as_ref())
        .filter(|_| flags & FLAG_PRIV != 0);
    let (scoped_part, priv_params) = match privacy {
        Some((proto, key)) => {
            let (data, params) = proto.encrypt(key, boots, time, salt, scoped);
            let mut out = Vec::new();
            ber::octets(&mut out, &data);
            (out, params)
        }
        None => (scoped.to_vec(), Vec::new()),
    };
    let placeholder = vec![0u8; auth.map_or(0, |(p, _)| p.mac_len())];
    let mut usm = Vec::new();
    ber::wrap(&mut usm, ber::SEQUENCE, |out| {
        ber::octets(out, engine_id);
        ber::integer(out, i64::from(boots));
        ber::integer(out, i64::from(time));
        ber::octets(out, user_name);
        ber::octets(out, &placeholder);
        ber::octets(out, &priv_params);
    });
    let mut out = Vec::new();
    ber::wrap(&mut out, ber::SEQUENCE, |out| {
        ber::integer(out, 3);
        ber::wrap(out, ber::SEQUENCE, |out| {
            ber::integer(out, i64::from(msg_id));
            ber::integer(out, MAX_SIZE);
            ber::octets(out, &[flags]);
            ber::integer(out, USM);
        });
        ber::octets(out, &usm);
        out.extend_from_slice(&scoped_part);
    });
    if let Some((proto, key)) = auth {
        // The placeholder is the zeros right before the privacy parameters
        // at the end of the USM sequence.
        let usm_at = find(&out, &usm).expect("the USM parameters are in the message");
        let auth_at = usm_at + usm.len()
            - header_len(priv_params.len())
            - priv_params.len()
            - placeholder.len();
        let mac = proto.mac(key, &out);
        out[auth_at..auth_at + mac.len()].copy_from_slice(&mac);
    }
    out
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Check a received message's authentication with `keys`.
fn authentic(data: &[u8], m: &V3Message, keys: &Keys) -> bool {
    let Some((proto, key)) = &keys.auth else {
        return false;
    };
    if m.usm.auth.len() != proto.mac_len() {
        return false;
    }
    let mut copy = data.to_vec();
    copy[m.auth_at..m.auth_at + m.usm.auth.len()].fill(0);
    let mac = proto.mac(key, &copy);
    // Equal length, compared in full.
    mac.iter()
        .zip(&m.usm.auth)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn open_scoped(m: &V3Message, keys: &Keys) -> Result<Vec<u8>, String> {
    match &m.scoped {
        Scoped::Plain(bytes) => Ok(bytes.clone()),
        Scoped::Encrypted(bytes) => {
            let (proto, key) = keys
                .privacy
                .as_ref()
                .ok_or("an encrypted PDU, but no privacy password is set")?;
            proto.decrypt(key, m.usm.boots, m.usm.time, &m.usm.privacy, bytes)
        }
    }
}

/// Where a request goes, and how it is secured.
#[derive(Debug, Clone)]
pub(crate) enum Security {
    /// v1 or v2c: the community for reads, and the one for SETs.
    Community {
        version: Version,
        read: Vec<u8>,
        write: Vec<u8>,
    },
    V3 {
        user: User,
        context_name: Vec<u8>,
        /// contextEngineID, when it is not the agent's own snmpEngineID
        /// (a proxy).
        context_engine_id: Option<Vec<u8>>,
    },
}

impl Security {
    pub(crate) fn version(&self) -> Version {
        match self {
            Security::Community { version, .. } => *version,
            Security::V3 { .. } => Version::V3,
        }
    }
}

/// What to ask an agent.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Request {
    Get(Vec<Oid>),
    GetNext(Vec<Oid>),
    GetBulk {
        non_repeaters: u32,
        max_repetitions: u32,
        oids: Vec<Oid>,
    },
    Set(Vec<VarBind>),
    /// Every object under `root`, with GETBULK of `bulk` repetitions or
    /// (when `None`, or on v1) GETNEXT; stops after `limit` objects when
    /// given.
    Walk {
        root: Oid,
        bulk: Option<u32>,
        limit: Option<usize>,
    },
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SnmpError {
    Timeout,
    /// The agent answered with an error-status (RFC 3416 §3); `index`
    /// counts variable bindings from 1, 0 when none is to blame.
    Status {
        status: i64,
        index: i64,
        oid: Option<Oid>,
    },
    /// The agent refused the user, its keys or its security level.
    Auth(String),
    /// The agent sent something this end cannot use, or reported another
    /// problem.
    Protocol(String),
}

impl std::fmt::Display for SnmpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnmpError::Timeout => f.write_str("no response within the timeout"),
            SnmpError::Status { status, index, oid } => {
                write!(f, "{}", status_name(*status))?;
                match oid {
                    Some(o) => write!(f, " at variable binding {index} ({o})"),
                    None if *index > 0 => write!(f, " at variable binding {index}"),
                    None => Ok(()),
                }
            }
            SnmpError::Auth(m) | SnmpError::Protocol(m) => f.write_str(m),
        }
    }
}

/// A finished request.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reply {
    pub(crate) tag: u64,
    pub(crate) result: Result<Vec<VarBind>, SnmpError>,
    /// From the last send to the response, when there was one.
    pub(crate) round_trip: Option<Millis>,
}

struct InFlight {
    tag: u64,
    request: Request,
    /// The PDU sent now: for a walk, the step under way.
    pdu: Pdu,
    write: bool,
    sent_at: Millis,
    deadline: Millis,
    tries: u32,
    /// A notInTimeWindow report has been answered by resending once.
    time_resent: bool,
    /// A walk's results so far.
    walked: Vec<VarBind>,
}

/// The agent's engine, as learned (RFC 3414 §2.3).
#[derive(Debug, Clone)]
struct Engine {
    id: Vec<u8>,
    boots: u32,
    time: u32,
    /// When `time` was learned, on the session's clock.
    at: Millis,
}

impl Engine {
    fn time_now(&self, now: Millis) -> u32 {
        self.time
            .saturating_add(((now.saturating_sub(self.at)) / 1000) as u32)
    }
}

/// A manager talking to one agent.
pub(crate) struct Client {
    to: SocketAddr,
    socket: Key,
    timer: Key,
    security: Security,
    timeout: Millis,
    retries: u32,
    next_id: i32,
    inflight: BTreeMap<i32, InFlight>,
    engine: Option<Engine>,
    /// The discovery request in flight: its msgID, when sent, and tries.
    discovery: Option<(i32, Millis, u32)>,
    /// Requests waiting for discovery.
    held: VecDeque<(u64, Request)>,
    salt: u64,
}

impl Client {
    /// A client for the agent at `to`, sending from `socket` (opened here,
    /// ephemeral) and keeping its retransmissions on `timer`. `seed` starts
    /// the request-ids and the privacy salt; give it a random value.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        cx: &mut Cx,
        to: SocketAddr,
        socket: Key,
        timer: Key,
        security: Security,
        timeout: Millis,
        retries: u32,
        seed: u64,
    ) -> Client {
        cx.udp_open(socket, Bind::Ephemeral);
        Client {
            to,
            socket,
            timer,
            security,
            timeout: timeout.max(1),
            retries,
            // Positive, and far from wrapping (RFC 3416 allows any Integer32).
            next_id: ((seed >> 33) as i32 & 0x3FFF_FFFF).max(1),
            inflight: BTreeMap::new(),
            engine: None,
            discovery: None,
            held: VecDeque::new(),
            salt: seed,
        }
    }

    pub(crate) fn version(&self) -> Version {
        self.security.version()
    }

    /// The agent's snmpEngineID, engineBoots and engineTime now (v3, once
    /// discovered).
    pub(crate) fn engine(&self, now: Millis) -> Option<(Vec<u8>, u32, u32)> {
        self.engine
            .as_ref()
            .map(|e| (e.id.clone(), e.boots, e.time_now(now)))
    }

    /// Requests not yet answered.
    #[cfg(test)]
    pub(crate) fn pending(&self) -> usize {
        self.inflight.len() + self.held.len()
    }

    fn id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id = if self.next_id >= i32::MAX - 1 {
            1
        } else {
            self.next_id + 1
        };
        id
    }

    /// Send a request; its [`Reply`] comes from [`Client::datagram`] or
    /// [`Client::timer`] under `tag`. A request the version cannot carry
    /// (GETBULK over v1) fails at once, in the returned list.
    pub(crate) fn send(&mut self, cx: &mut Cx, tag: u64, request: Request) -> Vec<Reply> {
        if matches!(request, Request::GetBulk { .. }) && self.version() == Version::V1 {
            return vec![Reply {
                tag,
                result: Err(SnmpError::Protocol("SNMPv1 has no GETBULK".into())),
                round_trip: None,
            }];
        }
        if self.version() == Version::V3 && self.engine.is_none() {
            self.held.push_back((tag, request));
            if self.discovery.is_none() {
                self.discover(cx, 0);
            }
            self.arm(cx);
            return Vec::new();
        }
        self.start(cx, tag, request);
        self.arm(cx);
        Vec::new()
    }

    fn start(&mut self, cx: &mut Cx, tag: u64, request: Request) {
        let id = self.id();
        let (pdu_tag, a, b, varbinds, write) = match &request {
            Request::Get(oids) => (GET, 0, 0, null_binds(oids), false),
            Request::GetNext(oids) => (GET_NEXT, 0, 0, null_binds(oids), false),
            Request::GetBulk {
                non_repeaters,
                max_repetitions,
                oids,
            } => (
                GET_BULK,
                i64::from(*non_repeaters),
                i64::from(*max_repetitions),
                null_binds(oids),
                false,
            ),
            Request::Set(vbs) => (SET, 0, 0, vbs.clone(), true),
            Request::Walk { root, .. } => {
                let (t, a, b) = self.walk_step(&request);
                (t, a, b, null_binds(std::slice::from_ref(root)), false)
            }
        };
        let pdu = Pdu {
            tag: pdu_tag,
            request_id: id,
            error_status: a,
            error_index: b,
            varbinds,
        };
        let now = cx.now();
        let flight = InFlight {
            tag,
            request,
            pdu,
            write,
            sent_at: now,
            deadline: now + self.timeout,
            tries: 0,
            time_resent: false,
            walked: Vec::new(),
        };
        self.inflight.insert(id, flight);
        self.transmit(cx, id);
    }

    /// A walk's next PDU type and its two numbers.
    fn walk_step(&self, request: &Request) -> (u8, i64, i64) {
        match request {
            Request::Walk {
                bulk: Some(reps), ..
            } if self.version() != Version::V1 => (GET_BULK, 0, i64::from(*reps)),
            _ => (GET_NEXT, 0, 0),
        }
    }

    /// Encode and send the request under `id` as it stands.
    fn transmit(&mut self, cx: &mut Cx, id: i32) {
        let now = cx.now();
        let Some(flight) = self.inflight.get(&id) else {
            return;
        };
        let data = match &mut self.security {
            Security::Community {
                version,
                read,
                write,
            } => {
                let community = if flight.write && !write.is_empty() {
                    &*write
                } else {
                    &*read
                };
                encode_community(*version, community, &flight.pdu)
            }
            Security::V3 {
                user,
                context_name,
                context_engine_id,
            } => {
                let engine = self
                    .engine
                    .as_ref()
                    .expect("v3 requests wait for the engine");
                let context = context_engine_id
                    .clone()
                    .unwrap_or_else(|| engine.id.clone());
                let scoped = encode_scoped(&context, context_name, &flight.pdu);
                let keys = user.keys(&engine.id);
                self.salt = self.salt.wrapping_add(1);
                encode_v3(
                    id,
                    user.flags() | FLAG_REPORTABLE,
                    (&engine.id, engine.boots, engine.time_now(now)),
                    &user.name.clone(),
                    Some(&keys),
                    self.salt,
                    &scoped,
                )
            }
        };
        if let Some(flight) = self.inflight.get_mut(&id) {
            flight.sent_at = now;
            flight.deadline = now + self.timeout;
        }
        cx.udp_send(self.socket, self.to, data);
    }

    /// Ask for the engine's ID, boots and time (RFC 3414 §4).
    fn discover(&mut self, cx: &mut Cx, tries: u32) {
        let id = self.id();
        let pdu = Pdu {
            tag: GET,
            request_id: id,
            error_status: 0,
            error_index: 0,
            varbinds: Vec::new(),
        };
        let scoped = encode_scoped(&[], &[], &pdu);
        let data = encode_v3(id, FLAG_REPORTABLE, (&[], 0, 0), &[], None, 0, &scoped);
        self.discovery = Some((id, cx.now(), tries));
        cx.udp_send(self.socket, self.to, data);
    }

    /// Arm the timer for the earliest deadline, or cancel it.
    fn arm(&self, cx: &mut Cx) {
        let now = cx.now();
        let mut next = self.inflight.values().map(|f| f.deadline).min();
        if let Some((_, at, _)) = self.discovery {
            let d = at + self.timeout;
            next = Some(next.map_or(d, |n| n.min(d)));
        }
        match next {
            Some(at) => cx.set_timer(self.timer, at.saturating_sub(now).max(1)),
            None => cx.cancel_timer(self.timer),
        }
    }

    /// The client's timer fired: resend what is overdue, fail what has had
    /// its last try.
    pub(crate) fn timer(&mut self, cx: &mut Cx) -> Vec<Reply> {
        let now = cx.now();
        let mut out = Vec::new();
        if let Some((_, at, tries)) = self.discovery {
            if now >= at + self.timeout {
                if tries < self.retries {
                    self.discover(cx, tries + 1);
                } else {
                    self.discovery = None;
                    for (tag, _) in self.held.drain(..) {
                        out.push(Reply {
                            tag,
                            result: Err(SnmpError::Timeout),
                            round_trip: None,
                        });
                    }
                }
            }
        }
        let due: Vec<i32> = self
            .inflight
            .iter()
            .filter(|(_, f)| f.deadline <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in due {
            let tries = self.inflight[&id].tries;
            if tries < self.retries {
                self.inflight.get_mut(&id).expect("due").tries += 1;
                self.transmit(cx, id);
            } else if let Some(f) = self.inflight.remove(&id) {
                out.push(Reply {
                    tag: f.tag,
                    result: Err(SnmpError::Timeout),
                    round_trip: None,
                });
            }
        }
        self.arm(cx);
        out
    }

    /// A datagram arrived on the client's socket.
    pub(crate) fn datagram(&mut self, cx: &mut Cx, from: SocketAddr, data: &[u8]) -> Vec<Reply> {
        if from.ip() != self.to.ip() {
            return Vec::new();
        }
        let mut out = Vec::new();
        match decode(data) {
            Ok(Message::Community {
                version,
                pdu: AnyPdu::Pdu(pdu),
                ..
            }) if version == self.version() && pdu.tag == RESPONSE => {
                self.response(cx, pdu.request_id, pdu, &mut out);
            }
            Ok(Message::V3(m)) if self.version() == Version::V3 => self.v3(cx, data, m, &mut out),
            Ok(_) => cx.log(
                crate::module::Level::Debug,
                format!("an SNMP message from {from} that answers nothing was ignored"),
            ),
            Err(e) => cx.log(
                crate::module::Level::Debug,
                format!("an undecodable SNMP message from {from}: {e}"),
            ),
        }
        self.arm(cx);
        out
    }

    fn v3(&mut self, cx: &mut Cx, data: &[u8], m: V3Message, out: &mut Vec<Reply>) {
        let now = cx.now();
        // The answer to discovery: a report naming the engine.
        if self.discovery.is_some_and(|(id, _, _)| id == m.msg_id) {
            if m.usm.engine_id.is_empty() {
                return;
            }
            self.discovery = None;
            self.engine = Some(Engine {
                id: m.usm.engine_id.clone(),
                boots: m.usm.boots,
                time: m.usm.time,
                at: now,
            });
            while let Some((tag, request)) = self.held.pop_front() {
                self.start(cx, tag, request);
            }
            return;
        }
        if !self.inflight.contains_key(&m.msg_id) {
            return;
        }
        let Security::V3 { user, .. } = &mut self.security else {
            return;
        };
        let keys = user.keys(&m.usm.engine_id);
        let authenticated = m.flags & FLAG_AUTH != 0;
        if authenticated && !authentic(data, &m, &keys) {
            // Not this user's: ignored, and the request keeps waiting
            // (RFC 3414 §3.2 step 6 drops it).
            cx.log(
                crate::module::Level::Debug,
                "an SNMPv3 message that fails authentication was ignored",
            );
            return;
        }
        let scoped = match open_scoped(&m, &keys).and_then(|s| decode_scoped(&s)) {
            Ok(s) => s,
            Err(e) => {
                if let Some(f) = self.inflight.remove(&m.msg_id) {
                    out.push(Reply {
                        tag: f.tag,
                        result: Err(SnmpError::Auth(format!("the response cannot be read: {e}"))),
                        round_trip: None,
                    });
                }
                return;
            }
        };
        // An authenticated message from the agent keeps its clock right
        // (RFC 3414 §3.2 step 7b).
        if authenticated {
            if let Some(engine) = &mut self.engine {
                if m.usm.engine_id == engine.id
                    && (m.usm.boots > engine.boots
                        || (m.usm.boots == engine.boots && m.usm.time >= engine.time_now(now)))
                {
                    engine.boots = m.usm.boots;
                    engine.time = m.usm.time;
                    engine.at = now;
                }
            }
        }
        let AnyPdu::Pdu(pdu) = scoped.pdu else {
            return;
        };
        if pdu.tag == REPORT {
            let oid = pdu.varbinds.first().map(|vb| vb.oid.clone());
            let name = oid.as_ref().map_or_else(|| "a report".into(), report_name);
            let counter = oid
                .as_ref()
                .filter(|o| o.0.len() == 11 && o.0[..9] == USM_STATS_PREFIX)
                .map(|o| o.0[9]);
            match counter {
                // notInTimeWindows: the report carries the agent's clock;
                // resend once with it.
                Some(2) => {
                    let first = self.inflight.get(&m.msg_id).is_some_and(|f| !f.time_resent);
                    if first {
                        self.engine = Some(Engine {
                            id: m.usm.engine_id.clone(),
                            boots: m.usm.boots,
                            time: m.usm.time,
                            at: now,
                        });
                        let flight = self.inflight.get_mut(&m.msg_id).expect("in flight");
                        flight.time_resent = true;
                        self.transmit(cx, m.msg_id);
                        return;
                    }
                }
                // unknownEngineIDs: the agent's engine changed; learn it
                // and resend.
                Some(4) if !m.usm.engine_id.is_empty() => {
                    let changed = self.engine.as_ref().is_none_or(|e| e.id != m.usm.engine_id);
                    if changed {
                        self.engine = Some(Engine {
                            id: m.usm.engine_id.clone(),
                            boots: m.usm.boots,
                            time: m.usm.time,
                            at: now,
                        });
                        self.transmit(cx, m.msg_id);
                        return;
                    }
                }
                _ => {}
            }
            if let Some(f) = self.inflight.remove(&m.msg_id) {
                let error = match counter {
                    Some(1 | 3 | 5 | 6) => {
                        SnmpError::Auth(format!("the agent refused the request: {name}"))
                    }
                    _ => SnmpError::Protocol(format!("the agent reported {name}")),
                };
                out.push(Reply {
                    tag: f.tag,
                    result: Err(error),
                    round_trip: Some(now.saturating_sub(f.sent_at)),
                });
            }
            return;
        }
        if pdu.tag == RESPONSE {
            self.response(cx, m.msg_id, pdu, out);
        }
    }

    fn response(&mut self, cx: &mut Cx, id: i32, pdu: Pdu, out: &mut Vec<Reply>) {
        let now = cx.now();
        let Some(flight) = self.inflight.get(&id) else {
            return;
        };
        if pdu.request_id != flight.pdu.request_id {
            return;
        }
        let round_trip = Some(now.saturating_sub(flight.sent_at));
        if let Request::Walk { root, limit, .. } = flight.request.clone() {
            let mut f = self.inflight.remove(&id).expect("in flight");
            // v1 ends a walk with noSuchName.
            if pdu.error_status == 2 && self.version() == Version::V1 {
                out.push(Reply {
                    tag: f.tag,
                    result: Ok(f.walked),
                    round_trip,
                });
                return;
            }
            if pdu.error_status != 0 {
                out.push(Reply {
                    tag: f.tag,
                    result: Err(status_error(&pdu, &f.pdu)),
                    round_trip,
                });
                return;
            }
            let mut last = f.pdu.varbinds[0].oid.clone();
            let mut done = pdu.varbinds.is_empty();
            for vb in pdu.varbinds {
                if vb.value == Value::EndOfMibView || !vb.oid.starts_with(&root) {
                    done = true;
                    break;
                }
                if vb.oid <= last {
                    out.push(Reply {
                        tag: f.tag,
                        result: Err(SnmpError::Protocol(format!(
                            "the agent answered {} after {last}, which is not later: the walk would not end",
                            vb.oid
                        ))),
                        round_trip,
                    });
                    return;
                }
                last = vb.oid.clone();
                f.walked.push(vb);
                if limit.is_some_and(|l| f.walked.len() >= l) {
                    done = true;
                    break;
                }
            }
            if done {
                out.push(Reply {
                    tag: f.tag,
                    result: Ok(f.walked),
                    round_trip,
                });
                return;
            }
            // The next step, under a new request-id.
            let next = self.id();
            f.pdu.request_id = next;
            f.pdu.varbinds = null_binds(&[last]);
            f.tries = 0;
            f.time_resent = false;
            cx.round_trip(now.saturating_sub(f.sent_at));
            self.inflight.insert(next, f);
            self.transmit(cx, next);
            return;
        }
        let f = self.inflight.remove(&id).expect("in flight");
        let result = if pdu.error_status != 0 {
            Err(status_error(&pdu, &f.pdu))
        } else {
            Ok(pdu.varbinds)
        };
        out.push(Reply {
            tag: f.tag,
            result,
            round_trip,
        });
    }

    /// Forget every request, failing none: for a module stopping or
    /// starting afresh.
    pub(crate) fn clear(&mut self, cx: &mut Cx) {
        self.inflight.clear();
        self.held.clear();
        self.discovery = None;
        cx.cancel_timer(self.timer);
    }
}

fn status_error(pdu: &Pdu, sent: &Pdu) -> SnmpError {
    let oid = usize::try_from(pdu.error_index)
        .ok()
        .filter(|i| *i > 0)
        .and_then(|i| sent.varbinds.get(i - 1).or_else(|| pdu.varbinds.get(i - 1)))
        .map(|vb| vb.oid.clone());
    SnmpError::Status {
        status: pdu.error_status,
        index: pdu.error_index,
        oid,
    }
}

fn null_binds(oids: &[Oid]) -> Vec<VarBind> {
    oids.iter()
        .map(|oid| VarBind {
            oid: oid.clone(),
            value: Value::Null,
        })
        .collect()
}

/// A notification received.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Notification {
    pub(crate) source: SocketAddr,
    pub(crate) version: Version,
    /// `trap` or `inform`.
    pub(crate) kind: &'static str,
    /// sysUpTime.0 of a v2 notification, a v1 trap's time-stamp.
    pub(crate) uptime: Option<u32>,
    /// snmpTrapOID.0, or a v1 trap's equivalent (RFC 3584 §3.1).
    pub(crate) trap_oid: Option<Oid>,
    /// The variable bindings after sysUpTime.0 and snmpTrapOID.0.
    pub(crate) varbinds: Vec<VarBind>,
    /// A v1 trap's enterprise, agent-addr, generic-trap and specific-trap.
    pub(crate) v1: Option<(Oid, [u8; 4], i64, i64)>,
    /// The v3 user, or (never shown) nothing for v1 and v2c.
    pub(crate) user: Option<String>,
}

/// Receives traps and informs (RFC 3416 §4.2.6, §4.2.7) on a socket the
/// module opens, and answers informs. For v3 it is the authoritative engine
/// of informs sent to it (RFC 3414 §1.5.1), with its own snmpEngineID,
/// and checks traps with keys localized to the sender's engine.
pub(crate) struct Receiver {
    socket: Key,
    /// For v1 and v2c: accept only this community; any when empty.
    community: Vec<u8>,
    /// For v3: the user notifications are accepted for.
    user: Option<User>,
    engine_id: Vec<u8>,
    boots: u32,
    /// The session time this engine's clock started from.
    started: Millis,
    /// usmStats counters this engine reports.
    unknown_engine_ids: u32,
    not_in_time_windows: u32,
    salt: u64,
}

impl Receiver {
    pub(crate) fn new(
        cx: &Cx,
        socket: Key,
        community: Vec<u8>,
        user: Option<User>,
        engine_id: Vec<u8>,
        seed: u64,
    ) -> Receiver {
        Receiver {
            socket,
            community,
            user,
            engine_id,
            boots: 1,
            started: cx.now(),
            unknown_engine_ids: 0,
            not_in_time_windows: 0,
            salt: seed,
        }
    }

    fn engine_time(&self, now: Millis) -> u32 {
        (now.saturating_sub(self.started) / 1000) as u32
    }

    /// A datagram arrived on the listening socket: the notification it
    /// carries, if it is one this receiver accepts. Informs and v3
    /// discovery are answered on the same socket.
    pub(crate) fn datagram(
        &mut self,
        cx: &mut Cx,
        from: SocketAddr,
        data: &[u8],
    ) -> Result<Option<Notification>, String> {
        match decode(data)? {
            Message::Community {
                version,
                community,
                pdu,
            } => {
                if !self.community.is_empty() && community != self.community {
                    return Err(format!(
                        "a notification from {from} with another community was ignored"
                    ));
                }
                match pdu {
                    AnyPdu::TrapV1(t) if version == Version::V1 => {
                        Ok(Some(v1_notification(from, t)))
                    }
                    AnyPdu::Pdu(pdu) if pdu.tag == TRAP_V2 || pdu.tag == INFORM => {
                        if pdu.tag == INFORM {
                            let answer = Pdu {
                                tag: RESPONSE,
                                request_id: pdu.request_id,
                                error_status: 0,
                                error_index: 0,
                                varbinds: pdu.varbinds.clone(),
                            };
                            cx.udp_send(
                                self.socket,
                                from,
                                encode_community(version, &community, &answer),
                            );
                        }
                        Ok(Some(v2_notification(from, version, &pdu, None)))
                    }
                    _ => Ok(None),
                }
            }
            Message::V3(m) => self.v3(cx, from, data, m),
        }
    }

    fn v3(
        &mut self,
        cx: &mut Cx,
        from: SocketAddr,
        data: &[u8],
        m: V3Message,
    ) -> Result<Option<Notification>, String> {
        let now = cx.now();
        let our_time = self.engine_time(now);
        let our_engine = self.engine_id.clone();
        let boots = self.boots;
        let Some(user) = &mut self.user else {
            return Err(format!(
                "an SNMPv3 notification from {from}, but no v3 user is set"
            ));
        };
        let reportable = m.flags & FLAG_REPORTABLE != 0;
        // An inform's sender discovers this engine first: an empty or
        // unknown engine ID is answered with a report naming it.
        if reportable && m.usm.engine_id != self.engine_id {
            self.unknown_engine_ids = self.unknown_engine_ids.wrapping_add(1);
            let counter = Oid(vec![1, 3, 6, 1, 6, 3, 15, 1, 1, 4, 0]);
            let pdu = report(&m, counter, self.unknown_engine_ids);
            let scoped = encode_scoped(&self.engine_id, &[], &pdu);
            let reply = encode_v3(
                m.msg_id,
                0,
                (&our_engine, boots, our_time),
                &m.usm.user,
                None,
                0,
                &scoped,
            );
            cx.udp_send(self.socket, from, reply);
            return Ok(None);
        }
        if m.usm.user != user.name {
            return Err(format!(
                "an SNMPv3 notification from {from} for another user ({}) was ignored",
                String::from_utf8_lossy(&m.usm.user)
            ));
        }
        let wanted = user.flags();
        if m.flags & (FLAG_AUTH | FLAG_PRIV) != wanted {
            return Err(format!(
                "an SNMPv3 notification from {from} at another security level was ignored"
            ));
        }
        let keys = user.keys(&m.usm.engine_id);
        if wanted & FLAG_AUTH != 0 && !authentic(data, &m, &keys) {
            return Err(format!(
                "an SNMPv3 notification from {from} failed authentication"
            ));
        }
        // This engine's clock (RFC 3414 §3.2 step 7a): an inform within 150
        // seconds of it.
        let ours = m.usm.engine_id == self.engine_id;
        if ours && wanted & FLAG_AUTH != 0 {
            let time = i64::from(our_time);
            if m.usm.boots != boots || (i64::from(m.usm.time) - time).abs() > 150 {
                self.not_in_time_windows = self.not_in_time_windows.wrapping_add(1);
                if reportable {
                    let counter = Oid(vec![1, 3, 6, 1, 6, 3, 15, 1, 1, 2, 0]);
                    let pdu = report(&m, counter, self.not_in_time_windows);
                    let scoped = encode_scoped(&self.engine_id, &[], &pdu);
                    let reply = encode_v3(
                        m.msg_id,
                        FLAG_AUTH,
                        (&our_engine, boots, our_time),
                        &user.name.clone(),
                        Some(&keys),
                        0,
                        &scoped,
                    );
                    cx.udp_send(self.socket, from, reply);
                }
                return Ok(None);
            }
        }
        let scoped = decode_scoped(&open_scoped(&m, &keys)?)?;
        let AnyPdu::Pdu(pdu) = scoped.pdu else {
            return Ok(None);
        };
        if pdu.tag != TRAP_V2 && pdu.tag != INFORM {
            return Ok(None);
        }
        if pdu.tag == INFORM {
            let answer = Pdu {
                tag: RESPONSE,
                request_id: pdu.request_id,
                error_status: 0,
                error_index: 0,
                varbinds: pdu.varbinds.clone(),
            };
            let reply_scoped =
                encode_scoped(&scoped.context_engine_id, &scoped.context_name, &answer);
            self.salt = self.salt.wrapping_add(1);
            let reply = encode_v3(
                m.msg_id,
                wanted,
                (&our_engine, boots, our_time),
                &user.name.clone(),
                Some(&keys),
                self.salt,
                &reply_scoped,
            );
            cx.udp_send(self.socket, from, reply);
        }
        let name = String::from_utf8_lossy(&user.name).into_owned();
        Ok(Some(v2_notification(from, Version::V3, &pdu, Some(name))))
    }
}

/// A report answering `m` (RFC 3412 §7.1 step 3): the counter and its value.
fn report(m: &V3Message, counter: Oid, value: u32) -> Pdu {
    // The request-id of an encrypted PDU cannot be read; 0 then.
    let request_id = match &m.scoped {
        Scoped::Plain(s) => match decode_scoped(s).map(|s| s.pdu) {
            Ok(AnyPdu::Pdu(p)) => p.request_id,
            _ => 0,
        },
        Scoped::Encrypted(_) => 0,
    };
    Pdu {
        tag: REPORT,
        request_id,
        error_status: 0,
        error_index: 0,
        varbinds: vec![VarBind {
            oid: counter,
            value: Value::Counter32(value),
        }],
    }
}

fn v2_notification(
    from: SocketAddr,
    version: Version,
    pdu: &Pdu,
    user: Option<String>,
) -> Notification {
    let mut uptime = None;
    let mut trap_oid = None;
    let mut rest = Vec::new();
    for (i, vb) in pdu.varbinds.iter().enumerate() {
        match (&vb.value, i) {
            (Value::TimeTicks(t), 0) if vb.oid == sys_up_time() => uptime = Some(*t),
            (Value::Oid(o), 1) if vb.oid == snmp_trap_oid() => trap_oid = Some(o.clone()),
            _ => rest.push(vb.clone()),
        }
    }
    Notification {
        source: from,
        version,
        kind: if pdu.tag == INFORM { "inform" } else { "trap" },
        uptime,
        trap_oid,
        varbinds: rest,
        v1: None,
        user,
    }
}

/// A v1 trap, with its snmpTrapOID as RFC 3584 §3.1 derives it: a generic
/// trap is snmpTraps.(generic-trap + 1), an enterprise-specific one
/// enterprise.0.specific-trap.
fn v1_notification(from: SocketAddr, t: TrapV1) -> Notification {
    let trap_oid = if (0..6).contains(&t.generic) {
        Oid(vec![1, 3, 6, 1, 6, 3, 1, 1, 5, t.generic as u32 + 1])
    } else {
        t.enterprise
            .child(&[0, t.specific.clamp(0, i64::from(u32::MAX)) as u32])
    };
    Notification {
        source: from,
        version: Version::V1,
        kind: "trap",
        uptime: Some(t.timestamp),
        trap_oid: Some(trap_oid),
        varbinds: t.varbinds,
        v1: Some((t.enterprise, t.agent, t.generic, t.specific)),
        user: None,
    }
}

/// Where a receiver listens: the session's shared port (routed by sender,
/// so several devices share 162) or a port of its own at an address.
pub(crate) fn listen_bind(address: Option<IpAddr>, port: u16, any_sender: bool) -> Bind {
    if address.is_none() && !any_sender {
        Bind::Shared(port)
    } else {
        Bind::Any { address, port }
    }
}

/// OIDs in dotted decimal, separated by commas, semicolons or spaces.
pub(crate) fn parse_oids(text: &str) -> Result<Vec<Oid>, String> {
    let oids: Vec<Oid> = text
        .split([',', ';', ' '])
        .filter(|e| !e.is_empty())
        .map(Oid::parse)
        .collect::<Result<_, _>>()?;
    if oids.is_empty() {
        return Err("give at least one OID".into());
    }
    Ok(oids)
}

/// A variable binding as JSON: {oid, type, value}, and `hex` for an
/// OCTET STRING or Opaque.
pub(crate) fn varbind_json(vb: &VarBind) -> crate::value::Value {
    let mut o = crate::json!({
        "oid": vb.oid.to_string(),
        "type": vb.value.type_name(),
        "value": vb.value.to_json(),
    });
    if let Some(bytes) = vb.value.bytes() {
        o["hex"] = crate::json!(ber::hex(bytes));
    }
    o
}

/// A failed request as a command's error.
pub(crate) fn command_error(e: &SnmpError) -> crate::module::CommandError {
    match e {
        SnmpError::Timeout => crate::module::CommandError::Timeout,
        SnmpError::Status { status, .. } => crate::module::CommandError::DeviceError {
            code: Some(status_name(*status)),
            message: e.to_string(),
        },
        SnmpError::Auth(m) => crate::module::CommandError::Auth { message: m.clone() },
        SnmpError::Protocol(m) => crate::module::CommandError::DeviceError {
            code: None,
            message: m.clone(),
        },
    }
}

/// The settings every SNMP integration shares (generic-snmp's, which the
/// vendor integrations declare too): the agent, its security and timing,
/// and where notifications are received.
pub(crate) struct Settings {
    /// None when opened with no host, to receive notifications only.
    pub(crate) agent: Option<SocketAddr>,
    pub(crate) security: Security,
    /// The v3 user, which also checks v3 notifications.
    pub(crate) user: Option<User>,
    pub(crate) timeout: Millis,
    pub(crate) retries: u32,
    pub(crate) max_repetitions: u32,
    pub(crate) interval: Millis,
    pub(crate) trap: Option<(Option<IpAddr>, u16)>,
    pub(crate) trap_community: Vec<u8>,
    pub(crate) local_engine_id: Vec<u8>,
    /// Random: starts request-ids and the privacy salt.
    pub(crate) seed: u64,
}

fn text_setting<'a>(s: &'a crate::catalog::Params, name: &str) -> &'a str {
    s.get(name)
        .and_then(crate::value::Value::as_str)
        .unwrap_or("")
        .trim()
}

fn random_seed() -> u64 {
    let mut bytes = [0u8; 8];
    match getrandom::fill(&mut bytes) {
        Ok(()) => u64::from_be_bytes(bytes),
        // Uniqueness is what the salt and request-ids need.
        Err(_) => u64::from(std::process::id()).wrapping_mul(0x9E37_79B9_7F4A_7C15),
    }
}

impl Settings {
    /// Read the shared settings of a device being opened; `interval` is the
    /// poll interval when `poll_interval_ms` is not given. A device needs a
    /// host unless it receives notifications (`trap_port`).
    pub(crate) fn parse(
        ctx: &crate::module::OpenContext,
        interval: Millis,
    ) -> Result<Settings, String> {
        let s = &ctx.settings;
        let num = |k: &str, d: u64| s.get(k).and_then(crate::value::Value::as_u64).unwrap_or(d);
        let version = match text_setting(s, "version") {
            "v1" => Version::V1,
            "" | "v2c" => Version::V2c,
            "v3" => Version::V3,
            other => return Err(format!("version '{other}' is not v1, v2c or v3")),
        };
        let raw = |k: &str| s.get(k).and_then(crate::value::Value::as_str);
        let user = if version == Version::V3 {
            let level = match text_setting(s, "security_level") {
                "no_auth_no_priv" => Level::NoAuthNoPriv,
                "auth_no_priv" => Level::AuthNoPriv,
                "" | "auth_priv" => Level::AuthPriv,
                other => return Err(format!("unknown security_level '{other}'")),
            };
            let auth = Auth::from_name(match text_setting(s, "auth_protocol") {
                "" => "sha",
                a => a,
            })
            .ok_or("auth_protocol is not md5, sha, sha224, sha256, sha384 or sha512")?;
            let privacy = Privacy::from_name(match text_setting(s, "priv_protocol") {
                "" => "aes",
                p => p,
            })
            .ok_or("priv_protocol is not des, aes, aes192 or aes256")?;
            let name = text_setting(s, "user");
            if name.is_empty() {
                return Err("SNMPv3 needs the user setting".into());
            }
            // Passwords are taken as given, spaces included.
            let password = |k: &str| raw(k).unwrap_or("").to_string();
            Some(User::new(
                name,
                level,
                Some((auth, &password("auth_password"))),
                Some((privacy, &password("priv_password"))),
            )?)
        } else {
            None
        };
        let hex_setting = |k: &str| -> Result<Option<Vec<u8>>, String> {
            match text_setting(s, k) {
                "" => Ok(None),
                h => ber::parse_hex(h).map(Some).map_err(|e| format!("{k}: {e}")),
            }
        };
        let context_engine_id = hex_setting("context_engine_id")?;
        let seed = random_seed();
        let local_engine_id = match hex_setting("local_engine_id")? {
            Some(id) if (5..=32).contains(&id.len()) => id,
            Some(_) => return Err("local_engine_id must be 5 to 32 bytes (RFC 3411)".into()),
            // RFC 3411's format 5 (octets) behind the high bit, with random
            // octets: unique, though not kept across restarts.
            None => {
                let mut id = vec![0x80, 0, 0, 0, 5];
                id.extend_from_slice(&seed.to_be_bytes());
                id
            }
        };
        let trap = match s.get("trap_port").and_then(crate::value::Value::as_u64) {
            None => None,
            Some(p) => {
                let address = match text_setting(s, "trap_bind_address") {
                    "" => None,
                    a => Some(
                        a.parse::<IpAddr>()
                            .map_err(|_| format!("trap_bind_address '{a}' is not an IP address"))?,
                    ),
                };
                Some((
                    address,
                    u16::try_from(p).map_err(|_| "trap_port is out of range")?,
                ))
            }
        };
        let agent = if ctx.host.is_unspecified() {
            if trap.is_none() {
                return Err("give the agent's address as host, or set trap_port to receive notifications from any sender".into());
            }
            None
        } else {
            Some(SocketAddr::new(ctx.host, ctx.port.unwrap_or(161)))
        };
        let security = match &user {
            Some(user) => Security::V3 {
                user: user.clone(),
                context_name: text_setting(s, "context_name").as_bytes().to_vec(),
                context_engine_id,
            },
            None => Security::Community {
                version,
                read: raw("community").unwrap_or("public").as_bytes().to_vec(),
                write: raw("write_community").unwrap_or("").as_bytes().to_vec(),
            },
        };
        Ok(Settings {
            agent,
            security,
            user,
            timeout: num("timeout_ms", 1500).max(1),
            retries: num("retries", 2) as u32,
            max_repetitions: num("max_repetitions", 20).max(1) as u32,
            interval: num("poll_interval_ms", interval).max(100),
            trap,
            trap_community: raw("trap_community").unwrap_or("").as_bytes().to_vec(),
            local_engine_id,
            seed,
        })
    }
}

/// An agent's side of the exchange, for tests of modules built on this
/// layer: it answers GET, GETNEXT, GETBULK and SET over a sorted table,
/// for v1, v2c and v3 with one user, and sends notifications.
#[cfg(test)]
pub(crate) mod agent {
    use super::*;

    pub(crate) struct Agent {
        pub(crate) table: BTreeMap<Oid, Value>,
        pub(crate) community: Vec<u8>,
        pub(crate) user: Option<User>,
        pub(crate) engine_id: Vec<u8>,
        pub(crate) boots: u32,
        pub(crate) time: u32,
        pub(crate) read_only: Vec<Oid>,
    }

    impl Agent {
        pub(crate) fn new(table: &[(&str, Value)]) -> Agent {
            Agent {
                table: table
                    .iter()
                    .map(|(o, v)| (Oid::parse(o).unwrap(), v.clone()))
                    .collect(),
                community: b"public".to_vec(),
                user: None,
                engine_id: vec![0x80, 0, 0, 0, 0x04, b'f', b'a', b'k', b'e'],
                boots: 5,
                time: 1000,
                read_only: Vec::new(),
            }
        }

        fn answer(&mut self, version: Version, pdu: &Pdu) -> Pdu {
            let mut vbs = Vec::new();
            let mut status = 0;
            let mut index = 0;
            let next = |table: &BTreeMap<Oid, Value>, oid: &Oid| {
                table
                    .range((
                        std::ops::Bound::Excluded(oid.clone()),
                        std::ops::Bound::Unbounded,
                    ))
                    .next()
                    .map(|(o, v)| VarBind {
                        oid: o.clone(),
                        value: v.clone(),
                    })
            };
            let missing = |oid: &Oid, value: Value| VarBind {
                oid: oid.clone(),
                value,
            };
            match pdu.tag {
                GET => {
                    for (i, vb) in pdu.varbinds.iter().enumerate() {
                        match self.table.get(&vb.oid) {
                            Some(v) => vbs.push(missing(&vb.oid, v.clone())),
                            None if version == Version::V1 => {
                                status = 2;
                                index = i as i64 + 1;
                                vbs.push(vb.clone());
                            }
                            None => vbs.push(missing(&vb.oid, Value::NoSuchObject)),
                        }
                    }
                }
                GET_NEXT => {
                    for (i, vb) in pdu.varbinds.iter().enumerate() {
                        match next(&self.table, &vb.oid) {
                            Some(n) => vbs.push(n),
                            None if version == Version::V1 => {
                                status = 2;
                                index = i as i64 + 1;
                                vbs.push(vb.clone());
                            }
                            None => vbs.push(missing(&vb.oid, Value::EndOfMibView)),
                        }
                    }
                }
                GET_BULK => {
                    let n = pdu.error_status.max(0) as usize;
                    for vb in pdu.varbinds.iter().take(n) {
                        vbs.push(
                            next(&self.table, &vb.oid)
                                .unwrap_or(missing(&vb.oid, Value::EndOfMibView)),
                        );
                    }
                    let mut cursors: Vec<Oid> =
                        pdu.varbinds.iter().skip(n).map(|v| v.oid.clone()).collect();
                    for _ in 0..pdu.error_index.max(0) {
                        for c in cursors.iter_mut() {
                            match next(&self.table, c) {
                                Some(n) => {
                                    *c = n.oid.clone();
                                    vbs.push(n);
                                }
                                None => vbs.push(missing(c, Value::EndOfMibView)),
                            }
                        }
                    }
                }
                SET => {
                    for (i, vb) in pdu.varbinds.iter().enumerate() {
                        if self.read_only.contains(&vb.oid) {
                            status = 17;
                            index = i as i64 + 1;
                        }
                    }
                    if status == 0 {
                        for vb in &pdu.varbinds {
                            self.table.insert(vb.oid.clone(), vb.value.clone());
                        }
                    }
                    vbs = pdu.varbinds.clone();
                }
                _ => {}
            }
            Pdu {
                tag: RESPONSE,
                request_id: pdu.request_id,
                error_status: status,
                error_index: index,
                varbinds: vbs,
            }
        }

        /// The agent's answer to a request, if it gives one.
        pub(crate) fn handle(&mut self, data: &[u8]) -> Option<Vec<u8>> {
            match decode(data).ok()? {
                Message::Community {
                    version,
                    community,
                    pdu: AnyPdu::Pdu(pdu),
                } => {
                    if community != self.community && !(pdu.tag == SET && community == b"private") {
                        return None;
                    }
                    let answer = self.answer(version, &pdu);
                    Some(encode_community(version, &community, &answer))
                }
                Message::V3(m) => {
                    let mut user = self.user.clone()?;
                    let engine_id = self.engine_id.clone();
                    let engine = (engine_id.as_slice(), self.boots, self.time);
                    if m.usm.engine_id.is_empty() {
                        let pdu = report(&m, Oid(vec![1, 3, 6, 1, 6, 3, 15, 1, 1, 4, 0]), 1);
                        let scoped = encode_scoped(&self.engine_id, &[], &pdu);
                        return Some(encode_v3(m.msg_id, 0, engine, &[], None, 0, &scoped));
                    }
                    if m.usm.user != user.name {
                        return None;
                    }
                    let keys = user.keys(&self.engine_id);
                    if m.flags & FLAG_AUTH != 0 && !authentic(data, &m, &keys) {
                        return None;
                    }
                    if m.flags & FLAG_AUTH != 0
                        && (m.usm.boots != self.boots || m.usm.time.abs_diff(self.time) > 150)
                    {
                        let pdu = report(&m, Oid(vec![1, 3, 6, 1, 6, 3, 15, 1, 1, 2, 0]), 1);
                        let scoped = encode_scoped(&self.engine_id, &[], &pdu);
                        let name = user.name.clone();
                        return Some(encode_v3(
                            m.msg_id,
                            FLAG_AUTH,
                            engine,
                            &name,
                            Some(&keys),
                            0,
                            &scoped,
                        ));
                    }
                    let scoped = decode_scoped(&open_scoped(&m, &keys).ok()?).ok()?;
                    let AnyPdu::Pdu(pdu) = scoped.pdu else {
                        return None;
                    };
                    let answer = self.answer(Version::V3, &pdu);
                    let out =
                        encode_scoped(&scoped.context_engine_id, &scoped.context_name, &answer);
                    let name = user.name.clone();
                    Some(encode_v3(
                        m.msg_id,
                        m.flags & !FLAG_REPORTABLE,
                        engine,
                        &name,
                        Some(&keys),
                        77,
                        &out,
                    ))
                }
                _ => None,
            }
        }

        /// A v2c trap or inform, or (with a user) a v3 one from this
        /// agent's engine (a trap) or for `receiver_engine` (an inform).
        pub(crate) fn notification(
            &mut self,
            inform: bool,
            request_id: i32,
            trap: &str,
            varbinds: Vec<VarBind>,
            receiver_engine: Option<&[u8]>,
        ) -> Vec<u8> {
            let mut vbs = vec![
                VarBind {
                    oid: sys_up_time(),
                    value: Value::TimeTicks(4242),
                },
                VarBind {
                    oid: snmp_trap_oid(),
                    value: Value::Oid(Oid::parse(trap).unwrap()),
                },
            ];
            vbs.extend(varbinds);
            let pdu = Pdu {
                tag: if inform { INFORM } else { TRAP_V2 },
                request_id,
                error_status: 0,
                error_index: 0,
                varbinds: vbs,
            };
            match self.user.clone() {
                None => encode_community(Version::V2c, &self.community, &pdu),
                Some(mut user) => {
                    let (engine_id, boots, time) = match receiver_engine {
                        Some(e) => (e.to_vec(), 1, 0),
                        None => (self.engine_id.clone(), self.boots, self.time),
                    };
                    let keys = user.keys(&engine_id);
                    let scoped = encode_scoped(&engine_id, &[], &pdu);
                    let flags = user.flags() | if inform { FLAG_REPORTABLE } else { 0 };
                    let name = user.name.clone();
                    encode_v3(
                        request_id,
                        flags,
                        (&engine_id, boots, time),
                        &name,
                        Some(&keys),
                        9,
                        &scoped,
                    )
                }
            }
        }

        /// An empty, reportable v3 message: an inform sender's discovery.
        pub(crate) fn probe(&self, msg_id: i32) -> Vec<u8> {
            let pdu = Pdu {
                tag: GET,
                request_id: msg_id,
                error_status: 0,
                error_index: 0,
                varbinds: Vec::new(),
            };
            let scoped = encode_scoped(&[], &[], &pdu);
            encode_v3(msg_id, FLAG_REPORTABLE, (&[], 0, 0), &[], None, 0, &scoped)
        }

        /// The engine ID a report from a receiver names.
        pub(crate) fn reported_engine(data: &[u8]) -> Option<Vec<u8>> {
            match decode(data).ok()? {
                Message::V3(m) => Some(m.usm.engine_id),
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::agent::Agent;
    use super::*;
    use crate::module::Action;

    const TO: &str = "192.0.2.7:161";

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    /// Run requests against the fake agent until they are all answered.
    fn exchange(client: &mut Client, agent: &mut Agent, mut actions: Vec<Action>) -> Vec<Reply> {
        let mut replies = Vec::new();
        for step in 0..50 {
            let datagrams = sent(&actions);
            if datagrams.is_empty() {
                break;
            }
            actions.clear();
            for d in datagrams {
                if let Some(answer) = agent.handle(&d) {
                    let mut cx = Cx::new(10 + step);
                    replies.extend(client.datagram(&mut cx, TO.parse().unwrap(), &answer));
                    actions.extend(cx.take());
                }
            }
        }
        replies
    }

    fn table() -> Agent {
        Agent::new(&[
            (
                "1.3.6.1.2.1.1.1.0",
                Value::OctetString(b"Test agent".to_vec()),
            ),
            ("1.3.6.1.2.1.1.3.0", Value::TimeTicks(12345)),
            (
                "1.3.6.1.2.1.1.5.0",
                Value::OctetString(b"rack-pdu".to_vec()),
            ),
            (
                "1.3.6.1.2.1.2.2.1.2.1",
                Value::OctetString(b"eth0".to_vec()),
            ),
            (
                "1.3.6.1.2.1.2.2.1.2.2",
                Value::OctetString(b"eth1".to_vec()),
            ),
            ("1.3.6.1.2.1.2.2.1.10.1", Value::Counter32(77)),
            ("1.3.6.1.2.1.31.1.1.1.6.1", Value::Counter64(1 << 40)),
        ])
    }

    fn client(cx: &mut Cx, security: Security) -> Client {
        Client::new(
            cx,
            TO.parse().unwrap(),
            "snmp",
            "snmp_timer",
            security,
            1000,
            1,
            42,
        )
    }

    fn v2c() -> Security {
        Security::Community {
            version: Version::V2c,
            read: b"public".to_vec(),
            write: b"private".to_vec(),
        }
    }

    fn oids(list: &[&str]) -> Vec<Oid> {
        list.iter().map(|o| Oid::parse(o).unwrap()).collect()
    }

    /// RFC 3416 §4.2.1's GetRequest-PDU in a v2c message, byte for byte.
    #[test]
    fn a_v2c_get_is_encoded_as_rfc_3416_and_1901_say() {
        let pdu = Pdu {
            tag: GET,
            request_id: 1,
            error_status: 0,
            error_index: 0,
            varbinds: null_binds(&oids(&["1.3.6.1.2.1.1.3.0"])),
        };
        let wire = encode_community(Version::V2c, b"public", &pdu);
        assert_eq!(
            ber::hex(&wire),
            "302602010104067075626c6963a019020101020100020100300e300c06082b060102010103000500"
        );
        assert_eq!(
            decode(&wire).unwrap(),
            Message::Community {
                version: Version::V2c,
                community: b"public".to_vec(),
                pdu: AnyPdu::Pdu(pdu)
            }
        );
    }

    #[test]
    fn get_set_and_errors_over_v2c() {
        let mut agent = table();
        agent
            .read_only
            .push(Oid::parse("1.3.6.1.2.1.1.1.0").unwrap());
        let mut cx = Cx::new(0);
        let mut c = client(&mut cx, v2c());
        c.send(
            &mut cx,
            1,
            Request::Get(oids(&["1.3.6.1.2.1.1.5.0", "1.3.6.1.2.1.1.9.0"])),
        );
        let replies = exchange(&mut c, &mut agent, cx.take());
        let vbs = replies[0].result.clone().unwrap();
        assert_eq!(vbs[0].value, Value::OctetString(b"rack-pdu".to_vec()));
        assert_eq!(vbs[1].value, Value::NoSuchObject);
        // A SET goes with the write community and is read back.
        let mut cx = Cx::new(20);
        let set = VarBind {
            oid: Oid::parse("1.3.6.1.2.1.1.5.0").unwrap(),
            value: Value::OctetString(b"stage-left".to_vec()),
        };
        c.send(&mut cx, 2, Request::Set(vec![set.clone()]));
        let actions = cx.take();
        assert!(find(&sent(&actions)[0], b"private").is_some());
        let replies = exchange(&mut c, &mut agent, actions);
        assert_eq!(replies[0].result, Ok(vec![set]));
        assert_eq!(
            agent.table[&Oid::parse("1.3.6.1.2.1.1.5.0").unwrap()],
            Value::OctetString(b"stage-left".to_vec())
        );
        // A refused SET names the error and the object.
        let mut cx = Cx::new(40);
        c.send(
            &mut cx,
            3,
            Request::Set(vec![VarBind {
                oid: Oid::parse("1.3.6.1.2.1.1.1.0").unwrap(),
                value: Value::OctetString(b"x".to_vec()),
            }]),
        );
        let replies = exchange(&mut c, &mut agent, cx.take());
        let error = replies[0].result.clone().unwrap_err();
        assert_eq!(
            error.to_string(),
            "notWritable at variable binding 1 (1.3.6.1.2.1.1.1.0)"
        );
    }

    #[test]
    fn walks_end_at_the_subtree_with_getnext_getbulk_and_v1() {
        for (security, bulk) in [
            (v2c(), Some(2)),
            (v2c(), None),
            (
                Security::Community {
                    version: Version::V1,
                    read: b"public".to_vec(),
                    write: Vec::new(),
                },
                Some(5),
            ),
        ] {
            let mut agent = table();
            let mut cx = Cx::new(0);
            let mut c = client(&mut cx, security);
            c.send(
                &mut cx,
                9,
                Request::Walk {
                    root: Oid::parse("1.3.6.1.2.1.2").unwrap(),
                    bulk,
                    limit: None,
                },
            );
            let replies = exchange(&mut c, &mut agent, cx.take());
            assert_eq!(replies.len(), 1);
            let got: Vec<String> = replies[0]
                .result
                .clone()
                .unwrap()
                .iter()
                .map(|v| v.oid.to_string())
                .collect();
            assert_eq!(
                got,
                [
                    "1.3.6.1.2.1.2.2.1.2.1",
                    "1.3.6.1.2.1.2.2.1.2.2",
                    "1.3.6.1.2.1.2.2.1.10.1"
                ]
            );
        }
        // To the end of the agent's view, and with a limit.
        let mut agent = table();
        let mut cx = Cx::new(0);
        let mut c = client(&mut cx, v2c());
        c.send(
            &mut cx,
            1,
            Request::Walk {
                root: Oid::parse("1.3").unwrap(),
                bulk: Some(3),
                limit: Some(4),
            },
        );
        let replies = exchange(&mut c, &mut agent, cx.take());
        assert_eq!(replies[0].result.clone().unwrap().len(), 4);
        let mut cx = Cx::new(0);
        c.send(
            &mut cx,
            2,
            Request::Walk {
                root: Oid::parse("1.3").unwrap(),
                bulk: Some(3),
                limit: None,
            },
        );
        let replies = exchange(&mut c, &mut agent, cx.take());
        assert_eq!(replies[0].result.clone().unwrap().len(), 7);
    }

    #[test]
    fn unanswered_requests_are_resent_then_time_out() {
        let mut cx = Cx::new(0);
        let mut c = client(&mut cx, v2c());
        c.send(&mut cx, 5, Request::Get(oids(&["1.3.6.1.2.1.1.3.0"])));
        let first = sent(&cx.take());
        let mut cx = Cx::new(1000);
        assert!(c.timer(&mut cx).is_empty());
        let again = sent(&cx.take());
        assert_eq!(again, first, "the same request-id is resent");
        let mut cx = Cx::new(2000);
        let replies = c.timer(&mut cx);
        assert_eq!(
            replies,
            vec![Reply {
                tag: 5,
                result: Err(SnmpError::Timeout),
                round_trip: None
            }]
        );
        assert_eq!(c.pending(), 0);
    }

    fn v3_user(level: Level, auth: Auth, privacy: Privacy) -> User {
        User::new(
            "operator",
            level,
            Some((auth, "maplesyrup")),
            Some((privacy, "pancakes99")),
        )
        .unwrap()
    }

    #[test]
    fn v3_discovers_the_engine_then_reads_at_every_level_and_cipher() {
        for (level, auth, privacy) in [
            (Level::NoAuthNoPriv, Auth::Md5, Privacy::Aes128),
            (Level::AuthNoPriv, Auth::Md5, Privacy::Aes128),
            (Level::AuthNoPriv, Auth::Sha512, Privacy::Aes128),
            (Level::AuthPriv, Auth::Sha1, Privacy::Des),
            (Level::AuthPriv, Auth::Sha1, Privacy::Aes128),
            (Level::AuthPriv, Auth::Sha1, Privacy::Aes256),
            (Level::AuthPriv, Auth::Sha256, Privacy::Aes192),
            (Level::AuthPriv, Auth::Sha384, Privacy::Aes256),
        ] {
            let user = v3_user(level, auth, privacy);
            let mut agent = table();
            agent.user = Some(user.clone());
            let mut cx = Cx::new(0);
            let mut c = client(
                &mut cx,
                Security::V3 {
                    user,
                    context_name: Vec::new(),
                    context_engine_id: None,
                },
            );
            c.send(&mut cx, 1, Request::Get(oids(&["1.3.6.1.2.1.1.5.0"])));
            let replies = exchange(&mut c, &mut agent, cx.take());
            assert_eq!(replies.len(), 1, "{level:?} {auth:?} {privacy:?}");
            assert_eq!(
                replies[0].result.clone().unwrap()[0].value,
                Value::OctetString(b"rack-pdu".to_vec()),
                "{level:?} {auth:?} {privacy:?}"
            );
            assert_eq!(c.engine(10).unwrap().0, agent.engine_id);
        }
    }

    #[test]
    fn v3_resends_once_with_the_clock_a_not_in_time_window_report_gives() {
        let user = v3_user(Level::AuthPriv, Auth::Sha256, Privacy::Aes128);
        let mut agent = table();
        agent.user = Some(user.clone());
        let mut cx = Cx::new(0);
        let mut c = client(
            &mut cx,
            Security::V3 {
                user,
                context_name: Vec::new(),
                context_engine_id: None,
            },
        );
        c.send(&mut cx, 1, Request::Get(oids(&["1.3.6.1.2.1.1.3.0"])));
        let replies = exchange(&mut c, &mut agent, cx.take());
        assert!(replies[0].result.is_ok());
        // The agent rebooted: its boots moved on.
        agent.boots = 6;
        agent.time = 3;
        let mut cx = Cx::new(100);
        c.send(&mut cx, 2, Request::Get(oids(&["1.3.6.1.2.1.1.3.0"])));
        let replies = exchange(&mut c, &mut agent, cx.take());
        assert!(replies[0].result.is_ok(), "{:?}", replies[0]);
        assert_eq!(c.engine(100).unwrap().1, 6);
    }

    #[test]
    fn v3_with_wrong_passwords_is_not_answered_and_times_out() {
        let mut agent = table();
        agent.user = Some(v3_user(Level::AuthPriv, Auth::Sha1, Privacy::Aes128));
        let wrong = User::new(
            "operator",
            Level::AuthPriv,
            Some((Auth::Sha1, "not the password")),
            Some((Privacy::Aes128, "pancakes99")),
        )
        .unwrap();
        let mut cx = Cx::new(0);
        let mut c = client(
            &mut cx,
            Security::V3 {
                user: wrong,
                context_name: Vec::new(),
                context_engine_id: None,
            },
        );
        c.send(&mut cx, 1, Request::Get(oids(&["1.3.6.1.2.1.1.3.0"])));
        assert!(exchange(&mut c, &mut agent, cx.take()).is_empty());
        assert_eq!(c.pending(), 1);
        assert!(User::new("u", Level::AuthNoPriv, Some((Auth::Sha1, "short")), None).is_err());
    }

    #[test]
    fn notifications_are_received_and_informs_answered() {
        let from: SocketAddr = "192.0.2.7:50000".parse().unwrap();
        let cx0 = Cx::new(0);
        let mut r = Receiver::new(
            &cx0,
            "trap",
            b"public".to_vec(),
            None,
            b"\x80\0\0\0\x05recv".to_vec(),
            1,
        );
        let mut agent = table();
        let extra = vec![VarBind {
            oid: Oid::parse("1.3.6.1.2.1.2.2.1.1.2").unwrap(),
            value: Value::Integer(2),
        }];
        // A v2c trap: nothing is sent back.
        let mut cx = Cx::new(5);
        let n = r
            .datagram(
                &mut cx,
                from,
                &agent.notification(false, 7, "1.3.6.1.6.3.1.1.5.3", extra.clone(), None),
            )
            .unwrap()
            .unwrap();
        assert!(cx.take().is_empty());
        assert_eq!((n.kind, n.uptime), ("trap", Some(4242)));
        assert_eq!(n.trap_oid, Some(Oid::parse("1.3.6.1.6.3.1.1.5.3").unwrap()));
        assert_eq!(n.varbinds, extra);
        // A v2c inform is answered with its request-id.
        let mut cx = Cx::new(6);
        let n = r
            .datagram(
                &mut cx,
                from,
                &agent.notification(true, 8, "1.3.6.1.6.3.1.1.5.4", vec![], None),
            )
            .unwrap()
            .unwrap();
        assert_eq!(n.kind, "inform");
        let answer = sent(&cx.take());
        match decode(&answer[0]).unwrap() {
            Message::Community {
                pdu: AnyPdu::Pdu(p),
                ..
            } => assert_eq!((p.tag, p.request_id), (RESPONSE, 8)),
            other => panic!("{other:?}"),
        }
        // Another community is ignored.
        agent.community = b"other".to_vec();
        let mut cx = Cx::new(7);
        assert!(r
            .datagram(
                &mut cx,
                from,
                &agent.notification(false, 9, "1.3.6.1.6.3.1.1.5.1", vec![], None)
            )
            .is_err());
        // A v1 trap: RFC 3584 §3.1's snmpTrapOID.
        let trap = TrapV1 {
            enterprise: Oid::parse("1.3.6.1.4.1.318").unwrap(),
            agent: [192, 0, 2, 7],
            generic: 6,
            specific: 41,
            timestamp: 99,
            varbinds: vec![],
        };
        let mut cx = Cx::new(8);
        let n = r
            .datagram(&mut cx, from, &encode_trap_v1(b"public", &trap))
            .unwrap()
            .unwrap();
        assert_eq!(
            n.trap_oid,
            Some(Oid::parse("1.3.6.1.4.1.318.0.41").unwrap())
        );
        let linkdown = TrapV1 { generic: 2, ..trap };
        let n = r
            .datagram(&mut cx, from, &encode_trap_v1(b"public", &linkdown))
            .unwrap()
            .unwrap();
        assert_eq!(n.trap_oid, Some(Oid::parse("1.3.6.1.6.3.1.1.5.3").unwrap()));
    }

    #[test]
    fn v3_traps_and_informs_are_authenticated_and_informs_discover_this_engine() {
        let from: SocketAddr = "192.0.2.7:50000".parse().unwrap();
        let user = v3_user(Level::AuthPriv, Auth::Sha256, Privacy::Aes128);
        let ours = b"\x80\0\0\0\x05recv".to_vec();
        let cx0 = Cx::new(0);
        let mut r = Receiver::new(
            &cx0,
            "trap",
            Vec::new(),
            Some(user.clone()),
            ours.clone(),
            1,
        );
        let mut agent = table();
        agent.user = Some(user);
        // A trap, keyed to the agent's own engine.
        let mut cx = Cx::new(1000);
        let n = r
            .datagram(
                &mut cx,
                from,
                &agent.notification(false, 3, "1.3.6.1.6.3.1.1.5.1", vec![], None),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            (n.version, n.kind, n.user.as_deref()),
            (Version::V3, "trap", Some("operator"))
        );
        // An inform's sender first learns this engine's ID from a report.
        let mut cx = Cx::new(1000);
        assert_eq!(r.datagram(&mut cx, from, &agent.probe(11)).unwrap(), None);
        let report = sent(&cx.take());
        assert_eq!(Agent::reported_engine(&report[0]), Some(ours.clone()));
        // Then sends it keyed to that engine, at its time (boots 1, time 0
        // here, the receiver's clock at 1 s).
        let mut cx = Cx::new(1000);
        let n = r
            .datagram(
                &mut cx,
                from,
                &agent.notification(true, 12, "1.3.6.1.6.3.1.1.5.2", vec![], Some(&ours)),
            )
            .unwrap()
            .unwrap();
        assert_eq!(n.kind, "inform");
        let answer = sent(&cx.take());
        match decode(&answer[0]).unwrap() {
            Message::V3(m) => {
                assert_eq!((m.msg_id, m.flags), (12, FLAG_AUTH | FLAG_PRIV));
                assert_eq!(m.usm.engine_id, ours);
            }
            other => panic!("{other:?}"),
        }
        // A forged one (another password) is refused.
        let mut forger = table();
        forger.user = Some(v3_user(Level::AuthPriv, Auth::Sha256, Privacy::Aes128));
        forger.user = Some(
            User::new(
                "operator",
                Level::AuthPriv,
                Some((Auth::Sha256, "guessed wrong")),
                Some((Privacy::Aes128, "pancakes99")),
            )
            .unwrap(),
        );
        let mut cx = Cx::new(1000);
        assert!(r
            .datagram(
                &mut cx,
                from,
                &forger.notification(false, 4, "1.3.6.1.6.3.1.1.5.1", vec![], None)
            )
            .is_err());
    }
}
