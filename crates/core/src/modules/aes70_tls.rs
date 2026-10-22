//! Secure OCP.1: the TLS 1.2 client AES70 asks for, with the one cipher
//! suite it names, TLS_DHE_PSK_WITH_AES_128_CBC_SHA (0x0090), and the
//! pre-shared key identity hint "OCA-PSK" (AES70-2024 11.2.4, as SwiftOCA
//! documents it). The core's TLS library (rustls) has no pre-shared keys, so
//! this is a small client for exactly that suite, built from RustCrypto
//! primitives and, like the modules, without I/O of its own: bytes in, bytes
//! and events out. The DTLS 1.2 client in [`super::dtls`] shares its
//! cryptography and handshake messages.
//!
//! # Security note
//!
//! What is implemented, client side only:
//!
//! - TLS 1.2 (RFC 5246) and DTLS 1.2 (RFC 6347), no other version, with the
//!   single suite TLS_DHE_PSK_WITH_AES_128_CBC_SHA (RFC 4279 section 3).
//! - The key exchange: the server's ServerKeyExchange carries the identity
//!   hint and its Diffie-Hellman group and public value, unsigned; the
//!   pre-master secret is the shared secret Z (leading zero bytes stripped,
//!   RFC 5246 8.1.2) and the PSK, each with a 16-bit length. The server's
//!   parameters are checked before use: p odd and of 2048 to 8192 bits,
//!   1 < g < p-1 and 1 < Ys < p-1, and the shared secret neither 0, 1 nor
//!   p-1. The private exponent is fresh for each connection, bits(p)-1
//!   random bits from the operating system's generator (getrandom), and the
//!   exponentiation is constant-time (crypto-bigint). The primality of p is
//!   not tested: the PSK, not the group, authenticates the server, and the
//!   group only protects that connection's forward secrecy.
//! - The PRF with SHA-256 (RFC 5246 5). The extended master secret (RFC
//!   7627) is always offered and used when the server agrees; otherwise the
//!   RFC 5246 8.1 master secret.
//! - Records: AES-128-CBC with a fresh random explicit IV for every record,
//!   and HMAC-SHA1. Encrypt-then-MAC (RFC 7366) is always offered and used
//!   when the server agrees. Otherwise MAC-then-encrypt, whose check takes
//!   the same time whatever the padding (the Lucky Thirteen mitigations): the
//!   padding is checked over the last 256 bytes with masks, the MAC is
//!   computed with dummy SHA-1 compressions so every padding length costs the
//!   same number of them, the received MAC is read with a constant-time scan,
//!   and every comparison is constant-time. A failure to decrypt, of padding
//!   or of MAC is one error with one message: TLS answers it with a single
//!   fatal bad_record_mac alert and ends the session; DTLS discards the
//!   record silently (RFC 6347 4.1.2.7).
//! - Renegotiation is refused: renegotiation_info (RFC 5746) is sent empty
//!   and must come back empty; a HelloRequest after the handshake is answered
//!   with a no_renegotiation warning, and one during it is ignored.
//! - A strict state machine: ServerHello, ServerKeyExchange, ServerHelloDone,
//!   ChangeCipherSpec and Finished, in that order. Anything else (a
//!   Certificate, a CertificateRequest, a NewSessionTicket, application data
//!   before the handshake ends, a ChangeCipherSpec inside a handshake
//!   message) is a fatal unexpected_message; an unknown or repeated extension
//!   in the ServerHello is fatal; the server's Finished is compared in
//!   constant time.
//! - Sequence numbers per direction, from 0 after each ChangeCipherSpec; the
//!   session ends rather than let one wrap. Record limits: 2^14 bytes of
//!   plaintext and 2^14 + 2048 of ciphertext (record_overflow beyond), no
//!   empty handshake, alert or ChangeCipherSpec record, at most 32 empty
//!   application data records in a row, handshake messages of at most 64 KiB.
//! - Alerts: close_notify is answered with close_notify and ends the session;
//!   any fatal alert ends it; of the warnings only user_canceled and
//!   no_renegotiation are let pass, any other ends it.
//! - Secrets are zeroized when dropped (zeroize): the PSK, the private
//!   exponent, Z, the pre-master and master secrets and the keys. The PSK is
//!   a secret setting and never logged; the identity travels in the clear,
//!   as TLS sends it.
//!
//! Not implemented, and refused: any other suite or version (TLS 1.0, 1.1,
//! 1.3, DTLS 1.0, 1.3), certificates, compression (only null is offered),
//! session resumption and tickets (neither offered; a session id from the
//! server is ignored), renegotiation, heartbeats, SNI and ALPN (not
//! offered), and the server role.

use std::fmt;

use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use crypto_bigint::modular::{BoxedMontyForm, BoxedMontyParams};
use crypto_bigint::{BoxedUint, Odd};
use hmac::{Hmac, KeyInit, Mac};
use sha1::{Digest, Sha1};
use sha2_tls::Sha256;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, ConstantTimeGreater};
use zeroize::{Zeroize, Zeroizing};

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;
type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

/// TLS_DHE_PSK_WITH_AES_128_CBC_SHA (RFC 4279).
pub(crate) const SUITE: u16 = 0x0090;
/// The PSK identity hint AES70 has a device send, and the identity a
/// controller sends by default.
pub(crate) const HINT: &str = "OCA-PSK";
pub(crate) const TLS12: u16 = 0x0303;

pub(crate) const MAX_PLAINTEXT: usize = 1 << 14;
pub(crate) const MAX_CIPHERTEXT: usize = MAX_PLAINTEXT + 2048;
/// The largest handshake message accepted: a ServerKeyExchange with an
/// 8192-bit group is under 4 KiB, and no certificate is ever sent.
pub(crate) const MAX_HANDSHAKE: usize = 1 << 16;
/// Empty application data records accepted in a row.
pub(crate) const MAX_EMPTY: u32 = 32;
pub(crate) const MIN_DH_BITS: u32 = 2048;
pub(crate) const MAX_DH_BITS: u32 = 8192;

const MAC_LEN: usize = 20;
const BLOCK: usize = 16;
/// Bytes a record adds to its plaintext at most: the IV, the MAC and a
/// block of padding.
pub(crate) const RECORD_OVERHEAD: usize = BLOCK + MAC_LEN + BLOCK;

// Content types.
pub(crate) const CT_CHANGE_CIPHER_SPEC: u8 = 20;
pub(crate) const CT_ALERT: u8 = 21;
pub(crate) const CT_HANDSHAKE: u8 = 22;
pub(crate) const CT_APPLICATION_DATA: u8 = 23;

// Handshake types.
pub(crate) const HT_HELLO_REQUEST: u8 = 0;
pub(crate) const HT_CLIENT_HELLO: u8 = 1;
pub(crate) const HT_SERVER_HELLO: u8 = 2;
pub(crate) const HT_HELLO_VERIFY_REQUEST: u8 = 3;
pub(crate) const HT_SERVER_KEY_EXCHANGE: u8 = 12;
pub(crate) const HT_SERVER_HELLO_DONE: u8 = 14;
pub(crate) const HT_CLIENT_KEY_EXCHANGE: u8 = 16;
pub(crate) const HT_FINISHED: u8 = 20;

