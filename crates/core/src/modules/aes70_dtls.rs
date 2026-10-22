//! Secure OCP.1 over UDP: a DTLS 1.2 client (RFC 6347) with the suite and
//! the cryptography of [`super::tls`], whose security note applies here too.
//! What DTLS adds:
//!
//! - The cookie exchange (RFC 6347 4.2.1): a HelloVerifyRequest is answered
//!   with the same ClientHello carrying its cookie (at most four times); the
//!   first ClientHello and the request are left out of the handshake hash.
//! - Flights and retransmission (4.2.4): the last flight sent is kept and
//!   sent again when the timer expires, from 1 s doubling to 60 s, or when
//!   the device sends its previous flight again; records get new sequence
//!   numbers each time. A handshake message larger than a datagram is
//!   fragmented, and the device's fragments are reassembled, in order of
//!   message_seq, whatever order they arrive in.
//! - Epochs: epoch 0 is unprotected; epoch 1 begins with each side's
//!   ChangeCipherSpec. The 48-bit record sequence numbers are per epoch, and
//!   the MAC covers the epoch and sequence number.
//! - Replay protection (4.1.2.6): a 64-record sliding window over epoch 1,
//!   updated only by records that authenticate.
//! - A record that does not authenticate, or arrives in an epoch that cannot
//!   be read, is discarded silently, as RFC 6347 4.1.2.7 asks; it does not
//!   end the session. Alerts and unexpected handshake messages do, as over
//!   TLS.
//!
//! Each record carries at most 2^14 bytes; the module keeps each OCP.1 PDU
//! to one record and one datagram.

use std::collections::BTreeMap;

use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

use super::tls::{
    alert, alert_outcome, client_hello_body, client_key_exchange_body, fatal, finished_mismatch,
    handshake_name, key_block, master_secret, parse_server_hello, parse_server_key_exchange,
    premaster, random, sha256, verify_data, AlertOutcome, DhRequest, DhShared, Direction, Event,
    Failure, Psk, Session, CT_ALERT, CT_APPLICATION_DATA, CT_CHANGE_CIPHER_SPEC, CT_HANDSHAKE,
    HT_CLIENT_HELLO, HT_CLIENT_KEY_EXCHANGE, HT_FINISHED, HT_HELLO_REQUEST,
    HT_HELLO_VERIFY_REQUEST, HT_SERVER_HELLO, HT_SERVER_HELLO_DONE, HT_SERVER_KEY_EXCHANGE,
    MAX_CIPHERTEXT, MAX_EMPTY, MAX_HANDSHAKE, MAX_PLAINTEXT, R, RECORD_OVERHEAD, W,
};
use crate::module::Millis;

pub(crate) const DTLS12: u16 = 0xfefd;
/// The version a HelloVerifyRequest may carry whatever is negotiated
/// (RFC 6347 4.2.1).
const DTLS10: u16 = 0xfeff;
pub(crate) const RECORD_HEADER: usize = 13;
const HANDSHAKE_HEADER: usize = 12;
/// The datagram size handshake flights are packed into: within the
/// smallest path MTU DTLS assumes without discovery.
const MTU: usize = 1200;
/// The handshake bytes in one fragment, so a protected record still fits.
const FRAGMENT: usize = MTU - RECORD_HEADER - HANDSHAKE_HEADER - RECORD_OVERHEAD;
/// What a record adds to its plaintext at most.
pub(crate) const OVERHEAD: usize = RECORD_HEADER + RECORD_OVERHEAD;
const RTO_MIN: Millis = 1_000;
const RTO_MAX: Millis = 60_000;
/// The least time between retransmissions prompted by the device's own.
const RESEND_GAP: Millis = 250;
const MAX_COOKIES: u32 = 4;
const SEQ_MAX: u64 = (1 << 48) - 1;
/// Handshake messages accepted ahead of the next expected one.
const AHEAD: u16 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum St {
    /// ClientHello sent: waiting for a HelloVerifyRequest or ServerHello.
    Hello,
    KeyExchange,
    HelloDone,
    Dh,
    /// The client's last flight sent: waiting for the ChangeCipherSpec and
    /// Finished.
    Finished,
    Open,
    Closed,
}

