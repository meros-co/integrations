//! RTP-MIDI: MIDI over UDP as Apple's network MIDI driver and the programs
//! that interoperate with it carry it, for the MIDI integrations. The core
//! takes either part: the initiator's, inviting a session another host
//! offers (advertised as `_apple-midi._udp`, found by mDNS discovery) and
//! joining it, or the responder's, hosting a session on its own ports and
//! accepting the invitation of the host that joins it.
//!
//! Session protocol from Apple's "MIDI Network Driver Protocol" (2016):
//!
//! - Each participant uses two UDP ports, N (control) and N+1 (data). The
//!   initiator sends an invitation (`IN`) to the control port, and on its
//!   acceptance (`OK`) another to the data port, from its own control and
//!   data ports. A refusal is `NO`; leaving the session is `BY`.
//! - Session packets are `FF FF`, the two-character command, protocol
//!   version 2, the initiator's token, the sender's SSRC and, for `IN` and
//!   `OK`, a NUL-terminated UTF-8 name, all 32-bit fields big-endian. An
//!   unanswered invitation is sent again every second, twelve times.
//! - Clock synchronisation (`CK`) runs on the data ports: `FF FF 'C' 'K'`,
//!   SSRC, a count byte and three unused, then three 64-bit timestamps in
//!   units of 100 microseconds. The initiator sends count 0 with its time,
//!   the responder count 1 adding its own, the initiator count 2 adding its
//!   time again; it must do so at least once a minute or the responder may
//!   end the session, and more often at first.
//! - Receiver feedback (`RS`) on the control port names the last packet
//!   received, so the sender can shorten its recovery journal: SSRC, then the
//!   sequence number in a 32-bit field. The core sends it a second after a
//!   packet arrives, and moves its own journal's checkpoint on the peer's.
//! - MIDI travels in RTP packets on the data ports (RFC 6295): version 2,
//!   payload type 0x61, the timestamp in the same units (its low 32 bits).
//!   Apple sends a recovery journal in every packet; received packets need
//!   not carry one.
//!
//! MIDI payload from RFC 6295, "RTP Payload Format for MIDI" (2011), §2-3:
//! the MIDI command section's header (B, J, Z, P and a 4- or 12-bit length),
//! then commands each after a delta time (1 to 4 bytes, 7 bits each, high bit
//! set on all but the last; the first command's only when Z is set), running
//! status within a packet, and SysEx whole or in segments
//! (`F0..F0`, `F7..F0`, `F7..F7`, cancelled by `F7 F4`, a dropped F7 coded
//! as F5). The core sends whole messages with delta times of 0, segments a
//! SysEx longer than fits in one packet, and puts a recovery journal in
//! every packet (J set; [`super::rtp_journal`]). After MIDI goes out it
//! sends guard packets (an empty MIDI list with the journal) 100 ms later,
//! again 100 ms after that, then doubling to 1 s until the peer's feedback
//! says it has the last packet, so a lost last packet is repaired without
//! waiting for more MIDI (RFC 4696 §4.2, the guard packets of its [NMP]).
//!
//! A packet that arrives after a gap in the sequence numbers is repaired
//! from its recovery journal, when it carries one (RFC 6295 §5 and
//! Appendix A): the channel chapters P (program, with bank select), C
//! (controllers coded with the value tool), W (pitch wheel), N (notes on and
//! off), T (channel pressure) and A (poly pressure) are replayed as MIDI
//! before the packet's own commands. Chapters M and E, and the system
//! journal, are skipped by their lengths. Duplicate and late packets are
//! dropped.
//!
//! Liveness: a session gives no sign of the peer but its answers, so the
//! peer's reply to each clock synchronisation is its proof of life and its
//! round trip is the device's latency. Synchronisation runs every 1.5 s six
//! times after joining, then every 10 s; three unanswered in a row end the
//! session, which is then invited again after 1 s, doubling to 30 s, as after
//! a refusal, an unanswered invitation or the peer's `BY`.
//!
//! Hosting, the core listens on its control port (5004 unless set) and the
//! next, answers an invitation (`IN`) on each with `OK` from the same port,
//! and sends to the ports the invitations came from. Only the device's host
//! may join when one is given, otherwise any host; while one host is in the
//! session another's invitation is refused (`NO`). The initiator keeps the
//! session with clock synchronisation, which the core answers (the round
//! trip of the exchange is the latency); Apple has it run at least once a
//! minute, so a session with none for 90 s is ended (`BY`), and the core
//! waits for an invitation again, as after the peer's `BY`.

use std::net::{IpAddr, SocketAddr};

use crate::json;

use super::rtp_journal::{Item, Journal};
use crate::midi::Message;
use crate::module::{Bind, CommandError, Connection, Cx, Key, Millis};

pub(crate) const CONTROL: Key = "rtp-control";
pub(crate) const DATA: Key = "rtp-data";
const INVITE: Key = "rtp-invite";
const SYNC: Key = "rtp-sync";
const FEEDBACK: Key = "rtp-feedback";
const RETRY: Key = "rtp-retry";
const GUARD: Key = "rtp-guard";
const WATCH: Key = "rtp-watch";

/// The default control port: the one Apple's Audio MIDI Setup offers for a
/// new session.
pub(crate) const DEFAULT_PORT: u16 = 5004;

const INVITE_EVERY: Millis = 1000;
const INVITE_TRIES: u32 = 12;
const SYNC_FAST: Millis = 1500;
const SYNC_FAST_COUNT: u32 = 6;
const SYNC_EVERY: Millis = 10_000;
const SYNC_MISSES: u32 = 3;
const FEEDBACK_AFTER: Millis = 1000;
const RETRY_MIN: Millis = 1000;
const RETRY_MAX: Millis = 30_000;
const GUARD_FIRST: Millis = 100;
const GUARD_MAX: Millis = 1000;
/// Hosting, a session whose initiator has not synchronised clocks for this
/// long is ended, and how often that is checked.
const PEER_SYNC_TIMEOUT: Millis = 90_000;
const WATCH_EVERY: Millis = 10_000;
/// The MIDI list of one packet stays under this, so a packet is never
/// fragmented on an Ethernet path.
const MAX_LIST: usize = 1000;
const VERSION: u32 = 2;
const PAYLOAD_TYPE: u8 = 0x61;
/// The ephemeral range a session's control port is chosen in, with room for
/// its data port above it.
const LOCAL_PORTS: (u16, u16) = (49152, 65534);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// Waiting to invite again.
    Idle,
    Control {
        tries: u32,
    },
    Data {
        tries: u32,
    },
    /// Hosting: waiting for an invitation.
    Listen,
    /// Hosting: the control port's invitation accepted, the data port's
    /// awaited.
    Accepted,
    Open,
}

/// One RTP-MIDI session, the core as initiator or hosting it.
#[derive(Debug)]
pub(crate) struct Rtp {
    peer: IpAddr,
    peer_control: u16,
    peer_data: u16,
    /// Hosting the session rather than joining one, and the only host that
    /// may join, if any.
    hosting: bool,
    only: Option<IpAddr>,
    /// Hosting: when the initiator last synchronised clocks.
    last_sync: Millis,
    name: String,
    /// The local control port the operator fixed, if any.
    fixed_local: Option<u16>,
    local: u16,
    ssrc: u32,
    token: u32,
    phase: Phase,
    peer_ssrc: Option<u32>,
    /// The next packet's extended sequence number (its low 16 bits go on
    /// the wire).
    seq: u64,
    journal: Journal,
    /// Guard packets sent since MIDI last went out, while they are being
    /// sent, and the last packet that carried MIDI.
    guards: Option<u32>,
    last_midi: u64,
    /// The last sequence number received, for feedback.
    received_seq: Option<u16>,
    feedback_armed: bool,
    /// The timestamp of the clock synchronisation awaiting its answer.
    sync_sent: Option<u64>,
    syncs: u32,
    misses: u32,
    retry: Millis,
    decoder: Decoder,
    rng: u64,
}