// Extensions.
const EXT_ENCRYPT_THEN_MAC: u16 = 0x0016;
const EXT_EXTENDED_MASTER_SECRET: u16 = 0x0017;
const EXT_RENEGOTIATION_INFO: u16 = 0xff01;

/// Alert descriptions (RFC 5246 7.2, RFC 4279).
pub(crate) mod alert {
    pub(crate) const WARNING: u8 = 1;
    pub(crate) const FATAL: u8 = 2;
    pub(crate) const CLOSE_NOTIFY: u8 = 0;
    pub(crate) const UNEXPECTED_MESSAGE: u8 = 10;
    pub(crate) const BAD_RECORD_MAC: u8 = 20;
    pub(crate) const RECORD_OVERFLOW: u8 = 22;
    pub(crate) const HANDSHAKE_FAILURE: u8 = 40;
    pub(crate) const ILLEGAL_PARAMETER: u8 = 47;
    pub(crate) const DECODE_ERROR: u8 = 50;
    pub(crate) const DECRYPT_ERROR: u8 = 51;
    pub(crate) const PROTOCOL_VERSION: u8 = 70;
    pub(crate) const INSUFFICIENT_SECURITY: u8 = 71;
    pub(crate) const INTERNAL_ERROR: u8 = 80;
    pub(crate) const USER_CANCELED: u8 = 90;
    pub(crate) const NO_RENEGOTIATION: u8 = 100;
    pub(crate) const UNSUPPORTED_EXTENSION: u8 = 110;
    pub(crate) const UNKNOWN_PSK_IDENTITY: u8 = 115;

    pub(crate) fn name(code: u8) -> &'static str {
        match code {
            0 => "close_notify",
            10 => "unexpected_message",
            20 => "bad_record_mac",
            21 => "decryption_failed",
            22 => "record_overflow",
            30 => "decompression_failure",
            40 => "handshake_failure",
            42 => "bad_certificate",
            43 => "unsupported_certificate",
            44 => "certificate_revoked",
            45 => "certificate_expired",
            46 => "certificate_unknown",
            47 => "illegal_parameter",
            48 => "unknown_ca",
            49 => "access_denied",
            50 => "decode_error",
            51 => "decrypt_error",
            60 => "export_restriction",
            70 => "protocol_version",
            71 => "insufficient_security",
            80 => "internal_error",
            86 => "inappropriate_fallback",
            90 => "user_canceled",
            100 => "no_renegotiation",
            110 => "unsupported_extension",
            115 => "unknown_psk_identity",
            _ => "unknown",
        }
    }
}

// --- Errors, events, settings ----------------------------------------------

/// A session-ending error: the alert this side sends for it (none when the
/// device ended it), and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
    pub(crate) alert: Option<u8>,
    pub(crate) reason: String,
}

pub(crate) fn fatal(alert: u8, reason: impl Into<String>) -> Failure {
    Failure {
        alert: Some(alert),
        reason: reason.into(),
    }
}

fn unexpected(reason: impl Into<String>) -> Failure {
    fatal(alert::UNEXPECTED_MESSAGE, reason)
}

/// The one error every record that fails to decrypt or authenticate gives.
fn bad_record() -> Failure {
    fatal(alert::BAD_RECORD_MAC, "a record failed authentication")
}

/// What the session asks of its owner.
#[derive(Debug)]
pub(crate) enum Event {
    /// Bytes for the stream (TLS), or one datagram (DTLS).
    Send(Vec<u8>),
    /// Compute the Diffie-Hellman key exchange ([`dh_compute`], off the
    /// session's task) and hand the outcome back.
    Dh(DhRequest),
    /// The handshake is done: application data may flow.
    Established(Session),
    /// Application data from the device: a byte stream over TLS, one
    /// record's worth over DTLS.
    Data(Vec<u8>),
    /// The device closed the session with close_notify.
    Closed,
}

/// What the handshake agreed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Session {
    pub(crate) extended_master_secret: bool,
    pub(crate) encrypt_then_mac: bool,
    /// The identity hint the device sent, if any.
    pub(crate) hint: Option<Vec<u8>>,
}

impl Session {
    /// One line for the log: what was negotiated, and a hint other than
    /// AES70's.
    pub(crate) fn describe(&self) -> String {
        let mut s = format!(
            "TLS_DHE_PSK_WITH_AES_128_CBC_SHA, extended master secret {}, {}",
            if self.extended_master_secret {
                "yes"
            } else {
                "no"
            },
            if self.encrypt_then_mac {
                "encrypt-then-MAC"
            } else {
                "MAC-then-encrypt"
            }
        );
        match &self.hint {
            Some(h) if h.as_slice() == HINT.as_bytes() => {}
            Some(h) => {
                let shown: String = String::from_utf8_lossy(h).chars().take(64).collect();
                s.push_str(&format!(
                    "; the device's identity hint is {shown:?}, not {HINT}"
                ));
            }
            None => s.push_str(&format!(
                "; the device sent no identity hint ({HINT} expected)"
            )),
        }
        s
    }
}

/// The pre-shared key and the identity sent with it.
#[derive(Clone)]
pub(crate) struct Psk {
    pub(crate) identity: Vec<u8>,
    pub(crate) key: Zeroizing<Vec<u8>>,
}

impl fmt::Debug for Psk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Psk")
            .field("identity", &String::from_utf8_lossy(&self.identity))
            .field("key", &"<secret>")
            .finish()
    }
}

impl Psk {
    /// The key from its hex digits (spaces and colons between bytes
    /// allowed). The error never repeats the key.
    pub(crate) fn from_hex(identity: &str, hex: &str) -> Result<Psk, String> {
        let digits: Zeroizing<Vec<u8>> = Zeroizing::new(
            hex.bytes()
                .filter(|b| !matches!(b, b' ' | b':' | b'\t'))
                .collect(),
        );
        if digits.is_empty() {
            return Err("the pre-shared key (psk) is not set".into());
        }
        if !digits.len().is_multiple_of(2) || !digits.iter().all(u8::is_ascii_hexdigit) {
            return Err("the pre-shared key (psk) must be hex digits, two per byte".into());
        }
        if digits.len() / 2 > usize::from(u16::MAX) {
            return Err("the pre-shared key (psk) is longer than TLS can carry".into());
        }
        if identity.len() > usize::from(u16::MAX) {
            return Err("the PSK identity is longer than TLS can carry".into());
        }
        let nibble = |c: u8| match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => c - b'A' + 10,
        };
        let key = Zeroizing::new(
            digits
                .chunks(2)
                .map(|p| (nibble(p[0]) << 4) | nibble(p[1]))
                .collect(),
        );
        Ok(Psk {
            identity: identity.as_bytes().to_vec(),
            key,
        })
    }
}

/// Random bytes from the operating system.
pub(crate) fn random(buf: &mut [u8]) -> Result<(), Failure> {
    getrandom::fill(buf).map_err(|e| {
        fatal(
            alert::INTERNAL_ERROR,
            format!("the system's random number generator failed: {e}"),
        )
    })
}

