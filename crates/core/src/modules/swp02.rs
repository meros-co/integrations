//! Probel SW-P-02, the General Switcher Communication Protocol, on a serial
//! port or over TCP.
//!
//! Wire format from Pro-Bel's SW-P-88 Issue 1 (2001), §1-§4, section
//! numbers below being that document's.
//!
//! - A message is FFh (start of message), a command byte, the command's
//!   fixed number of data bytes, and a checksum: the 7-bit two's complement
//!   of the sum of the command and data bytes, its top bit 0 (§3.1). Every
//!   data byte is 7 bits, so FFh only ever starts a message; a message whose
//!   checksum is wrong is dropped and the reader looks for the next FFh.
//! - Destinations and sources are 10 bits: a multiplier byte holds the
//!   destination DIV 128 in bits 4-6 and the source DIV 128 in bits 0-2, and
//!   the next bytes the numbers MOD 128; bit 3 flags a bad source or
//!   crosspoint updates disabled, depending on the router (§3.2.1).
//!   Numbers are 1-based in commands and state, 0-based on the wire.
//! - INTERROGATE (01) is answered by TALLY (03), CONNECT (02) by CONNECTED
//!   (04), which the router sends on all its ports (§3.2.2-§3.2.4), CONNECT
//!   ON GO (05) by its acknowledge (12) and GO (06) by GO DONE ACKNOWLEDGE
//!   (13), which a GO that set crosspoints sends on all ports, with no
//!   CONNECTED for them (§3.2.5-§3.2.8). There is no NAK: the controller
//!   assumes success until a reply fails to come (§3.3.1). One request is
//!   outstanding at a time; its reply is matched by command and destination.
//! - A TALLY with source 1023 says the destination is out of range (§3.2.3).
//! - Kept current by interrogating the first `destinations` destinations
//!   every `poll_ms`, as Pro-Bel controllers interrogate in the background
//!   (§3.3.1), and by the CONNECTED messages the router broadcasts; after a
//!   GO that set crosspoints they are interrogated again at once.
//! - The `link` setting chooses a serial port of the host (RS-422/RS-485 or
//!   RS-232, §2) or TCP. A port that fails, or a connection that drops, is
//!   opened again after 1 s, doubling to 30 s. The port being open says
//!   nothing of the router: the device is connected once a message arrives,
//!   and disconnected when three requests in a row go unanswered.
//!
//! Opened for commands only (`monitor` false), nothing is interrogated
//! beyond what commands ask, except one INTERROGATE of destination 1 after
//! 10 s without traffic, the only way the protocol shows the router is there.
//! What the router broadcasts still reaches state. Every answered request
//! reports its round trip as the device's latency.

use std::collections::VecDeque;

use serde_json::{json, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const SOCKET: Key = "swp02";
const REPLY_TIMER: Key = "reply";
const POLL_TIMER: Key = "poll";
const PROBE_TIMER: Key = "probe";
const RETRY_TIMER: Key = "retry";

const SOM: u8 = 0xFF;
const INTERROGATE: u8 = 1;
const CONNECT: u8 = 2;
const TALLY: u8 = 3;
const CONNECTED: u8 = 4;
const CONNECT_ON_GO: u8 = 5;
const GO: u8 = 6;
const CONNECT_ON_GO_ACK: u8 = 12;
const GO_DONE_ACK: u8 = 13;

/// How long a request waits for its reply. The protocol sets none (§3.3.1);
/// a router answers within milliseconds.
const REPLY_TIMEOUT: Millis = 1_000;
/// Requests unanswered in a row before the router is reported gone.
const MISSES: u32 = 3;
const PROBE_EVERY: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// Wire source 1023 in a TALLY: the destination is out of range (§3.2.3).
const OUT_OF_RANGE: u16 = 1023;

/// The data bytes after each command byte (§3.2).
fn data_len(command: u8) -> Option<usize> {
    match command {
        INTERROGATE => Some(2),
        CONNECT | TALLY | CONNECTED | CONNECT_ON_GO | CONNECT_ON_GO_ACK => Some(3),
        GO | GO_DONE_ACK => Some(1),
        _ => None,
    }
}

/// The checksum of a command and its data (§3.1).
fn checksum(body: &[u8]) -> u8 {
    let sum = body.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    sum.wrapping_neg() & 0x7F
}

/// Frame one message, command byte first.
pub(crate) fn frame(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 2);
    out.push(SOM);
    out.extend_from_slice(body);
    out.push(checksum(body));
    out
}