fn session_packet(command: &[u8; 2], token: u32, ssrc: u32, name: Option<&str>) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFF, command[0], command[1]];
    out.extend_from_slice(&VERSION.to_be_bytes());
    out.extend_from_slice(&token.to_be_bytes());
    out.extend_from_slice(&ssrc.to_be_bytes());
    if let Some(name) = name {
        out.extend_from_slice(name.replace('\0', "").as_bytes());
        out.push(0);
    }
    out
}

fn sync_packet(ssrc: u32, count: u8, stamps: [u64; 3]) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFF, b'C', b'K'];
    out.extend_from_slice(&ssrc.to_be_bytes());
    out.extend_from_slice(&[count, 0, 0, 0]);
    for s in stamps {
        out.extend_from_slice(&s.to_be_bytes());
    }
    out
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

impl Rtp {
    /// A session with the peer whose control port is `peer`, announcing
    /// `name`. `seed` makes the SSRC, token and chosen ports.
    pub(crate) fn new(peer: SocketAddr, name: &str, fixed_local: Option<u16>, seed: u64) -> Rtp {
        let mut rtp = Rtp {
            peer: peer.ip(),
            peer_control: peer.port(),
            peer_data: peer.port().wrapping_add(1),
            hosting: false,
            only: None,
            last_sync: 0,
            name: name.to_string(),
            fixed_local,
            local: 0,
            ssrc: 0,
            token: 0,
            phase: Phase::Idle,
            peer_ssrc: None,
            seq: 0,
            journal: Journal::new(),
            guards: None,
            last_midi: 0,
            received_seq: None,
            feedback_armed: false,
            sync_sent: None,
            syncs: 0,
            misses: 0,
            retry: RETRY_MIN,
            decoder: Decoder::default(),
            rng: seed | 1,
        };
        rtp.ssrc = rtp.random() as u32;
        rtp.seq = rtp.random() as u16 as u64;
        rtp.local = rtp.pick_local();
        rtp
    }