// --- Encoding ----------------------------------------------------------------

#[derive(Default)]
pub(crate) struct W(pub(crate) Vec<u8>);

impl W {
    pub(crate) fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    pub(crate) fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub(crate) fn u24(&mut self, v: usize) -> &mut Self {
        self.0.extend_from_slice(&(v as u32).to_be_bytes()[1..]);
        self
    }
    pub(crate) fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.0.extend_from_slice(b);
        self
    }
    pub(crate) fn vec8(&mut self, b: &[u8]) -> &mut Self {
        self.u8(b.len() as u8).bytes(b)
    }
    pub(crate) fn vec16(&mut self, b: &[u8]) -> &mut Self {
        self.u16(b.len() as u16).bytes(b)
    }
}

/// A bounds-checked reader; running short is a decode_error naming what was
/// being read.
pub(crate) struct R<'a> {
    b: &'a [u8],
    what: &'static str,
}

impl<'a> R<'a> {
    pub(crate) fn new(b: &'a [u8], what: &'static str) -> R<'a> {
        R { b, what }
    }
    fn short(&self) -> Failure {
        fatal(alert::DECODE_ERROR, format!("a malformed {}", self.what))
    }
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], Failure> {
        if self.b.len() < n {
            return Err(self.short());
        }
        let (head, rest) = self.b.split_at(n);
        self.b = rest;
        Ok(head)
    }
    pub(crate) fn u8(&mut self) -> Result<u8, Failure> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn u16(&mut self) -> Result<u16, Failure> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    pub(crate) fn u24(&mut self) -> Result<usize, Failure> {
        let b = self.take(3)?;
        Ok(usize::from(b[0]) << 16 | usize::from(b[1]) << 8 | usize::from(b[2]))
    }
    pub(crate) fn vec8(&mut self) -> Result<&'a [u8], Failure> {
        let n = self.u8()?;
        self.take(usize::from(n))
    }
    pub(crate) fn vec16(&mut self) -> Result<&'a [u8], Failure> {
        let n = self.u16()?;
        self.take(usize::from(n))
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.b.is_empty()
    }
    /// Nothing may follow.
    pub(crate) fn end(&self) -> Result<(), Failure> {
        if self.b.is_empty() {
            Ok(())
        } else {
            Err(fatal(
                alert::DECODE_ERROR,
                format!("bytes after the end of a {}", self.what),
            ))
        }
    }
}

/// A TLS handshake message: type, 24-bit length, body.
pub(crate) fn handshake_message(ty: u8, body: &[u8]) -> Vec<u8> {
    let mut w = W::default();
    w.u8(ty).u24(body.len()).bytes(body);
    w.0
}

pub(crate) fn handshake_name(ty: u8) -> &'static str {
    match ty {
        0 => "HelloRequest",
        1 => "ClientHello",
        2 => "ServerHello",
        3 => "HelloVerifyRequest",
        4 => "NewSessionTicket",
        11 => "Certificate",
        12 => "ServerKeyExchange",
        13 => "CertificateRequest",
        14 => "ServerHelloDone",
        15 => "CertificateVerify",
        16 => "ClientKeyExchange",
        20 => "Finished",
        _ => "an unknown handshake message",
    }
}

// --- Handshake messages ------------------------------------------------------

/// The ClientHello body: the one suite, null compression, and the
/// renegotiation_info (empty), extended_master_secret and encrypt_then_mac
/// extensions. DTLS adds the cookie field after the session id.
pub(crate) fn client_hello_body(version: u16, random: &[u8; 32], cookie: Option<&[u8]>) -> Vec<u8> {
    let mut w = W::default();
    w.u16(version).bytes(random).vec8(&[]);
    if let Some(cookie) = cookie {
        w.vec8(cookie);
    }
    w.vec16(&SUITE.to_be_bytes()).vec8(&[0]);
    let mut ext = W::default();
    ext.u16(EXT_RENEGOTIATION_INFO).vec16(&[0]);
    ext.u16(EXT_EXTENDED_MASTER_SECRET).vec16(&[]);
    ext.u16(EXT_ENCRYPT_THEN_MAC).vec16(&[]);
    w.vec16(&ext.0);
    w.0
}

/// What the ServerHello settles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServerHello {
    pub(crate) random: [u8; 32],
    pub(crate) extended_master_secret: bool,
    pub(crate) encrypt_then_mac: bool,
}

pub(crate) fn parse_server_hello(body: &[u8], version: u16) -> Result<ServerHello, Failure> {
    let mut r = R::new(body, "ServerHello");
    let v = r.u16()?;
    if v != version {
        return Err(fatal(
            alert::PROTOCOL_VERSION,
            format!("the device chose protocol version {v:#06x}, not {version:#06x}"),
        ));
    }
    let mut random = [0u8; 32];
    random.copy_from_slice(r.take(32)?);
    let session_id = r.vec8()?;
    if session_id.len() > 32 {
        return Err(fatal(
            alert::ILLEGAL_PARAMETER,
            "a session id longer than 32 bytes",
        ));
    }
    let suite = r.u16()?;
    if suite != SUITE {
        return Err(fatal(
            alert::ILLEGAL_PARAMETER,
            format!("the device chose cipher suite {suite:#06x}, which was not offered"),
        ));
    }
    if r.u8()? != 0 {
        return Err(fatal(
            alert::ILLEGAL_PARAMETER,
            "the device chose compression",
        ));
    }
    let mut hello = ServerHello {
        random,
        extended_master_secret: false,
        encrypt_then_mac: false,
    };
    if r.is_empty() {
        return Ok(hello);
    }
    let mut ext = R::new(r.vec16()?, "ServerHello extension");
    r.end()?;
    let mut seen = Vec::new();
    while !ext.is_empty() {
        let ty = ext.u16()?;
        let data = ext.vec16()?;
        if seen.contains(&ty) {
            return Err(fatal(
                alert::ILLEGAL_PARAMETER,
                format!("the ServerHello repeats extension {ty:#06x}"),
            ));
        }
        seen.push(ty);
        match ty {
            EXT_RENEGOTIATION_INFO => {
                // An initial handshake: renegotiated_connection is empty.
                if data != [0] {
                    return Err(fatal(
                        alert::HANDSHAKE_FAILURE,
                        "the device's renegotiation_info is not empty on the initial handshake",
                    ));
                }
            }
            EXT_EXTENDED_MASTER_SECRET | EXT_ENCRYPT_THEN_MAC => {
                if !data.is_empty() {
                    return Err(fatal(
                        alert::DECODE_ERROR,
                        format!("extension {ty:#06x} carries data"),
                    ));
                }
                if ty == EXT_EXTENDED_MASTER_SECRET {
                    hello.extended_master_secret = true;
                } else {
                    hello.encrypt_then_mac = true;
                }
            }
            _ => {
                return Err(fatal(
                    alert::UNSUPPORTED_EXTENSION,
                    format!("the ServerHello has extension {ty:#06x}, which was not offered"),
                ))
            }
        }
    }
    Ok(hello)
}