/// The multiplier byte (§3.2.1).
fn multiplier(destination: u16, source: u16) -> u8 {
    (((destination >> 7) & 7) << 4) as u8 | ((source >> 7) & 7) as u8
}

/// Splits the byte stream into messages (command byte first).
#[derive(Debug, Default)]
struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            let Some(start) = self.buf.iter().position(|&b| b == SOM) else {
                self.buf.clear();
                break;
            };
            self.buf.drain(..start);
            let Some(&command) = self.buf.get(1) else {
                break;
            };
            let Some(len) = data_len(command) else {
                // Not a command this module knows: its length is unknown,
                // so look for the next start of message.
                self.buf.remove(0);
                continue;
            };
            if self.buf.len() < len + 3 {
                // A start of message inside means the frame was cut short.
                if let Some(next) = self.buf[1..].iter().position(|&b| b == SOM) {
                    self.buf.drain(..=next);
                    continue;
                }
                break;
            }
            let body = &self.buf[1..len + 2];
            if checksum(body) == self.buf[len + 2] {
                out.push(body.to_vec());
                self.buf.drain(..len + 3);
            } else {
                self.buf.remove(0);
            }
        }
        out
    }
}

/// A crosspoint as a message carries it: wire numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Crosspoint {
    destination: u16,
    source: u16,
    flag: bool,
}

fn crosspoint(body: &[u8]) -> Crosspoint {
    let m = body[1];
    Crosspoint {
        destination: (u16::from((m >> 4) & 7) << 7) | u16::from(body[2] & 0x7F),
        source: (u16::from(m & 7) << 7) | u16::from(body[3] & 0x7F),
        flag: m & 0x08 != 0,
    }
}

/// What a request waits for.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Expect {
    /// A message with this command byte about this wire destination.
    Crosspoint(u8, u16),
    /// GO DONE ACKNOWLEDGE.
    GoDone,
}

#[derive(Debug)]
struct Job {
    id: Option<CommandId>,
    message: Vec<u8>,
    expect: Expect,
}

pub(crate) struct Swp02 {
    link: Link,
    monitor: bool,
    destinations: u16,
    poll_ms: Millis,
    open: bool,
    /// The link failed and waits to be opened again.
    closed: bool,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    deframer: Deframer,
    queue: VecDeque<Job>,
    in_flight: Option<(Job, Millis)>,
}

impl Swp02 {
    pub(crate) fn new(ctx: OpenContext) -> Result<Swp02, String> {
        let s = &ctx.settings;
        let tcp = s.get("link").and_then(Value::as_str) == Some("tcp");
        if tcp && ctx.port.is_none() {
            return Err("SW-P-02 over TCP has no standard port: give the router's port".into());
        }
        // Serial unless the settings choose TCP: the protocol's own link.
        let mut settings = s.clone();
        if !tcp {
            settings.insert("link".into(), "serial".into());
        }
        let link = Link::from_settings(
            &OpenContext {
                settings,
                ..ctx.clone()
            },
            0,
        )?;
        let num = |key: &str, default: u64| s.get(key).and_then(Value::as_u64).unwrap_or(default);
        Ok(Swp02 {
            link,
            monitor: ctx.monitor,
            destinations: num("destinations", 16).clamp(1, 1024) as u16,
            poll_ms: num("poll_ms", 1_000).max(100),
            open: false,
            closed: false,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            deframer: Deframer::default(),
            queue: VecDeque::new(),
            in_flight: None,
        })
    }

