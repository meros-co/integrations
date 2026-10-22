//! Tests of the TLS client: published vectors for its primitives, an
//! in-process fake device for the handshake and the refusals, and, where
//! OpenSSL is installed, a handshake with `openssl s_server` on loopback.

use super::*;

pub(crate) fn unhex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// RFC 7919's ffdhe2048 group, g = 2.
pub(crate) const FFDHE2048: &str = "FFFFFFFFFFFFFFFFADF85458A2BB4A9AAFDC5620273D3CF1D8B9C583CE2D3695A9E13641146433FBCC939DCE249B3EF97D2FE363630C75D8F681B202AEC4617AD3DF1ED5D5FD65612433F51F5F066ED0856365553DED1AF3B557135E7F57C935984F0C70E0E68B77E2A689DAF3EFE8721DF158A136ADE73530ACCA4F483A797ABC0AB182B324FB61D108A94BB2C8E3FBB96ADAB760D7F4681D4F42A3DE394DF4AE56EDE76372BB190B07A7C8EE0A6D709E02FCE1CDF7E2ECC03404CD28342F619172FE9CE98583FF8E4F1232EEF28183C3FE3B1B4C6FAD733BB5FCBC2EC22005C58EF1837D1683B2C6F34A26C1B2EFFA886B423861285C97FFFFFFFFFFFFFFFF";

pub(crate) fn psk(key: &[u8]) -> Psk {
    Psk {
        identity: HINT.as_bytes().to_vec(),
        key: Zeroizing::new(key.to_vec()),
    }
}

pub(crate) const KEY: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];

// --- Published vectors ----------------------------------------------------------

#[test]
fn prf_sha256_matches_the_published_vector() {
    // The TLS 1.2 PRF (SHA-256) vector circulated on the IETF TLS list,
    // checked against an independent HMAC-SHA256 implementation.
    let mut out = [0u8; 100];
    prf(
        &unhex("9bbe436ba940f017b17652849a71db35"),
        b"test label",
        &unhex("a0ba9f936cda311827a6f796ffd5198c"),
        &mut out,
    );
    assert_eq!(
        out.to_vec(),
        unhex(
            "e3f229ba727be17b8d122620557cd453c2aab21d07c3d495329b52d4e61edb5a6b301791e90d35c9c9a46b4e14baf9af\
             0fa022f7077def17abfd3797c0564bab4fbc91666e9def9b97fce34f796789baa48082d122ee42c5a72e5a5110fff701\
             87347b66"
        )
    );
}

#[test]
fn hmac_matches_rfc_2202_and_rfc_4231() {
    let tag = |m: HmacSha1| m.finalize().into_bytes().to_vec();
    // RFC 2202 test cases 1 and 2 (HMAC-SHA1).
    assert_eq!(
        tag(hmac_sha1(&[0x0b; 20]).chain_update(b"Hi There")),
        unhex("b617318655057264e28bc0b6fb378c8ef146be00")
    );
    assert_eq!(
        tag(hmac_sha1(b"Jefe").chain_update(b"what do ya want for nothing?")),
        unhex("effcdf6ae5eb2fa2d27416d5f184df9c259a7c79")
    );
    // RFC 4231 test cases 1 and 2 (HMAC-SHA256).
    let tag = |m: HmacSha256| m.finalize().into_bytes().to_vec();
    assert_eq!(
        tag(hmac_sha256(&[0x0b; 20]).chain_update(b"Hi There")),
        unhex("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7")
    );
    assert_eq!(
        tag(hmac_sha256(b"Jefe").chain_update(b"what do ya want for nothing?")),
        unhex("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843")
    );
}

#[test]
fn aes_128_cbc_matches_nist_sp_800_38a() {
    // NIST SP 800-38A F.2.1 and F.2.2 (CBC-AES128).
    let key: [u8; 16] = unhex("2b7e151628aed2a6abf7158809cf4f3c")
        .try_into()
        .unwrap();
    let iv: [u8; 16] = unhex("000102030405060708090a0b0c0d0e0f")
        .try_into()
        .unwrap();
    let plain = unhex(
        "6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51\
         30c81c46a35ce411e5fbc1191a0a52eff69f2445df4f9b17ad2b417be66c3710",
    );
    let cipher = unhex(
        "7649abac8119b246cee98e9b12e9197d5086cb9b507219ee95db113a917678b2\
         73bed6b8e3c1743b7116e69e222295163ff1caa1681fac09120eca307586e1a7",
    );
    let mut buf = plain.clone();
    cbc_encrypt(&key, &iv, &mut buf);
    assert_eq!(buf, cipher);
    cbc_decrypt(&key, &iv, &mut buf);
    assert_eq!(buf, plain);
}

// --- Records ---------------------------------------------------------------------

pub(crate) fn directions(etm: bool) -> (Direction, Direction) {
    key_block(&[7; 48], &[1; 32], &[2; 32], etm)
}

#[test]
fn records_round_trip_at_every_padding_length() {
    for etm in [false, true] {
        let (c, _) = directions(etm);
        for len in 0..80 {
            let data: Vec<u8> = (0..len as u8).collect();
            let sealed = c.seal(9, CT_APPLICATION_DATA, TLS12, &data).unwrap();
            assert_eq!(
                c.open(9, CT_APPLICATION_DATA, TLS12, &sealed),
                Ok(data.clone())
            );
            // A different sequence number, type or version fails.
            assert!(c.open(10, CT_APPLICATION_DATA, TLS12, &sealed).is_err());
            assert!(c.open(9, CT_HANDSHAKE, TLS12, &sealed).is_err());
            assert!(c.open(9, CT_APPLICATION_DATA, 0xfefd, &sealed).is_err());
            // Any byte changed fails.
            for i in 0..sealed.len() {
                let mut t = sealed.clone();
                t[i] ^= 0x01;
                assert!(
                    c.open(9, CT_APPLICATION_DATA, TLS12, &t).is_err(),
                    "byte {i} of {len}"
                );
            }
            // Cut short fails.
            assert!(c
                .open(9, CT_APPLICATION_DATA, TLS12, &sealed[..sealed.len() - 1])
                .is_err());
        }
    }
}