/// A message of the last flight, kept to be sent again.
struct Outgoing {
    epoch: u16,
    /// A ChangeCipherSpec, or a handshake message of this type.
    ty: Option<u8>,
    seq: u16,
    body: Vec<u8>,
}

/// A handshake message being reassembled.
struct Partial {
    ty: u8,
    data: Vec<u8>,
    have: Vec<bool>,
    missing: usize,
}

/// A DTLS 1.2 client session over datagrams to one device.
pub(crate) struct DtlsClient {
    psk: Psk,
    st: St,
    client_random: [u8; 32],
    server_random: [u8; 32],
    cookie: Vec<u8>,
    cookies: u32,
    transcript: Vec<u8>,
    session: Session,
    dh: Option<DhRequest>,
    master: Zeroizing<[u8; 48]>,
    write: Option<Direction>,
    read: Option<Direction>,
    pending_read: Option<Direction>,
    /// The epoch application data and alerts go out in.
    epoch: u16,
    /// The next record sequence number of epochs 0 and 1.
    seq: [u64; 2],
    read_epoch: u16,
    /// The replay window: the highest sequence number authenticated in
    /// epoch 1 and a bit for each of the 64 below it.
    window: Option<(u64, u64)>,
    send_msg_seq: u16,
    recv_msg_seq: u16,
    partial: BTreeMap<u16, Partial>,
    flight: Vec<Outgoing>,
    deadline: Option<Millis>,
    rto: Millis,
    last_resend: Option<Millis>,
    empty: u32,
    discarded: u64,
}

fn dtls_message(ty: u8, seq: u16, body: &[u8]) -> Vec<u8> {
    let mut w = W::default();
    w.u8(ty)
        .u24(body.len())
        .u16(seq)
        .u24(0)
        .u24(body.len())
        .bytes(body);
    w.0
}

impl DtlsClient {
    /// Start a session: the first ClientHello goes out in `out`.
    pub(crate) fn new(psk: Psk, now: Millis, out: &mut Vec<Event>) -> Result<DtlsClient, Failure> {
        let mut client_random = [0u8; 32];
        random(&mut client_random)?;
        let mut c = DtlsClient {
            psk,
            st: St::Hello,
            client_random,
            server_random: [0; 32],
            cookie: Vec::new(),
            cookies: 0,
            transcript: Vec::new(),
            session: Session::default(),
            dh: None,
            master: Zeroizing::new([0; 48]),
            write: None,
            read: None,
            pending_read: None,
            epoch: 0,
            seq: [0, 0],
            read_epoch: 0,
            window: None,
            send_msg_seq: 0,
            recv_msg_seq: 0,
            partial: BTreeMap::new(),
            flight: Vec::new(),
            deadline: None,
            rto: RTO_MIN,
            last_resend: None,
            empty: 0,
            discarded: 0,
        };
        c.hello(now, out)?;
        Ok(c)
    }

    pub(crate) fn is_open(&self) -> bool {
        self.st == St::Open
    }

    /// When the retransmission timer expires, if it runs.
    pub(crate) fn deadline(&self) -> Option<Millis> {
        self.deadline
    }

    /// Records discarded because they did not authenticate.
    #[cfg(test)]
    pub(crate) fn discarded(&self) -> u64 {
        self.discarded
    }

    /// A datagram from the device.
    pub(crate) fn receive(
        &mut self,
        datagram: &[u8],
        now: Millis,
        out: &mut Vec<Event>,
    ) -> Result<(), Failure> {
        if self.st == St::Closed {
            return Ok(());
        }
        let result = self.records(datagram, now, out);
        if let Err(f) = &result {
            self.abort(f, out);
        }
        result
    }

    /// The retransmission timer.
    pub(crate) fn timer(&mut self, now: Millis, out: &mut Vec<Event>) -> Result<(), Failure> {
        match self.deadline {
            Some(at) if now >= at => {}
            _ => return Ok(()),
        }
        let result = (|| {
            self.rto = (self.rto * 2).min(RTO_MAX);
            self.send_flight(out)?;
            self.deadline = Some(now + self.rto);
            Ok(())
        })();
        if let Err(f) = &result {
            self.abort(f, out);
        }
        result
    }