/// The DHE_PSK ServerKeyExchange: the identity hint and the group, checked.
pub(crate) fn parse_server_key_exchange(
    body: &[u8],
) -> Result<(Option<Vec<u8>>, DhRequest), Failure> {
    let mut r = R::new(body, "ServerKeyExchange");
    let hint = r.vec16()?;
    let p = r.vec16()?;
    let g = r.vec16()?;
    let ys = r.vec16()?;
    r.end()?;
    let dh = dh_check(p, g, ys)?;
    Ok(((!hint.is_empty()).then(|| hint.to_vec()), dh))
}

/// The DHE_PSK ClientKeyExchange: the identity and the client's public
/// value.
pub(crate) fn client_key_exchange_body(identity: &[u8], yc: &[u8]) -> Vec<u8> {
    let mut w = W::default();
    w.vec16(identity).vec16(yc);
    w.0
}

// --- Diffie-Hellman ----------------------------------------------------------

/// A group and public value from the device, checked by [`dh_check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DhRequest {
    p: Vec<u8>,
    g: Vec<u8>,
    ys: Vec<u8>,
    bits: u32,
}

impl DhRequest {
    pub(crate) fn bits(&self) -> u32 {
        self.bits
    }
}

/// The client's side of the exchange: its public value and the shared
/// secret, leading zeros stripped.
pub(crate) struct DhShared {
    pub(crate) yc: Vec<u8>,
    pub(crate) z: Zeroizing<Vec<u8>>,
}

fn trim(b: &[u8]) -> &[u8] {
    let start = b.iter().position(|&x| x != 0).unwrap_or(b.len());
    &b[start..]
}

fn bit_len(trimmed: &[u8]) -> u32 {
    match trimmed.first() {
        None => 0,
        Some(&top) => 8 * trimmed.len() as u32 - top.leading_zeros(),
    }
}

/// Bits of the big integers for a group of `bits` bits: whole 64-bit limbs.
fn precision(bits: u32) -> u32 {
    bits.div_ceil(64) * 64
}

fn uint(bytes: &[u8], precision: u32) -> BoxedUint {
    BoxedUint::from_be_slice(trim(bytes), precision).expect("checked to fit the precision")
}

/// Check the device's group and public value: p odd and of 2048 to 8192
/// bits, 1 < g < p-1, 1 < Ys < p-1. All public, so checked in variable time.
pub(crate) fn dh_check(p: &[u8], g: &[u8], ys: &[u8]) -> Result<DhRequest, Failure> {
    let p = trim(p);
    let bits = bit_len(p);
    if bits < MIN_DH_BITS {
        return Err(fatal(
            alert::INSUFFICIENT_SECURITY,
            format!("the device's Diffie-Hellman group has {bits} bits, fewer than {MIN_DH_BITS}"),
        ));
    }
    if bits > MAX_DH_BITS {
        return Err(fatal(
            alert::ILLEGAL_PARAMETER,
            format!("the device's Diffie-Hellman group has {bits} bits, more than {MAX_DH_BITS}"),
        ));
    }
    if p[p.len() - 1] & 1 == 0 {
        return Err(fatal(
            alert::ILLEGAL_PARAMETER,
            "the device's Diffie-Hellman modulus is even",
        ));
    }
    let prec = precision(bits);
    let pn = uint(p, prec);
    let p_minus_1 = pn.wrapping_sub(BoxedUint::one_with_precision(prec));
    let two = uint(&[2], prec);
    for (what, v) in [("generator", g), ("public value", ys)] {
        let v = trim(v);
        let fits = v.len() <= p.len();
        if !fits || {
            let n = uint(v, prec);
            n.cmp_vartime(&two).is_lt() || n.cmp_vartime(&p_minus_1).is_ge()
        } {
            return Err(fatal(
                alert::ILLEGAL_PARAMETER,
                format!("the device's Diffie-Hellman {what} is outside 2 to p-2"),
            ));
        }
    }
    Ok(DhRequest {
        p: p.to_vec(),
        g: trim(g).to_vec(),
        ys: trim(ys).to_vec(),
        bits,
    })
}

/// The client's exponentiations, constant-time in the private exponent:
/// Yc = g^x mod p and Z = Ys^x mod p, with x fresh from the operating
/// system's generator. Refuses a shared secret of 0, 1 or p-1.
pub(crate) fn dh_compute(req: &DhRequest) -> Result<DhShared, Failure> {
    let prec = precision(req.bits);
    let len = (prec / 8) as usize;
    let p = uint(&req.p, prec);
    let modulus = Odd::new(p.clone())
        .into_option()
        .ok_or_else(|| fatal(alert::ILLEGAL_PARAMETER, "an even modulus"))?;
    let params = BoxedMontyParams::new_vartime(modulus);
    // x has bits(p) - 1 random bits, so 0 < x < p - 1 but for the
    // negligible chance of x < 2, which is drawn again.
    let keep = req.bits - 1;
    let clear = (prec - keep) as usize;
    let mut bytes = Zeroizing::new(vec![0u8; len]);
    let mut x = loop {
        random(&mut bytes)?;
        bytes[..clear / 8].fill(0);
        if !clear.is_multiple_of(8) {
            bytes[clear / 8] &= 0xff >> (clear % 8);
        }
        let x = BoxedUint::from_be_slice(&bytes, prec).expect("sized to the precision");
        if x.bits_vartime() >= 2 {
            break x;
        }
    };
    let yc = BoxedMontyForm::new(uint(&req.g, prec), &params)
        .pow(&x)
        .retrieve();
    let mut zm = BoxedMontyForm::new(uint(&req.ys, prec), &params).pow(&x);
    x.zeroize();
    let mut zn = zm.retrieve();
    zm.zeroize();
    let z = Zeroizing::new(Vec::from(zn.to_be_bytes()));
    zn.zeroize();
    let one = BoxedUint::one_with_precision(prec);
    let zero = vec![0u8; z.len()];
    let one_b = one.to_be_bytes();
    let p_minus_1 = p.wrapping_sub(&one).to_be_bytes();
    let degenerate =
        z.as_slice().ct_eq(&zero) | z.as_slice().ct_eq(&one_b) | z.as_slice().ct_eq(&p_minus_1);
    if bool::from(degenerate) {
        return Err(fatal(
            alert::ILLEGAL_PARAMETER,
            "the device's Diffie-Hellman public value gives a degenerate shared secret",
        ));
    }
    // RFC 5246 8.1.2: leading zero bytes of Z are stripped. The exponent is
    // never reused, so the length leaks nothing reusable (the Raccoon attack
    // needs a static secret).
    let start = z.iter().position(|&b| b != 0).unwrap_or(z.len() - 1);
    Ok(DhShared {
        yc: trim(&yc.to_be_bytes()).to_vec(),
        z: Zeroizing::new(z[start..].to_vec()),
    })
}

