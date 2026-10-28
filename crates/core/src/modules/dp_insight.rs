//! Digital Projection INSIGHT 4K Laser projectors: the binary protocol on
//! TCP 43728, or the same messages on the RS-232 port (38400 8N1).
//!
//! Written from Digital Projection's Protocol Guide 115-482F (Rev F, October
//! 2016), "INSIGHT 4K Laser Series" (pages 61-90); see the spec's sources.
//!
//! - A request is three command bytes, a two-byte length (MSB first), the
//!   command data and a checksum, the least significant byte of the sum of
//!   every byte before it (p.62). The projector ignores a request whose
//!   checksum is wrong.
//! - A reply repeats the command bytes with 20h added to the first, then a
//!   status byte (C0h in every example), a one-byte length, the data and the
//!   checksum. The control system waits for each reply before sending the
//!   next request (p.62).
//! - Nothing is pushed: the running status (power, mode, light) is read every
//!   `poll_ms`, the douser, light power, light status and title every minute.
//!   Every command that changes one of them is followed by its read.
//!
//! Opened for commands only (`monitor` false), nothing is read beyond what
//! commands ask and their read-back, except the running status every 10 s
//! while idle, the only sign the projector is there. Every answered request
//! reports its round trip.

use std::collections::VecDeque;

use serde_json::{json, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const SOCKET: Key = "insight";
const REPLY_TIMER: Key = "reply";
const FAST_TIMER: Key = "poll-fast";
const SLOW_TIMER: Key = "poll-slow";
const PROBE_TIMER: Key = "probe";
const RETRY_TIMER: Key = "retry";

pub(crate) const DEFAULT_PORT: u16 = 43728;

const REPLY_TIMEOUT: Millis = 3_000;
const MISSES: u32 = 3;
const PROBE_EVERY: Millis = 10_000;
const SLOW_EVERY: Millis = 60_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

/// The least significant byte of the sum of `bytes` (p.62).
fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

/// A request: the three command bytes, the data, framed with its length and
/// checksum.
pub(crate) fn frame(command: [u8; 3], data: &[u8]) -> Vec<u8> {
    let len = data.len() as u16;
    let mut out = Vec::with_capacity(data.len() + 6);
    out.extend_from_slice(&command);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(data);
    out.push(checksum(&out));
    out
}

/// A reply read off the stream.
#[derive(Debug, Clone, PartialEq)]
struct Reply {
    command: [u8; 3],
    status: u8,
    data: Vec<u8>,
}

#[derive(Debug, Default)]
struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Reply> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if self.buf.len() < 6 {
                break;
            }
            // Replies to the documented requests start 20h, 22h or 23h.
            if self.buf[0] & 0xF0 != 0x20 {
                self.buf.remove(0);
                continue;
            }
            let size = 6 + usize::from(self.buf[4]);
            if self.buf.len() < size {
                break;
            }
            if checksum(&self.buf[..size - 1]) != self.buf[size - 1] {
                self.buf.remove(0);
                continue;
            }
            out.push(Reply {
                command: [self.buf[0] & 0x0F, self.buf[1], self.buf[2]],
                status: self.buf[3],
                data: self.buf[5..size - 1].to_vec(),
            });
            self.buf.drain(..size);
        }
        out
    }
}

/// What a request reads, so its reply can be put into state.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Read {
    Running,
    Douser,
    LightPower,
    LightStatus,
    Title,
}