/// A MAC-then-encrypt record with chosen padding bytes.
fn mte_record(d: &Direction, seq: u64, data: &[u8], padding: &[u8], good_mac: bool) -> Vec<u8> {
    let mut tag = hmac_sha1(&d.mac[..])
        .chain_update(mac_header(seq, CT_APPLICATION_DATA, TLS12, data.len()))
        .chain_update(data)
        .finalize()
        .into_bytes()
        .to_vec();
    if !good_mac {
        tag[0] ^= 0x80;
    }
    let mut body = data.to_vec();
    body.extend(tag);
    body.extend_from_slice(padding);
    assert_eq!(body.len() % 16, 0);
    let iv = [0x5a; 16];
    cbc_encrypt(&d.key, &iv, &mut body);
    let mut out = iv.to_vec();
    out.extend(body);
    out
}

#[test]
fn bad_padding_and_bad_mac_fail_alike() {
    let (c, _) = directions(false);
    let data = [0x41u8; 11];
    // 11 + 20 = 31: one byte of padding (value 0) makes 32.
    assert_eq!(
        c.open(
            3,
            CT_APPLICATION_DATA,
            TLS12,
            &mte_record(&c, 3, &data, &[0], true)
        ),
        Ok(data.to_vec())
    );
    // Longer valid padding: 17 bytes of value 16.
    assert_eq!(
        c.open(
            3,
            CT_APPLICATION_DATA,
            TLS12,
            &mte_record(&c, 3, &data, &[16; 17], true)
        ),
        Ok(data.to_vec())
    );
    let bad_mac = mte_record(&c, 3, &data, &[0], false);
    let bad_padding = mte_record(
        &c,
        3,
        &data,
        &[
            16, 16, 16, 16, 16, 16, 16, 16, 15, 16, 16, 16, 16, 16, 16, 16, 16,
        ],
        true,
    );
    let too_long_padding = mte_record(&c, 3, &data, &[0x40; 17], true);
    for record in [&bad_mac, &bad_padding, &too_long_padding] {
        assert_eq!(c.open(3, CT_APPLICATION_DATA, TLS12, record), Err(()));
    }
}

// --- A fake device -------------------------------------------------------------

/// What the fake device does wrong, for the refusal tests.
#[derive(Clone, Default)]
pub(crate) struct Faults {
    pub(crate) p: Option<Vec<u8>>,
    pub(crate) g: Option<Vec<u8>>,
    pub(crate) ys: Option<Vec<u8>>,
    pub(crate) suite: Option<u16>,
    pub(crate) version: Option<u16>,
    pub(crate) extra_extension: Option<(u16, Vec<u8>)>,
    pub(crate) certificate: bool,
    pub(crate) bad_finished: bool,
    pub(crate) early_data: bool,
}

/// The device's side of DHE_PSK, as far as the tests need it.
pub(crate) struct Fake {
    pub(crate) psk: Vec<u8>,
    pub(crate) hint: Vec<u8>,
    /// Agree to the extended master secret and encrypt-then-MAC when
    /// offered.
    pub(crate) ems: bool,
    pub(crate) etm: bool,
    pub(crate) faults: Faults,
    pub(crate) p: Vec<u8>,
    y: BoxedUint,
    pub(crate) client_random: [u8; 32],
    pub(crate) server_random: [u8; 32],
    pub(crate) transcript: Vec<u8>,
    pub(crate) session: Session,
    pub(crate) identity: Vec<u8>,
    pub(crate) master: [u8; 48],
    pub(crate) read: Option<Direction>,
    pub(crate) write: Option<Direction>,
    pub(crate) pending_read: Option<Direction>,
    pub(crate) pending_write: Option<Direction>,
    pub(crate) read_seq: u64,
    pub(crate) write_seq: u64,
    input: Vec<u8>,
    pub(crate) received: Vec<u8>,
    pub(crate) alerts: Vec<(u8, u8)>,
    pub(crate) finished: bool,
}

impl Fake {
    pub(crate) fn new(key: &[u8]) -> Fake {
        let mut y = [0u8; 32];
        random(&mut y).unwrap();
        Fake {
            psk: key.to_vec(),
            hint: HINT.as_bytes().to_vec(),
            ems: true,
            etm: true,
            faults: Faults::default(),
            p: unhex(FFDHE2048),
            y: BoxedUint::from_be_slice(&y, 2048).unwrap(),
            client_random: [0; 32],
            server_random: [0x33; 32],
            transcript: Vec::new(),
            session: Session::default(),
            identity: Vec::new(),
            master: [0; 48],
            read: None,
            write: None,
            pending_read: None,
            pending_write: None,
            read_seq: 0,
            write_seq: 0,
            input: Vec::new(),
            received: Vec::new(),
            alerts: Vec::new(),
            finished: false,
        }
    }

    fn params(&self) -> BoxedMontyParams {
        let p = BoxedUint::from_be_slice(&self.p, 2048).unwrap();
        BoxedMontyParams::new_vartime(Odd::new(p).into_option().unwrap())
    }

    /// g^y mod p for the fake's group.
    pub(crate) fn ys(&self) -> Vec<u8> {
        let g = BoxedUint::from_be_slice(&[2], 2048).unwrap();
        trim(
            &BoxedMontyForm::new(g, &self.params())
                .pow(&self.y)
                .retrieve()
                .to_be_bytes(),
        )
        .to_vec()
    }

    /// The shared secret from the client's public value.
    pub(crate) fn z(&self, yc: &[u8]) -> Vec<u8> {
        let yc = BoxedUint::from_be_slice(trim(yc), 2048).unwrap();
        trim(
            &BoxedMontyForm::new(yc, &self.params())
                .pow(&self.y)
                .retrieve()
                .to_be_bytes(),
        )
        .to_vec()
    }

