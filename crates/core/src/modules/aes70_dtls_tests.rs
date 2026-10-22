//! Tests of the DTLS client: an in-process fake device with the cookie
//! exchange, fragments, lost flights, replays and refusals, and, where
//! OpenSSL is installed, a handshake with `openssl s_server -dtls1_2` on
//! loopback.

use super::super::tls::tests::{psk, unhex, Fake, KEY};
use super::super::tls::{dh_compute, handshake_message};
use super::*;

/// The fake device over DTLS framing, around the TLS fake's cryptography.
pub(crate) struct DFake {
    f: Fake,
    /// Ask for a cookie first.
    cookie: bool,
    /// Split handshake messages into fragments of this many bytes, sent in
    /// reverse order.
    fragment: Option<usize>,
    /// Server flights to lose before one gets through.
    lose_flights: usize,
    /// Lose the client's last flight this many times.
    lose_client_flights: usize,
    seq: [u64; 2],
    msg_seq: u16,
    read_epoch: u16,
    /// The ServerHello flight, to send again.
    flight: Vec<u8>,
    pub(crate) received: Vec<Vec<u8>>,
    pub(crate) alerts: Vec<(u8, u8)>,
}

impl DFake {
    pub(crate) fn new(key: &[u8]) -> DFake {
        DFake {
            f: Fake::new(key),
            cookie: true,
            fragment: None,
            lose_flights: 0,
            lose_client_flights: 0,
            seq: [0, 0],
            msg_seq: 0,
            read_epoch: 0,
            flight: Vec::new(),
            received: Vec::new(),
            alerts: Vec::new(),
        }
    }

    fn record(&mut self, ty: u8, epoch: u16, data: &[u8]) -> Vec<u8> {
        let seq = self.seq[usize::from(epoch)];
        self.seq[usize::from(epoch)] += 1;
        let full = (u64::from(epoch) << 48) | seq;
        let fragment = if epoch == 1 {
            self.f
                .write
                .as_ref()
                .unwrap()
                .seal(full, ty, DTLS12, data)
                .unwrap()
        } else {
            data.to_vec()
        };
        let mut w = W::default();
        w.u8(ty).u16(DTLS12).bytes(&full.to_be_bytes()[..2]);
        w.bytes(&full.to_be_bytes()[2..]).vec16(&fragment);
        w.0
    }

    /// One handshake message as records, fragmented when asked.
    fn message(&mut self, ty: u8, body: &[u8], epoch: u16) -> Vec<u8> {
        let seq = self.msg_seq;
        self.msg_seq += 1;
        self.f.transcript.extend(dtls_message(ty, seq, body));
        let size = self.fragment.unwrap_or(usize::MAX).max(1);
        let mut pieces = Vec::new();
        let mut offset = 0;
        loop {
            let n = (body.len() - offset).min(size);
            let mut w = W::default();
            w.u8(ty)
                .u24(body.len())
                .u16(seq)
                .u24(offset)
                .u24(n)
                .bytes(&body[offset..offset + n]);
            pieces.push(w.0);
            offset += n;
            if offset >= body.len() {
                break;
            }
        }
        if self.fragment.is_some() {
            pieces.reverse();
        }
        pieces
            .into_iter()
            .flat_map(|p| self.record(CT_HANDSHAKE, epoch, &p))
            .collect()
    }