/// [`dh_compute`]'s outcome as bytes, for a computation run off the
/// session's task: 0, Yc's length (u16), Yc and Z; or 1, the alert and the
/// reason.
pub(crate) fn dh_outcome(req: &DhRequest) -> Vec<u8> {
    match dh_compute(req) {
        Ok(shared) => {
            let mut w = W::default();
            w.u8(0).vec16(&shared.yc).bytes(&shared.z);
            w.0
        }
        Err(f) => {
            let mut w = W::default();
            w.u8(1)
                .u8(f.alert.unwrap_or(alert::INTERNAL_ERROR))
                .bytes(f.reason.as_bytes());
            w.0
        }
    }
}

/// Read [`dh_outcome`]'s bytes, zeroizing them.
pub(crate) fn dh_from_outcome(mut bytes: Vec<u8>) -> Result<DhShared, Failure> {
    let parsed = (|| {
        let mut r = R::new(&bytes, "key exchange outcome");
        match r.u8()? {
            0 => {
                let yc = r.vec16()?.to_vec();
                let z = Zeroizing::new(r.take(r.b.len())?.to_vec());
                Ok(DhShared { yc, z })
            }
            _ => {
                let alert = r.u8()?;
                let reason = String::from_utf8_lossy(r.take(r.b.len())?).into_owned();
                Err(fatal(alert, reason))
            }
        }
    })();
    bytes.zeroize();
    parsed.map_err(|f| match f.alert {
        Some(alert::DECODE_ERROR) => fatal(alert::INTERNAL_ERROR, f.reason),
        _ => f,
    })
}

// --- Key schedule ------------------------------------------------------------

fn hmac_sha256(key: &[u8]) -> HmacSha256 {
    <HmacSha256 as KeyInit>::new_from_slice(key).expect("HMAC takes a key of any length")
}

fn hmac_sha1(key: &[u8]) -> HmacSha1 {
    <HmacSha1 as KeyInit>::new_from_slice(key).expect("HMAC takes a key of any length")
}

/// The TLS 1.2 PRF with SHA-256 (RFC 5246 5): P_SHA256(secret, label +
/// seed), as many bytes as `out` holds.
pub(crate) fn prf(secret: &[u8], label: &[u8], seed: &[u8], out: &mut [u8]) {
    let mac = hmac_sha256(secret);
    let mut labeled = Vec::with_capacity(label.len() + seed.len());
    labeled.extend_from_slice(label);
    labeled.extend_from_slice(seed);
    // A(1) = HMAC(secret, label + seed); A(i) = HMAC(secret, A(i-1)).
    let mut a = Zeroizing::new(
        mac.clone()
            .chain_update(&labeled)
            .finalize()
            .into_bytes()
            .to_vec(),
    );
    let mut filled = 0;
    while filled < out.len() {
        let block = Zeroizing::new(
            mac.clone()
                .chain_update(&a[..])
                .chain_update(&labeled)
                .finalize()
                .into_bytes()
                .to_vec(),
        );
        let n = (out.len() - filled).min(block.len());
        out[filled..filled + n].copy_from_slice(&block[..n]);
        filled += n;
        a = Zeroizing::new(
            mac.clone()
                .chain_update(&a[..])
                .finalize()
                .into_bytes()
                .to_vec(),
        );
    }
}

pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// The pre-master secret of DHE_PSK (RFC 4279 3): Z and the PSK, each with
/// a 16-bit length.
pub(crate) fn premaster(z: &[u8], psk: &[u8]) -> Zeroizing<Vec<u8>> {
    let mut w = W::default();
    w.vec16(z).vec16(psk);
    Zeroizing::new(std::mem::take(&mut w.0))
}

/// The master secret: RFC 7627's extended one over the session hash when
/// `session_hash` is given, else RFC 5246 8.1's over the randoms.
pub(crate) fn master_secret(
    premaster: &[u8],
    session_hash: Option<&[u8; 32]>,
    client_random: &[u8; 32],
    server_random: &[u8; 32],
) -> Zeroizing<[u8; 48]> {
    let mut master = Zeroizing::new([0u8; 48]);
    match session_hash {
        Some(hash) => prf(premaster, b"extended master secret", hash, &mut master[..]),
        None => {
            let mut seed = [0u8; 64];
            seed[..32].copy_from_slice(client_random);
            seed[32..].copy_from_slice(server_random);
            prf(premaster, b"master secret", &seed, &mut master[..]);
        }
    }
    master
}

/// One direction's record protection.
pub(crate) struct Direction {
    mac: Zeroizing<[u8; MAC_LEN]>,
    key: Zeroizing<[u8; 16]>,
    encrypt_then_mac: bool,
}

/// The key block (RFC 5246 6.3): the client's and the server's MAC keys,
/// then their encryption keys; TLS 1.2 CBC needs no IVs from it.
pub(crate) fn key_block(
    master: &[u8; 48],
    client_random: &[u8; 32],
    server_random: &[u8; 32],
    encrypt_then_mac: bool,
) -> (Direction, Direction) {
    let mut seed = [0u8; 64];
    seed[..32].copy_from_slice(server_random);
    seed[32..].copy_from_slice(client_random);
    let mut block = Zeroizing::new([0u8; 2 * MAC_LEN + 2 * 16]);
    prf(master, b"key expansion", &seed, &mut block[..]);
    let direction = |mac: &[u8], key: &[u8]| {
        let mut d = Direction {
            mac: Zeroizing::new([0u8; MAC_LEN]),
            key: Zeroizing::new([0u8; 16]),
            encrypt_then_mac,
        };
        d.mac.copy_from_slice(mac);
        d.key.copy_from_slice(key);
        d
    };
    let client = direction(&block[..20], &block[40..56]);
    let server = direction(&block[20..40], &block[56..72]);
    (client, server)
}

/// A Finished message's verify_data over the handshake so far.
pub(crate) fn verify_data(master: &[u8; 48], label: &[u8], transcript: &[u8]) -> [u8; 12] {
    let mut out = [0u8; 12];
    prf(master, label, &sha256(transcript), &mut out);
    out
}

// --- Record protection ---------------------------------------------------------

fn cbc_encrypt(key: &[u8; 16], iv: &[u8; 16], data: &mut [u8]) {
    let mut enc = Aes128CbcEnc::new(key.into(), iv.into());
    for chunk in data.as_chunks_mut::<BLOCK>().0 {
        let block: &mut cbc::cipher::Block<aes::Aes128> = chunk.into();
        enc.encrypt_block(block);
    }
}

fn cbc_decrypt(key: &[u8; 16], iv: &[u8], data: &mut [u8]) {
    let iv: &[u8; 16] = iv.try_into().expect("a whole block");
    let mut dec = Aes128CbcDec::new(key.into(), iv.into());
    for chunk in data.as_chunks_mut::<BLOCK>().0 {
        let block: &mut cbc::cipher::Block<aes::Aes128> = chunk.into();
        dec.decrypt_block(block);
    }
}

/// What the MAC covers before the data: the sequence number (DTLS: epoch
/// and sequence), the type, the version and the length.
fn mac_header(seq: u64, ty: u8, version: u16, len: usize) -> [u8; 13] {
    let mut h = [0u8; 13];
    h[..8].copy_from_slice(&seq.to_be_bytes());
    h[8] = ty;
    h[9..11].copy_from_slice(&version.to_be_bytes());
    h[11..].copy_from_slice(&(len as u16).to_be_bytes());
    h
}