    /// The Diffie-Hellman outcome asked for by [`Event::Dh`].
    pub(crate) fn dh_done(
        &mut self,
        shared: Result<DhShared, Failure>,
        now: Millis,
        out: &mut Vec<Event>,
    ) -> Result<(), Failure> {
        if self.st != St::Dh {
            return Ok(());
        }
        let result = self.key_exchange(shared, now, out);
        if let Err(f) = &result {
            self.abort(f, out);
        }
        result
    }

    /// One datagram of application data: a single record.
    pub(crate) fn seal(&mut self, data: &[u8]) -> Result<Vec<u8>, Failure> {
        if self.st != St::Open {
            return Err(fatal(alert::INTERNAL_ERROR, "the session is not open"));
        }
        if data.len() > MAX_PLAINTEXT {
            return Err(fatal(
                alert::INTERNAL_ERROR,
                "a datagram larger than a record",
            ));
        }
        self.record(CT_APPLICATION_DATA, 1, data)
    }

    /// End the session: the close_notify to send, if it is still up.
    pub(crate) fn close(&mut self) -> Option<Vec<u8>> {
        if self.st == St::Closed {
            return None;
        }
        let bytes = self
            .record(CT_ALERT, self.epoch, &[alert::WARNING, alert::CLOSE_NOTIFY])
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
        self.flight.clear();
        self.partial.clear();
        self.deadline = None;
    }

    fn abort(&mut self, f: &Failure, out: &mut Vec<Event>) {
        if let Some(a) = f.alert {
            if let Ok(bytes) = self.record(CT_ALERT, self.epoch, &[alert::FATAL, a]) {
                out.push(Event::Send(bytes));
            }
        }
        self.shut();
    }

    /// One record in `epoch`, protected in epoch 1.
    fn record(&mut self, ty: u8, epoch: u16, data: &[u8]) -> Result<Vec<u8>, Failure> {
        let e = usize::from(epoch);
        let seq = self.seq[e];
        if seq > SEQ_MAX {
            return Err(fatal(
                alert::INTERNAL_ERROR,
                "the sequence number would wrap",
            ));
        }
        self.seq[e] = seq + 1;
        let full = (u64::from(epoch) << 48) | seq;
        let fragment = if epoch == 0 {
            data.to_vec()
        } else {
            let d = self
                .write
                .as_ref()
                .ok_or_else(|| fatal(alert::INTERNAL_ERROR, "no keys for epoch 1"))?;
            d.seal(full, ty, DTLS12, data)?
        };
        let mut w = W::default();
        w.u8(ty).u16(DTLS12).bytes(&full.to_be_bytes()[..2]);
        w.bytes(&full.to_be_bytes()[2..]).vec16(&fragment);
        Ok(w.0)
    }

    /// Send the last flight (again), packing its records into datagrams.
    fn send_flight(&mut self, out: &mut Vec<Event>) -> Result<(), Failure> {
        let mut records = Vec::new();
        for i in 0..self.flight.len() {
            let (epoch, ty, seq) = (self.flight[i].epoch, self.flight[i].ty, self.flight[i].seq);
            match ty {
                None => records.push(self.record(CT_CHANGE_CIPHER_SPEC, epoch, &[1])?),
                Some(ty) => {
                    let body = self.flight[i].body.clone();
                    let mut offset = 0;
                    loop {
                        let n = (body.len() - offset).min(FRAGMENT);
                        let mut w = W::default();
                        w.u8(ty).u24(body.len()).u16(seq).u24(offset).u24(n);
                        w.bytes(&body[offset..offset + n]);
                        records.push(self.record(CT_HANDSHAKE, epoch, &w.0)?);
                        offset += n;
                        if offset >= body.len() {
                            break;
                        }
                    }
                }
            }
        }
        let mut datagram: Vec<u8> = Vec::new();
        for r in records {
            if !datagram.is_empty() && datagram.len() + r.len() > MTU {
                out.push(Event::Send(std::mem::take(&mut datagram)));
            }
            datagram.extend(r);
        }
        if !datagram.is_empty() {
            out.push(Event::Send(datagram));
        }
        Ok(())
    }