    /// A datagram from the client; the datagrams the device answers with.
    pub(crate) fn receive(&mut self, datagram: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut rest = datagram;
        let mut client_flight_done = false;
        while rest.len() >= RECORD_HEADER {
            let ty = rest[0];
            let epoch = u16::from_be_bytes([rest[3], rest[4]]);
            let mut seq = [0u8; 8];
            seq[2..].copy_from_slice(&rest[5..11]);
            let seq = u64::from_be_bytes(seq);
            let len = usize::from(u16::from_be_bytes([rest[11], rest[12]]));
            let fragment = rest[RECORD_HEADER..RECORD_HEADER + len].to_vec();
            rest = &rest[RECORD_HEADER + len..];
            let plain = if epoch == 1 {
                let full = (1u64 << 48) | seq;
                match self
                    .f
                    .read
                    .as_ref()
                    .map(|d| d.open(full, ty, DTLS12, &fragment))
                {
                    Some(Ok(p)) => p,
                    _ => {
                        out.push(self.record(CT_ALERT, 0, &[2, alert::BAD_RECORD_MAC]));
                        return out;
                    }
                }
            } else {
                fragment
            };
            match ty {
                CT_HANDSHAKE => {
                    let msg_ty = plain[0];
                    let body = plain[HANDSHAKE_HEADER..].to_vec();
                    match msg_ty {
                        HT_CLIENT_HELLO => {
                            let mut r = R::new(&body, "hello");
                            r.take(34).unwrap();
                            r.vec8().unwrap();
                            let has_cookie = !r.vec8().unwrap().is_empty();
                            if self.cookie && !has_cookie {
                                let mut hvr = W::default();
                                hvr.u16(DTLS10).vec8(&[0xab; 16]);
                                let mut w = W::default();
                                let b = hvr.0;
                                w.u8(HT_HELLO_VERIFY_REQUEST)
                                    .u24(b.len())
                                    .u16(0)
                                    .u24(0)
                                    .u24(b.len())
                                    .bytes(&b);
                                self.msg_seq = 1;
                                out.push(self.record(CT_HANDSHAKE, 0, &w.0));
                                continue;
                            }
                            if self.flight.is_empty() {
                                self.f.transcript = plain.clone();
                                self.f.client_hello(&body, true);
                                let mut d = Vec::new();
                                for (ty, body) in self.f.flight(DTLS12) {
                                    d.extend(self.message(ty, &body, 0));
                                }
                                self.flight = d;
                            }
                            if self.lose_flights > 0 {
                                self.lose_flights -= 1;
                            } else {
                                out.push(self.flight.clone());
                            }
                        }
                        HT_CLIENT_KEY_EXCHANGE => {
                            if self.lose_client_flights > 0 {
                                // Lost on the way: the device sends its
                                // flight again, as its timer would.
                                self.lose_client_flights -= 1;
                                out.push(self.flight.clone());
                                return out;
                            }
                            if self.f.pending_read.is_none() && self.f.read.is_none() {
                                self.f.transcript.extend_from_slice(&plain);
                                self.f.client_key_exchange(&body);
                            }
                        }
                        HT_FINISHED => {
                            if !self.f.client_finished_ok(&body) {
                                out.push(self.record(CT_ALERT, 1, &[2, alert::DECRYPT_ERROR]));
                                return out;
                            }
                            self.f.transcript.extend_from_slice(&plain);
                            let fin = self.f.finished_body();
                            let mut d = self.record(CT_CHANGE_CIPHER_SPEC, 0, &[1]);
                            self.f.write = self.f.pending_write.take();
                            d.extend(self.message(HT_FINISHED, &fin, 1));
                            out.push(d);
                            self.f.finished = true;
                            client_flight_done = true;
                        }
                        ty => panic!("handshake type {ty}"),
                    }
                }
                CT_CHANGE_CIPHER_SPEC => {
                    if self.read_epoch == 0 {
                        self.f.read = self.f.pending_read.take();
                        self.read_epoch = 1;
                    }
                }
                CT_ALERT => self.alerts.push((plain[0], plain[1])),
                CT_APPLICATION_DATA => {
                    self.received.push(plain.clone());
                    out.push(self.record(CT_APPLICATION_DATA, 1, &plain));
                }
                _ => panic!("record type {ty}"),
            }
        }
        let _ = client_flight_done;
        out
    }
}