/// SHA-1 compressions HMAC's inner hash makes over `len` bytes after the
/// 13-byte header (the key block is hashed when the key is set).
fn sha1_blocks(len: usize) -> usize {
    (13 + len + 9).div_ceil(64)
}

impl Direction {
    /// Protect one record's plaintext: the IV, then the ciphertext, and the
    /// MAC after it with encrypt-then-MAC.
    pub(crate) fn seal(
        &self,
        seq: u64,
        ty: u8,
        version: u16,
        plain: &[u8],
    ) -> Result<Vec<u8>, Failure> {
        let mut iv = [0u8; 16];
        random(&mut iv)?;
        let mut body = Vec::with_capacity(plain.len() + MAC_LEN + BLOCK);
        body.extend_from_slice(plain);
        if !self.encrypt_then_mac {
            let tag = hmac_sha1(&self.mac[..])
                .chain_update(mac_header(seq, ty, version, plain.len()))
                .chain_update(plain)
                .finalize()
                .into_bytes();
            body.extend_from_slice(&tag);
        }
        let pad = BLOCK - 1 - body.len() % BLOCK;
        body.extend(std::iter::repeat_n(pad as u8, pad + 1));
        cbc_encrypt(&self.key, &iv, &mut body);
        let mut out = Vec::with_capacity(BLOCK + body.len() + MAC_LEN);
        out.extend_from_slice(&iv);
        out.extend_from_slice(&body);
        body.zeroize();
        if self.encrypt_then_mac {
            let tag = hmac_sha1(&self.mac[..])
                .chain_update(mac_header(seq, ty, version, out.len()))
                .chain_update(&out)
                .finalize()
                .into_bytes();
            out.extend_from_slice(&tag);
        }
        Ok(out)
    }

    /// Open one record's fragment. Every failure is the same `Err(())`.
    pub(crate) fn open(
        &self,
        seq: u64,
        ty: u8,
        version: u16,
        fragment: &[u8],
    ) -> Result<Vec<u8>, ()> {
        if self.encrypt_then_mac {
            self.open_etm(seq, ty, version, fragment)
        } else {
            self.open_mte(seq, ty, version, fragment)
        }
    }

    fn open_etm(&self, seq: u64, ty: u8, version: u16, fragment: &[u8]) -> Result<Vec<u8>, ()> {
        // The IV, at least one block, and the MAC; lengths are public.
        if fragment.len() < 2 * BLOCK + MAC_LEN || !(fragment.len() - MAC_LEN).is_multiple_of(BLOCK)
        {
            return Err(());
        }
        let (sealed, tag) = fragment.split_at(fragment.len() - MAC_LEN);
        hmac_sha1(&self.mac[..])
            .chain_update(mac_header(seq, ty, version, sealed.len()))
            .chain_update(sealed)
            .verify_slice(tag)
            .map_err(|_| ())?;
        // Authenticated: the padding check can no longer be an oracle.
        let (iv, ct) = sealed.split_at(BLOCK);
        let mut pt = ct.to_vec();
        cbc_decrypt(&self.key, iv, &mut pt);
        let pad = usize::from(pt[pt.len() - 1]);
        let ok = pad < pt.len()
            && pt[pt.len() - 1 - pad..]
                .iter()
                .all(|&b| usize::from(b) == pad);
        if !ok {
            pt.zeroize();
            return Err(());
        }
        pt.truncate(pt.len() - 1 - pad);
        Ok(pt)
    }

    /// MAC-then-encrypt, in time that does not depend on the padding.
    fn open_mte(&self, seq: u64, ty: u8, version: u16, fragment: &[u8]) -> Result<Vec<u8>, ()> {
        // The IV and at least two blocks (a MAC and a padding byte); lengths
        // are public.
        if fragment.len() < 3 * BLOCK || !(fragment.len() - BLOCK).is_multiple_of(BLOCK) {
            return Err(());
        }
        let (iv, ct) = fragment.split_at(BLOCK);
        let mut pt = ct.to_vec();
        cbc_decrypt(&self.key, iv, &mut pt);
        let len = pt.len();
        let pad = u32::from(pt[len - 1]);
        // The padding and the MAC fit: pad + 1 + MAC_LEN <= len.
        let mut good: Choice = !(pad + 1 + MAC_LEN as u32).ct_gt(&(len as u32));
        // Every byte the padding may cover equals the padding length.
        for i in 0..len.min(256) {
            let in_pad = !(i as u32).ct_gt(&pad);
            let same = pt[len - 1 - i].ct_eq(&(pad as u8));
            good &= !in_pad | same;
        }
        let pad = u32::conditional_select(&0, &pad, good) as usize;
        let n = len - MAC_LEN - 1 - pad;
        let computed = hmac_sha1(&self.mac[..])
            .chain_update(mac_header(seq, ty, version, n))
            .chain_update(&pt[..n])
            .finalize()
            .into_bytes();
        // As many compressions again as the longest data would have needed.
        let extra = sha1_blocks(len - MAC_LEN - 1) - sha1_blocks(n);
        let mut dummy = Sha1::new();
        for _ in 0..extra {
            dummy.update([0u8; 64]);
        }
        std::hint::black_box(dummy.finalize());
        // The received MAC, read without an address that depends on n.
        let mut received = [0u8; MAC_LEN];
        let start = len.saturating_sub(MAC_LEN + 256);
        for (k, &b) in pt.iter().enumerate().skip(start) {
            for (j, r) in received.iter_mut().enumerate() {
                let here = (k as u32).ct_eq(&((n + j) as u32));
                *r |= u8::conditional_select(&0, &b, here);
            }
        }
        good &= computed.as_slice().ct_eq(&received);
        if bool::from(good) {
            pt.truncate(n);
            Ok(pt)
        } else {
            pt.zeroize();
            Err(())
        }
    }
}

/// How an alert from the device ends, or does not end, the session.
pub(crate) enum AlertOutcome {
    Ignore,
    Close,
    Fatal(Failure),
}

pub(crate) fn alert_outcome(level: u8, desc: u8, handshaking: bool) -> AlertOutcome {
    if desc == alert::CLOSE_NOTIFY {
        return AlertOutcome::Close;
    }
    if level == alert::WARNING && matches!(desc, alert::USER_CANCELED | alert::NO_RENEGOTIATION) {
        return AlertOutcome::Ignore;
    }
    let hint = if handshaking
        && matches!(
            desc,
            alert::BAD_RECORD_MAC
                | alert::DECRYPT_ERROR
                | alert::UNKNOWN_PSK_IDENTITY
                | alert::HANDSHAKE_FAILURE
        ) {
        " (check the pre-shared key and its identity)"
    } else {
        ""
    };
    AlertOutcome::Fatal(Failure {
        alert: None,
        reason: format!(
            "the device sent {} alert {} ({desc}){hint}",
            if level == alert::WARNING {
                "warning"
            } else {
                "fatal"
            },
            alert::name(desc)
        ),
    })
}