    /// Parse a ClientHello body: the client random and which extensions it
    /// offers; DTLS bodies have a cookie.
    pub(crate) fn client_hello(&mut self, body: &[u8], dtls: bool) {
        let mut r = R::new(body, "ClientHello");
        r.u16().unwrap();
        self.client_random.copy_from_slice(r.take(32).unwrap());
        r.vec8().unwrap();
        if dtls {
            r.vec8().unwrap();
        }
        assert_eq!(r.vec16().unwrap(), SUITE.to_be_bytes());
        assert_eq!(r.vec8().unwrap(), [0]);
        let mut ext = R::new(r.vec16().unwrap(), "extensions");
        r.end().unwrap();
        let (mut ems, mut etm, mut ri) = (false, false, false);
        while !ext.is_empty() {
            let ty = ext.u16().unwrap();
            let data = ext.vec16().unwrap();
            match ty {
                EXT_EXTENDED_MASTER_SECRET => ems = true,
                EXT_ENCRYPT_THEN_MAC => etm = true,
                EXT_RENEGOTIATION_INFO => ri = data == [0],
                _ => panic!("unexpected extension {ty:#x}"),
            }
        }
        assert!(ri, "renegotiation_info offered empty");
        self.session.extended_master_secret = ems && self.ems;
        self.session.encrypt_then_mac = etm && self.etm;
    }

    /// The ServerHello, ServerKeyExchange and ServerHelloDone bodies (or a
    /// Certificate in place of the key exchange).
    pub(crate) fn flight(&self, version: u16) -> Vec<(u8, Vec<u8>)> {
        let mut hello = W::default();
        hello
            .u16(self.faults.version.unwrap_or(version))
            .bytes(&self.server_random)
            .vec8(&[9; 8])
            .u16(self.faults.suite.unwrap_or(SUITE))
            .u8(0);
        let mut ext = W::default();
        ext.u16(EXT_RENEGOTIATION_INFO).vec16(&[0]);
        if self.session.extended_master_secret {
            ext.u16(EXT_EXTENDED_MASTER_SECRET).vec16(&[]);
        }
        if self.session.encrypt_then_mac {
            ext.u16(EXT_ENCRYPT_THEN_MAC).vec16(&[]);
        }
        if let Some((ty, data)) = &self.faults.extra_extension {
            ext.u16(*ty).vec16(data);
        }
        hello.vec16(&ext.0);
        let mut flight = vec![(HT_SERVER_HELLO, hello.0)];
        if self.faults.certificate {
            flight.push((11, vec![0, 0, 0]));
        }
        let mut ske = W::default();
        ske.vec16(&self.hint)
            .vec16(self.faults.p.as_deref().unwrap_or(&self.p))
            .vec16(self.faults.g.as_deref().unwrap_or(&[2]))
            .vec16(&self.faults.ys.clone().unwrap_or_else(|| self.ys()));
        flight.push((HT_SERVER_KEY_EXCHANGE, ske.0));
        flight.push((HT_SERVER_HELLO_DONE, Vec::new()));
        flight
    }