/// Run the client against the fake, firing the retransmission timer when
/// nothing else is moving. The clock ends where it got to.
fn run(
    c: &mut DtlsClient,
    fake: &mut DFake,
    first: Vec<Event>,
    now: &mut Millis,
) -> (Vec<Event>, Result<(), Failure>) {
    let mut events = Vec::new();
    let mut pending = first;
    let mut result = Ok(());
    for _ in 0..64 {
        if pending.is_empty() {
            match c.deadline() {
                Some(at) if !c.is_open() => {
                    *now = at;
                    let r = c.timer(*now, &mut pending);
                    if result.is_ok() {
                        result = r;
                    }
                }
                _ => break,
            }
        }
        let mut next = Vec::new();
        for e in std::mem::take(&mut pending) {
            match e {
                Event::Send(d) => {
                    for answer in fake.receive(&d) {
                        let r = c.receive(&answer, *now, &mut next);
                        if result.is_ok() {
                            result = r;
                        }
                    }
                }
                Event::Dh(req) => {
                    let r = c.dh_done(dh_compute(&req), *now, &mut next);
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

fn connect(fake: &mut DFake, now: &mut Millis) -> (DtlsClient, Vec<Event>, Result<(), Failure>) {
    let mut first = Vec::new();
    let mut c = DtlsClient::new(psk(&KEY), *now, &mut first).unwrap();
    let (events, result) = run(&mut c, fake, first, now);
    (c, events, result)
}

fn echo(c: &mut DtlsClient, fake: &mut DFake, data: &[u8]) -> Vec<Vec<u8>> {
    let sealed = c.seal(data).unwrap();
    let mut out = Vec::new();
    for answer in fake.receive(&sealed) {
        c.receive(&answer, 0, &mut out).unwrap();
    }
    out.into_iter()
        .filter_map(|e| match e {
            Event::Data(d) => Some(d),
            _ => None,
        })
        .collect()
}

#[test]
fn a_handshake_with_and_without_a_cookie_carries_datagrams() {
    for (cookie, ems, etm) in [
        (true, true, true),
        (false, true, false),
        (true, false, false),
    ] {
        let mut fake = DFake::new(&KEY);
        fake.cookie = cookie;
        fake.f.ems = ems;
        fake.f.etm = etm;
        let mut now = 0;
        let (mut c, events, result) = connect(&mut fake, &mut now);
        assert_eq!(result, Ok(()));
        assert!(c.is_open() && fake.f.finished);
        assert_eq!(now, 0, "nothing was resent");
        let Some(Event::Established(s)) = events.first() else {
            panic!("{events:?}")
        };
        assert_eq!((s.extended_master_secret, s.encrypt_then_mac), (ems, etm));
        assert_eq!(c.deadline(), None);
        // Each datagram is one record, read back as one.
        assert_eq!(echo(&mut c, &mut fake, b"one PDU"), [b"one PDU".to_vec()]);
        assert_eq!(echo(&mut c, &mut fake, &[7; 1300]), [vec![7; 1300]]);
        assert_eq!(fake.received.len(), 2);
        let bye = c.close().unwrap();
        fake.receive(&bye);
        assert_eq!(fake.alerts, [(alert::WARNING, alert::CLOSE_NOTIFY)]);
    }
}

#[test]
fn fragments_in_any_order_are_reassembled() {
    let mut fake = DFake::new(&KEY);
    fake.fragment = Some(37);
    let mut now = 0;
    let (c, _, result) = connect(&mut fake, &mut now);
    assert_eq!(result, Ok(()));
    assert!(c.is_open());
}

#[test]
fn a_lost_flight_is_sent_again_on_the_doubling_timer() {
    let mut fake = DFake::new(&KEY);
    fake.lose_flights = 2;
    let mut now = 0;
    let (c, _, result) = connect(&mut fake, &mut now);
    assert_eq!(result, Ok(()));
    assert!(c.is_open());
    // Resent at 1 s, then 2 s later.
    assert_eq!(now, 3_000);
}

#[test]
fn the_devices_repeated_flight_brings_ours_again() {
    let mut fake = DFake::new(&KEY);
    fake.lose_client_flights = 1;
    let mut now = 0;
    let (c, _, result) = connect(&mut fake, &mut now);
    assert_eq!(result, Ok(()));
    assert!(c.is_open());
    assert_eq!(now, 0, "resent without waiting for the timer");
}

fn open() -> (DtlsClient, DFake) {
    let mut fake = DFake::new(&KEY);
    let mut now = 0;
    let (c, _, result) = connect(&mut fake, &mut now);
    result.unwrap();
    (c, fake)
}

#[test]
fn tampered_and_replayed_records_are_discarded_silently() {
    let (mut c, mut fake) = open();
    let good = fake.record(CT_APPLICATION_DATA, 1, b"first");
    let mut tampered = fake.record(CT_APPLICATION_DATA, 1, b"second");
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    let mut out = Vec::new();
    c.receive(&good, 0, &mut out).unwrap();
    c.receive(&tampered, 0, &mut out).unwrap();
    // The same record again.
    c.receive(&good, 0, &mut out).unwrap();
    let data: Vec<_> = out
        .iter()
        .filter_map(|e| match e {
            Event::Data(d) => Some(d.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(data, [b"first".to_vec()]);
    assert_eq!(c.discarded(), 2);
    assert!(c.is_open());
    assert!(out.iter().all(|e| matches!(e, Event::Data(_))));
    // Out of order within the window is fine; far behind it is not.
    let r1 = fake.record(CT_APPLICATION_DATA, 1, b"a");
    let r2 = fake.record(CT_APPLICATION_DATA, 1, b"b");
    let mut out = Vec::new();
    c.receive(&r2, 0, &mut out).unwrap();
    c.receive(&r1, 0, &mut out).unwrap();
    assert_eq!(out.len(), 2);
    for _ in 0..70 {
        let r = fake.record(CT_APPLICATION_DATA, 1, b"x");
        c.receive(&r, 0, &mut Vec::new()).unwrap();
    }
    let before = c.discarded();
    c.receive(&r1, 0, &mut Vec::new()).unwrap();
    assert_eq!(c.discarded(), before + 1);
}

#[test]
fn a_wrong_key_or_a_weak_group_ends_the_handshake() {
    let mut fake = DFake::new(&[0x42; 16]);
    let mut now = 0;
    let (c, _, result) = connect(&mut fake, &mut now);
    let f = result.unwrap_err();
    assert!(
        f.reason.contains("bad_record_mac") && f.reason.contains("pre-shared key"),
        "{f:?}"
    );
    assert!(!c.is_open());

    let mut fake = DFake::new(&KEY);
    fake.f.faults.p = Some(unhex(&super::super::tls::tests::FFDHE2048[..256]));
    let mut now = 0;
    let (_, _, result) = connect(&mut fake, &mut now);
    assert_eq!(
        result.unwrap_err().alert,
        Some(alert::INSUFFICIENT_SECURITY)
    );
    assert_eq!(
        fake.alerts.last(),
        Some(&(alert::FATAL, alert::INSUFFICIENT_SECURITY))
    );
}

#[test]
fn close_notify_and_renegotiation() {
    let (mut c, mut fake) = open();
    let req = fake.record(CT_HANDSHAKE, 1, &dtls_message(HT_HELLO_REQUEST, 9, &[]));
    let mut out = Vec::new();
    // A HelloRequest in epoch 1 is not a Finished, so it is discarded.
    c.receive(&req, 0, &mut out).unwrap();
    assert!(c.is_open());
    // Nothing unprotected is read once the handshake is done: neither a
    // handshake message nor an alert.
    fake.msg_seq = 3;
    let forged = fake.message(HT_SERVER_HELLO, &[0; 40], 0);
    let forged_alert = fake.record(CT_ALERT, 0, &[alert::FATAL, alert::INTERNAL_ERROR]);
    let mut out = Vec::new();
    c.receive(&forged, 0, &mut out).unwrap();
    c.receive(&forged_alert, 0, &mut out).unwrap();
    assert!(out.is_empty() && c.is_open());
    let bye = fake.record(CT_ALERT, 1, &[alert::WARNING, alert::CLOSE_NOTIFY]);
    let mut out = Vec::new();
    c.receive(&bye, 0, &mut out).unwrap();
    assert!(matches!(out.last(), Some(Event::Closed)));
    assert!(!c.is_open());
    let _ = handshake_message;
}

// --- OpenSSL on loopback ---------------------------------------------------------

#[test]
fn interoperates_with_openssl_s_server_over_dtls() {
    use super::super::tls::tests::openssl;
    use std::time::{Duration, Instant};
    let Some(bin) = openssl::binary() else {
        eprintln!("skipped: no OpenSSL binary on this machine");
        return;
    };
    let Some(mut server) = openssl::Server::start(
        &bin,
        "00112233445566778899aabbccddeeff",
        Some(openssl::FFDHE2048_PEM),
        true,
    ) else {
        eprintln!("skipped: openssl s_server -dtls1_2 did not start with the AES70 suite");
        return;
    };
    // s_server sends this once the handshake is done.
    server.send_line("from s_server");
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.connect(("127.0.0.1", server.port)).unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let start = Instant::now();
    let now = || start.elapsed().as_millis() as Millis;
    let mut out = Vec::new();
    let mut c = DtlsClient::new(psk(&KEY), now(), &mut out).unwrap();
    let line = b"hello over DTLS\n";
    let mut session = None;
    let mut reply = Vec::new();
    while start.elapsed() < Duration::from_secs(20) && !reply.ends_with(b"\n") {
        for e in std::mem::take(&mut out) {
            match e {
                Event::Send(d) => {
                    sock.send(&d).unwrap();
                }
                Event::Dh(req) => c.dh_done(dh_compute(&req), now(), &mut out).unwrap(),
                Event::Established(s) => session = Some(s),
                Event::Data(d) => reply.extend(d),
                Event::Closed => panic!("s_server closed"),
            }
        }
        if !out.is_empty() {
            continue;
        }
        let mut buf = [0u8; 2048];
        match sock.recv(&mut buf) {
            Ok(n) => c.receive(&buf[..n], now(), &mut out).unwrap(),
            Err(_) => c.timer(now(), &mut out).unwrap(),
        }
    }
    let session = session.expect("the DTLS handshake with s_server finished");
    assert!(session.extended_master_secret);
    assert_eq!(session.hint.as_deref(), Some(&b"OCA-PSK"[..]));
    assert_eq!(reply, b"from s_server\n");
    // And the other way: s_server prints what it reads. On Windows it
    // reads the socket only between lines of its input, so a line goes
    // there each time round.
    sock.send(&c.seal(line).unwrap()).unwrap();
    let printed = (0..20).any(|_| {
        server.send_line("tick");
        server.printed("hello over DTLS", Duration::from_millis(500))
    });
    assert!(printed, "s_server printed the client's line");
    if let Some(bye) = c.close() {
        let _ = sock.send(&bye);
    }
    drop(server);
}