/// The error for a Finished that does not verify.
pub(crate) fn finished_mismatch() -> Failure {
    fatal(
        alert::DECRYPT_ERROR,
        "the device's Finished did not verify (check the pre-shared key and its identity)",
    )
}

// --- TLS client ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum St {
    ServerHello,
    KeyExchange,
    HelloDone,
    /// Waiting for the Diffie-Hellman computation.
    Dh,
    ChangeCipherSpec,
    Finished,
    Open,
    Closed,
}

/// A TLS 1.2 client session over a byte stream.
pub(crate) struct TlsClient {
    psk: Psk,
    st: St,
    client_random: [u8; 32],
    server_random: [u8; 32],
    /// Every handshake message so far, for the session hash and Finished.
    transcript: Vec<u8>,
    session: Session,
    dh: Option<DhRequest>,
    master: Zeroizing<[u8; 48]>,
    write: Option<Direction>,
    read: Option<Direction>,
    pending_read: Option<Direction>,
    write_seq: u64,
    read_seq: u64,
    input: Vec<u8>,
    handshake: Vec<u8>,
    empty: u32,
}

impl TlsClient {
    /// Start a session: the ClientHello goes out in `out`.
    pub(crate) fn new(psk: Psk, out: &mut Vec<Event>) -> Result<TlsClient, Failure> {
        let mut client_random = [0u8; 32];
        random(&mut client_random)?;
        let hello = handshake_message(
            HT_CLIENT_HELLO,
            &client_hello_body(TLS12, &client_random, None),
        );
        let mut c = TlsClient {
            psk,
            st: St::ServerHello,
            client_random,
            server_random: [0; 32],
            transcript: hello.clone(),
            session: Session::default(),
            dh: None,
            master: Zeroizing::new([0; 48]),
            write: None,
            read: None,
            pending_read: None,
            write_seq: 0,
            read_seq: 0,
            input: Vec::new(),
            handshake: Vec::new(),
            empty: 0,
        };
        // The first record's version is 0x0301, which every TLS server
        // reads (RFC 5246 E.1); the ClientHello itself says 1.2.
        let bytes = c.records(CT_HANDSHAKE, 0x0301, &hello)?;
        out.push(Event::Send(bytes));
        Ok(c)
    }

    pub(crate) fn is_open(&self) -> bool {
        self.st == St::Open
    }

    /// Bytes from the stream.
    pub(crate) fn receive(&mut self, bytes: &[u8], out: &mut Vec<Event>) -> Result<(), Failure> {
        if self.st == St::Closed {
            return Ok(());
        }
        self.input.extend_from_slice(bytes);
        let result = self.read_records(out);
        if let Err(f) = &result {
            self.abort(f, out);
        }
        result
    }

    /// The Diffie-Hellman outcome asked for by [`Event::Dh`]: the
    /// ClientKeyExchange, ChangeCipherSpec and Finished go out.
    pub(crate) fn dh_done(
        &mut self,
        shared: Result<DhShared, Failure>,
        out: &mut Vec<Event>,
    ) -> Result<(), Failure> {
        if self.st != St::Dh {
            return Ok(());
        }
        let result = self.key_exchange(shared, out);
        if let Err(f) = &result {
            self.abort(f, out);
        }
        result
    }

    /// Application data, as records.
    pub(crate) fn seal(&mut self, data: &[u8]) -> Result<Vec<u8>, Failure> {
        if self.st != St::Open {
            return Err(fatal(alert::INTERNAL_ERROR, "the session is not open"));
        }
        self.records(CT_APPLICATION_DATA, TLS12, data)
    }

    /// End the session: the close_notify to send, if the session is still
    /// up.
    pub(crate) fn close(&mut self) -> Option<Vec<u8>> {
        if self.st == St::Closed {
            return None;
        }
        let bytes = self
            .records(CT_ALERT, TLS12, &[alert::WARNING, alert::CLOSE_NOTIFY])
            .ok();
        self.shut();
        bytes
    }

    fn shut(&mut self) {
        self.st = St::Closed;
        self.write = None;
        self.read = None;
        self.pending_read = None;
        self.master.zeroize();
        self.input.clear();
        self.handshake.clear();
    }

    fn abort(&mut self, f: &Failure, out: &mut Vec<Event>) {
        if let Some(a) = f.alert {
            if let Ok(bytes) = self.records(CT_ALERT, TLS12, &[alert::FATAL, a]) {
                out.push(Event::Send(bytes));
            }
        }
        self.shut();
    }

    /// `data` as records of at most 2^14 bytes, protected once the
    /// ChangeCipherSpec is sent.
    fn records(&mut self, ty: u8, version: u16, data: &[u8]) -> Result<Vec<u8>, Failure> {
        let mut out = Vec::new();
        for chunk in data.chunks(MAX_PLAINTEXT) {
            let fragment = match &self.write {
                Some(d) => {
                    let seq = self.write_seq;
                    self.write_seq = seq.checked_add(1).ok_or_else(|| {
                        fatal(alert::INTERNAL_ERROR, "the sequence number would wrap")
                    })?;
                    d.seal(seq, ty, version, chunk)?
                }
                None => chunk.to_vec(),
            };
            out.push(ty);
            out.extend_from_slice(&version.to_be_bytes());
            out.extend_from_slice(&(fragment.len() as u16).to_be_bytes());
            out.extend_from_slice(&fragment);
        }
        Ok(out)
    }

    fn read_records(&mut self, out: &mut Vec<Event>) -> Result<(), Failure> {
        while self.st != St::Closed && self.input.len() >= 5 {
            let ty = self.input[0];
            let version = u16::from_be_bytes([self.input[1], self.input[2]]);
            let len = usize::from(u16::from_be_bytes([self.input[3], self.input[4]]));
            // Before the ServerHello any 3.x record version (an alert from a
            // server refusing the hello may say 3.1); after it, 1.2 only.
            if version >> 8 != 3 || (self.st != St::ServerHello && version != TLS12) {
                return Err(fatal(
                    alert::PROTOCOL_VERSION,
                    format!("a record of version {version:#06x}"),
                ));
            }
            if len > MAX_CIPHERTEXT {
                return Err(fatal(
                    alert::RECORD_OVERFLOW,
                    "a record longer than TLS allows",
                ));
            }
            if self.input.len() < 5 + len {
                break;
            }
            let fragment: Vec<u8> = self.input.drain(..5 + len).skip(5).collect();
            self.record(ty, fragment, out)?;
        }
        Ok(())
    }