    /// The client's key exchange: derive the keys as the device would.
    pub(crate) fn client_key_exchange(&mut self, body: &[u8]) {
        let mut r = R::new(body, "ClientKeyExchange");
        self.identity = r.vec16().unwrap().to_vec();
        let yc = r.vec16().unwrap().to_vec();
        r.end().unwrap();
        let pre = premaster(&self.z(&yc), &self.psk);
        let hash = sha256(&self.transcript);
        self.master = *master_secret(
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
        self.pending_read = Some(client);
        self.pending_write = Some(server);
    }

    /// The device's Finished body over the transcript so far.
    pub(crate) fn finished_body(&self) -> Vec<u8> {
        let mut v = verify_data(&self.master, b"server finished", &self.transcript).to_vec();
        if self.faults.bad_finished {
            v[0] ^= 1;
        }
        v
    }

    /// Check the client's Finished body against the transcript before it.
    pub(crate) fn client_finished_ok(&self, body: &[u8]) -> bool {
        verify_data(&self.master, b"client finished", &self.transcript) == body
    }

    // TLS framing.

    pub(crate) fn record(&mut self, ty: u8, data: &[u8]) -> Vec<u8> {
        let fragment = match &self.write {
            Some(d) => {
                let seq = self.write_seq;
                self.write_seq += 1;
                d.seal(seq, ty, TLS12, data).unwrap()
            }
            None => data.to_vec(),
        };
        let mut w = W::default();
        w.u8(ty).u16(TLS12).vec16(&fragment);
        w.0
    }

    /// Bytes from the client; the bytes the device answers with.
    pub(crate) fn receive(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.input.extend_from_slice(bytes);
        let mut out = Vec::new();
        while self.input.len() >= 5 {
            let len = usize::from(u16::from_be_bytes([self.input[3], self.input[4]]));
            if self.input.len() < 5 + len {
                break;
            }
            let rec: Vec<u8> = self.input.drain(..5 + len).collect();
            let ty = rec[0];
            let plain = match &self.read {
                Some(d) => {
                    let seq = self.read_seq;
                    self.read_seq += 1;
                    match d.open(seq, ty, TLS12, &rec[5..]) {
                        Ok(p) => p,
                        Err(()) => {
                            out.extend(self.record(CT_ALERT, &[2, alert::BAD_RECORD_MAC]));
                            return out;
                        }
                    }
                }
                None => rec[5..].to_vec(),
            };
            match ty {
                CT_HANDSHAKE => {
                    let mut at = 0;
                    while at < plain.len() {
                        let len = usize::from(plain[at + 1]) << 16
                            | usize::from(plain[at + 2]) << 8
                            | usize::from(plain[at + 3]);
                        let msg = plain[at..at + 4 + len].to_vec();
                        at += 4 + len;
                        out.extend(self.handshake(msg));
                    }
                }
                CT_CHANGE_CIPHER_SPEC => {
                    self.read = self.pending_read.take();
                    self.read_seq = 0;
                }
                CT_ALERT => self.alerts.push((plain[0], plain[1])),
                CT_APPLICATION_DATA => {
                    self.received.extend_from_slice(&plain);
                    // Echo it back.
                    out.extend(self.record(CT_APPLICATION_DATA, &plain));
                }
                _ => panic!("record type {ty}"),
            }
        }
        out
    }

    fn handshake(&mut self, msg: Vec<u8>) -> Vec<u8> {
        let body = msg[4..].to_vec();
        let mut out = Vec::new();
        match msg[0] {
            HT_CLIENT_HELLO => {
                self.client_hello(&body, false);
                self.transcript.extend_from_slice(&msg);
                let mut hs = Vec::new();
                for (ty, body) in self.flight(TLS12) {
                    let m = handshake_message(ty, &body);
                    self.transcript.extend_from_slice(&m);
                    hs.extend(m);
                }
                if self.faults.early_data {
                    out.extend(self.record(CT_APPLICATION_DATA, b"early"));
                }
                out.extend(self.record(CT_HANDSHAKE, &hs));
            }
            HT_CLIENT_KEY_EXCHANGE => {
                self.transcript.extend_from_slice(&msg);
                self.client_key_exchange(&body);
            }
            HT_FINISHED => {
                if !self.client_finished_ok(&body) {
                    out.extend(self.record(CT_ALERT, &[2, alert::DECRYPT_ERROR]));
                    return out;
                }
                self.transcript.extend_from_slice(&msg);
                let fin = handshake_message(HT_FINISHED, &self.finished_body());
                out.extend(self.record(CT_CHANGE_CIPHER_SPEC, &[1]));
                self.write = self.pending_write.take();
                self.write_seq = 0;
                out.extend(self.record(CT_HANDSHAKE, &fin));
                self.finished = true;
            }
            ty => panic!("handshake type {ty}"),
        }
        out
    }
}

/// Run the client against the fake until neither has anything more to
/// say: the client's events and its first error.
pub(crate) fn exchange(
    c: &mut TlsClient,
    fake: &mut Fake,
    first: Vec<Event>,
) -> (Vec<Event>, Result<(), Failure>) {
    let mut events = Vec::new();
    let mut pending = first;
    let mut result = Ok(());
    while !pending.is_empty() {
        let mut next = Vec::new();
        for e in pending {
            match e {
                Event::Send(bytes) => {
                    let answer = fake.receive(&bytes);
                    if !answer.is_empty() {
                        let r = c.receive(&answer, &mut next);
                        if result.is_ok() {
                            result = r;
                        }
                    }
                }
                Event::Dh(req) => {
                    let r = c.dh_done(dh_compute(&req), &mut next);
                    if result.is_ok() {
                        result = r;
                    }
                }
                other => events.push(other),
            }
        }
        pending = next;
    }
    (events, result)
}

fn connect(key: &[u8], fake: &mut Fake) -> (TlsClient, Vec<Event>, Result<(), Failure>) {
    let mut first = Vec::new();
    let mut c = TlsClient::new(psk(key), &mut first).unwrap();
    let (events, result) = exchange(&mut c, fake, first);
    (c, events, result)
}

#[test]
fn a_handshake_with_each_record_protection_carries_data() {
    for (ems, etm) in [(true, true), (true, false), (false, true), (false, false)] {
        let mut fake = Fake::new(&KEY);
        fake.ems = ems;
        fake.etm = etm;
        let (mut c, events, result) = connect(&KEY, &mut fake);
        assert_eq!(result, Ok(()));
        assert!(c.is_open() && fake.finished);
        assert_eq!(fake.identity, b"OCA-PSK");
        let Some(Event::Established(session)) = events.first() else {
            panic!("{events:?}");
        };
        assert_eq!(session.extended_master_secret, ems);
        assert_eq!(session.encrypt_then_mac, etm);
        assert_eq!(session.hint.as_deref(), Some(&b"OCA-PSK"[..]));
        // Application data both ways, larger than one record.
        let data: Vec<u8> = (0..40_000u32).map(|i| i as u8).collect();
        let sealed = c.seal(&data).unwrap();
        let echo = fake.receive(&sealed);
        assert_eq!(fake.received, data);
        let mut out = Vec::new();
        c.receive(&echo, &mut out).unwrap();
        let back: Vec<u8> = out
            .into_iter()
            .flat_map(|e| match e {
                Event::Data(d) => d,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(back, data);
        // Closing sends close_notify.
        let bye = c.close().unwrap();
        fake.receive(&bye);
        assert_eq!(fake.alerts, [(alert::WARNING, alert::CLOSE_NOTIFY)]);
        assert!(c.close().is_none());
    }
}

#[test]
fn the_hello_offers_one_suite_and_the_extensions() {
    let mut out = Vec::new();
    TlsClient::new(psk(&KEY), &mut out).unwrap();
    let Event::Send(bytes) = &out[0] else {
        panic!()
    };
    assert_eq!(&bytes[..3], [CT_HANDSHAKE, 3, 1]);
    let msg = &bytes[5..];
    assert_eq!(msg[0], HT_CLIENT_HELLO);
    let mut fake = Fake::new(&KEY);
    fake.client_hello(&msg[4..], false);
    assert!(fake.session.extended_master_secret && fake.session.encrypt_then_mac);
}

/// The alert the client sent last, read by the fake with its keys.
fn sent_alert(fake: &mut Fake, events: &[Event]) -> Option<(u8, u8)> {
    for e in events {
        if let Event::Send(b) = e {
            fake.receive(b);
        }
    }
    fake.alerts.last().copied()
}

fn refused(fake: &mut Fake) -> (Failure, Option<(u8, u8)>) {
    let mut first = Vec::new();
    let mut c = TlsClient::new(psk(&KEY), &mut first).unwrap();
    let mut events = Vec::new();
    // Deliver the hello, then feed every answer, keeping the client's
    // output for inspection.
    let mut pending = first;
    let mut failure = None;
    while !pending.is_empty() && failure.is_none() {
        let mut next = Vec::new();
        for e in pending {
            match e {
                Event::Send(bytes) => {
                    let answer = fake.receive(&bytes);
                    if !answer.is_empty() {
                        let mut out = Vec::new();
                        if let Err(f) = c.receive(&answer, &mut out) {
                            failure.get_or_insert(f);
                        }
                        events.extend(out.iter().filter_map(|e| match e {
                            Event::Send(b) => Some(Event::Send(b.clone())),
                            _ => None,
                        }));
                        next.extend(out);
                    }
                }
                Event::Dh(req) => {
                    let mut out = Vec::new();
                    if let Err(f) = c.dh_done(dh_compute(&req), &mut out) {
                        failure.get_or_insert(f);
                    }
                    next.extend(out);
                }
                _ => {}
            }
        }
        if failure.is_some() {
            events = next
                .into_iter()
                .filter(|e| matches!(e, Event::Send(_)))
                .collect();
            break;
        }
        pending = next;
    }
    let failure = failure.expect("the client refused");
    let alert = sent_alert(fake, &events);
    (failure, alert)
}

#[test]
fn a_wrong_key_fails_the_handshake() {
    // The device has another key: it cannot read the client's Finished.
    let mut fake = Fake::new(&[0x42; 16]);
    let (_, events, result) = connect(&KEY, &mut fake);
    let f = result.unwrap_err();
    assert_eq!(f.alert, None);
    assert!(
        f.reason.contains("bad_record_mac") && f.reason.contains("pre-shared key"),
        "{f:?}"
    );
    assert!(!events.iter().any(|e| matches!(e, Event::Established(_))));
}

#[test]
fn a_finished_that_does_not_verify_is_refused() {
    let mut fake = Fake::new(&KEY);
    fake.faults.bad_finished = true;
    let (f, alert) = refused(&mut fake);
    assert_eq!(f.alert, Some(alert::DECRYPT_ERROR));
    assert_eq!(alert, Some((alert::FATAL, alert::DECRYPT_ERROR)));
}

#[test]
fn weak_or_malformed_groups_are_refused() {
    let p = unhex(FFDHE2048);
    let mut p_minus_1 = p.clone();
    *p_minus_1.last_mut().unwrap() -= 1;
    let mut even = p.clone();
    *even.last_mut().unwrap() ^= 1;
    let cases: Vec<(Faults, u8)> = vec![
        // A 1024-bit group (RFC 2409's second Oakley group).
        (
            Faults {
                p: Some(unhex(&FFDHE2048[..256])),
                ..Faults::default()
            },
            alert::INSUFFICIENT_SECURITY,
        ),
        (
            Faults {
                p: Some(even),
                ..Faults::default()
            },
            alert::ILLEGAL_PARAMETER,
        ),
        (
            Faults {
                g: Some(vec![1]),
                ..Faults::default()
            },
            alert::ILLEGAL_PARAMETER,
        ),
        (
            Faults {
                g: Some(p_minus_1.clone()),
                ..Faults::default()
            },
            alert::ILLEGAL_PARAMETER,
        ),
        (
            Faults {
                ys: Some(vec![0]),
                ..Faults::default()
            },
            alert::ILLEGAL_PARAMETER,
        ),
        (
            Faults {
                ys: Some(vec![1]),
                ..Faults::default()
            },
            alert::ILLEGAL_PARAMETER,
        ),
        (
            Faults {
                ys: Some(p_minus_1),
                ..Faults::default()
            },
            alert::ILLEGAL_PARAMETER,
        ),
        (
            Faults {
                ys: Some(p.clone()),
                ..Faults::default()
            },
            alert::ILLEGAL_PARAMETER,
        ),
        (
            Faults {
                ys: Some([vec![1], p].concat()),
                ..Faults::default()
            },
            alert::ILLEGAL_PARAMETER,
        ),
    ];
    for (faults, code) in cases {
        let mut fake = Fake::new(&KEY);
        fake.faults = faults;
        let (f, alert) = refused(&mut fake);
        assert_eq!(f.alert, Some(code), "{f:?}");
        assert_eq!(alert, Some((alert::FATAL, code)));
    }
}

#[test]
fn unexpected_server_hellos_and_messages_are_refused() {
    let cases: Vec<(Faults, u8, &str)> = vec![
        (
            Faults {
                suite: Some(0x002f),
                ..Faults::default()
            },
            alert::ILLEGAL_PARAMETER,
            "cipher suite",
        ),
        (
            Faults {
                version: Some(0x0302),
                ..Faults::default()
            },
            alert::PROTOCOL_VERSION,
            "version",
        ),
        (
            Faults {
                extra_extension: Some((0x0023, Vec::new())),
                ..Faults::default()
            },
            alert::UNSUPPORTED_EXTENSION,
            "not offered",
        ),
        (
            Faults {
                extra_extension: Some((EXT_RENEGOTIATION_INFO, vec![0])),
                ..Faults::default()
            },
            alert::ILLEGAL_PARAMETER,
            "repeats",
        ),
        (
            Faults {
                certificate: true,
                ..Faults::default()
            },
            alert::UNEXPECTED_MESSAGE,
            "Certificate",
        ),
        (
            Faults {
                early_data: true,
                ..Faults::default()
            },
            alert::UNEXPECTED_MESSAGE,
            "application data",
        ),
    ];
    for (faults, code, words) in cases {
        let mut fake = Fake::new(&KEY);
        fake.faults = faults;
        let (f, alert) = refused(&mut fake);
        assert_eq!(f.alert, Some(code), "{f:?}");
        assert!(f.reason.contains(words), "{f:?}");
        assert_eq!(alert, Some((alert::FATAL, code)));
    }
}

#[test]
fn renegotiation_info_must_be_empty() {
    let mut out = Vec::new();
    TlsClient::new(psk(&KEY), &mut out).unwrap();
    let mut hello = W::default();
    hello.u16(TLS12).bytes(&[1; 32]).vec8(&[]).u16(SUITE).u8(0);
    let mut ext = W::default();
    ext.u16(EXT_RENEGOTIATION_INFO).vec16(&[1, 7]);
    hello.vec16(&ext.0);
    assert_eq!(
        parse_server_hello(&hello.0, TLS12).unwrap_err().alert,
        Some(alert::HANDSHAKE_FAILURE)
    );
}

fn open_session() -> (TlsClient, Fake) {
    let mut fake = Fake::new(&KEY);
    let (c, _, result) = connect(&KEY, &mut fake);
    result.unwrap();
    (c, fake)
}

#[test]
fn a_tampered_record_ends_the_session_with_bad_record_mac() {
    for etm in [true, false] {
        let mut fake = Fake::new(&KEY);
        fake.etm = etm;
        let (mut c, _, result) = connect(&KEY, &mut fake);
        result.unwrap();
        let mut rec = fake.record(CT_APPLICATION_DATA, b"0123456789");
        let last = rec.len() - 1;
        rec[last] ^= 0x01;
        let mut out = Vec::new();
        let f = c.receive(&rec, &mut out).unwrap_err();
        assert_eq!(
            f,
            fatal(alert::BAD_RECORD_MAC, "a record failed authentication")
        );
        assert_eq!(
            sent_alert(&mut fake, &out),
            Some((alert::FATAL, alert::BAD_RECORD_MAC))
        );
        // Nothing more is read.
        let more = fake.record(CT_APPLICATION_DATA, b"x");
        let mut out = Vec::new();
        assert_eq!(c.receive(&more, &mut out), Ok(()));
        assert!(out.is_empty());
    }
}

#[test]
fn a_record_with_bad_padding_gives_the_same_error_as_a_bad_mac() {
    // Bad padding under a good MAC, in a MAC-then-encrypt session.
    let mut fake = Fake::new(&KEY);
    fake.etm = false;
    let (mut c, _, result) = connect(&KEY, &mut fake);
    result.unwrap();
    let w = fake.write.take().unwrap();
    // 3 bytes and the 20-byte MAC: nine bytes of padding, one of them wrong.
    let mut padding = [8u8; 9];
    padding[3] = 7;
    let body = mte_record(&w, fake.write_seq, b"abc", &padding, true);
    let mut record = vec![CT_APPLICATION_DATA, 3, 3];
    record.extend((body.len() as u16).to_be_bytes());
    record.extend(body);
    let padding_failure = c.receive(&record, &mut Vec::new()).unwrap_err();
    // A changed byte, which fails the MAC.
    let mut fake = Fake::new(&KEY);
    fake.etm = false;
    let (mut c, _, result) = connect(&KEY, &mut fake);
    result.unwrap();
    let mut record = fake.record(CT_APPLICATION_DATA, b"abc");
    record[10] ^= 0x40;
    let mac_failure = c.receive(&record, &mut Vec::new()).unwrap_err();
    assert_eq!(padding_failure, mac_failure);
    assert_eq!(padding_failure.alert, Some(alert::BAD_RECORD_MAC));
}

#[test]
fn records_beyond_the_limits_are_refused() {
    let (mut c, _) = open_session();
    let mut out = Vec::new();
    let mut header = vec![CT_APPLICATION_DATA, 3, 3];
    header.extend(((MAX_CIPHERTEXT + 1) as u16).to_be_bytes());
    assert_eq!(
        c.receive(&header, &mut out).unwrap_err().alert,
        Some(alert::RECORD_OVERFLOW)
    );

    let (mut c, _) = open_session();
    let mut out = Vec::new();
    assert_eq!(
        c.receive(&[CT_APPLICATION_DATA, 3, 1, 0, 0], &mut out)
            .unwrap_err()
            .alert,
        Some(alert::PROTOCOL_VERSION)
    );

    // Too many empty records in a row.
    let (mut c, mut fake) = open_session();
    let mut out = Vec::new();
    let mut result = Ok(());
    for _ in 0..=MAX_EMPTY {
        result = c.receive(&fake.record(CT_APPLICATION_DATA, &[]), &mut out);
    }
    assert_eq!(result.unwrap_err().alert, Some(alert::UNEXPECTED_MESSAGE));
}

#[test]
fn close_notify_is_answered_and_renegotiation_refused() {
    let (mut c, mut fake) = open_session();
    let mut out = Vec::new();
    c.receive(
        &fake.record(CT_HANDSHAKE, &handshake_message(HT_HELLO_REQUEST, &[])),
        &mut out,
    )
    .unwrap();
    assert_eq!(
        sent_alert(&mut fake, &out),
        Some((alert::WARNING, alert::NO_RENEGOTIATION))
    );
    assert!(c.is_open());
    let mut out = Vec::new();
    c.receive(
        &fake.record(CT_ALERT, &[alert::WARNING, alert::CLOSE_NOTIFY]),
        &mut out,
    )
    .unwrap();
    assert!(matches!(out.last(), Some(Event::Closed)));
    assert_eq!(
        sent_alert(&mut fake, &out),
        Some((alert::WARNING, alert::CLOSE_NOTIFY))
    );
    assert!(!c.is_open());

    let (mut c, mut fake) = open_session();
    let mut out = Vec::new();
    let f = c
        .receive(
            &fake.record(CT_ALERT, &[alert::FATAL, alert::INTERNAL_ERROR]),
            &mut out,
        )
        .unwrap_err();
    assert_eq!(f.alert, None);
    assert!(f.reason.contains("internal_error"));
    // A warning other than the two tolerated ends the session too.
    let (mut c, mut fake) = open_session();
    let mut out = Vec::new();
    assert!(c
        .receive(&fake.record(CT_ALERT, &[alert::WARNING, 42]), &mut out)
        .is_err());
    // user_canceled passes.
    let (mut c, mut fake) = open_session();
    let mut out = Vec::new();
    c.receive(
        &fake.record(CT_ALERT, &[alert::WARNING, alert::USER_CANCELED]),
        &mut out,
    )
    .unwrap();
    assert!(c.is_open());
}

#[test]
fn a_change_cipher_spec_out_of_place_is_refused() {
    let mut first = Vec::new();
    let mut c = TlsClient::new(psk(&KEY), &mut first).unwrap();
    let mut out = Vec::new();
    let f = c
        .receive(&[CT_CHANGE_CIPHER_SPEC, 3, 3, 0, 1, 1], &mut out)
        .unwrap_err();
    assert_eq!(f.alert, Some(alert::UNEXPECTED_MESSAGE));
    // An empty handshake record.
    let mut c = TlsClient::new(psk(&KEY), &mut Vec::new()).unwrap();
    assert_eq!(
        c.receive(&[CT_HANDSHAKE, 3, 3, 0, 0], &mut out)
            .unwrap_err()
            .alert,
        Some(alert::UNEXPECTED_MESSAGE)
    );
}

#[test]
fn the_key_exchange_agrees_with_the_device() {
    let fake = Fake::new(&KEY);
    let req = dh_check(&fake.p, &[2], &fake.ys()).unwrap();
    assert_eq!(req.bits(), 2048);
    let shared = dh_compute(&req).unwrap();
    assert_eq!(shared.z.to_vec(), fake.z(&shared.yc));
    // Through the outcome bytes, as the module carries it.
    let back = dh_from_outcome(dh_outcome(&req)).unwrap();
    assert!(!back.z.is_empty() && !back.yc.is_empty());
}

#[test]
fn psk_settings_are_checked_and_never_shown() {
    let p = Psk::from_hex("OCA-PSK", "00:11 22aaBB").unwrap();
    assert_eq!(p.key.to_vec(), [0x00, 0x11, 0x22, 0xaa, 0xbb]);
    let shown = format!("{p:?}");
    assert!(!shown.contains("aabb") && shown.contains("<secret>"));
    for bad in ["", "0", "zz", "123"] {
        assert!(Psk::from_hex("OCA-PSK", bad).is_err(), "{bad}");
    }
    let e = Psk::from_hex("OCA-PSK", "c0ffee5").unwrap_err();
    assert!(!e.contains("c0ffee5"), "{e}");
}

// --- OpenSSL on loopback ---------------------------------------------------------

pub(crate) mod openssl {
    use std::io::{BufRead, BufReader, Write};
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    /// An OpenSSL binary already on this machine, or none (the tests then
    /// skip). Nothing is installed.
    pub(crate) fn binary() -> Option<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(p) = std::env::var("OPENSSL_BIN") {
            candidates.push(PathBuf::from(p));
        }
        if cfg!(windows) {
            // Git for Windows' own; checked first as the one most machines
            // with Git have.
            candidates.push(PathBuf::from(r"C:\Program Files\Git\usr\bin\openssl.exe"));
        }
        candidates.push(PathBuf::from("openssl"));
        candidates.into_iter().find(|c| {
            Command::new(c)
                .arg("version")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .is_ok_and(|o| o.status.success() && o.stdout.starts_with(b"OpenSSL"))
        })
    }

    /// A port free for both TCP and UDP on loopback.
    pub(crate) fn free_port() -> u16 {
        loop {
            let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = tcp.local_addr().unwrap().port();
            if std::net::UdpSocket::bind(("127.0.0.1", port)).is_ok() {
                return port;
            }
        }
    }

    /// `openssl s_server` bound to 127.0.0.1, stopped and its files removed
    /// when dropped.
    pub(crate) struct Server {
        child: Child,
        files: Vec<PathBuf>,
        /// What s_server prints, read so its output never blocks it.
        lines: mpsc::Receiver<String>,
        pub(crate) port: u16,
    }

    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            for f in &self.files {
                let _ = std::fs::remove_file(f);
            }
        }
    }