    /// Send a new flight and start its timer.
    fn start_flight(
        &mut self,
        flight: Vec<Outgoing>,
        now: Millis,
        out: &mut Vec<Event>,
    ) -> Result<(), Failure> {
        self.flight = flight;
        self.rto = RTO_MIN;
        self.send_flight(out)?;
        self.deadline = Some(now + self.rto);
        Ok(())
    }

    /// The ClientHello, with the cookie once the device has asked for one;
    /// the handshake hash starts with it.
    fn hello(&mut self, now: Millis, out: &mut Vec<Event>) -> Result<(), Failure> {
        let body = client_hello_body(DTLS12, &self.client_random, Some(&self.cookie));
        let seq = self.send_msg_seq;
        self.send_msg_seq += 1;
        self.transcript = dtls_message(HT_CLIENT_HELLO, seq, &body);
        self.start_flight(
            vec![Outgoing {
                epoch: 0,
                ty: Some(HT_CLIENT_HELLO),
                seq,
                body,
            }],
            now,
            out,
        )
    }

    fn records(
        &mut self,
        datagram: &[u8],
        now: Millis,
        out: &mut Vec<Event>,
    ) -> Result<(), Failure> {
        let mut rest = datagram;
        while self.st != St::Closed && rest.len() >= RECORD_HEADER {
            let ty = rest[0];
            let version = u16::from_be_bytes([rest[1], rest[2]]);
            let epoch = u16::from_be_bytes([rest[3], rest[4]]);
            let mut seq = [0u8; 8];
            seq[2..].copy_from_slice(&rest[5..11]);
            let seq = u64::from_be_bytes(seq);
            let len = usize::from(u16::from_be_bytes([rest[11], rest[12]]));
            if rest.len() < RECORD_HEADER + len {
                // A record cut short: the rest of the datagram is unusable.
                self.discarded += 1;
                break;
            }
            let fragment = &rest[RECORD_HEADER..RECORD_HEADER + len];
            rest = &rest[RECORD_HEADER + len..];
            let version_ok = version == DTLS12 || (version == DTLS10 && self.st == St::Hello);
            if !version_ok || len > MAX_CIPHERTEXT {
                self.discarded += 1;
                continue;
            }
            let plain = match (epoch, self.read_epoch) {
                (0, 0) => fragment.to_vec(),
                // After the device's ChangeCipherSpec only a retransmitted
                // handshake message of epoch 0 means anything, and once the
                // handshake is done nothing unprotected does.
                (0, _) if ty == CT_HANDSHAKE && self.st != St::Open => fragment.to_vec(),
                (1, 1) => {
                    if self.replayed(seq) {
                        self.discarded += 1;
                        continue;
                    }
                    let full = (1u64 << 48) | seq;
                    let opened = self
                        .read
                        .as_ref()
                        .map(|d| d.open(full, ty, DTLS12, fragment));
                    match opened {
                        Some(Ok(p)) => {
                            self.accept(seq);
                            p
                        }
                        _ => {
                            self.discarded += 1;
                            continue;
                        }
                    }
                }
                _ => {
                    self.discarded += 1;
                    continue;
                }
            };
            if plain.len() > MAX_PLAINTEXT {
                self.discarded += 1;
                continue;
            }
            self.record_in(ty, epoch, plain, now, out)?;
        }
        Ok(())
    }

    fn replayed(&self, seq: u64) -> bool {
        match self.window {
            None => false,
            Some((top, _)) if seq > top => false,
            Some((top, bits)) => {
                let back = top - seq;
                back >= 64 || bits & (1 << back) != 0
            }
        }
    }

    fn accept(&mut self, seq: u64) {
        self.window = Some(match self.window {
            None => (seq, 1),
            Some((top, bits)) if seq > top => {
                let shift = seq - top;
                (seq, if shift >= 64 { 1 } else { (bits << shift) | 1 })
            }
            Some((top, bits)) => (top, bits | (1 << (top - seq))),
        });
    }