    /// A session the core hosts on `local` (control) and the port after it,
    /// which `only` may join when given, otherwise any host.
    pub(crate) fn host(local: u16, only: Option<IpAddr>, name: &str, seed: u64) -> Rtp {
        let unspecified = IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED);
        let mut rtp = Rtp::new(
            SocketAddr::new(only.unwrap_or(unspecified), 0),
            name,
            Some(local),
            seed,
        );
        rtp.hosting = true;
        rtp.only = only;
        rtp.phase = Phase::Listen;
        rtp
    }

    /// Whether the core hosts the session.
    pub(crate) fn hosting(&self) -> bool {
        self.hosting
    }

    /// xorshift64*: unpredictable enough for SSRCs, tokens and ports.
    fn random(&mut self) -> u64 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn pick_local(&mut self) -> u16 {
        match self.fixed_local {
            Some(port) => port,
            None => {
                let span = (LOCAL_PORTS.1 - LOCAL_PORTS.0) as u64;
                LOCAL_PORTS.0 + (self.random() % span) as u16
            }
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.phase == Phase::Open
    }

    /// The peer's control port, as the source of what it sends.
    pub(crate) fn source(&self) -> SocketAddr {
        SocketAddr::new(self.peer, self.peer_control)
    }

    /// Our clock: 100-microsecond units since the session started.
    fn stamp(cx: &Cx) -> u64 {
        cx.now() * 10
    }

    fn control_addr(&self) -> SocketAddr {
        SocketAddr::new(self.peer, self.peer_control)
    }

    fn data_addr(&self) -> SocketAddr {
        SocketAddr::new(self.peer, self.peer_data)
    }

    fn open_sockets(&self, cx: &mut Cx) {
        cx.udp_open(
            CONTROL,
            Bind::Any {
                address: None,
                port: self.local,
            },
        );
        cx.udp_open(
            DATA,
            Bind::Any {
                address: None,
                port: self.local + 1,
            },
        );
    }

    pub(crate) fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.open_sockets(cx);
        if self.hosting {
            self.phase = Phase::Listen;
        } else {
            self.invite(cx);
        }
    }

    fn invite(&mut self, cx: &mut Cx) {
        self.token = self.random() as u32;
        self.peer_ssrc = None;
        self.phase = Phase::Control { tries: 1 };
        let packet = session_packet(b"IN", self.token, self.ssrc, Some(&self.name));
        cx.udp_send(CONTROL, self.control_addr(), packet);
        cx.set_timer(INVITE, INVITE_EVERY);
    }

    fn bye(&self, cx: &mut Cx) {
        let packet = session_packet(b"BY", self.token, self.ssrc, None);
        cx.udp_send(CONTROL, self.control_addr(), packet);
    }

    /// The session failed: report it and invite again after a while, or,
    /// hosting, wait for the next invitation.
    fn fail(&mut self, cx: &mut Cx, reason: String) {
        for key in [INVITE, SYNC, FEEDBACK, GUARD, WATCH] {
            cx.cancel_timer(key);
        }
        self.sync_sent = None;
        self.feedback_armed = false;
        if self.hosting {
            self.phase = Phase::Listen;
            self.peer_ssrc = None;
            cx.connection(Connection::Disconnected {
                reason: format!("{reason}; waiting for an invitation"),
            });
            return;
        }
        self.phase = Phase::Idle;
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry);
        self.retry = (self.retry * 2).min(RETRY_MAX);
    }

    /// Both ports joined: the session is open.
    fn opened(&mut self, cx: &mut Cx) {
        cx.cancel_timer(INVITE);
        self.phase = Phase::Open;
        self.retry = RETRY_MIN;
        self.syncs = 0;
        self.misses = 0;
        self.sync_sent = None;
        self.received_seq = None;
        self.decoder = Decoder::default();
        self.journal = Journal::new();
        self.guards = None;
        cx.connection(Connection::Connected);
        if self.hosting {
            self.last_sync = cx.now();
            cx.set_timer(WATCH, WATCH_EVERY);
        } else {
            self.sync(cx);
        }
    }

    fn sync(&mut self, cx: &mut Cx) {
        if self.sync_sent.is_some() {
            self.misses += 1;
            if self.misses >= SYNC_MISSES {
                self.bye(cx);
                self.fail(
                    cx,
                    format!(
                        "the peer stopped answering clock synchronisation ({SYNC_MISSES} in a row)"
                    ),
                );
                return;
            }
        }
        let now = Rtp::stamp(cx);
        self.sync_sent = Some(now);
        cx.udp_send(
            DATA,
            self.data_addr(),
            sync_packet(self.ssrc, 0, [now, 0, 0]),
        );
        self.syncs += 1;
        let next = if self.syncs < SYNC_FAST_COUNT {
            SYNC_FAST
        } else {
            SYNC_EVERY
        };
        cx.set_timer(SYNC, next);
    }

    /// A timer of the session's; false for any other.
    pub(crate) fn timer(&mut self, cx: &mut Cx, key: Key) -> bool {
        match key {
            INVITE => {
                let (tries, socket, to, port) = match self.phase {
                    Phase::Control { tries } => (tries, CONTROL, self.control_addr(), "control"),
                    Phase::Data { tries } => (tries, DATA, self.data_addr(), "data"),
                    _ => return true,
                };
                if tries >= INVITE_TRIES {
                    self.fail(
                        cx,
                        format!(
                            "no answer to the invitation on the {port} port {} ({INVITE_TRIES} tries)",
                            to.port()
                        ),
                    );
                    return true;
                }
                let packet = session_packet(b"IN", self.token, self.ssrc, Some(&self.name));
                cx.udp_send(socket, to, packet);
                self.phase = match self.phase {
                    Phase::Control { .. } => Phase::Control { tries: tries + 1 },
                    _ => Phase::Data { tries: tries + 1 },
                };
                cx.set_timer(INVITE, INVITE_EVERY);
            }
            SYNC => {
                if self.is_open() {
                    self.sync(cx);
                }
            }
            FEEDBACK => {
                self.feedback_armed = false;
                if let (true, Some(seq)) = (self.is_open(), self.received_seq) {
                    let mut out = vec![0xFF, 0xFF, b'R', b'S'];
                    out.extend_from_slice(&self.ssrc.to_be_bytes());
                    out.extend_from_slice(&((seq as u32) << 16).to_be_bytes());
                    cx.udp_send(CONTROL, self.control_addr(), out);
                }
            }
            RETRY if self.hosting => {
                cx.connection(Connection::Connecting);
                self.open_sockets(cx);
                self.phase = Phase::Listen;
            }
            RETRY => self.invite(cx),
            WATCH => {
                if self.is_open() {
                    if cx.now().saturating_sub(self.last_sync) >= PEER_SYNC_TIMEOUT {
                        self.bye(cx);
                        self.fail(
                            cx,
                            format!(
                                "the peer has not synchronised clocks for {} s",
                                PEER_SYNC_TIMEOUT / 1000
                            ),
                        );
                    } else {
                        cx.set_timer(WATCH, WATCH_EVERY);
                    }
                }
            }
            GUARD => {
                if let Some(sent) = self.guards {
                    if self.is_open() && !self.journal.acknowledged(self.last_midi) {
                        self.packet(cx, &[], Vec::new());
                        let sent = sent + 1;
                        self.guards = Some(sent);
                        cx.set_timer(GUARD, (GUARD_FIRST << (sent - 1).min(4)).min(GUARD_MAX));
                    } else {
                        self.guards = None;
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// A socket of the session's could not be bound: choose other ports,
    /// unless the operator fixed them. False for another socket.
    pub(crate) fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) -> bool {
        if socket != CONTROL && socket != DATA {
            return false;
        }
        cx.udp_close(CONTROL);
        cx.udp_close(DATA);
        if self.hosting {
            let reason = format!(
                "local UDP port {} or {}: {message}",
                self.local,
                self.local + 1
            );
            if self.phase == Phase::Open {
                self.bye(cx);
            }
            for key in [SYNC, FEEDBACK, GUARD, WATCH] {
                cx.cancel_timer(key);
            }
            self.phase = Phase::Idle;
            cx.connection(Connection::Disconnected { reason });
            cx.set_timer(RETRY, self.retry);
            self.retry = (self.retry * 2).min(RETRY_MAX);
            return true;
        }
        let fixed = self.fixed_local.is_some();
        let reason = format!(
            "local UDP port {} or {}: {message}",
            self.local,
            self.local + 1
        );
        self.local = self.pick_local();
        self.open_sockets(cx);
        if fixed {
            self.fail(cx, reason);
        } else if self.phase != Phase::Idle {
            self.invite(cx);
        }
        true
    }

    /// The peer and every packet it sends; the MIDI it carried, as bytes.
    pub(crate) fn datagram(
        &mut self,
        cx: &mut Cx,
        socket: Key,
        from: SocketAddr,
        data: &[u8],
    ) -> Vec<u8> {
        let session = data.starts_with(&[0xFF, 0xFF]) && data.len() >= 4;
        if self.hosting && session && &data[2..4] == b"IN" {
            self.invited(cx, socket, from, data);
            return Vec::new();
        }
        if from.ip() != self.peer {
            return Vec::new();
        }
        if session {
            self.session(cx, socket, data);
            return Vec::new();
        }
        if socket != DATA || !self.is_open() || data.first().is_none_or(|b| b >> 6 != 2) {
            return Vec::new();
        }
        let Some((midi, seq)) = self.decoder.packet(data) else {
            return Vec::new();
        };
        cx.alive();
        self.received_seq = Some(seq);
        if !self.feedback_armed {
            self.feedback_armed = true;
            cx.set_timer(FEEDBACK, FEEDBACK_AFTER);
        }
        midi
    }

    fn session(&mut self, cx: &mut Cx, socket: Key, data: &[u8]) {
        let command = [data[2], data[3]];
        match &command {
            b"OK" | b"NO" => {
                let (Some(token), Some(ssrc)) = (u32_at(data, 8), u32_at(data, 12)) else {
                    return;
                };
                if token != self.token {
                    return;
                }
                let expected = match self.phase {
                    Phase::Control { .. } => CONTROL,
                    Phase::Data { .. } => DATA,
                    _ => return,
                };
                if socket != expected {
                    return;
                }
                if &command == b"NO" {
                    self.fail(cx, "the peer declined the invitation".into());
                    return;
                }
                cx.alive();
                if socket == CONTROL {
                    self.peer_ssrc = Some(ssrc);
                    let name = data
                        .get(16..)
                        .map(|n| {
                            let end = n.iter().position(|&b| b == 0).unwrap_or(n.len());
                            String::from_utf8_lossy(&n[..end]).into_owned()
                        })
                        .unwrap_or_default();
                    cx.state(json!({"session": {
                        "peer_name": name,
                        "peer_ssrc": ssrc,
                        "peer_address": self.control_addr().to_string(),
                        "local_port": self.local,
                    }}));
                    self.phase = Phase::Data { tries: 1 };
                    let packet = session_packet(b"IN", self.token, self.ssrc, Some(&self.name));
                    cx.udp_send(DATA, self.data_addr(), packet);
                    cx.set_timer(INVITE, INVITE_EVERY);
                } else {
                    self.opened(cx);
                }
            }
            b"BY" => {
                let ssrc = u32_at(data, 12);
                let joined = !matches!(self.phase, Phase::Idle | Phase::Listen);
                if joined && ssrc.is_some() && ssrc == self.peer_ssrc {
                    self.fail(cx, "the peer ended the session".into());
                }
            }
            b"IN" => {
                // The core does not take invitations: it is the initiator.
                if let Some(token) = u32_at(data, 8) {
                    let to = if socket == DATA {
                        self.data_addr()
                    } else {
                        self.control_addr()
                    };
                    cx.udp_send(socket, to, session_packet(b"NO", token, self.ssrc, None));
                }
            }
            b"CK" => {
                let (Some(ssrc), Some(&count)) = (u32_at(data, 4), data.get(8)) else {
                    return;
                };
                if self.peer_ssrc.is_some_and(|p| p != ssrc) || !self.is_open() {
                    return;
                }
                let stamps = [u64_at(data, 12), u64_at(data, 20), u64_at(data, 28)];
                let now = Rtp::stamp(cx);
                match count {
                    0 => {
                        let Some(t1) = stamps[0] else { return };
                        cx.alive();
                        self.last_sync = cx.now();
                        let reply = sync_packet(self.ssrc, 1, [t1, now, 0]);
                        cx.udp_send(DATA, self.data_addr(), reply);
                    }
                    1 => {
                        let (Some(t1), Some(t2)) = (stamps[0], stamps[1]) else {
                            return;
                        };
                        if Some(t1) != self.sync_sent {
                            return;
                        }
                        self.sync_sent = None;
                        self.misses = 0;
                        cx.alive();
                        cx.round_trip(now.saturating_sub(t1) / 10);
                        let reply = sync_packet(self.ssrc, 2, [t1, t2, now]);
                        cx.udp_send(DATA, self.data_addr(), reply);
                    }
                    // The end of an exchange the peer started: our answer's
                    // time came back, so its round trip is the latency.
                    2 => {
                        cx.alive();
                        if let Some(t2) = stamps[1] {
                            cx.round_trip(now.saturating_sub(t2) / 10);
                        }
                    }
                    _ => cx.alive(),
                }
            }
            b"RS" => {
                let (Some(ssrc), Some(seq)) = (u32_at(data, 4), u32_at(data, 8)) else {
                    return;
                };
                if self.peer_ssrc.is_some_and(|p| p != ssrc) || !self.is_open() {
                    return;
                }
                cx.alive();
                self.journal.acknowledge((seq >> 16) as u16);
            }
            // A bitrate limit (RL) asks a sender to keep under a rate; the
            // core sends only what it is asked to.
            _ => cx.alive(),
        }
    }

    /// Send messages; fails with `not_connected` unless the session is open.
    pub(crate) fn send(&mut self, cx: &mut Cx, messages: &[Message]) -> Result<(), CommandError> {
        if !self.is_open() {
            return Err(CommandError::NotConnected);
        }
        for (list, items) in pack(messages) {
            self.last_midi = self.seq;
            self.packet(cx, &list, items);
        }
        self.guards = Some(0);
        cx.set_timer(GUARD, GUARD_FIRST);
        Ok(())
    }

    /// Send one RTP packet: the MIDI list (empty for a guard packet) and
    /// the recovery journal, which then takes in what the list carried.
    fn packet(&mut self, cx: &mut Cx, list: &[u8], items: Vec<Item>) {
        let now = cx.now();
        let mut packet = vec![0x80, 0x80 | PAYLOAD_TYPE];
        packet.extend_from_slice(&(self.seq as u16).to_be_bytes());
        packet.extend_from_slice(&(Rtp::stamp(cx) as u32).to_be_bytes());
        packet.extend_from_slice(&self.ssrc.to_be_bytes());
        // J set: a journal follows the list.
        if list.len() <= 15 {
            packet.push(0x40 | list.len() as u8);
        } else {
            packet.push(0x80 | 0x40 | (list.len() >> 8) as u8);
            packet.push(list.len() as u8);
        }
        packet.extend_from_slice(list);
        packet.extend(self.journal.encode(self.seq, now));
        self.journal.record(self.seq, now, &items);
        self.seq += 1;
        cx.udp_send(DATA, self.data_addr(), packet);
    }

    /// Leave the session.
    pub(crate) fn stop(&mut self, cx: &mut Cx) {
        if !matches!(self.phase, Phase::Idle | Phase::Listen) {
            self.bye(cx);
        }
        self.phase = Phase::Idle;
    }

    /// Leave and join again now; hosting, end the session and wait for an
    /// invitation.
    pub(crate) fn reconnect(&mut self, cx: &mut Cx) {
        self.stop(cx);
        for key in [INVITE, SYNC, FEEDBACK, RETRY, GUARD, WATCH] {
            cx.cancel_timer(key);
        }
        self.retry = RETRY_MIN;
        cx.connection(Connection::Connecting);
        if self.hosting {
            self.peer_ssrc = None;
            self.open_sockets(cx);
            self.phase = Phase::Listen;
        } else {
            self.invite(cx);
        }
    }

    /// Hosting: an invitation on the control or the data port.
    fn invited(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        let (Some(token), Some(ssrc)) = (u32_at(data, 8), u32_at(data, 12)) else {
            return;
        };
        let refuse = |cx: &mut Cx, ssrc_ours: u32| {
            cx.udp_send(socket, from, session_packet(b"NO", token, ssrc_ours, None));
        };
        if self.only.is_some_and(|only| only != from.ip()) {
            refuse(cx, self.ssrc);
            return;
        }
        let joined = matches!(self.phase, Phase::Accepted | Phase::Open);
        let same_peer = joined && from.ip() == self.peer && Some(ssrc) == self.peer_ssrc;
        if socket == CONTROL {
            if joined && !same_peer {
                // Another host's: one is in the session already.
                refuse(cx, self.ssrc);
                return;
            }
            if self.phase == Phase::Open {
                // The peer invites again (it restarted): a new session.
                for key in [FEEDBACK, GUARD, WATCH] {
                    cx.cancel_timer(key);
                }
            }
            let name = data
                .get(16..)
                .map(|n| {
                    let end = n.iter().position(|&b| b == 0).unwrap_or(n.len());
                    String::from_utf8_lossy(&n[..end]).into_owned()
                })
                .unwrap_or_default();
            self.peer = from.ip();
            self.peer_control = from.port();
            self.peer_data = from.port().wrapping_add(1);
            self.token = token;
            self.peer_ssrc = Some(ssrc);
            self.phase = Phase::Accepted;
            cx.alive();
            cx.state(json!({"session": {
                "peer_name": name,
                "peer_ssrc": ssrc,
                "peer_address": from.to_string(),
                "local_port": self.local,
            }}));
            let ok = session_packet(b"OK", token, self.ssrc, Some(&self.name));
            cx.udp_send(CONTROL, from, ok);
        } else if socket == DATA {
            if !same_peer || token != self.token {
                if !same_peer {
                    refuse(cx, self.ssrc);
                }
                return;
            }
            self.peer_data = from.port();
            let ok = session_packet(b"OK", token, self.ssrc, Some(&self.name));
            cx.udp_send(DATA, from, ok);
            if self.phase != Phase::Open {
                self.opened(cx);
            }
        }
    }
}

/// The MIDI lists of the packets that carry `messages`, with what each
/// carries for the journal: whole messages, each after a delta time of 0
/// but the first, a SysEx too long for one packet in segments.
fn pack(messages: &[Message]) -> Vec<(Vec<u8>, Vec<Item>)> {
    let mut commands: Vec<(Vec<u8>, Item)> = Vec::new();
    for m in messages {
        match m {
            Message::SysEx(body) if body.len() + 2 > MAX_LIST - 1 => {
                let chunk = MAX_LIST - 3;
                let pieces: Vec<&[u8]> = body.chunks(chunk).collect();
                for (i, piece) in pieces.iter().enumerate() {
                    let last = i + 1 == pieces.len();
                    let head = if i == 0 { 0xF0 } else { 0xF7 };
                    let tail = if last { 0xF7 } else { 0xF0 };
                    let mut c = vec![head];
                    c.extend_from_slice(piece);
                    c.push(tail);
                    let item = Item::Segment {
                        data: piece.to_vec(),
                        first: i == 0,
                        last,
                    };
                    commands.push((c, item));
                }
            }
            other => commands.push((other.encode(), Item::Message(other.clone()))),
        }
    }
    let mut packets: Vec<(Vec<u8>, Vec<Item>)> = Vec::new();
    let mut list: Vec<u8> = Vec::new();
    let mut items: Vec<Item> = Vec::new();
    for (c, item) in commands {
        if !list.is_empty() && list.len() + 1 + c.len() > MAX_LIST {
            packets.push((std::mem::take(&mut list), std::mem::take(&mut items)));
        }
        if !list.is_empty() {
            list.push(0x00);
        }
        list.extend_from_slice(&c);
        items.push(item);
    }
    if !list.is_empty() {
        packets.push((list, items));
    }
    packets
}

/// Received RTP-MIDI packets to MIDI bytes, whole messages with their status
/// bytes.
#[derive(Debug, Default)]
struct Decoder {
    last_seq: Option<u16>,
    running: Option<u8>,
    /// A segmented SysEx being put together.
    sysex: Option<Vec<u8>>,
}

fn channel_data_len(status: u8) -> usize {
    match status & 0xF0 {
        0xC0 | 0xD0 => 1,
        _ => 2,
    }
}

impl Decoder {
    /// The MIDI a packet carries, preceded by what its journal repairs
    /// after a gap, and its sequence number. `None` for a malformed,
    /// duplicate or late packet.
    fn packet(&mut self, data: &[u8]) -> Option<(Vec<u8>, u16)> {
        let csrc = (data.first()? & 0x0F) as usize;
        let extension = data[0] & 0x10 != 0;
        let mut end = data.len();
        if data[0] & 0x20 != 0 {
            end = end.checked_sub(*data.last()? as usize)?;
        }
        let seq = u16::from_be_bytes([*data.get(2)?, *data.get(3)?]);
        let mut at = 12 + 4 * csrc;
        if extension {
            let words = u16::from_be_bytes([*data.get(at + 2)?, *data.get(at + 3)?]) as usize;
            at += 4 + 4 * words;
        }
        let payload = data.get(at..end)?;
        let gap = match self.last_seq {
            Some(last) => {
                let ahead = seq.wrapping_sub(last);
                if ahead == 0 || ahead >= 0x8000 {
                    return None;
                }
                ahead > 1
            }
            None => false,
        };
        let b0 = *payload.first()?;
        let (len, header) = if b0 & 0x80 != 0 {
            ((((b0 & 0x0F) as usize) << 8) | *payload.get(1)? as usize, 2)
        } else {
            ((b0 & 0x0F) as usize, 1)
        };
        let list = payload.get(header..header + len)?;
        self.last_seq = Some(seq);
        let mut out = Vec::new();
        if gap && b0 & 0x40 != 0 {
            recover(&payload[header + len..], &mut out);
            // A segmented SysEx cut by the loss cannot be completed.
            self.sysex = None;
        }
        self.list(list, b0 & 0x20 != 0, &mut out);
        Some((out, seq))
    }

    fn list(&mut self, list: &[u8], z: bool, out: &mut Vec<u8>) {
        let mut i = 0;
        let mut first = true;
        while i < list.len() {
            if !first || z {
                // A delta time: up to four bytes, the last without its high bit.
                let mut n = 0;
                while i < list.len() && n < 4 {
                    let b = list[i];
                    i += 1;
                    n += 1;
                    if b & 0x80 == 0 {
                        break;
                    }
                }
                if i >= list.len() {
                    return;
                }
            }
            first = false;
            let status = if list[i] >= 0x80 {
                i += 1;
                list[i - 1]
            } else {
                match self.running {
                    Some(s) => s,
                    None => return,
                }
            };
            match status {
                0x80..=0xEF => {
                    let n = channel_data_len(status);
                    let Some(d) = list.get(i..i + n) else { return };
                    self.running = Some(status);
                    out.push(status);
                    out.extend_from_slice(d);
                    i += n;
                }
                0xF0 | 0xF7 => {
                    self.running = None;
                    let Some(j) = list[i..]
                        .iter()
                        .position(|b| matches!(b, 0xF0 | 0xF7 | 0xF4 | 0xF5))
                        .map(|p| i + p)
                    else {
                        return;
                    };
                    let body = &list[i..j];
                    let tail = list[j];
                    i = j + 1;
                    match (status, tail) {
                        (0xF0, 0xF7 | 0xF5) => {
                            self.sysex = None;
                            out.push(0xF0);
                            out.extend_from_slice(body);
                            out.push(0xF7);
                        }
                        (0xF0, 0xF0) => self.sysex = Some(body.to_vec()),
                        (0xF7, 0xF0) => {
                            if let Some(s) = self.sysex.as_mut() {
                                s.extend_from_slice(body);
                            }
                        }
                        (0xF7, 0xF7 | 0xF5) => {
                            if let Some(mut s) = self.sysex.take() {
                                s.extend_from_slice(body);
                                out.push(0xF0);
                                out.extend_from_slice(&s);
                                out.push(0xF7);
                            }
                        }
                        _ => self.sysex = None,
                    }
                }
                0xF4 | 0xF5 => {
                    // An undefined command, terminated by F7 (RFC 6295 §3.2).
                    self.running = None;
                    match list[i..].iter().position(|&b| b == 0xF7) {
                        Some(p) => i += p + 1,
                        None => return,
                    }
                }
                0xF1..=0xF6 => {
                    self.running = None;
                    let n = match status {
                        0xF2 => 2,
                        0xF1 | 0xF3 => 1,
                        _ => 0,
                    };
                    let Some(d) = list.get(i..i + n) else { return };
                    out.push(status);
                    out.extend_from_slice(d);
                    i += n;
                }
                _ => out.push(status),
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn recover_for_tests(journal: &[u8], out: &mut Vec<u8>) {
    recover(journal, out)
}

/// Replay what a recovery journal records of each channel (RFC 6295 §5 and
/// Appendix A) as MIDI: chapters P, C (value tool), W, N, T and A.
fn recover(journal: &[u8], out: &mut Vec<u8>) {
    let Some(&b0) = journal.first() else { return };
    if journal.len() < 3 {
        return;
    }
    let length = |at: usize| -> Option<usize> {
        Some((((*journal.get(at)? & 0x03) as usize) << 8) | *journal.get(at + 1)? as usize)
    };
    let mut at = 3;
    if b0 & 0x40 != 0 {
        // The system journal, skipped whole.
        let Some(len) = length(at) else { return };
        at += len.max(2);
    }
    if b0 & 0x20 == 0 {
        return;
    }
    for _ in 0..=(b0 & 0x0F) {
        let (Some(&head), Some(len), Some(&toc)) =
            (journal.get(at), length(at), journal.get(at + 2))
        else {
            return;
        };
        let channel = (head >> 3) & 0x0F;
        let end = (at + len.max(3)).min(journal.len());
        if let Some(chapters) = journal.get(at + 3..end) {
            recover_channel(channel, toc, chapters, out);
        }
        at += len.max(3);
    }
}

fn recover_channel(channel: u8, toc: u8, c: &[u8], out: &mut Vec<u8>) {
    let get = |j: usize| c.get(j).map(|b| b & 0x7F);
    let flag = |j: usize| c.get(j).map(|b| b & 0x80 != 0);
    let mut j = 0;
    let mut emit = |bytes: &[u8]| out.extend_from_slice(bytes);
    // Chapter P: program, with the bank select before it.
    if toc & 0x80 != 0 {
        let (Some(program), Some(bank), Some(msb), Some(lsb)) =
            (get(j), flag(j + 1), get(j + 1), get(j + 2))
        else {
            return;
        };
        if bank {
            emit(&[0xB0 | channel, 0x00, msb, 0xB0 | channel, 0x20, lsb]);
        }
        emit(&[0xC0 | channel, program]);
        j += 3;
    }
    // Chapter C: controller logs; the value tool's values are replayed.
    if toc & 0x40 != 0 {
        let Some(n) = get(j) else { return };
        j += 1;
        for _ in 0..=n {
            let (Some(number), Some(alt), Some(value)) = (get(j), flag(j + 1), get(j + 1)) else {
                return;
            };
            if !alt {
                emit(&[0xB0 | channel, number, value]);
            }
            j += 2;
        }
    }
    // Chapter M: skipped by its LENGTH.
    if toc & 0x20 != 0 {
        let (Some(&hi), Some(&lo)) = (c.get(j), c.get(j + 1)) else {
            return;
        };
        j += ((((hi & 0x03) as usize) << 8) | lo as usize).max(2);
    }
    // Chapter W: the pitch wheel.
    if toc & 0x10 != 0 {
        let (Some(first), Some(second)) = (get(j), get(j + 1)) else {
            return;
        };
        emit(&[0xE0 | channel, first, second]);
        j += 2;
    }
    // Chapter N: notes off from the OFFBITS, then notes on from the logs.
    if toc & 0x08 != 0 {
        let (Some(len), Some(&bits)) = (get(j), c.get(j + 1)) else {
            return;
        };
        let (low, high) = (bits >> 4, bits & 0x0F);
        let logs = if len == 127 && low == 15 && high == 0 {
            128
        } else {
            len as usize
        };
        j += 2;
        let notes_at = j;
        j += 2 * logs;
        if low <= high {
            for k in 0..=(high - low) as usize {
                let Some(&octet) = c.get(j + k) else { return };
                for bit in 0..8 {
                    if octet & (0x80 >> bit) != 0 {
                        let note = 8 * (low as usize + k) + bit;
                        emit(&[0x80 | channel, note as u8, 0x40]);
                    }
                }
            }
            j += (high - low) as usize + 1;
        }
        for n in 0..logs {
            let at = notes_at + 2 * n;
            let (Some(note), Some(play), Some(velocity)) = (get(at), flag(at + 1), get(at + 1))
            else {
                return;
            };
            if play && velocity > 0 {
                emit(&[0x90 | channel, note, velocity]);
            }
        }
    }
    // Chapter E: skipped.
    if toc & 0x04 != 0 {
        let Some(n) = get(j) else { return };
        j += 1 + 2 * (n as usize + 1);
    }
    // Chapter T: channel pressure.
    if toc & 0x02 != 0 {
        let Some(pressure) = get(j) else { return };
        emit(&[0xD0 | channel, pressure]);
        j += 1;
    }
    // Chapter A: poly pressure.
    if toc & 0x01 != 0 {
        let Some(n) = get(j) else { return };
        j += 1;
        for _ in 0..=n {
            let (Some(note), Some(pressure)) = (get(j), get(j + 1)) else {
                return;
            };
            emit(&[0xA0 | channel, note, pressure]);
            j += 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;

    fn peer() -> SocketAddr {
        "192.0.2.10:5004".parse().unwrap()
    }

    fn sent(actions: &[Action]) -> Vec<(Key, SocketAddr, Vec<u8>)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { socket, to, data } => Some((*socket, *to, data.clone())),
                _ => None,
            })
            .collect()
    }

    /// A session joined: invitation on both ports accepted.
    fn joined() -> (Rtp, u32) {
        let mut rtp = Rtp::new(peer(), "Meros", Some(6000), 42);
        let mut cx = Cx::new(0);
        rtp.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: CONTROL,
            bind: Bind::Any {
                address: None,
                port: 6000
            }
        }));
        let s = sent(&a);
        assert_eq!(s.len(), 1);
        let (socket, to, packet) = &s[0];
        assert_eq!((*socket, *to), (CONTROL, peer()));
        assert_eq!(&packet[..8], &[0xFF, 0xFF, b'I', b'N', 0, 0, 0, 2]);
        assert!(packet.ends_with(b"Meros\0"));
        let token = u32_at(packet, 8).unwrap();
        let ok = |token: u32| session_packet(b"OK", token, 0xABCD, Some("Studio"));
        let mut cx = Cx::new(10);
        rtp.datagram(&mut cx, CONTROL, peer(), &ok(token));
        let s = sent(&cx.take());
        assert_eq!(s[0].0, DATA);
        assert_eq!(s[0].1, "192.0.2.10:5005".parse().unwrap());
        let mut cx = Cx::new(20);
        rtp.datagram(
            &mut cx,
            DATA,
            "192.0.2.10:5005".parse().unwrap(),
            &ok(token),
        );
        let a = cx.take();
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // Clock synchronisation starts at once, count 0 with our time.
        let s = sent(&a);
        assert_eq!(s[0].2, sync_packet(rtp.ssrc, 0, [200, 0, 0]));
        (rtp, token)
    }

    #[test]
    fn a_session_is_joined_synchronised_and_left() {
        let (mut rtp, token) = joined();
        // The peer answers count 1; the core finishes with count 2 and
        // reports the round trip.
        let mut cx = Cx::new(25);
        rtp.datagram(
            &mut cx,
            DATA,
            "192.0.2.10:5005".parse().unwrap(),
            &sync_packet(0xABCD, 1, [200, 99_999, 0]),
        );
        let a = cx.take();
        assert!(a.contains(&Action::RoundTrip(5)));
        assert_eq!(sent(&a)[0].2, sync_packet(rtp.ssrc, 2, [200, 99_999, 250]));
        // The peer's own synchronisation is answered.
        let mut cx = Cx::new(30);
        rtp.datagram(
            &mut cx,
            DATA,
            "192.0.2.10:5005".parse().unwrap(),
            &sync_packet(0xABCD, 0, [7, 0, 0]),
        );
        assert_eq!(sent(&cx.take())[0].2, sync_packet(rtp.ssrc, 1, [7, 300, 0]));
        // Leaving sends BY with the token.
        let mut cx = Cx::new(40);
        rtp.stop(&mut cx);
        assert_eq!(
            sent(&cx.take())[0].2,
            session_packet(b"BY", token, rtp.ssrc, None)
        );
    }

    #[test]
    fn three_unanswered_synchronisations_end_the_session() {
        let (mut rtp, _) = joined();
        for t in [1, 2] {
            let mut cx = Cx::new(t * 1500);
            assert!(rtp.timer(&mut cx, SYNC));
            assert!(!cx.take().iter().any(|a| matches!(a, Action::Connection(_))));
        }
        let mut cx = Cx::new(4500);
        rtp.timer(&mut cx, SYNC);
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(
            a,
            Action::Connection(Connection::Disconnected { reason }) if reason.contains("clock")
        )));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: 1000
        }));
        assert_eq!(&sent(&a)[0].2[2..4], b"BY");
    }

    #[test]
    fn a_refusal_and_silence_are_retried_with_backoff() {
        let mut rtp = Rtp::new(peer(), "Meros", None, 7);
        let mut cx = Cx::new(0);
        rtp.start(&mut cx);
        let token = u32_at(&sent(&cx.take())[0].2, 8).unwrap();
        let mut cx = Cx::new(5);
        rtp.datagram(
            &mut cx,
            CONTROL,
            peer(),
            &session_packet(b"NO", token, 1, None),
        );
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(
            a,
            Action::Connection(Connection::Disconnected { reason }) if reason.contains("declined")
        )));
        let mut cx = Cx::new(1005);
        rtp.timer(&mut cx, RETRY);
        assert_eq!(&sent(&cx.take())[0].2[2..4], b"IN");
        for t in 1..INVITE_TRIES {
            let mut cx = Cx::new(1005 + t as u64 * 1000);
            rtp.timer(&mut cx, INVITE);
            assert_eq!(sent(&cx.take()).len(), 1);
        }
        let mut cx = Cx::new(20_000);
        rtp.timer(&mut cx, INVITE);
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(
            a,
            Action::Connection(Connection::Disconnected { reason }) if reason.contains("no answer")
        )));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: 2000
        }));
        // A bind failure moves to other ports.
        let mut cx = Cx::new(20_001);
        let before = rtp.local;
        rtp.socket_error(&mut cx, CONTROL, "in use");
        assert_ne!(rtp.local, before);
    }

    #[test]
    fn midi_goes_out_in_rtp_packets_and_long_sysex_in_segments() {
        let (mut rtp, _) = joined();
        let seq = rtp.seq;
        let mut cx = Cx::new(100);
        rtp.send(
            &mut cx,
            &[
                Message::NoteOn {
                    channel: 0,
                    note: 60,
                    velocity: 100,
                },
                Message::ControlChange {
                    channel: 1,
                    controller: 7,
                    value: 90,
                },
            ],
        )
        .unwrap();
        let s = sent(&cx.take());
        let p = &s[0].2;
        assert_eq!(s[0].0, DATA);
        assert_eq!(&p[..2], &[0x80, 0xE1]);
        assert_eq!(u16::from_be_bytes([p[2], p[3]]), seq as u16);
        assert_eq!(u32_at(p, 4), Some(1000));
        // J set, then the list, then an empty journal (nothing sent before)
        // whose checkpoint is this packet.
        assert_eq!(
            &p[12..21],
            &[0x40 | 7, 0x90, 60, 100, 0x00, 0xB1, 7, 90, 0x80]
        );
        assert_eq!(&p[21..], &(seq as u16).to_be_bytes());
        // A SysEx of 2500 bytes: three packets, F0..F0, F7..F0, F7..F7.
        let mut cx = Cx::new(101);
        rtp.send(&mut cx, &[Message::SysEx(vec![0x11; 2500])])
            .unwrap();
        let s = sent(&cx.take());
        assert_eq!(s.len(), 3);
        let lists: Vec<&[u8]> = s
            .iter()
            .map(|(_, _, p)| {
                let len = ((p[12] as usize & 0x0F) << 8) | p[13] as usize;
                &p[14..14 + len]
            })
            .collect();
        assert_eq!((lists[0][0], *lists[0].last().unwrap()), (0xF0, 0xF0));
        assert_eq!((lists[1][0], *lists[1].last().unwrap()), (0xF7, 0xF0));
        assert_eq!((lists[2][0], *lists[2].last().unwrap()), (0xF7, 0xF7));
        // And it reads back whole.
        let mut d = Decoder::default();
        let mut out = Vec::new();
        for (_, _, p) in &s {
            out.extend(d.packet(p).unwrap().0);
        }
        assert_eq!(out.len(), 2502);
    }

    #[test]
    fn the_journal_follows_feedback_and_guards_the_last_packet() {
        let (mut rtp, _) = joined();
        let first = rtp.seq;
        let mut cx = Cx::new(100);
        rtp.send(
            &mut cx,
            &[Message::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            }],
        )
        .unwrap();
        assert!(cx.take().contains(&Action::SetTimer {
            key: GUARD,
            after: 100
        }));
        // A guard packet: an empty list, and the journal coding the note.
        let mut cx = Cx::new(200);
        assert!(rtp.timer(&mut cx, GUARD));
        let a = cx.take();
        let p = &sent(&a)[0].2;
        assert_eq!(p[12], 0x40);
        let journal = &p[13..];
        assert_eq!(&journal[1..3], &(first as u16).to_be_bytes());
        assert_eq!(journal[0] & 0x2F, 0x20, "one channel journal");
        assert_eq!(journal[5], 0x08, "chapter N");
        assert!(a.contains(&Action::SetTimer {
            key: GUARD,
            after: 100
        }));
        // Unacknowledged, the next waits twice as long.
        let mut cx = Cx::new(300);
        rtp.timer(&mut cx, GUARD);
        assert!(cx.take().contains(&Action::SetTimer {
            key: GUARD,
            after: 200
        }));
        // The peer has the packets: no more guards, and the journal starts
        // after the last one it has.
        let mut rs = vec![0xFF, 0xFF, b'R', b'S'];
        rs.extend_from_slice(&0xABCDu32.to_be_bytes());
        rs.extend_from_slice(&(((first + 2) as u16 as u32) << 16).to_be_bytes());
        let mut cx = Cx::new(350);
        rtp.datagram(&mut cx, CONTROL, peer(), &rs);
        let mut cx = Cx::new(500);
        rtp.timer(&mut cx, GUARD);
        assert!(sent(&cx.take()).is_empty());
        let mut cx = Cx::new(600);
        rtp.send(&mut cx, &[Message::Clock]).unwrap();
        let p = &sent(&cx.take())[0].2;
        let checkpoint = ((first + 3) as u16).to_be_bytes();
        assert_eq!(&p[12..], &[0x41, 0xF8, 0x80, checkpoint[0], checkpoint[1]]);
    }

    #[test]
    fn a_lost_packet_is_repaired_from_the_cores_journal() {
        let (mut rtp, _) = joined();
        let mut packets = Vec::new();
        for (t, m) in [
            (
                100,
                Message::NoteOn {
                    channel: 3,
                    note: 64,
                    velocity: 90,
                },
            ),
            (
                200,
                Message::NoteOff {
                    channel: 3,
                    note: 64,
                    velocity: 64,
                },
            ),
            (
                300,
                Message::ControlChange {
                    channel: 3,
                    controller: 7,
                    value: 20,
                },
            ),
            (400, Message::Clock),
        ] {
            let mut cx = Cx::new(t);
            rtp.send(&mut cx, &[m]).unwrap();
            packets.push(sent(&cx.take())[0].2.clone());
        }
        let mut d = Decoder::default();
        assert_eq!(d.packet(&packets[0]).unwrap().0, [0x93, 64, 90]);
        // The note off and the volume are lost; the clock's packet repairs
        // both before its own command.
        let (midi, _) = d.packet(&packets[3]).unwrap();
        assert_eq!(midi, [0xB3, 7, 20, 0x83, 64, 0x40, 0xF8]);
    }

    fn joiner() -> SocketAddr {
        "192.0.2.20:6100".parse().unwrap()
    }

    fn joiner_data() -> SocketAddr {
        "192.0.2.20:6101".parse().unwrap()
    }

    /// A hosted session joined by `joiner()`.
    fn hosted() -> Rtp {
        let mut rtp = Rtp::host(5004, None, "Meros", 9);
        let mut cx = Cx::new(0);
        rtp.start(&mut cx);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: DATA,
            bind: Bind::Any {
                address: None,
                port: 5005
            }
        }));
        assert!(sent(&a).is_empty(), "a host invites no one");
        let invite = session_packet(b"IN", 77, 0x1234, Some("Desk"));
        let mut cx = Cx::new(10);
        rtp.datagram(&mut cx, CONTROL, joiner(), &invite);
        let a = cx.take();
        assert_eq!(
            sent(&a),
            [(
                CONTROL,
                joiner(),
                session_packet(b"OK", 77, rtp.ssrc, Some("Meros"))
            )]
        );
        assert!(!a.contains(&Action::Connection(Connection::Connected)));
        let mut cx = Cx::new(20);
        rtp.datagram(&mut cx, DATA, joiner_data(), &invite);
        let a = cx.take();
        assert_eq!(
            sent(&a),
            [(
                DATA,
                joiner_data(),
                session_packet(b"OK", 77, rtp.ssrc, Some("Meros"))
            )]
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::SetTimer {
            key: WATCH,
            after: WATCH_EVERY
        }));
        rtp
    }

    #[test]
    fn a_hosted_session_is_joined_synchronised_and_left() {
        let mut rtp = hosted();
        // The initiator's synchronisation is answered, and the end of its
        // exchange gives the round trip.
        let mut cx = Cx::new(30);
        rtp.datagram(
            &mut cx,
            DATA,
            joiner_data(),
            &sync_packet(0x1234, 0, [5, 0, 0]),
        );
        assert_eq!(
            sent(&cx.take()),
            [(DATA, joiner_data(), sync_packet(rtp.ssrc, 1, [5, 300, 0]))]
        );
        let mut cx = Cx::new(34);
        rtp.datagram(
            &mut cx,
            DATA,
            joiner_data(),
            &sync_packet(0x1234, 2, [5, 300, 9]),
        );
        assert!(cx.take().contains(&Action::RoundTrip(4)));
        // MIDI goes to the port the data invitation came from.
        let mut cx = Cx::new(40);
        rtp.send(&mut cx, &[Message::Start]).unwrap();
        assert_eq!(sent(&cx.take())[0].1, joiner_data());
        // Another host is refused while one is in the session.
        let other: SocketAddr = "192.0.2.30:7000".parse().unwrap();
        let mut cx = Cx::new(50);
        rtp.datagram(
            &mut cx,
            CONTROL,
            other,
            &session_packet(b"IN", 5, 0x99, Some("Other")),
        );
        assert_eq!(
            sent(&cx.take()),
            [(CONTROL, other, session_packet(b"NO", 5, rtp.ssrc, None))]
        );
        // The peer leaves: disconnected, waiting for the next invitation.
        let mut cx = Cx::new(60);
        rtp.datagram(
            &mut cx,
            CONTROL,
            joiner(),
            &session_packet(b"BY", 77, 0x1234, None),
        );
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(
            a,
            Action::Connection(Connection::Disconnected { reason })
                if reason.contains("ended") && reason.contains("invitation")
        )));
        assert!(!a
            .iter()
            .any(|a| matches!(a, Action::SetTimer { key: RETRY, .. })));
        // The other host may join now.
        let mut cx = Cx::new(70);
        rtp.datagram(
            &mut cx,
            CONTROL,
            other,
            &session_packet(b"IN", 5, 0x99, Some("Other")),
        );
        assert_eq!(&sent(&cx.take())[0].2[2..4], b"OK");
    }

    #[test]
    fn a_hosted_session_without_synchronisation_is_ended() {
        let mut rtp = hosted();
        let mut cx = Cx::new(WATCH_EVERY);
        rtp.timer(&mut cx, WATCH);
        assert!(cx.take().contains(&Action::SetTimer {
            key: WATCH,
            after: WATCH_EVERY
        }));
        let mut cx = Cx::new(20 + PEER_SYNC_TIMEOUT);
        rtp.timer(&mut cx, WATCH);
        let a = cx.take();
        assert_eq!(&sent(&a)[0].2[2..4], b"BY");
        assert!(a.iter().any(|a| matches!(
            a,
            Action::Connection(Connection::Disconnected { reason }) if reason.contains("synchronised")
        )));
    }

    #[test]
    fn a_host_given_only_takes_that_hosts_invitation() {
        let only: IpAddr = "192.0.2.20".parse().unwrap();
        let mut rtp = Rtp::host(5004, Some(only), "Meros", 3);
        let mut cx = Cx::new(0);
        rtp.start(&mut cx);
        cx.take();
        let stranger: SocketAddr = "192.0.2.31:6000".parse().unwrap();
        let mut cx = Cx::new(1);
        rtp.datagram(
            &mut cx,
            CONTROL,
            stranger,
            &session_packet(b"IN", 1, 2, None),
        );
        assert_eq!(&sent(&cx.take())[0].2[2..4], b"NO");
        let mut cx = Cx::new(2);
        rtp.datagram(
            &mut cx,
            CONTROL,
            joiner(),
            &session_packet(b"IN", 1, 2, None),
        );
        assert_eq!(&sent(&cx.take())[0].2[2..4], b"OK");
    }

    fn rtp_packet(seq: u16, section: &[u8]) -> Vec<u8> {
        let mut p = vec![0x80, 0xE1];
        p.extend_from_slice(&seq.to_be_bytes());
        p.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0xAB, 0xCD]);
        p.extend_from_slice(section);
        p
    }

    #[test]
    fn received_lists_decode_running_status_delta_times_and_segments() {
        let mut d = Decoder::default();
        // Z set: a delta time before the first command; running status for
        // the second note; a two-byte delta time; a clock.
        let list = [0x05, 0x90, 60, 100, 0x00, 61, 101, 0x81, 0x00, 0xF8];
        let mut section = vec![0x20 | list.len() as u8];
        section.extend_from_slice(&list);
        let (midi, _) = d.packet(&rtp_packet(1, &section)).unwrap();
        assert_eq!(midi, [0x90, 60, 100, 0x90, 61, 101, 0xF8]);
        // A long list (B set), a SysEx cancelled, then a dropped-F7 SysEx.
        let list = [
            0xF0, 1, 0xF0, 0x00, 0xF7, 0xF4, 0x00, 0xF0, 2, 3, 0xF5, 0x00, 0xC1, 9,
        ];
        let mut section = vec![0x80, list.len() as u8];
        section.extend_from_slice(&list);
        let (midi, _) = d.packet(&rtp_packet(2, &section)).unwrap();
        assert_eq!(midi, [0xF0, 2, 3, 0xF7, 0xC1, 9]);
        // A duplicate is dropped.
        assert!(d.packet(&rtp_packet(2, &section)).is_none());
    }

    #[test]
    fn a_gap_is_repaired_from_the_recovery_journal() {
        let mut d = Decoder::default();
        d.packet(&rtp_packet(10, &[0x00])).unwrap();
        // Packet 12 (11 lost): journal with one channel journal for channel
        // 2 holding chapters P (program 5, bank 1/2), C (CC 7 = 80, and a
        // toggle-tool log skipped), W, N (note 64 on, note 9 off), T and A.
        let chapters = [
            5,
            0x80 | 1,
            2, // P
            1,
            7,
            80,
            64,
            0x80 | 3, // C: two logs
            0x00,
            0x40, // W
            1,
            0x11,
            64,
            0x80 | 90,
            0b0100_0000, // N: LEN 1, LOW 1 HIGH 1, log, offbits (note 9)
            33,          // T
            0,
            70,
            12, // A: one log
        ];
        let mut channel = vec![2 << 3, 0, 0b1101_1011];
        channel.extend_from_slice(&chapters);
        channel[1] = channel.len() as u8;
        let mut journal = vec![0x20, 0, 10];
        journal.extend_from_slice(&channel);
        let mut section = vec![0x40 | 1, 0xF8];
        section.extend_from_slice(&journal);
        let (midi, seq) = d.packet(&rtp_packet(12, &section)).unwrap();
        assert_eq!(seq, 12);
        assert_eq!(
            midi,
            [
                0xB2, 0x00, 1, 0xB2, 0x20, 2, 0xC2, 5, // P
                0xB2, 7, 80, // C
                0xE2, 0x00, 0x40, // W
                0x82, 9, 0x40, 0x92, 64, 90, // N
                0xD2, 33, // T
                0xA2, 70, 12,   // A
                0xF8  // the packet's own command
            ]
        );
    }
}