    impl Server {
        /// Send a line to the client through s_server's input.
        pub(crate) fn send_line(&mut self, line: &str) {
            let stdin = self.child.stdin.as_mut().expect("piped");
            stdin.write_all(line.as_bytes()).unwrap();
            stdin.write_all(b"\n").unwrap();
            stdin.flush().unwrap();
        }

        /// Whether s_server prints a line containing `text` in time.
        pub(crate) fn printed(&self, text: &str, within: Duration) -> bool {
            let until = std::time::Instant::now() + within;
            loop {
                let left = until.saturating_duration_since(std::time::Instant::now());
                match self.lines.recv_timeout(left) {
                    Ok(line) if line.contains(text) => return true,
                    Ok(_) => {}
                    Err(_) => return false,
                }
            }
        }

        /// Start it with the PSK, the hint, the AES70 suite and, when given,
        /// a DH parameters PEM. Over TLS `-rev` answers each line reversed;
        /// over DTLS, which `-rev` does not serve, what the client sends is
        /// printed and what is written to s_server's input is sent. Waits
        /// until it accepts.
        pub(crate) fn start(
            bin: &PathBuf,
            key_hex: &str,
            dh_pem: Option<&str>,
            dtls: bool,
        ) -> Option<Server> {
            let port = free_port();
            let mut files = Vec::new();
            let mut cmd = Command::new(bin);
            cmd.arg("s_server")
                .arg(if dtls { "-dtls1_2" } else { "-tls1_2" })
                .args(["-accept", &format!("127.0.0.1:{port}")])
                .args(["-nocert", "-psk", key_hex, "-psk_hint", "OCA-PSK"])
                .args(["-psk_identity", "OCA-PSK"])
                .args(["-cipher", "DHE-PSK-AES128-CBC-SHA", "-naccept", "1"]);
            if dtls {
                // s_server reads its input before the socket on Windows, so
                // the tests feed it lines.
                cmd.arg("-ign_eof");
            } else {
                cmd.arg("-rev");
            }
            if let Some(pem) = dh_pem {
                let path = std::env::temp_dir()
                    .join(format!("meros-aes70-dh-{}-{port}.pem", std::process::id()));
                std::fs::write(&path, pem).ok()?;
                cmd.arg("-dhparam").arg(&path);
                files.push(path);
            }
            let mut child = cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            let stdout = child.stdout.take()?;
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
            // s_server prints ACCEPT once it listens.
            let ready = loop {
                match rx.recv_timeout(Duration::from_secs(10)) {
                    Ok(line) if line.contains("ACCEPT") => break true,
                    Ok(_) => {}
                    Err(_) => break false,
                }
            };
            let server = Server {
                child,
                files,
                lines: rx,
                port,
            };
            ready.then_some(server)
        }
    }