    fn record_in(
        &mut self,
        ty: u8,
        epoch: u16,
        plain: Vec<u8>,
        now: Millis,
        out: &mut Vec<Event>,
    ) -> Result<(), Failure> {
        match ty {
            CT_CHANGE_CIPHER_SPEC => {
                if plain != [1] {
                    self.discarded += 1;
                } else if self.st == St::Finished && self.read_epoch == 0 {
                    self.read = self.pending_read.take();
                    self.read_epoch = 1;
                    self.window = None;
                }
                Ok(())
            }
            CT_ALERT => {
                if plain.len() != 2 {
                    self.discarded += 1;
                    return Ok(());
                }
                match alert_outcome(plain[0], plain[1], self.st != St::Open) {
                    AlertOutcome::Ignore => Ok(()),
                    AlertOutcome::Close => {
                        if let Some(bytes) = self.close() {
                            out.push(Event::Send(bytes));
                        }
                        out.push(Event::Closed);
                        Ok(())
                    }
                    AlertOutcome::Fatal(f) => Err(f),
                }
            }
            CT_HANDSHAKE => {
                let mut r = R::new(&plain, "handshake fragment");
                while !r.is_empty() && self.st != St::Closed {
                    let ty = r.u8()?;
                    let len = r.u24()?;
                    let seq = r.u16()?;
                    let offset = r.u24()?;
                    let n = r.u24()?;
                    let data = r.take(n)?;
                    self.fragment(ty, len, seq, offset, data, epoch, now, out)?;
                }
                Ok(())
            }
            CT_APPLICATION_DATA => {
                // Application data is read in epoch 1 only; before the
                // handshake ends it is dropped (it may have overtaken the
                // device's Finished).
                if self.st != St::Open || epoch != 1 {
                    self.discarded += 1;
                } else if plain.is_empty() {
                    self.empty += 1;
                    if self.empty > MAX_EMPTY {
                        return Err(fatal(
                            alert::UNEXPECTED_MESSAGE,
                            "too many empty records in a row",
                        ));
                    }
                } else {
                    self.empty = 0;
                    out.push(Event::Data(plain));
                }
                Ok(())
            }
            _ => {
                self.discarded += 1;
                Ok(())
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fragment(
        &mut self,
        ty: u8,
        len: usize,
        seq: u16,
        offset: usize,
        data: &[u8],
        epoch: u16,
        now: Millis,
        out: &mut Vec<Event>,
    ) -> Result<(), Failure> {
        if len > MAX_HANDSHAKE || offset + data.len() > len {
            self.discarded += 1;
            return Ok(());
        }
        if ty == HT_HELLO_VERIFY_REQUEST {
            if self.st == St::Hello && epoch == 0 && offset == 0 && data.len() == len {
                return self.hello_verify(seq, data, now, out);
            }
            return Ok(());
        }
        if seq < self.recv_msg_seq {
            // The device sent its previous flight again: it has not had
            // ours, so ours goes again.
            if self.st == St::Finished
                && ty == HT_SERVER_HELLO_DONE
                && self
                    .last_resend
                    .is_none_or(|at| now.saturating_sub(at) >= RESEND_GAP)
            {
                self.last_resend = Some(now);
                self.send_flight(out)?;
            }
            return Ok(());
        }
        if seq - self.recv_msg_seq > AHEAD {
            return Ok(());
        }
        // The Finished comes protected, everything before it in the clear.
        if (ty == HT_FINISHED) != (epoch == 1) {
            self.discarded += 1;
            return Ok(());
        }
        let p = self.partial.entry(seq).or_insert_with(|| Partial {
            ty,
            data: vec![0; len],
            have: vec![false; len],
            missing: len,
        });
        if p.ty != ty || p.data.len() != len {
            self.discarded += 1;
            return Ok(());
        }
        for (i, &b) in data.iter().enumerate() {
            if !p.have[offset + i] {
                p.have[offset + i] = true;
                p.data[offset + i] = b;
                p.missing -= 1;
            }
        }
        while self.st != St::Closed && self.st != St::Dh {
            let complete =
                matches!(self.partial.get(&self.recv_msg_seq), Some(p) if p.missing == 0);
            if !complete {
                break;
            }
            let p = self.partial.remove(&self.recv_msg_seq).expect("present");
            let msg = dtls_message(p.ty, self.recv_msg_seq, &p.data);
            self.recv_msg_seq += 1;
            self.message(msg, out)?;
        }
        Ok(())
    }

    fn hello_verify(
        &mut self,
        seq: u16,
        body: &[u8],
        now: Millis,
        out: &mut Vec<Event>,
    ) -> Result<(), Failure> {
        if self.cookies >= MAX_COOKIES {
            return Err(fatal(
                alert::HANDSHAKE_FAILURE,
                "the device asked for a cookie too many times",
            ));
        }
        let mut r = R::new(body, "HelloVerifyRequest");
        let version = r.u16()?;
        let cookie = r.vec8()?.to_vec();
        r.end()?;
        if version != DTLS10 && version != DTLS12 {
            return Err(fatal(
                alert::PROTOCOL_VERSION,
                format!("a HelloVerifyRequest of version {version:#06x}"),
            ));
        }
        self.cookies += 1;
        self.cookie = cookie;
        self.recv_msg_seq = seq.wrapping_add(1);
        self.partial.clear();
        // The same ClientHello with the cookie; the first one and the
        // request are left out of the handshake hash (RFC 6347 4.2.1).
        self.hello(now, out)
    }

    fn message(&mut self, msg: Vec<u8>, out: &mut Vec<Event>) -> Result<(), Failure> {
        let ty = msg[0];
        let body = &msg[HANDSHAKE_HEADER..];
        if ty == HT_HELLO_REQUEST {
            if self.st == St::Open {
                let bytes = self.record(
                    CT_ALERT,
                    self.epoch,
                    &[alert::WARNING, alert::NO_RENEGOTIATION],
                )?;
                out.push(Event::Send(bytes));
            }
            return Ok(());
        }
        match (self.st, ty) {
            (St::Hello, HT_SERVER_HELLO) => {
                let hello = parse_server_hello(body, DTLS12)?;
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
                // The device's flight is complete: nothing to resend while
                // the key exchange is computed.
                self.deadline = None;
                let dh = self.dh.take().expect("set by the ServerKeyExchange");
                out.push(Event::Dh(dh));
            }
            (St::Finished, HT_FINISHED) => {
                let expected = verify_data(&self.master, b"server finished", &self.transcript);
                if body.len() != expected.len() || !bool::from(body.ct_eq(&expected)) {
                    return Err(finished_mismatch());
                }
                self.st = St::Open;
                self.deadline = None;
                self.flight.clear();
                self.transcript = Vec::new();
                out.push(Event::Established(self.session.clone()));
            }
            (_, ty) => {
                return Err(fatal(
                    alert::UNEXPECTED_MESSAGE,
                    format!("{} from the device out of place", handshake_name(ty)),
                ))
            }
        }
        Ok(())
    }

    fn key_exchange(
        &mut self,
        shared: Result<DhShared, Failure>,
        now: Millis,
        out: &mut Vec<Event>,
    ) -> Result<(), Failure> {
        let shared = shared?;
        let cke_body = client_key_exchange_body(&self.psk.identity, &shared.yc);
        let cke_seq = self.send_msg_seq;
        self.transcript
            .extend(dtls_message(HT_CLIENT_KEY_EXCHANGE, cke_seq, &cke_body));
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
        self.write = Some(client);
        self.pending_read = Some(server);
        self.epoch = 1;
        let fin_seq = cke_seq + 1;
        self.send_msg_seq = cke_seq + 2;
        let fin_body = verify_data(&self.master, b"client finished", &self.transcript).to_vec();
        self.transcript
            .extend(dtls_message(HT_FINISHED, fin_seq, &fin_body));
        self.st = St::Finished;
        self.start_flight(
            vec![
                Outgoing {
                    epoch: 0,
                    ty: Some(HT_CLIENT_KEY_EXCHANGE),
                    seq: cke_seq,
                    body: cke_body,
                },
                Outgoing {
                    epoch: 0,
                    ty: None,
                    seq: 0,
                    body: Vec::new(),
                },
                Outgoing {
                    epoch: 1,
                    ty: Some(HT_FINISHED),
                    seq: fin_seq,
                    body: fin_body,
                },
            ],
            now,
            out,
        )
    }
}

#[cfg(test)]
#[path = "aes70_dtls_tests.rs"]
pub(crate) mod tests;