    fn record(&mut self, ty: u8, fragment: Vec<u8>, out: &mut Vec<Event>) -> Result<(), Failure> {
        let plain = match &self.read {
            Some(d) => {
                let seq = self.read_seq;
                self.read_seq = seq.checked_add(1).ok_or_else(|| {
                    fatal(alert::INTERNAL_ERROR, "the sequence number would wrap")
                })?;
                d.open(seq, ty, TLS12, &fragment)
                    .map_err(|()| bad_record())?
            }
            None => fragment,
        };
        if plain.len() > MAX_PLAINTEXT {
            return Err(fatal(
                alert::RECORD_OVERFLOW,
                "a record longer than TLS allows",
            ));
        }
        match ty {
            CT_CHANGE_CIPHER_SPEC => {
                if plain != [1] {
                    return Err(fatal(alert::DECODE_ERROR, "a malformed ChangeCipherSpec"));
                }
                if self.st != St::ChangeCipherSpec || !self.handshake.is_empty() {
                    return Err(unexpected("a ChangeCipherSpec out of place"));
                }
                self.read = self.pending_read.take();
                self.read_seq = 0;
                self.st = St::Finished;
                Ok(())
            }
            CT_ALERT => self.alert(&plain, out),
            CT_HANDSHAKE => {
                if plain.is_empty() {
                    return Err(unexpected("an empty handshake record"));
                }
                self.handshake.extend_from_slice(&plain);
                self.handshake_messages(out)
            }
            CT_APPLICATION_DATA => {
                if self.st != St::Open {
                    return Err(unexpected("application data before the handshake finished"));
                }
                if plain.is_empty() {
                    self.empty += 1;
                    if self.empty > MAX_EMPTY {
                        return Err(unexpected("too many empty records in a row"));
                    }
                } else {
                    self.empty = 0;
                    out.push(Event::Data(plain));
                }
                Ok(())
            }
            _ => Err(unexpected(format!("a record of unknown type {ty}"))),
        }
    }

    fn alert(&mut self, plain: &[u8], out: &mut Vec<Event>) -> Result<(), Failure> {
        if plain.is_empty() || !plain.len().is_multiple_of(2) {
            return Err(fatal(alert::DECODE_ERROR, "a malformed alert"));
        }
        for a in plain.chunks(2) {
            match alert_outcome(a[0], a[1], self.st != St::Open) {
                AlertOutcome::Ignore => {}
                AlertOutcome::Close => {
                    if let Some(bytes) = self.close() {
                        out.push(Event::Send(bytes));
                    }
                    out.push(Event::Closed);
                    return Ok(());
                }
                AlertOutcome::Fatal(f) => return Err(f),
            }
        }
        Ok(())
    }

    fn handshake_messages(&mut self, out: &mut Vec<Event>) -> Result<(), Failure> {
        while self.st != St::Closed && self.handshake.len() >= 4 {
            let len = usize::from(self.handshake[1]) << 16
                | usize::from(self.handshake[2]) << 8
                | usize::from(self.handshake[3]);
            if len > MAX_HANDSHAKE {
                return Err(fatal(alert::DECODE_ERROR, "a handshake message too long"));
            }
            if self.handshake.len() < 4 + len {
                break;
            }
            let msg: Vec<u8> = self.handshake.drain(..4 + len).collect();
            self.handshake_message(msg, out)?;
        }
        Ok(())
    }

    fn handshake_message(&mut self, msg: Vec<u8>, out: &mut Vec<Event>) -> Result<(), Failure> {
        let ty = msg[0];
        let body = &msg[4..];
        if ty == HT_HELLO_REQUEST {
            if !body.is_empty() {
                return Err(fatal(alert::DECODE_ERROR, "a malformed HelloRequest"));
            }
            // Renegotiation is refused; during the handshake the request is
            // ignored (RFC 5246 7.4.1.1).
            if self.st == St::Open {
                let bytes =
                    self.records(CT_ALERT, TLS12, &[alert::WARNING, alert::NO_RENEGOTIATION])?;
                out.push(Event::Send(bytes));
            }
            return Ok(());
        }
        match (self.st, ty) {
            (St::ServerHello, HT_SERVER_HELLO) => {
                let hello = parse_server_hello(body, TLS12)?;
                self.server_random = hello.random;
                self.session.extended_master_secret = hello.extended_master_secret;
                self.session.encrypt_then_mac = hello.encrypt_then_mac;
                self.transcript.extend_from_slice(&msg);
                self.st = St::KeyExchange;
            }
            (St::KeyExchange, HT_SERVER_KEY_EXCHANGE) => {
                let (hint, dh) = parse_server_key_exchange(body)?;
                self.session.hint = hint;
                self.dh = Some(dh);
                self.transcript.extend_from_slice(&msg);
                self.st = St::HelloDone;
            }
            (St::HelloDone, HT_SERVER_HELLO_DONE) => {
                if !body.is_empty() {
                    return Err(fatal(alert::DECODE_ERROR, "a malformed ServerHelloDone"));
                }
                self.transcript.extend_from_slice(&msg);
                self.st = St::Dh;
                let dh = self.dh.take().expect("set by the ServerKeyExchange");
                out.push(Event::Dh(dh));
            }
            (St::Finished, HT_FINISHED) => {
                let expected = verify_data(&self.master, b"server finished", &self.transcript);
                if body.len() != expected.len() || !bool::from(body.ct_eq(&expected)) {
                    return Err(finished_mismatch());
                }
                self.st = St::Open;
                self.transcript = Vec::new();
                out.push(Event::Established(self.session.clone()));
            }
            (st, ty) => {
                return Err(unexpected(format!(
                    "{} from the device while waiting for {}",
                    handshake_name(ty),
                    match st {
                        St::ServerHello => "its ServerHello",
                        St::KeyExchange => "its ServerKeyExchange",
                        St::HelloDone => "its ServerHelloDone",
                        St::Dh => "nothing",
                        St::ChangeCipherSpec => "its ChangeCipherSpec",
                        St::Finished => "its Finished",
                        St::Open | St::Closed => "application data",
                    }
                )))
            }
        }
        Ok(())
    }

    fn key_exchange(
        &mut self,
        shared: Result<DhShared, Failure>,
        out: &mut Vec<Event>,
    ) -> Result<(), Failure> {
        let shared = shared?;
        let cke = handshake_message(
            HT_CLIENT_KEY_EXCHANGE,
            &client_key_exchange_body(&self.psk.identity, &shared.yc),
        );
        self.transcript.extend_from_slice(&cke);
        let pre = premaster(&shared.z, &self.psk.key);
        let hash = sha256(&self.transcript);
        self.master = master_secret(
            &pre,
            self.session.extended_master_secret.then_some(&hash),
            &self.client_random,
            &self.server_random,
        );
        let (client, server) = key_block(
            &self.master,
            &self.client_random,
            &self.server_random,
            self.session.encrypt_then_mac,
        );
        let mut bytes = self.records(CT_HANDSHAKE, TLS12, &cke)?;
        bytes.extend(self.records(CT_CHANGE_CIPHER_SPEC, TLS12, &[1])?);
        self.write = Some(client);
        self.write_seq = 0;
        self.pending_read = Some(server);
        let finished = handshake_message(
            HT_FINISHED,
            &verify_data(&self.master, b"client finished", &self.transcript),
        );
        self.transcript.extend_from_slice(&finished);
        bytes.extend(self.records(CT_HANDSHAKE, TLS12, &finished)?);
        out.push(Event::Send(bytes));
        self.st = St::ChangeCipherSpec;
        Ok(())
    }
}

#[cfg(test)]
#[path = "aes70_tls_tests.rs"]
pub(crate) mod tests;