    /// A 1024-bit group (made with `openssl dhparam 1024`), too small.
    pub(crate) const DH1024_PEM: &str = "-----BEGIN DH PARAMETERS-----
MIGHAoGBALVDrlwj5shIs8iMM6P4PV/cj1gl++Hj6dX89VSlQjlUa/3FrPovVlfR
7oa+TmzNiJ/3J93bVdsygKb+r6NlsWXj4Ihsme/IUioImoFAyXq0A29b1f4AP817
k5WB26qDpiE+c07VulawBcG7Fa6AwifvXf5gi8RaOn+eKBuqm88DAgEC
-----END DH PARAMETERS-----
";

    /// RFC 7919's ffdhe2048 as OpenSSL's DH parameters PEM.
    pub(crate) const FFDHE2048_PEM: &str = "-----BEGIN DH PARAMETERS-----
MIIBDAKCAQEA//////////+t+FRYortKmq/cViAnPTzx2LnFg84tNpWp4TZBFGQz
+8yTnc4kmz75fS/jY2MMddj2gbICrsRhetPfHtXV/WVhJDP1H18GbtCFY2VVPe0a
87VXE15/V8k1mE8McODmi3fipona8+/och3xWKE2rec1MKzKT0g6eXq8CrGCsyT7
YdEIqUuyyOP7uWrat2DX9GgdT0Kj3jlN9K5W7edjcrsZCwenyO4KbXCeAvzhzffi
7MA0BM0oNC9hkXL+nOmFg/+OTxIy7vKBg8P+OxtMb61zO7X8vC7CIAXFjvGDfRaD
ssbzSibBsu/6iGtCOGEoXJf//////////wIBAgICAOE=
-----END DH PARAMETERS-----
";
}