impl Read {
    fn request(self) -> Request {
        let (command, data): ([u8; 3], &[u8]) = match self {
            Read::Running => ([0x00, 0x85, 0x00], &[0x01]),
            Read::Douser => ([0x00, 0x85, 0x00], &[0x03]),
            Read::LightPower => ([0x03, 0x05, 0x00], &[0xC1, 0x00, 0x00]),
            Read::LightStatus => ([0x03, 0x2F, 0x00], &[0x1E]),
            // The guide prints this request starting 02, but its checksum
            // (88h) and the reply's 20h are those of 00, the other reads' byte.
            Read::Title => ([0x00, 0x85, 0x00], &[0x02]),
        };
        Request {
            command,
            data: data.to_vec(),
            read: Some(self),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Request {
    command: [u8; 3],
    data: Vec<u8>,
    read: Option<Read>,
}

impl Request {
    fn bytes(&self) -> Vec<u8> {
        frame(self.command, &self.data)
    }
}

#[derive(Debug)]
struct Job {
    id: Option<CommandId>,
    request: Request,
    /// Reads queued once the request is answered: the read-back.
    then: Vec<Read>,
}

const MODES: &[(u8, &str)] = &[
    (0x00, "standby"),
    (0x01, "power_on_protect"),
    (0x02, "ignition_first_attempt"),
    (0x03, "power_on_running"),
    (0x04, "running_light_on"),
    (0x05, "cooling"),
    (0x07, "reset_wait"),
    (0x08, "fan_stop_error"),
    (0x09, "light_ignition_retry"),
    (0x0A, "light_error"),
    (0x0C, "running_light_off"),
];

/// State from a read's reply data (the bytes after the length).
fn decode(read: Read, data: &[u8]) -> Option<(Value, Value)> {
    let at = |i: usize| data.get(i).copied();
    let le = |i: usize| Some(u16::from_le_bytes([at(i)?, at(i + 1)?]));
    let flag = |i: usize| at(i).map(|b| b != 0);
    match read {
        Read::Running => {
            let mode = at(5)?;
            let mode_name = MODES
                .iter()
                .find(|(c, _)| *c == mode)
                .map_or_else(|| format!("unknown_{mode:02x}"), |(_, n)| (*n).to_string());
            let patch = json!({
                "external_control": flag(1)?,
                "power": flag(2)?,
                "light_cooling": flag(3)?,
                "power_processing": flag(4)?,
                "mode": mode_name,
                "light": flag(9)?,
                "light_processing": flag(10)?,
            });
            Some((patch.clone(), patch))
        }
        Read::Douser => {
            let closed = match at(0)? {
                0x81 => true,
                0x00 => false,
                _ => return None,
            };
            Some((json!({ "douser_closed": closed }), json!(closed)))
        }
        Read::LightPower => {
            let level = at(7)?;
            Some((json!({ "light_power": level }), json!(level)))
        }
        Read::LightStatus => {
            let status = json!({
                "hours": le(1)?,
                "warning_hours": le(3)?,
                "remaining_percent": at(5)?,
                "strikes": le(6)?,
            });
            Some((json!({ "light_status": status }), status))
        }
        Read::Title => {
            let title = u16::from(at(1)?) + 1;
            Some((json!({ "title": title }), json!(title)))
        }
    }
}

fn bad(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn int(params: &Params, name: &str, min: i64, max: i64) -> Result<i64, CommandError> {
    let v = params
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| bad(format!("'{name}' is required")))?;
    if !(min..=max).contains(&v) {
        return Err(bad(format!("'{name}' must be {min} to {max}")));
    }
    Ok(v)
}

fn text<'a>(params: &'a Params, name: &str) -> Result<&'a str, CommandError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

/// A lens memory request (p.86-88): `op` 01 store, 02 recall, 00 delete.
fn lens_memory(op: u8, memory: u8) -> Request {
    let mut data = vec![0xEE, memory, op];
    data.extend([0u8; 13]);
    data.extend(b"memory ");
    data.push(b'0' + memory);
    data.extend([0u8; 22]);
    data.extend([0xFFu8; 20]);
    Request {
        command: [0x03, 0xB1, 0x00],
        data,
        read: None,
    }
}

/// What a command sends, and the reads that follow it.
fn plan(name: &str, params: &Params) -> Result<(Request, Vec<Read>), CommandError> {
    let req = |command: [u8; 3], data: &[u8]| Request {
        command,
        data: data.to_vec(),
        read: None,
    };
    let lens = |axis: u8, dir: u8| req([0x02, 0x18, 0x00], &[axis, dir]);
    Ok(match name {
        "power_on" => (req([0x02, 0x00, 0x00], &[]), vec![Read::Running]),
        "power_off" => (req([0x02, 0x01, 0x00], &[]), vec![Read::Running]),
        "light_on" => (req([0x03, 0x2F, 0x00], &[0x12, 0x01]), vec![Read::Running]),
        "light_off" => (req([0x03, 0x2F, 0x00], &[0x12, 0x02]), vec![Read::Running]),
        "set_light_power" => {
            let level = int(params, "percent", 30, 100)? as u8;
            (
                req([0x03, 0x10, 0x00], &[0xC1, 0xFF, 0x00, level, 0x00]),
                vec![Read::LightPower],
            )
        }
        "close_douser" => (req([0x02, 0x16, 0x00], &[]), vec![Read::Douser]),
        "open_douser" => (req([0x02, 0x17, 0x00], &[]), vec![Read::Douser]),
        "lens_shift" => {
            let (axis, dir) = match text(params, "direction")? {
                "up" => (0x03, 0x7F),
                "down" => (0x03, 0x81),
                "left" => (0x02, 0x81),
                "right" => (0x02, 0x7F),
                other => return Err(bad(format!("'{other}' is not a direction"))),
            };
            (lens(axis, dir), vec![])
        }
        "lens_shift_stop" => {
            let axis = match text(params, "axis")? {
                "vertical" => 0x03,
                "horizontal" => 0x02,
                other => return Err(bad(format!("'{other}' is not an axis"))),
            };
            (lens(axis, 0x00), vec![])
        }
        "zoom" | "focus" => {
            let axis = if name == "zoom" { 0x00 } else { 0x01 };
            let dir = match text(params, "direction")? {
                "in" => 0x7F,
                "out" => 0x81,
                "stop" => 0x00,
                other => return Err(bad(format!("'{other}' is not a direction"))),
            };
            (lens(axis, dir), vec![])
        }
        "store_lens_memory" | "recall_lens_memory" | "delete_lens_memory" => {
            let memory = int(params, "memory", 0, 9)? as u8;
            let op = match name {
                "store_lens_memory" => 0x01,
                "recall_lens_memory" => 0x02,
                _ => 0x00,
            };
            (lens_memory(op, memory), vec![])
        }
        "set_title" => {
            let title = int(params, "title", 1, 100)? as u8;
            (
                req([0x02, 0x03, 0x00], &[0x06, title - 1]),
                vec![Read::Title],
            )
        }
        "get_running_status" => (Read::Running.request(), vec![]),
        "get_douser" => (Read::Douser.request(), vec![]),
        "get_light_power" => (Read::LightPower.request(), vec![]),
        "get_light_status" => (Read::LightStatus.request(), vec![]),
        "get_title" => (Read::Title.request(), vec![]),
        other => {
            return Err(CommandError::UnknownCommand {
                command: other.into(),
            })
        }
    })
}

pub(crate) struct DpInsight {
    link: Link,
    monitor: bool,
    poll_ms: Millis,
    open: bool,
    closed: bool,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    deframer: Deframer,
    queue: VecDeque<Job>,
    in_flight: Option<(Job, Millis)>,
}

impl DpInsight {
    pub(crate) fn new(ctx: OpenContext) -> Result<DpInsight, String> {
        let link = Link::from_settings(&ctx, DEFAULT_PORT)?;
        let poll_ms = ctx
            .settings
            .get("poll_ms")
            .and_then(Value::as_u64)
            .unwrap_or(5_000)
            .max(1_000);
        Ok(DpInsight {
            link,
            monitor: ctx.monitor,
            poll_ms,
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

    fn queue_reads(&mut self, reads: &[Read]) {
        for read in reads {
            let pending = |j: &Job| j.id.is_none() && j.request.read == Some(*read);
            let queued = self.queue.iter().any(pending)
                || self.in_flight.as_ref().is_some_and(|(j, _)| pending(j));
            if !queued {
                self.queue.push_back(Job {
                    id: None,
                    request: read.request(),
                    then: Vec::new(),
                });
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
        cx.tcp_send(SOCKET, job.request.bytes());
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        cx.set_timer(PROBE_TIMER, PROBE_EVERY);
        self.in_flight = Some((job, cx.now()));
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.misses = 0;
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
    }

    fn received(&mut self, cx: &mut Cx, reply: Reply) {
        let answers = self
            .in_flight
            .as_ref()
            .is_some_and(|(j, _)| j.request.command == reply.command);
        if !answers {
            return;
        }
        let (job, sent_at) = self.in_flight.take().unwrap();
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.heard(cx);
        let value = match job.request.read {
            Some(read) => match decode(read, &reply.data) {
                Some((patch, value)) => {
                    cx.state(patch);
                    Some(value)
                }
                None => None,
            },
            None => None,
        };
        if let Some(id) = job.id {
            let outcome = match (job.request.read, value) {
                (Some(_), Some(v)) => Outcome::Value { value: v },
                (Some(_), None) => Outcome::Value { value: Value::Null },
                (None, _) => Outcome::Ack,
            };
            cx.complete(id, Ok(outcome));
        }
        if reply.status != 0xC0 {
            cx.log(
                Level::Debug,
                format!("reply status {:02x} (C0h in every example)", reply.status),
            );
        }
        for read in job.then.into_iter().rev() {
            self.queue.push_front(Job {
                id: None,
                request: read.request(),
                then: Vec::new(),
            });
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
        for key in [REPLY_TIMER, FAST_TIMER, SLOW_TIMER, PROBE_TIMER] {
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

const FAST_READS: &[Read] = &[Read::Running];
const SLOW_READS: &[Read] = &[
    Read::Douser,
    Read::LightPower,
    Read::LightStatus,
    Read::Title,
];

impl Module for DpInsight {
    fn start(&mut self, cx: &mut Cx) {
        self.open_link(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match plan(name, params) {
            Ok((request, then)) => {
                let job = Job {
                    id: Some(id),
                    request,
                    then,
                };
                // Commands go ahead of background reads.
                match self.queue.iter().position(|j| j.id.is_none()) {
                    Some(at) => self.queue.insert(at, job),
                    None => self.queue.push_back(job),
                }
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.open = true;
                self.retry_after = RETRY_MIN;
                self.deframer = Deframer::default();
                self.queue_reads(FAST_READS);
                if self.monitor {
                    self.queue_reads(SLOW_READS);
                    cx.set_timer(FAST_TIMER, self.poll_ms);
                    cx.set_timer(SLOW_TIMER, SLOW_EVERY);
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
                self.pump(cx);
            }
            TcpInput::Data(bytes) => {
                for reply in self.deframer.feed(&bytes) {
                    self.received(cx, reply);
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
                        cx.log(
                            Level::Warning,
                            "the projector answered none of three requests",
                        );
                        cx.connection(Connection::Disconnected {
                            reason: "no reply from the projector".into(),
                        });
                    }
                }
                self.pump(cx);
            }
            FAST_TIMER if self.open && self.monitor => {
                self.queue_reads(FAST_READS);
                self.pump(cx);
                cx.set_timer(FAST_TIMER, self.poll_ms);
            }
            SLOW_TIMER if self.open && self.monitor => {
                self.queue_reads(SLOW_READS);
                self.pump(cx);
                cx.set_timer(SLOW_TIMER, SLOW_EVERY);
            }
            PROBE_TIMER if self.open => {
                if self.in_flight.is_none() && self.queue.is_empty() {
                    self.queue_reads(FAST_READS);
                    self.pump(cx);
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
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
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    fn module(monitor: bool, settings: Value) -> DpInsight {
        let spec = Catalog::source_tree().devices["digitalprojection-insight-laser"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        DpInsight::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 8)),
            host_name: None,
            port: None,
            model: "insight-4k-laser".into(),
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

    fn opened(m: &mut DpInsight) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    fn feed(m: &mut DpInsight, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut DpInsight, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    const RUNNING_REPLY: &str = "20 85 00 c0 10 00 00 01 00 00 0c 00 00 00 00 00 ff 00 00 00 00 81";

    #[test]
    fn requests_match_the_guides_examples() {
        let p = |name: &str, v: Value| plan(name, v.as_object().unwrap()).unwrap().0.bytes();
        assert_eq!(p("power_on", json!({})), hex("02 00 00 00 00 02"));
        assert_eq!(p("power_off", json!({})), hex("02 01 00 00 00 03"));
        assert_eq!(p("light_on", json!({})), hex("03 2f 00 00 02 12 01 47"));
        assert_eq!(p("light_off", json!({})), hex("03 2f 00 00 02 12 02 48"));
        assert_eq!(
            p("set_light_power", json!({"percent": 30})),
            hex("03 10 00 00 05 c1 ff 00 1e 00 f6")
        );
        assert_eq!(
            p("set_light_power", json!({"percent": 100})),
            hex("03 10 00 00 05 c1 ff 00 64 00 3c")
        );
        assert_eq!(
            p("get_light_power", json!({})),
            hex("03 05 00 00 03 c1 00 00 cc")
        );
        assert_eq!(
            p("get_light_status", json!({})),
            hex("03 2f 00 00 01 1e 51")
        );
        assert_eq!(p("close_douser", json!({})), hex("02 16 00 00 00 18"));
        assert_eq!(p("open_douser", json!({})), hex("02 17 00 00 00 19"));
        assert_eq!(p("get_douser", json!({})), hex("00 85 00 00 01 03 89"));
        assert_eq!(
            p("get_running_status", json!({})),
            hex("00 85 00 00 01 01 87")
        );
        assert_eq!(
            p("lens_shift", json!({"direction": "up"})),
            hex("02 18 00 00 02 03 7f 9e")
        );
        assert_eq!(
            p("lens_shift", json!({"direction": "down"})),
            hex("02 18 00 00 02 03 81 a0")
        );
        assert_eq!(
            p("lens_shift_stop", json!({"axis": "vertical"})),
            hex("02 18 00 00 02 03 00 1f")
        );
        assert_eq!(
            p("lens_shift", json!({"direction": "left"})),
            hex("02 18 00 00 02 02 81 9f")
        );
        assert_eq!(
            p("lens_shift", json!({"direction": "right"})),
            hex("02 18 00 00 02 02 7f 9d")
        );
        assert_eq!(
            p("lens_shift_stop", json!({"axis": "horizontal"})),
            hex("02 18 00 00 02 02 00 1e")
        );
        assert_eq!(
            p("zoom", json!({"direction": "in"})),
            hex("02 18 00 00 02 00 7f 9b")
        );
        assert_eq!(
            p("zoom", json!({"direction": "out"})),
            hex("02 18 00 00 02 00 81 9d")
        );
        assert_eq!(
            p("zoom", json!({"direction": "stop"})),
            hex("02 18 00 00 02 00 00 1c")
        );
        assert_eq!(
            p("focus", json!({"direction": "in"})),
            hex("02 18 00 00 02 01 7f 9c")
        );
        assert_eq!(
            p("focus", json!({"direction": "out"})),
            hex("02 18 00 00 02 01 81 9e")
        );
        assert_eq!(
            p("focus", json!({"direction": "stop"})),
            hex("02 18 00 00 02 01 00 1d")
        );
        assert_eq!(
            p("set_title", json!({"title": 1})),
            hex("02 03 00 00 02 06 00 0d")
        );
        assert_eq!(p("get_title", json!({})), hex("00 85 00 00 01 02 88"));
        // Lens memory 0: store, recall and delete end in BA, BB and B9 (p.86-88).
        for (name, sum) in [
            ("store_lens_memory", 0xBA),
            ("recall_lens_memory", 0xBB),
            ("delete_lens_memory", 0xB9),
        ] {
            let bytes = p(name, json!({"memory": 0}));
            assert_eq!(bytes.len(), 72, "{name}");
            assert_eq!(&bytes[..5], &[0x03, 0xB1, 0x00, 0x00, 0x42]);
            assert_eq!(&bytes[21..29], b"memory 0");
            assert_eq!(*bytes.last().unwrap(), sum, "{name}");
        }
    }

    #[test]
    fn replies_are_read_as_the_guide_shows_them() {
        let mut d = Deframer::default();
        // Noise before a reply is skipped.
        let mut bytes = vec![0x55];
        bytes.extend(hex(
            "23 2f 00 c0 0f 1e 42 00 20 4e 64 47 00 00 00 20 4e 64 00 00 6c",
        ));
        let replies = d.feed(&bytes);
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].command, [0x03, 0x2F, 0x00]);
        assert_eq!(
            decode(Read::LightStatus, &replies[0].data).unwrap().0,
            json!({"light_status": {"hours": 66, "warning_hours": 20000, "remaining_percent": 100, "strikes": 71}})
        );
        let r = d.feed(&hex(
            "23 05 00 c0 10 02 64 00 1e 00 00 00 63 00 07 00 01 00 00 ff ff e5",
        ));
        assert_eq!(
            decode(Read::LightPower, &r[0].data).unwrap().0,
            json!({"light_power": 99})
        );
        let r = d.feed(&hex(
            "20 85 00 c0 10 81 00 00 00 00 00 00 00 00 00 ff ff ff ff ff ff f0",
        ));
        assert_eq!(
            decode(Read::Douser, &r[0].data).unwrap().0,
            json!({"douser_closed": true})
        );
        let r = d.feed(&hex(
            "20 85 00 c0 10 00 0e 04 0d 02 00 00 00 ff 00 00 00 00 00 00 00 95",
        ));
        assert_eq!(
            decode(Read::Title, &r[0].data).unwrap().0,
            json!({"title": 15})
        );
        let r = d.feed(&hex(RUNNING_REPLY));
        assert_eq!(
            decode(Read::Running, &r[0].data).unwrap().0,
            json!({"external_control": false, "power": true, "light_cooling": false,
                   "power_processing": false, "mode": "running_light_off", "light": false,
                   "light_processing": false})
        );
        // A wrong checksum is not a reply.
        assert!(d.feed(&hex("22 00 00 c0 00 e3")).is_empty());
    }

    #[test]
    fn monitored_it_reads_state_and_reads_back_a_change() {
        let mut m = module(true, json!({}));
        let a = opened(&mut m);
        assert_eq!(sent(&a), [hex("00 85 00 00 01 01 87")]);
        let a = feed(&mut m, 4, &hex(RUNNING_REPLY));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(4)));
        assert_eq!(state(&a)["mode"], json!("running_light_off"));
        // The douser read follows.
        assert_eq!(sent(&a), [hex("00 85 00 00 01 03 89")]);
        feed(
            &mut m,
            5,
            &hex("20 85 00 c0 10 81 00 00 00 00 00 00 00 00 00 ff ff ff ff ff ff f0"),
        );
        let a = run(&mut m, 1, "light_on", json!({}));
        // A read is in flight: the command waits for it.
        assert!(sent(&a).is_empty());
        let a = feed(
            &mut m,
            6,
            &hex("23 05 00 c0 10 02 64 00 1e 00 00 00 63 00 07 00 01 00 00 ff ff e5"),
        );
        assert_eq!(sent(&a), [hex("03 2f 00 00 02 12 01 47")]);
        let a = feed(&mut m, 7, &hex("23 2f 00 c0 02 12 00 26"));
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a), [hex("00 85 00 00 01 01 87")]);
    }

    #[test]
    fn opened_for_commands_only_it_reads_only_the_running_status() {
        let mut m = module(false, json!({}));
        let a = opened(&mut m);
        assert_eq!(sent(&a), [hex("00 85 00 00 01 01 87")]);
        let a = feed(&mut m, 2, &hex(RUNNING_REPLY));
        assert!(sent(&a).is_empty());
        let a = run(&mut m, 1, "get_title", json!({}));
        assert_eq!(sent(&a), [hex("00 85 00 00 01 02 88")]);
        let a = feed(
            &mut m,
            3,
            &hex("20 85 00 c0 10 00 0e 04 0d 02 00 00 00 ff 00 00 00 00 00 00 00 95"),
        );
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(15) }))
        );
        assert_eq!(state(&a), json!({"title": 15}));
    }

    #[test]
    fn the_serial_link_uses_38400() {
        let mut m = module(true, json!({"link": "serial", "serial_port": "COM5"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::SerialOpen { port, .. } if port.baud_rate == 38400)));
    }

    #[test]
    fn every_command_of_the_spec_is_planned() {
        let spec = Catalog::source_tree().devices["digitalprojection-insight-laser"].clone();
        for (name, command) in &spec.commands {
            let mut params = Params::new();
            for (p, decl) in &command.params {
                let value = match (&decl.values, decl.kind) {
                    (Some(values), _) => json!(values[0]),
                    _ => json!(decl.min.unwrap_or(0.0) as i64),
                };
                params.insert(p.clone(), value);
            }
            let planned = plan(name, &params);
            assert!(planned.is_ok(), "{name}: {planned:?}");
        }
    }

    #[test]
    fn unanswered_requests_report_the_projector_gone() {
        let mut m = module(false, json!({}));
        opened(&mut m);
        feed(&mut m, 1, &hex(RUNNING_REPLY));
        let mut last = Vec::new();
        for id in 1..=3 {
            run(&mut m, id, "get_douser", json!({}));
            let mut cx = Cx::new(0);
            m.timer(&mut cx, REPLY_TIMER);
            last = cx.take();
            assert_eq!(completed(&last, id), Some(Err(CommandError::Timeout)));
        }
        assert!(last
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. }))));
    }
}