    fn interrogate(destination: u16) -> Job {
        Job {
            id: None,
            message: vec![
                INTERROGATE,
                multiplier(destination, 0),
                (destination & 0x7F) as u8,
            ],
            expect: Expect::Crosspoint(TALLY, destination),
        }
    }

    /// Queue an interrogate of every polled destination not queued yet.
    fn sweep(&mut self) {
        for d in 0..self.destinations {
            let job = Swp02::interrogate(d);
            let queued = self.queue.iter().any(|j| j.message == job.message)
                || self
                    .in_flight
                    .as_ref()
                    .is_some_and(|(j, _)| j.message == job.message);
            if !queued {
                self.queue.push_back(job);
            }
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || !self.open {
            return;
        }
        let Some(job) = self.queue.pop_front() else {
            return;
        };
        cx.tcp_send(SOCKET, frame(&job.message));
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        cx.set_timer(PROBE_TIMER, PROBE_EVERY);
        self.in_flight = Some((job, cx.now()));
    }

    fn enqueue(&mut self, cx: &mut Cx, job: Job) {
        // Commands go ahead of the background interrogates.
        let at = self.queue.iter().position(|j| j.id.is_none());
        match (job.id, at) {
            (Some(_), Some(at)) => self.queue.insert(at, job),
            _ => self.queue.push_back(job),
        }
        self.pump(cx);
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.misses = 0;
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
    }

    fn apply(&self, cx: &mut Cx, x: Crosspoint) {
        if x.source == OUT_OF_RANGE {
            return;
        }
        let key = (x.destination + 1).to_string();
        cx.state(json!({"destinations": {key: {"source": x.source + 1, "flag": x.flag}}}));
    }

    fn received(&mut self, cx: &mut Cx, body: Vec<u8>) {
        self.heard(cx);
        let command = body[0];
        let mut answered = None;
        match command {
            TALLY | CONNECTED | CONNECT_ON_GO_ACK => {
                let x = crosspoint(&body);
                if command != CONNECT_ON_GO_ACK {
                    self.apply(cx, x);
                }
                if let Some((job, _)) = &self.in_flight {
                    if job.expect == Expect::Crosspoint(command, x.destination) {
                        answered = Some(match (command, x.source) {
                            (TALLY, OUT_OF_RANGE) => Err(CommandError::DeviceError {
                                code: None,
                                message: "the destination is out of range".into(),
                            }),
                            (TALLY, source) => Ok(Outcome::Value {
                                value: json!(source + 1),
                            }),
                            _ => Ok(Outcome::Ack),
                        });
                    }
                }
            }
            GO_DONE_ACK => {
                let result = match body[1] {
                    0 => "set",
                    1 => "cleared",
                    2 => "none",
                    _ => "unknown",
                };
                cx.state(json!({"salvo": {"last_go": result}}));
                // A GO sends no CONNECTED for what it set (§3.2.6).
                if body[1] == 0 && self.monitor {
                    self.sweep();
                }
                if matches!(&self.in_flight, Some((job, _)) if job.expect == Expect::GoDone) {
                    answered = Some(Ok(Outcome::Value {
                        value: json!(result),
                    }));
                }
            }
            _ => {}
        }
        if let Some(result) = answered {
            let (job, sent_at) = self.in_flight.take().expect("in flight");
            cx.cancel_timer(REPLY_TIMER);
            cx.round_trip(cx.now().saturating_sub(sent_at));
            if let Some(id) = job.id {
                cx.complete(id, result);
            }
        }
        self.pump(cx);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.open = false;
        self.closed = true;
        self.reachable = false;
        self.deframer = Deframer::default();
        for job in self
            .in_flight
            .take()
            .map(|(j, _)| j)
            .into_iter()
            .chain(self.queue.drain(..))
        {
            if let Some(id) = job.id {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        for key in [REPLY_TIMER, POLL_TIMER, PROBE_TIMER] {
            cx.cancel_timer(key);
        }
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn open_link(&mut self, cx: &mut Cx) {
        self.closed = false;
        cx.connection(Connection::Connecting);
        self.link.open(cx, SOCKET);
    }
}

/// A 1-based number parameter, as a wire number.
fn wire(params: &Params, name: &str) -> Option<u16> {
    params
        .get(name)
        .and_then(Value::as_u64)
        .filter(|n| (1..=1024).contains(n))
        .map(|n| (n - 1) as u16)
}

impl Module for Swp02 {
    fn start(&mut self, cx: &mut Cx) {
        self.open_link(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        // While the link opens, commands wait for it.
        if self.closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let destination = wire(params, "destination");
        let source = wire(params, "source");
        let job = match (name, destination, source) {
            ("interrogate", Some(d), _) => Job {
                id: Some(id),
                ..Swp02::interrogate(d)
            },
            ("connect" | "salvo_add", Some(d), Some(s)) => {
                let (command, reply) = if name == "connect" {
                    (CONNECT, CONNECTED)
                } else {
                    (CONNECT_ON_GO, CONNECT_ON_GO_ACK)
                };
                Job {
                    id: Some(id),
                    message: vec![
                        command,
                        multiplier(d, s),
                        (d & 0x7F) as u8,
                        (s & 0x7F) as u8,
                    ],
                    expect: Expect::Crosspoint(reply, d),
                }
            }
            ("salvo_fire" | "salvo_clear", _, _) => Job {
                id: Some(id),
                message: vec![GO, u8::from(name == "salvo_clear")],
                expect: Expect::GoDone,
            },
            ("refresh", _, _) => {
                self.sweep();
                self.pump(cx);
                cx.complete(id, Ok(Outcome::Ack));
                return;
            }
            ("interrogate" | "connect" | "salvo_add", _, _) => {
                cx.complete(
                    id,
                    Err(CommandError::InvalidParams {
                        message: "destination and source are 1 to 1024".into(),
                    }),
                );
                return;
            }
            (other, _, _) => {
                cx.complete(
                    id,
                    Err(CommandError::UnknownCommand {
                        command: other.into(),
                    }),
                );
                return;
            }
        };
        self.enqueue(cx, job);
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.open = true;
                self.retry_after = RETRY_MIN;
                self.deframer = Deframer::default();
                if self.monitor {
                    self.sweep();
                    cx.set_timer(POLL_TIMER, self.poll_ms);
                } else {
                    // Nothing to read: one interrogate shows the router is
                    // there, and the idle check repeats it.
                    self.queue.push_back(Swp02::interrogate(0));
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
                self.pump(cx);
            }
            TcpInput::Data(bytes) => {
                for body in self.deframer.feed(&bytes) {
                    self.received(cx, body);
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => self.open_link(cx),
            REPLY_TIMER => {
                if let Some((job, _)) = self.in_flight.take() {
                    if let Some(id) = job.id {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES && self.reachable {
                        self.reachable = false;
                        cx.log(Level::Warning, "the router answered none of three requests");
                        cx.connection(Connection::Disconnected {
                            reason: "no reply from the router".into(),
                        });
                    }
                }
                self.pump(cx);
            }
            POLL_TIMER => {
                if self.open && self.monitor {
                    self.sweep();
                    self.pump(cx);
                    cx.set_timer(POLL_TIMER, self.poll_ms);
                }
            }
            PROBE_TIMER => {
                if self.open {
                    if self.in_flight.is_none() && self.queue.is_empty() {
                        self.queue.push_back(Swp02::interrogate(0));
                        self.pump(cx);
                    }
                    cx.set_timer(PROBE_TIMER, PROBE_EVERY);
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult, FlowControl, Parity, SerialPort};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn module(monitor: bool, settings: Value) -> Swp02 {
        let spec = Catalog::source_tree().devices["probel-swp02"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        Swp02::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            host_name: None,
            port: None,
            model: "generic".into(),
            channels: None,
            settings,
            monitor,
        })
        .unwrap()
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    fn state(actions: &[Action]) -> Value {
        let mut merged = json!({});
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(&mut merged, p);
            }
        }
        merged
    }

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn opened(m: &mut Swp02) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    fn feed(m: &mut Swp02, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut Swp02, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    #[test]
    fn frames_carry_the_seven_bit_checksum() {
        // CONNECT destination 3 to source 6 (wire 2 and 5): 02+00+02+05 = 9,
        // whose 7-bit two's complement is 77h.
        assert_eq!(frame(&[2, 0, 2, 5]), [0xFF, 2, 0, 2, 5, 0x77]);
        // Destination 300 (wire 299 = 2*128 + 43) from source 1000 (wire 999
        // = 7*128 + 103): multiplier 27h.
        let body = [CONNECT, multiplier(299, 999), 43, 103];
        assert_eq!(body[1], 0x27);
        let x = crosspoint(&body);
        assert_eq!((x.destination, x.source, x.flag), (299, 999, false));
    }

    #[test]
    fn the_reader_resynchronises_on_bad_or_cut_frames() {
        let mut d = Deframer::default();
        let good = frame(&[TALLY, 0, 2, 5]);
        let mut bytes = vec![0x12, 0x34];
        bytes.extend(&good[..3]);
        bytes.extend([0xFF, 99, 1]);
        bytes.extend(&good);
        let mut bad = good.clone();
        bad[5] ^= 1;
        bytes.extend(&bad);
        assert_eq!(d.feed(&bytes), [vec![TALLY, 0, 2, 5]]);
        // Split anywhere, a message is still read once whole.
        let mut d = Deframer::default();
        assert!(d.feed(&good[..2]).is_empty());
        assert_eq!(d.feed(&good[2..]), [vec![TALLY, 0, 2, 5]]);
    }

    #[test]
    fn the_serial_link_is_opened_with_the_protocol_defaults() {
        let mut m = module(true, json!({"serial_port": "/dev/ttyUSB0"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let ports: Vec<SerialPort> = cx
            .take()
            .into_iter()
            .filter_map(|a| match a {
                Action::SerialOpen { port, .. } => Some(port),
                _ => None,
            })
            .collect();
        assert_eq!(
            ports,
            [SerialPort {
                path: "/dev/ttyUSB0".into(),
                baud_rate: 38400,
                data_bits: 8,
                parity: Parity::Even,
                stop_bits: 1,
                flow_control: FlowControl::None,
            }]
        );
    }

    #[test]
    fn monitored_it_interrogates_each_destination_and_keeps_tallies() {
        let mut m = module(true, json!({"serial_port": "COM1", "destinations": 2}));
        let a = opened(&mut m);
        assert_eq!(sent(&a), [frame(&[INTERROGATE, 0, 0])]);
        let a = feed(&mut m, 5, &frame(&[TALLY, 0, 0, 9]));
        assert_eq!(sent(&a), [frame(&[INTERROGATE, 0, 1])]);
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(5)));
        assert_eq!(
            state(&a),
            json!({"destinations": {"1": {"source": 10, "flag": false}}})
        );
        // A route made elsewhere is broadcast as CONNECTED.
        let a = feed(&mut m, 6, &frame(&[CONNECTED, 0x09, 1, 3]));
        assert_eq!(
            state(&a),
            json!({"destinations": {"2": {"source": 132, "flag": true}}})
        );
    }

    #[test]
    fn commands_wait_for_their_own_reply() {
        let mut m = module(false, json!({"serial_port": "COM1"}));
        let a = opened(&mut m);
        // Commands only: one interrogate of destination 1 as the liveness check.
        assert_eq!(sent(&a), [frame(&[INTERROGATE, 0, 0])]);
        feed(&mut m, 1, &frame(&[TALLY, 0, 0, 0]));

        let a = run(&mut m, 1, "connect", json!({"destination": 3, "source": 6}));
        assert_eq!(sent(&a), [frame(&[CONNECT, 0, 2, 5])]);
        // CONNECTED for another destination is not the answer.
        let a = feed(&mut m, 2, &frame(&[CONNECTED, 0, 7, 1]));
        assert_eq!(completed(&a, 1), None);
        let a = feed(&mut m, 3, &frame(&[CONNECTED, 0, 2, 5]));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));

        run(&mut m, 2, "interrogate", json!({"destination": 1025 - 1}));
        let a = feed(&mut m, 4, &frame(&[TALLY, 0x77, 0x7F, 0x7F]));
        assert!(matches!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError { .. }))
        ));

        let a = run(
            &mut m,
            3,
            "salvo_add",
            json!({"destination": 2, "source": 2}),
        );
        assert_eq!(sent(&a), [frame(&[CONNECT_ON_GO, 0, 1, 1])]);
        let a = feed(&mut m, 5, &frame(&[CONNECT_ON_GO_ACK, 0, 1, 1]));
        assert_eq!(completed(&a, 3), Some(Ok(Outcome::Ack)));

        let a = run(&mut m, 4, "salvo_fire", json!({}));
        assert_eq!(sent(&a), [frame(&[GO, 0])]);
        let a = feed(&mut m, 6, &frame(&[GO_DONE_ACK, 0]));
        assert_eq!(
            completed(&a, 4),
            Some(Ok(Outcome::Value {
                value: json!("set")
            }))
        );
        // Commands only: the GO's routes are not read again.
        assert!(sent(&a).is_empty());

        let a = run(&mut m, 5, "salvo_clear", json!({}));
        assert_eq!(sent(&a), [frame(&[GO, 1])]);
    }

    #[test]
    fn unanswered_requests_report_the_router_gone_and_a_failed_port_is_reopened() {
        let mut m = module(false, json!({"serial_port": "COM1"}));
        opened(&mut m);
        feed(&mut m, 1, &frame(&[TALLY, 0, 0, 0]));
        let mut gone = Vec::new();
        for id in 1..=3 {
            run(&mut m, id, "interrogate", json!({"destination": 1}));
            let mut cx = Cx::new(0);
            m.timer(&mut cx, REPLY_TIMER);
            gone = cx.take();
            assert_eq!(completed(&gone, id), Some(Err(CommandError::Timeout)));
        }
        assert!(gone
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
        // An unplugged adapter closes the port; it is opened again.
        let mut cx = Cx::new(0);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "unplugged".into(),
            },
        );
        assert!(cx.take().contains(&Action::SetTimer {
            key: RETRY_TIMER,
            after: RETRY_MIN
        }));
        let mut cx = Cx::new(0);
        m.timer(&mut cx, RETRY_TIMER);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::SerialOpen { .. })));
    }

    #[test]
    fn tcp_needs_a_port() {
        let spec = Catalog::source_tree().devices["probel-swp02"].clone();
        let settings =
            validate(&spec.settings, json!({"link": "tcp"}).as_object().unwrap()).unwrap();
        let ctx = OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 2)),
            host_name: None,
            port: None,
            model: "generic".into(),
            channels: None,
            settings,
            monitor: true,
        };
        assert!(Swp02::new(ctx.clone()).is_err());
        let mut m = Swp02::new(OpenContext {
            port: Some(2002),
            ..ctx
        })
        .unwrap();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx.take().iter().any(|a| matches!(
            a,
            Action::TcpOpen { to, .. } if to.port() == 2002
        )));
    }
}