/// Run a handshake with s_server over TCP and exchange a line; the session
/// agreed, or the client's error.
fn openssl_tls(
    key: &[u8],
    server_key: &str,
    dh_pem: Option<&str>,
) -> Option<Result<Session, Failure>> {
    use std::io::{Read, Write};
    let Some(bin) = openssl::binary() else {
        eprintln!("skipped: no OpenSSL binary on this machine");
        return None;
    };
    let Some(server) = openssl::Server::start(&bin, server_key, dh_pem, false) else {
        eprintln!("skipped: openssl s_server did not start with the AES70 suite");
        return None;
    };
    let mut sock = std::net::TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    sock.set_read_timeout(Some(std::time::Duration::from_secs(20)))
        .unwrap();
    let mut out = Vec::new();
    let mut c = TlsClient::new(psk(key), &mut out).unwrap();
    let mut session = None;
    let mut reply = Vec::new();
    let line = b"hello over secure OCP.1\n";
    loop {
        for e in std::mem::take(&mut out) {
            match e {
                Event::Send(b) => sock.write_all(&b).unwrap(),
                Event::Dh(req) => {
                    if let Err(f) = c.dh_done(dh_compute(&req), &mut out) {
                        for e in std::mem::take(&mut out) {
                            if let Event::Send(b) = e {
                                let _ = sock.write_all(&b);
                            }
                        }
                        return Some(Err(f));
                    }
                }
                Event::Established(s) => {
                    session = Some(s);
                    sock.write_all(&c.seal(line).unwrap()).unwrap();
                }
                Event::Data(d) => reply.extend(d),
                Event::Closed => {}
            }
        }
        if !out.is_empty() {
            continue;
        }
        if reply.ends_with(b"\n") {
            break;
        }
        let mut buf = [0u8; 4096];
        let n = match sock.read(&mut buf) {
            Ok(0) | Err(_) => {
                return Some(Err(Failure {
                    alert: None,
                    reason: "s_server closed the connection".into(),
                }))
            }
            Ok(n) => n,
        };
        if let Err(f) = c.receive(&buf[..n], &mut out) {
            return Some(Err(f));
        }
    }
    let mut expected = line[..line.len() - 1].to_vec();
    expected.reverse();
    expected.push(b'\n');
    assert_eq!(reply, expected);
    if let Some(bye) = c.close() {
        let _ = sock.write_all(&bye);
    }
    Some(Ok(session.unwrap()))
}

#[test]
fn interoperates_with_openssl_s_server() {
    let Some(result) = openssl_tls(
        &KEY,
        "00112233445566778899aabbccddeeff",
        Some(openssl::FFDHE2048_PEM),
    ) else {
        return;
    };
    let session = result.unwrap();
    assert!(session.extended_master_secret);
    assert!(session.encrypt_then_mac);
    assert_eq!(session.hint.as_deref(), Some(&b"OCA-PSK"[..]));
}

#[test]
fn openssl_with_another_key_is_refused() {
    let Some(result) = openssl_tls(
        &KEY,
        "ffeeddccbbaa99887766554433221100",
        Some(openssl::FFDHE2048_PEM),
    ) else {
        return;
    };
    let f = result.unwrap_err();
    assert!(
        f.reason.contains("pre-shared key") || f.reason.contains("closed"),
        "{f:?}"
    );
}

#[test]
fn openssl_with_a_1024_bit_group_is_refused() {
    // OpenSSL 1.1.1's own choice for this suite, without -dhparam.
    let Some(result) = openssl_tls(
        &KEY,
        "00112233445566778899aabbccddeeff",
        Some(openssl::DH1024_PEM),
    ) else {
        return;
    };
    let f = result.unwrap_err();
    assert_eq!(f.alert, Some(alert::INSUFFICIENT_SECURITY), "{f:?}");
}
