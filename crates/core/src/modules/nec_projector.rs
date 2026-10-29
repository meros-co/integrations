//! NEC and Sharp NEC projectors over NEC's binary control commands, on TCP
//! 7142 or a serial port.
//!
//! Protocol from NEC's Projector Control Command Reference Manual
//! (BDT140013 revision 7.1, 2020) and Sharp NEC's (VDT24004 revision 1.0,
//! 2024), with their appendixes for the models; section and command numbers
//! below are theirs.
//!
//! - A command is a class byte (00h-03h), a command byte, ID1 and ID2 (sent
//!   as 00h), a data length, the data, and a checksum: the low byte of the
//!   sum of every byte before it (§2.2). The projector answers with the class
//!   byte plus 20h on success or A0h on failure, the same command byte, its
//!   own ID1 and ID2, a length, the data and a checksum (§2.3); a failure's
//!   two data bytes are an error code pair (§2.4).
//! - One command is outstanding at a time; the answer is matched by its
//!   class and command byte. A reply whose checksum is wrong is dropped.
//! - The projector pushes nothing, so state is polled: the operation, input,
//!   mute and freeze status and the errors every 5 s; lamp or light, filter,
//!   picture, volume, lens, eco or light mode, picture in picture, edge
//!   blending, cover, signal frequencies and carbon savings every minute;
//!   the model, serial number, MAC address and names on connecting, after
//!   the projector has been unreachable, and on `refresh`. Only what the
//!   model's family supports is asked (the appendix's "Supported Command
//!   List"), and a query the projector refuses as not supported (00h 01h) is
//!   not asked again until the device is opened again.
//! - The `link` setting chooses TCP or a serial port of the host (§1.1, the
//!   PC CONTROL port). A connection that drops or a port that fails is
//!   opened again after 1 s, doubling to 30 s; three requests in a row
//!   unanswered close it and open it again.
//!
//! Opened for commands only (`monitor` false), nothing is polled and no set
//! is followed by a read; every 5 s one status request (305-3 BASIC
//! INFORMATION REQUEST, or 078-2 RUNNING STATUS REQUEST on a model without
//! it) shows the projector is there, since it sends nothing unasked. The
//! replies to commands still update the state, and every answered request
//! reports its round trip as the device's latency.

use std::collections::{HashSet, VecDeque};

use serde_json::{json, Map, Value};

use super::nec_projector_models::{Family, FAMILIES};
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

pub(crate) const DEFAULT_PORT: u16 = 7142;
const SOCKET: Key = "nec";
const REPLY: Key = "reply";
const POLL: Key = "poll";
const RETRY: Key = "retry";

/// The manual sets no reply time; projectors answer within milliseconds.
const REPLY_TIMEOUT: Millis = 5_000;
/// Power on and off accept nothing else while they run (§3.2, §3.3), and
/// may answer only once they have started.
const POWER_TIMEOUT: Millis = 15_000;
const POLL_EVERY: Millis = 5_000;
/// The slow reads run every this many polls (one minute).
const SLOW_EVERY: u32 = 12;
const MISSES: u32 = 3;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// The longest reply: 037 INFORMATION REQUEST's 98 data bytes, framed.
const MAX_FRAME: usize = 5 + 255 + 1;

/// The low byte of the sum of `bytes` (§2.2).
fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

/// A command: class, command byte, ID1 = ID2 = 00h, length, data, checksum.
fn frame(class: u8, code: u8, data: &[u8]) -> Vec<u8> {
    let mut out = vec![class, code, 0, 0, data.len() as u8];
    out.extend_from_slice(data);
    out.push(checksum(&out));
    out
}

/// Gains read with 060-1 and set with 030-1, 030-2 and 030-15.
const GAINS: &[(&str, u8)] = &[
    ("brightness", 0x00),
    ("contrast", 0x01),
    ("color", 0x02),
    ("hue", 0x03),
    ("sharpness", 0x04),
    ("volume", 0x05),
    ("light_adjust", 0x96),
];

const LENS_TARGETS: &[(&str, u8)] = &[
    ("zoom", 0x00),
    ("focus", 0x01),
    ("shift_horizontal", 0x02),
    ("shift_vertical", 0x03),
    ("periphery_focus", 0x06),
];

/// 053 LENS CONTROL's drive values.
const LENS_DRIVES: &[(&str, u8)] = &[
    ("stop", 0x00),
    ("plus_1s", 0x01),
    ("plus_0_5s", 0x02),
    ("plus_0_25s", 0x03),
    ("plus", 0x7F),
    ("minus", 0x81),
    ("minus_0_25s", 0xFD),
    ("minus_0_5s", 0xFE),
    ("minus_1s", 0xFF),
];

/// 050 REMOTE KEY CODE: the key codes of both editions.
const KEYS: &[(&str, u16)] = &[
    ("power_on", 2),
    ("power_off", 3),
    ("auto", 5),
    ("menu", 6),
    ("up", 7),
    ("down", 8),
    ("right", 9),
    ("left", 10),
    ("enter", 11),
    ("exit", 12),
    ("help", 13),
    ("magnify_up", 15),
    ("magnify_down", 16),
    ("mute", 19),
    ("picture", 41),
    ("test", 44),
    ("computer1", 75),
    ("computer2", 76),
    ("video1", 79),
    ("s_video1", 81),
    ("1", 100),
    ("2", 101),
    ("3", 102),
    ("4", 103),
    ("5", 104),
    ("6", 105),
    ("7", 106),
    ("8", 107),
    ("9", 108),
    ("0", 109),
    ("volume_up", 132),
    ("volume_down", 133),
    ("freeze", 138),
    ("shutter_open", 147),
    ("shutter_close", 148),
    ("aspect", 163),
    ("source", 215),
    ("hdbaset", 229),
    ("eco", 238),
    ("id_set", 259),
    ("hdmi", 261),
    ("geometric", 273),
    ("aux", 276),
    ("hdmi2", 280),
    ("osd_mute_off", 282),
    ("osd_mute_on", 283),
    ("user1", 284),
    ("user2", 285),
    ("user3", 286),
    ("sdi", 288),
    ("default", 289),
];

const PIP_MODES: &[(&str, u8)] = &[("picture_in_picture", 0), ("picture_by_picture", 1)];
const PIP_POSITIONS: &[(&str, u8)] = &[
    ("top_left", 0),
    ("top_right", 1),
    ("bottom_left", 2),
    ("bottom_right", 3),
];
/// 097-198 targets: mode, start position, then sub inputs 1-3.
const PIP_TARGETS: &[(&str, u8)] = &[
    ("mode", 0x00),
    ("start_position", 0x01),
    ("sub_input", 0x02),
    ("sub_input_2", 0x09),
    ("sub_input_3", 0x0A),
];

fn lookup<T: Copy>(table: &[(&str, T)], name: &str) -> Option<T> {
    table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

fn name_of<T: Copy + PartialEq>(
    table: &'static [(&'static str, T)],
    value: T,
) -> Option<&'static str> {
    table.iter().find(|(_, v)| *v == value).map(|(n, _)| *n)
}

/// How a reply is read.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Read {
    /// Success carries nothing to check.
    Ack,
    /// Success when the data byte at this index is 00h.
    Result(usize),
    /// Success when the first two data bytes are 0000h (030-x).
    ResultWord,
    Errors,
    Information,
    Filter,
    Lamp {
        lamp: u8,
        content: u8,
    },
    Carbon(u8),
    LensPosition(u8),
    LensOption(u8),
    LensStatus,
    LensProfile,
    Gain(u8),
    Setting,
    Running,
    InputStatus,
    MuteStatus,
    ModelName,
    Cover,
    Frequency(u8),
    Eco,
    ProjectorName,
    Mac,
    Pip(u8),
    EdgeBlending,
    BaseModel,
    Serial,
    Basic,
}

#[derive(Debug, Clone)]
struct Job {
    command: Option<CommandId>,
    /// The manual's command number, for the family's support and for
    /// queries the projector refuses as unsupported.
    number: &'static str,
    class: u8,
    code: u8,
    data: Vec<u8>,
    read: Read,
    /// The liveness request: asked even after the projector refused it, since
    /// any answer shows the projector is there.
    keep: bool,
    /// Reads that follow a successful set when monitoring, so the state
    /// shows its result.
    follow: Vec<Job>,
}

impl Job {
    fn new(number: &'static str, class: u8, code: u8, data: Vec<u8>, read: Read) -> Job {
        Job {
            command: None,
            number,
            class,
            code,
            data,
            read,
            keep: false,
            follow: Vec::new(),
        }
    }

    fn kept(mut self) -> Job {
        self.keep = true;
        self
    }

    fn then(mut self, follow: Job) -> Job {
        self.follow.push(follow);
        self
    }

    fn bytes(&self) -> Vec<u8> {
        frame(self.class, self.code, &self.data)
    }
}

// The requests, by command number.
fn errors() -> Job {
    Job::new("009", 0x00, 0x88, vec![], Read::Errors)
}
fn information() -> Job {
    Job::new("037", 0x03, 0x8A, vec![], Read::Information)
}
fn filter_usage() -> Job {
    Job::new("037-3", 0x03, 0x95, vec![], Read::Filter)
}
fn lamp(lamp: u8, content: u8) -> Job {
    Job::new(
        "037-4",
        0x03,
        0x96,
        vec![lamp, content],
        Read::Lamp { lamp, content },
    )
}
fn carbon(target: u8) -> Job {
    Job::new("037-6", 0x03, 0x9A, vec![target], Read::Carbon(target))
}
fn lens_position(target: u8) -> Job {
    Job::new(
        "053-1",
        0x02,
        0x1C,
        vec![target, 0x00],
        Read::LensPosition(target),
    )
}
fn lens_option(target: u8) -> Job {
    Job::new("053-5", 0x02, 0x20, vec![target], Read::LensOption(target))
}
fn lens_status() -> Job {
    Job::new("053-7", 0x02, 0x22, vec![0x00], Read::LensStatus)
}
fn lens_profile() -> Job {
    Job::new("053-11", 0x02, 0x28, vec![], Read::LensProfile)
}
fn gain(target: u8) -> Job {
    Job::new(
        "060-1",
        0x03,
        0x05,
        vec![target, 0x00, 0x00],
        Read::Gain(target),
    )
}
fn setting() -> Job {
    Job::new("078-1", 0x00, 0x85, vec![0x00], Read::Setting)
}
fn running() -> Job {
    Job::new("078-2", 0x00, 0x85, vec![0x01], Read::Running)
}
fn input_status() -> Job {
    Job::new("078-3", 0x00, 0x85, vec![0x02], Read::InputStatus)
}
fn mute_status() -> Job {
    Job::new("078-4", 0x00, 0x85, vec![0x03], Read::MuteStatus)
}
fn model_name() -> Job {
    Job::new("078-5", 0x00, 0x85, vec![0x04], Read::ModelName)
}
fn cover() -> Job {
    Job::new("078-6", 0x00, 0x85, vec![0x05], Read::Cover)
}
fn frequency(kind: u8) -> Job {
    Job::new(
        "084",
        0x00,
        0xD0,
        vec![0x00, kind, 0x01],
        Read::Frequency(kind),
    )
}
fn eco() -> Job {
    Job::new("097-8", 0x03, 0xB0, vec![0x07], Read::Eco)
}
fn projector_name() -> Job {
    Job::new("097-45", 0x03, 0xB0, vec![0x2C], Read::ProjectorName)
}
fn mac() -> Job {
    Job::new("097-155", 0x03, 0xB0, vec![0x9A, 0x00], Read::Mac)
}
fn pip(target: u8) -> Job {
    Job::new("097-198", 0x03, 0xB0, vec![0xC5, target], Read::Pip(target))
}
fn edge_blending() -> Job {
    Job::new(
        "097-243-1",
        0x03,
        0xB0,
        vec![0xDF, 0x00],
        Read::EdgeBlending,
    )
}
fn base_model() -> Job {
    Job::new("305-1", 0x00, 0xBF, vec![0x00], Read::BaseModel)
}
fn serial() -> Job {
    Job::new("305-2", 0x00, 0xBF, vec![0x01, 0x06], Read::Serial)
}
fn basic() -> Job {
    Job::new("305-3", 0x00, 0xBF, vec![0x02], Read::Basic)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Opening,
    Open,
}

pub(crate) struct NecProjector {
    link: Link,
    family: &'static Family,
    monitor: bool,
    phase: Phase,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    buf: Vec<u8>,
    commands: VecDeque<Job>,
    polls: VecDeque<Job>,
    in_flight: Option<(Job, Millis)>,
    cycle: u32,
    need_identity: bool,
    /// Query numbers the projector answered "not supported" to.
    unsupported: HashSet<(&'static str, Vec<u8>)>,
}

impl NecProjector {
    pub(crate) fn new(ctx: OpenContext) -> Result<NecProjector, String> {
        let family = FAMILIES
            .iter()
            .find(|f| f.id == ctx.model)
            .ok_or_else(|| format!("no NEC projector model '{}'", ctx.model))?;
        let link = Link::from_settings(&ctx, DEFAULT_PORT)?;
        Ok(NecProjector::with(link, family, ctx.monitor))
    }

    fn with(link: Link, family: &'static Family, monitor: bool) -> NecProjector {
        NecProjector {
            link,
            family,
            monitor,
            phase: Phase::Closed,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            buf: Vec::new(),
            commands: VecDeque::new(),
            polls: VecDeque::new(),
            in_flight: None,
            cycle: 0,
            need_identity: true,
            unsupported: HashSet::new(),
        }
    }

    fn has(&self, number: &str) -> bool {
        self.family.supports(number)
    }

    /// The status reads: 305-3 where the model has it, else 078-2/3/4.
    fn status_reads(&self) -> Vec<Job> {
        let mut jobs = vec![self.probe()];
        if self.has("078-2") && self.has("305-3") {
            jobs.push(running());
        }
        for (n, job) in [("078-3", input_status()), ("078-4", mute_status())] {
            if self.has(n) {
                jobs.push(job);
            }
        }
        if self.has("009") && jobs[0].number != "009" {
            jobs.push(errors());
        }
        jobs
    }

    /// The status request that shows the projector is there: 305-3, else
    /// 078-2, else 009; a model with none of them (XP-M421W) is asked
    /// 078-2 all the same, since its "not supported" answer is an answer.
    fn probe(&self) -> Job {
        let job = if self.has("305-3") {
            basic()
        } else if self.has("078-2") || !self.has("009") {
            running()
        } else {
            errors()
        };
        job.kept()
    }

    fn identity_reads(&self) -> Vec<Job> {
        [
            model_name(),
            serial(),
            base_model(),
            setting(),
            mac(),
            projector_name(),
        ]
        .into_iter()
        .filter(|j| self.has(j.number))
        .collect()
    }

    fn slow_reads(&self) -> Vec<Job> {
        let mut jobs = Vec::new();
        if self.has("037") {
            jobs.push(information());
        }
        if self.has("037-3") {
            jobs.push(filter_usage());
        }
        if self.has("037-4") {
            jobs.push(lamp(0, 0x01));
            if !self.family.gen2024 {
                jobs.push(lamp(0, 0x04));
                jobs.push(lamp(1, 0x01));
                jobs.push(lamp(1, 0x04));
            }
        }
        if self.has("060-1") {
            for (name, target) in GAINS {
                if *name == "volume" && !self.has("030-2") {
                    continue;
                }
                if *name == "light_adjust" && !self.has("030-15") {
                    continue;
                }
                jobs.push(gain(*target));
            }
        }
        if self.has("053-1") {
            for (name, target) in LENS_TARGETS {
                if *name == "periphery_focus" && !self.family.gen2024 {
                    continue;
                }
                jobs.push(lens_position(*target));
            }
        }
        if self.has("053-7") {
            jobs.push(lens_status());
        }
        if self.has("053-5") {
            jobs.push(lens_option(0));
            jobs.push(lens_option(1));
        }
        if self.has("053-11") {
            jobs.push(lens_profile());
        }
        if self.has("097-8") {
            jobs.push(eco());
        }
        if self.has("097-198") {
            for (_, target) in PIP_TARGETS {
                jobs.push(pip(*target));
            }
        }
        if self.has("097-243-1") {
            jobs.push(edge_blending());
        }
        if self.has("078-6") {
            jobs.push(cover());
        }
        if self.has("084") {
            jobs.push(frequency(0x03));
            jobs.push(frequency(0x04));
        }
        if self.has("037-6") {
            jobs.push(carbon(0));
            jobs.push(carbon(1));
        }
        jobs
    }

    fn queue_cycle(&mut self) {
        if !self.polls.is_empty() {
            return;
        }
        if !self.monitor {
            // Commands only: one status request, as the liveness check.
            let probe = self.probe();
            self.polls.push_back(probe);
            return;
        }
        self.queue_reads();
    }

    fn queue_reads(&mut self) {
        let mut jobs = Vec::new();
        if self.need_identity {
            self.need_identity = false;
            jobs.extend(self.identity_reads());
        }
        jobs.extend(self.status_reads());
        if self.cycle.is_multiple_of(SLOW_EVERY) {
            jobs.extend(self.slow_reads());
        }
        self.cycle = self.cycle.wrapping_add(1);
        self.polls.extend(jobs);
    }

    fn wanted(&self, job: &Job) -> bool {
        job.keep || !self.unsupported.contains(&(job.number, job.data.clone()))
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || self.phase != Phase::Open {
            return;
        }
        while let Some(job) = self.polls.front() {
            if self.wanted(job) {
                break;
            }
            self.polls.pop_front();
        }
        let Some(job) = self.commands.pop_front().or_else(|| self.polls.pop_front()) else {
            return;
        };
        cx.tcp_send(SOCKET, job.bytes());
        let timeout = if job.number == "015" || job.number == "016" {
            POWER_TIMEOUT
        } else {
            REPLY_TIMEOUT
        };
        cx.set_timer(REPLY, timeout);
        self.in_flight = Some((job, cx.now()));
    }

    fn open_link(&mut self, cx: &mut Cx) {
        self.phase = Phase::Opening;
        self.buf.clear();
        cx.connection(Connection::Connecting);
        self.link.open(cx, SOCKET);
    }

    /// The link failed or the projector stopped answering: fail what waits
    /// and open the link again with backoff.
    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if self.phase != Phase::Closed {
            cx.tcp_close(SOCKET);
        }
        self.phase = Phase::Closed;
        self.buf.clear();
        self.misses = 0;
        for job in self
            .in_flight
            .take()
            .map(|(j, _)| j)
            .into_iter()
            .chain(self.commands.drain(..))
        {
            if let Some(id) = job.command {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        self.polls.clear();
        cx.cancel_timer(REPLY);
        if self.reachable {
            self.need_identity = true;
        }
        self.reachable = false;
        cx.log(Level::Debug, format!("NEC projector: {reason}"));
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn data(&mut self, cx: &mut Cx, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        loop {
            // Resynchronise on a plausible header: a reply's class byte.
            let start = self
                .buf
                .iter()
                .position(|b| matches!(b, 0x20..=0x23 | 0xA0..=0xA3));
            match start {
                Some(0) => {}
                Some(n) => {
                    self.buf.drain(..n);
                }
                None => {
                    self.buf.clear();
                    return;
                }
            }
            if self.buf.len() < 5 {
                return;
            }
            let total = 5 + self.buf[4] as usize + 1;
            if total > MAX_FRAME {
                self.buf.drain(..1);
                continue;
            }
            if self.buf.len() < total {
                return;
            }
            let reply: Vec<u8> = self.buf.drain(..total).collect();
            if checksum(&reply[..total - 1]) != reply[total - 1] {
                cx.log(
                    Level::Debug,
                    "NEC projector: dropped a reply with a bad checksum",
                );
                continue;
            }
            self.reply(cx, &reply);
        }
    }

    fn reply(&mut self, cx: &mut Cx, reply: &[u8]) {
        let ok = reply[0] & 0xF0 == 0x20;
        let class = reply[0] & 0x0F;
        let code = reply[1];
        let data = &reply[5..reply.len() - 1];
        let matches = self
            .in_flight
            .as_ref()
            .is_some_and(|(j, _)| j.class == class && j.code == code);
        if !matches {
            cx.log(
                Level::Debug,
                format!(
                    "NEC projector: unrequested reply {:02X}h {:02X}h",
                    reply[0], code
                ),
            );
            return;
        }
        let (job, sent_at) = self.in_flight.take().expect("checked above");
        cx.cancel_timer(REPLY);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        cx.alive();
        self.misses = 0;
        self.retry_after = RETRY_MIN;
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
        let result = if ok {
            match self.interpret(job.read, data) {
                Ok((value, patch)) => {
                    if patch.as_object().is_some_and(|p| !p.is_empty()) {
                        cx.state(patch);
                    }
                    Ok(value)
                }
                Err(message) => Err(CommandError::DeviceError {
                    code: None,
                    message: format!("{}: {message}", job.number),
                }),
            }
        } else {
            let (e1, e2) = (
                data.first().copied().unwrap_or(0xFF),
                data.get(1).copied().unwrap_or(0xFF),
            );
            if (e1, e2) == (0x00, 0x01) && job.command.is_none() && !job.keep {
                self.unsupported.insert((job.number, job.data.clone()));
            }
            Err(device_error(job.number, e1, e2))
        };
        if result.is_ok() && self.monitor {
            for follow in job.follow.iter().rev() {
                self.polls.push_front(follow.clone());
            }
        }
        if let Some(id) = job.command {
            cx.complete(
                id,
                result.map(|v| match v {
                    Value::Null => Outcome::Ack,
                    value => Outcome::Value { value },
                }),
            );
        }
        self.pump(cx);
    }

    /// The returned value (null for an acknowledgement) and the state patch.
    fn interpret(&self, read: Read, d: &[u8]) -> Result<(Value, Value), String> {
        let need = |n: usize| {
            if d.len() < n {
                Err(format!("{} data bytes where {n} were expected", d.len()))
            } else {
                Ok(())
            }
        };
        let gen2024 = self.family.gen2024;
        Ok(match read {
            Read::Ack => (Value::Null, json!({})),
            Read::Result(i) => {
                need(i + 1)?;
                if d[i] != 0x00 {
                    return Err(format!("the projector reports failure ({:02X}h)", d[i]));
                }
                (Value::Null, json!({}))
            }
            Read::ResultWord => {
                need(2)?;
                if d[0] != 0 || d[1] != 0 {
                    return Err(format!(
                        "the projector reports failure ({:02X}{:02X}h)",
                        d[1], d[0]
                    ));
                }
                (Value::Null, json!({}))
            }
            Read::Errors => {
                need(12)?;
                let v = error_bits(d, gen2024);
                (v.clone(), json!({ "errors": v }))
            }
            Read::Information => {
                need(98)?;
                let name = cstr(&d[0..49]);
                let lamp = u32le(&d[82..86]);
                let mut patch = json!({
                    "info": { "projector_name": name },
                    "lamps": { "1": lamp_usage(lamp) },
                });
                let mut v = json!({ "projector_name": name, "lamp_usage_seconds": lamp });
                if !gen2024 {
                    let filter = u32le(&d[86..90]);
                    patch["filter"] = json!({ "usage_seconds": filter });
                    v["filter_usage_seconds"] = json!(filter);
                }
                (v, patch)
            }
            Read::Filter => {
                need(8)?;
                let usage = i32le(&d[0..4]);
                let alarm = i32le(&d[4..8]);
                let v = json!({
                    "usage_seconds": defined(usage),
                    "alarm_start_seconds": defined(alarm),
                });
                (v.clone(), json!({ "filter": v }))
            }
            Read::Lamp { lamp, content } => {
                need(6)?;
                let value = i32le(&d[2..6]);
                let key = (lamp + 1).to_string();
                let v = if content == 0x04 {
                    json!({ "remaining_percent": value })
                } else {
                    lamp_usage(value.max(0) as u32)
                };
                (v.clone(), json!({ "lamps": { key: v } }))
            }
            Read::Carbon(target) => {
                need(9)?;
                let kg = u32le(&d[1..5]) as f64 + u32le(&d[5..9]) as f64 / 1_000_000.0;
                let key = if target == 0 {
                    "total_kg"
                } else {
                    "operation_kg"
                };
                (json!(kg), json!({ "carbon_savings": { key: kg } }))
            }
            Read::LensPosition(target) => {
                need(8)?;
                let v = json!({
                    "max": i16le(&d[2..4]),
                    "min": i16le(&d[4..6]),
                    "position": i16le(&d[6..8]),
                });
                let name = name_of(LENS_TARGETS, target).unwrap_or("unknown");
                (v.clone(), json!({ "lens": { name: v } }))
            }
            Read::LensOption(target) => {
                need(2)?;
                let on = d[1] == 0x01;
                let key = if target == 0 {
                    "load_by_signal"
                } else {
                    "forced_mute"
                };
                (json!(on), json!({ "lens_memory": { key: on } }))
            }
            Read::LensStatus => {
                need(2)?;
                let b = d[1];
                let v = json!({
                    "memory": b & 0x01 != 0,
                    "zoom": b & 0x02 != 0,
                    "focus": b & 0x04 != 0,
                    "shift_horizontal": b & 0x08 != 0,
                    "shift_vertical": b & 0x10 != 0,
                });
                (v.clone(), json!({ "lens": { "moving": v } }))
            }
            Read::LensProfile => {
                need(1)?;
                let profile = d[0] as u64 + 1;
                (json!(profile), json!({ "lens_profile": profile }))
            }
            Read::Gain(target) => {
                need(14)?;
                let status = match d[0] {
                    0x00 => "not_displayed",
                    0x01 => "not_adjustable",
                    0x02 => "adjustable",
                    0xFF => "absent",
                    _ => "unknown",
                };
                let v = if d[0] == 0xFF {
                    json!({ "status": status })
                } else {
                    json!({
                        "status": status,
                        "max": i16le(&d[1..3]),
                        "min": i16le(&d[3..5]),
                        "default": if d[13] == 0x01 { json!(i16le(&d[5..7])) } else { Value::Null },
                        "value": i16le(&d[7..9]),
                        "wide_step": i16le(&d[9..11]),
                        "narrow_step": i16le(&d[11..13]),
                    })
                };
                let name = name_of(GAINS, target).unwrap_or("unknown");
                let patch = match name {
                    "volume" | "light_adjust" => json!({ name: v }),
                    _ => json!({ "picture": { name: v } }),
                };
                (v, patch)
            }
            Read::Setting => {
                need(5)?;
                let timers = match d[4] {
                    0x00 => "none",
                    0x01 => "clock",
                    0x02 => "sleep_timer",
                    0x03 => "clock_and_sleep_timer",
                    _ => "unknown",
                };
                let v = json!({
                    "base_model_type": hex(&d[0..3]),
                    "sound": d[3] == 0x01,
                    "timer_functions": timers,
                });
                (v.clone(), json!({ "info": v }))
            }
            Read::Running => {
                need(6)?;
                let mut v = Map::new();
                if d[2] != 0xFF {
                    v.insert("powered".into(), json!(d[2] == 0x01));
                }
                if d[3] != 0xFF {
                    v.insert("cooling".into(), json!(d[3] == 0x01));
                }
                if d[4] != 0xFF {
                    v.insert("power_changing".into(), json!(d[4] == 0x01));
                }
                if let Some(p) = operation(d[5]) {
                    v.insert("power".into(), json!(p));
                }
                let v = Value::Object(v);
                (v.clone(), v)
            }
            Read::InputStatus => {
                need(9)?;
                let mut input = Map::new();
                if d[0] != 0xFF {
                    input.insert("switching".into(), json!(d[0] == 0x01));
                }
                if d[1] != 0xFF {
                    input.insert("list_number".into(), json!(d[1] as u64 + 1));
                }
                input.extend(signal(d[2], d[3], gen2024));
                if d[4] != 0xFF {
                    input.insert(
                        "list_type".into(),
                        json!(if d[4] == 0x01 { "user" } else { "default" }),
                    );
                }
                if d[5] != 0xFF {
                    input.insert("test_pattern".into(), json!(d[5] == 0x01));
                }
                let mut patch = json!({ "input": input });
                let mut v = Value::Object(input);
                let shown = match (d[8], gen2024) {
                    (0x04, true) => Some("test_pattern"),
                    (0x04, false) => Some("lan"),
                    (c, _) => content(c),
                };
                if let Some(c) = shown {
                    patch["display"] = json!(c);
                    v["display"] = json!(c);
                }
                (v, patch)
            }
            Read::MuteStatus => {
                need(5)?;
                let mut v = Map::new();
                for (i, key) in [
                    "picture",
                    "sound",
                    "onscreen",
                    "forced_onscreen",
                    "onscreen_display",
                ]
                .iter()
                .enumerate()
                {
                    if d[i] != 0xFF {
                        v.insert((*key).into(), json!(d[i] == 0x01));
                    }
                }
                let v = Value::Object(v);
                (v.clone(), json!({ "mute": v }))
            }
            Read::ModelName => {
                let name = cstr(d);
                (json!(name), json!({ "info": { "model_name": name } }))
            }
            Read::Cover => {
                need(1)?;
                let closed = d[0] == 0x01;
                (json!(closed), json!({ "cover_closed": closed }))
            }
            Read::Frequency(kind) => {
                need(3)?;
                let len = d[2] as usize;
                let text = cstr(&d[3..(3 + len).min(d.len())]);
                let key = if kind == 0x03 {
                    "horizontal_frequency"
                } else {
                    "vertical_frequency"
                };
                (json!(text), json!({ "signal": { key: text } }))
            }
            Read::Eco => {
                need(2)?;
                let v = name_of(self.family.eco, d[1])
                    .map(|n| json!(n))
                    .unwrap_or_else(|| json!(format!("{:02x}h", d[1])));
                (v.clone(), json!({ "eco_mode": v }))
            }
            Read::ProjectorName => {
                need(1)?;
                let name = cstr(&d[1..]);
                (json!(name), json!({ "name": name }))
            }
            Read::Mac => {
                need(8)?;
                let mac = d[2..8]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(":");
                (json!(mac), json!({ "info": { "mac_address": mac } }))
            }
            Read::Pip(target) => {
                need(3)?;
                let value = d[2];
                let v = match target {
                    0x00 => name_of(PIP_MODES, value).map(|n| json!(n)),
                    0x01 => name_of(PIP_POSITIONS, value).map(|n| json!(n)),
                    _ => name_of(self.family.sub_inputs, value).map(|n| json!(n)),
                }
                .unwrap_or_else(|| json!(format!("{value:02x}h")));
                let key = name_of(PIP_TARGETS, target).unwrap_or("unknown");
                (v.clone(), json!({ "pip": { key: v } }))
            }
            Read::EdgeBlending => {
                need(3)?;
                let on = d[2] == 0x01;
                (json!(on), json!({ "edge_blending": on }))
            }
            Read::BaseModel => {
                need(14)?;
                // 00h, then DATA01-02 and DATA12-13 the base model type and
                // DATA03-11 the model name.
                let v = json!({
                    "base_model_type": hex(&[d[1], d[2], d[12], d[13]]),
                    "model_name": cstr(&d[3..12]),
                });
                (
                    v.clone(),
                    json!({ "info": { "base_model_type": v["base_model_type"] } }),
                )
            }
            Read::Serial => {
                need(2)?;
                let s = cstr(&d[2..]);
                (json!(s), json!({ "info": { "serial_number": s } }))
            }
            Read::Basic => {
                need(10)?;
                let mut v = Map::new();
                if let Some(p) = operation(d[1]) {
                    v.insert("power".into(), json!(p));
                }
                if let Some(c) = content(d[2]) {
                    v.insert("display".into(), json!(c));
                }
                v.insert("input".into(), Value::Object(signal(d[3], d[4], gen2024)));
                if !gen2024 {
                    v.insert("video_signal".into(), video_signal(d[5]));
                }
                v.insert(
                    "mute".into(),
                    json!({ "picture": d[6] == 0x01, "sound": d[7] == 0x01, "onscreen": d[8] == 0x01 }),
                );
                v.insert("freeze".into(), json!(d[9] == 0x01));
                let v = Value::Object(v);
                (v.clone(), v)
            }
        })
    }

    fn command_job(&self, name: &str, p: &Params) -> Result<Job, CommandError> {
        let text = |key: &str| {
            p.get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(format!("'{key}' is required")))
        };
        let flag = |key: &str| {
            p.get(key)
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid(format!("'{key}' is required")))
        };
        let int = |key: &str| {
            p.get(key)
                .and_then(Value::as_i64)
                .ok_or_else(|| invalid(format!("'{key}' is required")))
        };
        let word = |v: i64| -> Result<[u8; 2], CommandError> {
            let w = i16::try_from(v).map_err(|_| invalid(format!("{v} does not fit 16 bits")))?;
            Ok(w.to_le_bytes())
        };
        let pick = |table: &[(&str, u8)], key: &str, what: &str| -> Result<u8, CommandError> {
            let v = text(key)?;
            lookup(table, v).ok_or_else(|| {
                invalid(format!(
                    "{what} '{v}' is not one of this model's ({})",
                    table.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                ))
            })
        };
        let mode =
            || -> Result<u8, CommandError> { Ok(if flag("relative")? { 0x01 } else { 0x00 }) };
        let f = self.family;
        let job = match name {
            "set_power" => {
                let on = flag("on")?;
                let (number, code) = if on { ("015", 0x00) } else { ("016", 0x01) };
                Job::new(number, 0x02, code, vec![], Read::Ack).then(self.status_follow())
            }
            "set_input" => {
                let code = pick(f.inputs, "input", "input")?;
                Job::new("018", 0x02, 0x03, vec![0x01, code], Read::Result(0))
                    .then(self.status_follow())
            }
            "set_picture_mute" | "set_sound_mute" | "set_onscreen_mute" => {
                let base = match name {
                    "set_picture_mute" => 0x10,
                    "set_sound_mute" => 0x12,
                    _ => 0x14,
                };
                let on = flag("muted")?;
                let number = match (base, on) {
                    (0x10, true) => "020",
                    (0x10, false) => "021",
                    (0x12, true) => "022",
                    (0x12, false) => "023",
                    (_, true) => "024",
                    _ => "025",
                };
                Job::new(number, 0x02, base + u8::from(!on), vec![], Read::Ack)
                    .then(self.mute_follow())
            }
            "set_picture" => {
                let item = text("item")?;
                let target = lookup(&GAINS[..5], item)
                    .ok_or_else(|| invalid(format!("unknown picture item '{item}'")))?;
                let [lo, hi] = word(int("value")?)?;
                Job::new(
                    "030-1",
                    0x03,
                    0x10,
                    vec![target, 0xFF, mode()?, lo, hi],
                    Read::ResultWord,
                )
                .then(self.gain_follow(target))
            }
            "set_volume" => {
                let [lo, hi] = word(int("value")?)?;
                Job::new(
                    "030-2",
                    0x03,
                    0x10,
                    vec![0x05, 0x00, mode()?, lo, hi],
                    Read::ResultWord,
                )
                .then(self.gain_follow(0x05))
            }
            "set_light_adjust" => {
                let [lo, hi] = word(int("value")?)?;
                Job::new(
                    "030-15",
                    0x03,
                    0x10,
                    vec![0x96, 0xFF, mode()?, lo, hi],
                    Read::ResultWord,
                )
                .then(self.gain_follow(0x96))
            }
            "set_aspect" | "set_aspect_code" => {
                let code = if name == "set_aspect" {
                    pick(f.aspect, "aspect", "aspect")?
                } else {
                    u8::try_from(int("code")?).map_err(|_| invalid("code is 0-255"))?
                };
                Job::new(
                    "030-12",
                    0x03,
                    0x10,
                    vec![0x18, 0x00, 0x00, code, 0x00],
                    Read::ResultWord,
                )
            }
            "get_information" => information(),
            "get_filter_usage" => filter_usage(),
            "get_lamp_usage" => {
                let lamp_no = int("lamp")? as u8 - 1;
                let content = match text("content")? {
                    "remaining_percent" => 0x04,
                    _ => 0x01,
                };
                lamp(lamp_no, content)
            }
            "get_carbon_savings" => carbon(u8::from(text("scope")? == "operation")),
            "press_key" => {
                let key = text("key")?;
                let code =
                    lookup(KEYS, key).ok_or_else(|| invalid(format!("unknown key '{key}'")))?;
                let [lo, hi] = code.to_le_bytes();
                Job::new("050", 0x02, 0x0F, vec![lo, hi], Read::Result(0))
            }
            "set_shutter" => {
                let closed = flag("closed")?;
                let (number, code) = if closed { ("051", 0x16) } else { ("052", 0x17) };
                Job::new(number, 0x02, code, vec![], Read::Ack)
            }
            "drive_lens" => {
                let target = self.lens_target(text("target")?)?;
                let drive = pick(LENS_DRIVES, "drive", "drive")?;
                let job = Job::new("053", 0x02, 0x18, vec![target, drive], Read::Result(0));
                self.lens_follow(job, target)
            }
            "get_lens_position" => lens_position(self.lens_target(text("target")?)?),
            "set_lens_position" | "move_lens" => {
                let target = self.lens_target(text("target")?)?;
                let (key, mode) = if name == "set_lens_position" {
                    ("position", 0x00)
                } else {
                    ("offset", 0x02)
                };
                let [lo, hi] = word(int(key)?)?;
                let job = Job::new("053-2", 0x02, 0x1D, vec![target, mode, lo, hi], Read::Ack);
                self.lens_follow(job, target)
            }
            "stop_lens" => Job::new("053-2", 0x02, 0x1D, vec![0xFF, 0x00, 0x00, 0x00], Read::Ack),
            "lens_memory" | "reference_lens_memory" => {
                let op = match text("operation")? {
                    "move" => 0x00,
                    "store" => 0x01,
                    "reset" => 0x02,
                    other => return Err(invalid(format!("unknown operation '{other}'"))),
                };
                let (number, code) = if name == "lens_memory" {
                    ("053-3", 0x1E)
                } else {
                    ("053-4", 0x1F)
                };
                let mut job = Job::new(number, 0x02, code, vec![op], Read::Result(1));
                if op == 0x00 && self.has("053-1") {
                    for (n, t) in LENS_TARGETS {
                        if *n != "periphery_focus" || f.gen2024 {
                            job = job.then(lens_position(*t));
                        }
                    }
                }
                job
            }
            "get_lens_memory_option" => lens_option(self.lens_option(text("option")?)?),
            "set_lens_memory_option" => {
                let target = self.lens_option(text("option")?)?;
                let on = u8::from(flag("on")?);
                let mut job = Job::new("053-6", 0x02, 0x21, vec![target, on], Read::Result(1));
                if self.has("053-5") {
                    job = job.then(lens_option(target));
                }
                job
            }
            "get_lens_status" => lens_status(),
            "set_lens_profile" => {
                let profile = int("profile")? as u8 - 1;
                let mut job = Job::new("053-10", 0x02, 0x27, vec![profile], Read::Result(1));
                if self.has("053-11") {
                    job = job.then(lens_profile());
                }
                job
            }
            "get_lens_profile" => lens_profile(),
            "get_gain" => {
                let item = text("item")?;
                gain(lookup(GAINS, item).ok_or_else(|| invalid(format!("unknown item '{item}'")))?)
            }
            "get_settings" => setting(),
            "get_running_status" => running(),
            "get_input_status" => input_status(),
            "get_mute_status" => mute_status(),
            "get_model_name" => model_name(),
            "get_cover_status" => cover(),
            "set_freeze" => {
                let on = flag("frozen")?;
                let mut job = Job::new(
                    "079",
                    0x01,
                    0x98,
                    vec![if on { 0x01 } else { 0x02 }],
                    Read::Result(0),
                );
                if self.has("305-3") {
                    job = job.then(basic());
                }
                job
            }
            "get_signal_frequency" => frequency(if text("which")? == "horizontal" {
                0x03
            } else {
                0x04
            }),
            "get_eco_mode" => eco(),
            "set_eco_mode" => {
                let value = pick(f.eco, "mode", "mode")?;
                let mut job = Job::new("098-8", 0x03, 0xB1, vec![0x07, value], Read::Result(1));
                if self.has("097-8") {
                    job = job.then(eco());
                }
                job
            }
            "get_projector_name" => projector_name(),
            "set_projector_name" => {
                let name = text("name")?;
                if !name.is_ascii() || name.len() > 16 {
                    return Err(invalid("the name is up to 16 ASCII characters"));
                }
                let mut data = vec![0x2C];
                let mut bytes = name.as_bytes().to_vec();
                bytes.resize(16, 0);
                data.extend(bytes);
                data.push(0x00);
                let mut job = Job::new("098-45", 0x03, 0xB1, data, Read::Result(1));
                if self.has("097-45") {
                    job = job.then(projector_name());
                }
                job
            }
            "get_mac_address" => mac(),
            "get_pip" => {
                let target = text("target")?;
                pip(lookup(PIP_TARGETS, target)
                    .ok_or_else(|| invalid(format!("unknown target '{target}'")))?)
            }
            "set_pip_mode" | "set_pip_position" | "set_pip_sub_input" => {
                let (target, value) = match name {
                    "set_pip_mode" => (0x00, pick(PIP_MODES, "mode", "mode")?),
                    "set_pip_position" => (0x01, pick(PIP_POSITIONS, "position", "position")?),
                    _ => {
                        let target = match int("sub")? {
                            1 => 0x02,
                            2 => 0x09,
                            _ => 0x0A,
                        };
                        (target, pick(f.sub_inputs, "input", "sub input")?)
                    }
                };
                let mut job = Job::new(
                    "098-198",
                    0x03,
                    0xB1,
                    vec![0xC5, target, value],
                    Read::Result(2),
                );
                if self.has("097-198") {
                    job = job.then(pip(target));
                }
                job
            }
            "get_edge_blending" => edge_blending(),
            "set_edge_blending" => {
                let on = u8::from(flag("on")?);
                let mut job = Job::new(
                    "098-243-1",
                    0x03,
                    0xB1,
                    vec![0xDF, 0x00, on],
                    Read::Result(2),
                );
                if self.has("097-243-1") {
                    job = job.then(edge_blending());
                }
                job
            }
            "get_base_model_type" => base_model(),
            "get_serial_number" => serial(),
            "get_basic_information" => basic(),
            "set_audio_select" => {
                let terminal = pick(f.audio, "input", "audio input")?;
                let source = match text("source")? {
                    "input" => 0x00,
                    "bnc" => 0x01,
                    "computer" => 0x02,
                    other => return Err(invalid(format!("unknown source '{other}'"))),
                };
                Job::new(
                    "319-10",
                    0x03,
                    0xC9,
                    vec![0x09, terminal, source],
                    Read::Result(2),
                )
            }
            "get_errors" => errors(),
            _ => {
                return Err(CommandError::UnknownCommand {
                    command: name.into(),
                })
            }
        };
        if !self.has(job.number) {
            return Err(CommandError::UnsupportedForModel {
                command: name.into(),
                model: f.id.into(),
            });
        }
        Ok(job)
    }

    fn lens_target(&self, name: &str) -> Result<u8, CommandError> {
        match lookup(LENS_TARGETS, name) {
            Some(0x06) if !self.family.gen2024 => Err(invalid(
                "periphery focus is a target of the 2024 models only",
            )),
            Some(t) => Ok(t),
            None => Err(invalid(format!("unknown lens target '{name}'"))),
        }
    }

    fn lens_option(&self, name: &str) -> Result<u8, CommandError> {
        match name {
            "load_by_signal" => Ok(0),
            "forced_mute" => Ok(1),
            other => Err(invalid(format!("unknown lens memory option '{other}'"))),
        }
    }

    fn lens_follow(&self, mut job: Job, target: u8) -> Job {
        if self.has("053-1") {
            job = job.then(lens_position(target));
        }
        job
    }

    fn status_follow(&self) -> Job {
        if self.has("305-3") {
            basic()
        } else if self.has("078-3") {
            input_status()
        } else {
            running()
        }
    }

    fn mute_follow(&self) -> Job {
        if self.has("305-3") {
            basic()
        } else if self.has("078-4") {
            mute_status()
        } else {
            running()
        }
    }

    fn gain_follow(&self, target: u8) -> Job {
        if self.has("060-1") {
            gain(target)
        } else {
            self.status_follow()
        }
    }
}

impl Module for NecProjector {
    fn start(&mut self, cx: &mut Cx) {
        if self.monitor {
            // The model's input terminals, from the appendix's table: the
            // protocol has no request listing them.
            let names: Vec<&str> = self.family.inputs.iter().map(|(n, _)| *n).collect();
            cx.state(json!({ "inputs": { "available": names } }));
        }
        self.open_link(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if name == "refresh" {
            self.need_identity = true;
            self.polls.clear();
            self.cycle = 0;
            // Everything, once, even when opened for commands only.
            self.queue_reads();
            cx.complete(id, Ok(Outcome::Ack));
            self.pump(cx);
            return;
        }
        if self.phase == Phase::Closed {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match self.command_job(name, params) {
            Ok(mut job) => {
                job.command = Some(id);
                self.commands.push_back(job);
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                if self.phase != Phase::Opening {
                    return;
                }
                self.phase = Phase::Open;
                self.buf.clear();
                self.queue_cycle();
                cx.set_timer(POLL, POLL_EVERY);
                self.pump(cx);
            }
            TcpInput::Data(bytes) => {
                if self.phase == Phase::Open {
                    self.data(cx, &bytes);
                }
            }
            TcpInput::Closed { reason } => {
                if self.phase != Phase::Closed {
                    self.phase = Phase::Opening;
                    self.lost(cx, reason);
                }
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.open_link(cx),
            POLL => {
                if self.phase == Phase::Open {
                    self.queue_cycle();
                    cx.set_timer(POLL, POLL_EVERY);
                    self.pump(cx);
                }
            }
            REPLY => {
                if let Some((job, _)) = self.in_flight.take() {
                    if let Some(id) = job.command {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES {
                        self.lost(cx, "the projector answered none of three requests".into());
                        return;
                    }
                    self.pump(cx);
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.phase != Phase::Closed {
            cx.tcp_close(SOCKET);
        }
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

/// An error answer with the meaning §2.4 gives its code pair.
fn device_error(number: &str, e1: u8, e2: u8) -> CommandError {
    let meaning = match (e1, e2) {
        (0x00, 0x00) => "the command cannot be recognized",
        (0x00, 0x01) => "the command is not supported by the model in use",
        (0x01, 0x00) => "the specified value is invalid",
        (0x01, 0x01) => "the specified input terminal is invalid",
        (0x01, 0x02) => "the specified language is invalid",
        (0x02, 0x00) => "memory allocation error",
        (0x02, 0x02) => "memory in use",
        (0x02, 0x03) => "the specified value cannot be set",
        (0x02, 0x04) => "forced onscreen mute on",
        (0x02, 0x06) => "viewer error",
        (0x02, 0x07) => "no signal",
        (0x02, 0x08) => "a test pattern or filer is displayed",
        (0x02, 0x09) => "no PC card is inserted",
        (0x02, 0x0A) => "memory operation error",
        (0x02, 0x0C) => "an entry list is displayed",
        (0x02, 0x0D) => "the command cannot be accepted because the power is off",
        (0x02, 0x0E) => "the command execution failed",
        (0x02, 0x0F) => "there is no authority necessary for the operation",
        (0x03, 0x00) => "the specified gain number is incorrect",
        (0x03, 0x01) => "the specified gain is invalid",
        (0x03, 0x02) => "adjustment failed",
        _ => "an error the manual does not list",
    };
    CommandError::DeviceError {
        code: Some(format!("{e1:02X}{e2:02X}")),
        message: format!("{number}: {meaning}"),
    }
}

fn i16le(b: &[u8]) -> i64 {
    i16::from_le_bytes([b[0], b[1]]) as i64
}

fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn i32le(b: &[u8]) -> i64 {
    i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as i64
}

/// A time the manual reports as -1 when it is not defined (037-3).
fn defined(v: i64) -> Value {
    if v < 0 {
        Value::Null
    } else {
        json!(v)
    }
}

fn lamp_usage(seconds: u32) -> Value {
    json!({ "usage_seconds": seconds, "usage_hours": seconds / 3600 })
}

/// Text up to the first NUL.
fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim_end().to_string()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The operation status of 078-2 DATA06 and 305-3 DATA01.
fn operation(b: u8) -> Option<&'static str> {
    Some(match b {
        0x00 => "standby",
        0x04 => "on",
        0x05 => "cooling",
        0x06 => "standby_error",
        0x0F => "standby_power_saving",
        0x10 => "network_standby",
        _ => return None,
    })
}

/// What the projector shows (305-3 DATA02, 078-3 DATA09 but for 04h).
fn content(b: u8) -> Option<&'static str> {
    Some(match b {
        0x00 => "video",
        0x01 => "no_signal",
        0x02 => "viewer",
        0x03 => "test_pattern",
        0x04 => "lan",
        0x05 => "user_test_pattern",
        0x10 => "switching",
        _ => return None,
    })
}

/// Selection signal types 1 and 2 (078-3 DATA03-04, 305-3 DATA03-04).
fn signal(number: u8, kind: u8, gen2024: bool) -> Map<String, Value> {
    let mut m = Map::new();
    let name = match kind {
        0x01 if !gen2024 => Some("computer"),
        0x02 if !gen2024 => Some("video"),
        0x03 if !gen2024 => Some("s_video"),
        0x04 if !gen2024 => Some("component"),
        0x07 if !gen2024 => Some("viewer_1_5"),
        0x20 if !gen2024 => Some("dvi_d"),
        0x21 => Some("hdmi"),
        0x22 if !gen2024 => Some("displayport"),
        0x23 if !gen2024 => Some("viewer_6_10"),
        0x27 => Some("hdbaset"),
        0x28 => Some("sdi"),
        0xFF => None,
        _ => Some("other"),
    };
    match name {
        Some(n) => {
            m.insert("type".into(), json!(n));
            m.insert("type_code".into(), json!(kind));
            if (1..=5).contains(&number) {
                m.insert("number".into(), json!(number));
            }
        }
        None => {
            m.insert("type".into(), json!("none"));
            m.insert("type_code".into(), json!(kind));
        }
    }
    m
}

/// 305-3 DATA05, the video signal type of a video or S-video input.
fn video_signal(b: u8) -> Value {
    let s = match b {
        0x00 => "NTSC3.58",
        0x01 => "NTSC4.43",
        0x02 => "PAL",
        0x03 => "PAL60",
        0x04 => "SECAM",
        0x05 => "B/W60",
        0x06 => "B/W50",
        0x07 => "PALNM",
        0x08 => "NTSC3.58 LBX",
        0x09 => "NTSC3.58 SQZ",
        0x0A => "COMPONENT(60Hz)",
        0x0B => "COMPONENT(50Hz)",
        0x0C => "unknown",
        0x0D => "NTSC",
        0x0E => "PAL-M",
        0x0F => "PAL-L",
        _ => return Value::Null,
    };
    json!(s)
}

/// 009 ERROR STATUS REQUEST: one boolean per error the edition defines.
fn error_bits(d: &[u8], gen2024: bool) -> Value {
    let bit = |byte: usize, n: u8| d[byte] & (1 << n) != 0;
    let mut m = Map::new();
    let mut put = |name: &str, v: bool| {
        m.insert(name.into(), json!(v));
    };
    if gen2024 {
        put("cover", bit(0, 0));
        put("fan", bit(0, 4));
        put("light_off", bit(0, 6));
        put("phosphor_wheel", bit(1, 1));
        put("driver_version", bit(2, 0));
        put("cpu_communication", bit(2, 1));
        put("temperature_sensor", bit(2, 2));
        put("tec_driver", bit(2, 5));
        put("color_sensor", bit(3, 2));
        put("lens_not_installed", bit(3, 7));
    } else {
        put("cover", bit(0, 0));
        put("temperature_bimetal", bit(0, 1));
        put("fan", bit(0, 3) || bit(0, 4));
        put("power", bit(0, 5));
        put("lamp_off", bit(0, 6));
        put("lamp_replacement_moratorium", bit(0, 7));
        put("lamp_hours_exceeded", bit(1, 0));
        put("formatter", bit(1, 1));
        put("lamp2_off", bit(1, 2));
        put("fpga", bit(2, 1));
        put("temperature_sensor", bit(2, 2));
        put("lamp_missing", bit(2, 3));
        put("lamp_data", bit(2, 4));
        put("mirror_cover", bit(2, 5));
        put("lamp2_replacement_moratorium", bit(2, 6));
        put("lamp2_hours_exceeded", bit(2, 7));
        put("lamp2_missing", bit(3, 0));
        put("lamp2_data", bit(3, 1));
        put("dust_temperature", bit(3, 2));
        put("foreign_matter_sensor", bit(3, 3));
        put("ballast_communication", bit(3, 5));
        put("iris_calibration", bit(3, 6));
        put("lens_not_installed", bit(3, 7));
        // DATA09, the extended status, when DATA02 bit 7 says to read it.
        let extended = bit(1, 7);
        put("portrait_cover_up", extended && bit(8, 0));
        put("interlock_open", extended && bit(8, 1));
        put("system_error_slave_cpu", extended && bit(8, 2));
        put("system_error_formatter", extended && bit(8, 3));
    }
    Value::Object(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn family(id: &str) -> &'static Family {
        FAMILIES.iter().find(|f| f.id == id).unwrap()
    }

    fn module(id: &str, monitor: bool) -> NecProjector {
        NecProjector::with(
            Link::Tcp(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, 40)),
                DEFAULT_PORT,
            )),
            family(id),
            monitor,
        )
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

    fn connected(m: &mut NecProjector) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    /// A reply frame: class + 20h (or A0h), the command, ID1 01h, ID2 the
    /// model code, the data and its checksum.
    fn answer(ok: bool, class: u8, code: u8, data: &[u8]) -> Vec<u8> {
        let mut out = vec![
            if ok { 0x20 } else { 0xA0 } | class,
            code,
            0x01,
            0x10,
            data.len() as u8,
        ];
        out.extend_from_slice(data);
        out.push(checksum(&out));
        out
    }

    fn feed(m: &mut NecProjector, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn run(m: &mut NecProjector, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    /// Answer every queued request "not supported" until none is in flight.
    fn drain(m: &mut NecProjector) {
        while let Some((job, _)) = m.in_flight.clone() {
            feed(m, &answer(false, job.class, job.code, &[0x00, 0x01]));
        }
    }

    #[test]
    fn the_manuals_examples_frame_as_documented() {
        // §2.2: 20h 81h 01h 60h 01h 00h sums to 103h, checksum 03h.
        assert_eq!(checksum(&[0x20, 0x81, 0x01, 0x60, 0x01, 0x00]), 0x03);
        // 018 example: video (06h).
        assert_eq!(
            frame(0x02, 0x03, &[0x01, 0x06]),
            [0x02, 0x03, 0x00, 0x00, 0x02, 0x01, 0x06, 0x0E]
        );
        // 030-1 examples: brightness 10 and -10.
        assert_eq!(
            frame(0x03, 0x10, &[0x00, 0xFF, 0x00, 0x0A, 0x00]),
            [0x03, 0x10, 0x00, 0x00, 0x05, 0x00, 0xFF, 0x00, 0x0A, 0x00, 0x21]
        );
        assert_eq!(
            frame(0x03, 0x10, &[0x00, 0xFF, 0x00, 0xF6, 0xFF]),
            [0x03, 0x10, 0x00, 0x00, 0x05, 0x00, 0xFF, 0x00, 0xF6, 0xFF, 0x0C]
        );
        // 030-2 example: volume 10.
        assert_eq!(
            frame(0x03, 0x10, &[0x05, 0x00, 0x00, 0x0A, 0x00]),
            [0x03, 0x10, 0x00, 0x00, 0x05, 0x05, 0x00, 0x00, 0x0A, 0x00, 0x27]
        );
        // 050 example: AUTO; 037-4 example: lamp usage; 060-1 example.
        assert_eq!(frame(0x02, 0x0F, &[0x05, 0x00]).last(), Some(&0x18));
        assert_eq!(lamp(0, 1).bytes().last(), Some(&0x9C));
        assert_eq!(gain(0).bytes().last(), Some(&0x0B));
        // Fixed commands as printed.
        assert_eq!(errors().bytes(), [0x00, 0x88, 0x00, 0x00, 0x00, 0x88]);
        assert_eq!(information().bytes(), [0x03, 0x8A, 0x00, 0x00, 0x00, 0x8D]);
        assert_eq!(filter_usage().bytes(), [0x03, 0x95, 0x00, 0x00, 0x00, 0x98]);
        assert_eq!(
            lens_status().bytes(),
            [0x02, 0x22, 0x00, 0x00, 0x01, 0x00, 0x25]
        );
        assert_eq!(lens_profile().bytes(), [0x02, 0x28, 0x00, 0x00, 0x00, 0x2A]);
        assert_eq!(
            setting().bytes(),
            [0x00, 0x85, 0x00, 0x00, 0x01, 0x00, 0x86]
        );
        assert_eq!(
            running().bytes(),
            [0x00, 0x85, 0x00, 0x00, 0x01, 0x01, 0x87]
        );
        assert_eq!(
            input_status().bytes(),
            [0x00, 0x85, 0x00, 0x00, 0x01, 0x02, 0x88]
        );
        assert_eq!(
            mute_status().bytes(),
            [0x00, 0x85, 0x00, 0x00, 0x01, 0x03, 0x89]
        );
        assert_eq!(
            model_name().bytes(),
            [0x00, 0x85, 0x00, 0x00, 0x01, 0x04, 0x8A]
        );
        assert_eq!(cover().bytes(), [0x00, 0x85, 0x00, 0x00, 0x01, 0x05, 0x8B]);
        assert_eq!(eco().bytes(), [0x03, 0xB0, 0x00, 0x00, 0x01, 0x07, 0xBB]);
        assert_eq!(
            projector_name().bytes(),
            [0x03, 0xB0, 0x00, 0x00, 0x01, 0x2C, 0xE0]
        );
        assert_eq!(
            mac().bytes(),
            [0x03, 0xB0, 0x00, 0x00, 0x02, 0x9A, 0x00, 0x4F]
        );
        assert_eq!(
            edge_blending().bytes(),
            [0x03, 0xB0, 0x00, 0x00, 0x02, 0xDF, 0x00, 0x94]
        );
        assert_eq!(
            base_model().bytes(),
            [0x00, 0xBF, 0x00, 0x00, 0x01, 0x00, 0xC0]
        );
        assert_eq!(
            serial().bytes(),
            [0x00, 0xBF, 0x00, 0x00, 0x02, 0x01, 0x06, 0xC8]
        );
        assert_eq!(basic().bytes(), [0x00, 0xBF, 0x00, 0x00, 0x01, 0x02, 0xC2]);
    }

    #[test]
    fn commands_send_the_documented_bytes() {
        let cases: &[(&str, Value, &[u8])] = &[
            (
                "set_power",
                json!({"on": true}),
                &[0x02, 0x00, 0x00, 0x00, 0x00, 0x02],
            ),
            (
                "set_power",
                json!({"on": false}),
                &[0x02, 0x01, 0x00, 0x00, 0x00, 0x03],
            ),
            (
                "set_input",
                json!({"input": "hdmi"}),
                &[0x02, 0x03, 0x00, 0x00, 0x02, 0x01, 0xA1, 0xA9],
            ),
            (
                "set_picture_mute",
                json!({"muted": true}),
                &[0x02, 0x10, 0x00, 0x00, 0x00, 0x12],
            ),
            (
                "set_picture_mute",
                json!({"muted": false}),
                &[0x02, 0x11, 0x00, 0x00, 0x00, 0x13],
            ),
            (
                "set_sound_mute",
                json!({"muted": true}),
                &[0x02, 0x12, 0x00, 0x00, 0x00, 0x14],
            ),
            (
                "set_onscreen_mute",
                json!({"muted": false}),
                &[0x02, 0x15, 0x00, 0x00, 0x00, 0x17],
            ),
            (
                "set_shutter",
                json!({"closed": true}),
                &[0x02, 0x16, 0x00, 0x00, 0x00, 0x18],
            ),
            (
                "set_shutter",
                json!({"closed": false}),
                &[0x02, 0x17, 0x00, 0x00, 0x00, 0x19],
            ),
            (
                "set_freeze",
                json!({"frozen": true}),
                &[0x01, 0x98, 0x00, 0x00, 0x01, 0x01, 0x9B],
            ),
            (
                "set_light_adjust",
                json!({"value": -5, "relative": true}),
                &[
                    0x03, 0x10, 0x00, 0x00, 0x05, 0x96, 0xFF, 0x01, 0xFB, 0xFF, 0xA8,
                ],
            ),
            (
                "set_aspect",
                json!({"aspect": "full"}),
                &[
                    0x03, 0x10, 0x00, 0x00, 0x05, 0x18, 0x00, 0x00, 0x06, 0x00, 0x36,
                ],
            ),
            (
                "press_key",
                json!({"key": "hdmi2"}),
                &[0x02, 0x0F, 0x00, 0x00, 0x02, 0x18, 0x01, 0x2C],
            ),
            (
                "drive_lens",
                json!({"target": "zoom", "drive": "plus"}),
                &[0x02, 0x18, 0x00, 0x00, 0x02, 0x00, 0x7F, 0x9B],
            ),
            (
                "set_lens_position",
                json!({"target": "shift_vertical", "position": 300}),
                &[0x02, 0x1D, 0x00, 0x00, 0x04, 0x03, 0x00, 0x2C, 0x01, 0x53],
            ),
            (
                "move_lens",
                json!({"target": "focus", "offset": -1}),
                &[0x02, 0x1D, 0x00, 0x00, 0x04, 0x01, 0x02, 0xFF, 0xFF, 0x24],
            ),
            (
                "stop_lens",
                json!({}),
                &[0x02, 0x1D, 0x00, 0x00, 0x04, 0xFF, 0x00, 0x00, 0x00, 0x22],
            ),
            (
                "lens_memory",
                json!({"operation": "store"}),
                &[0x02, 0x1E, 0x00, 0x00, 0x01, 0x01, 0x22],
            ),
            (
                "reference_lens_memory",
                json!({"operation": "move"}),
                &[0x02, 0x1F, 0x00, 0x00, 0x01, 0x00, 0x22],
            ),
            (
                "set_lens_memory_option",
                json!({"option": "forced_mute", "on": true}),
                &[0x02, 0x21, 0x00, 0x00, 0x02, 0x01, 0x01, 0x27],
            ),
            (
                "set_lens_profile",
                json!({"profile": 2}),
                &[0x02, 0x27, 0x00, 0x00, 0x01, 0x01, 0x2B],
            ),
            (
                "set_eco_mode",
                json!({"mode": "eco1"}),
                &[0x03, 0xB1, 0x00, 0x00, 0x02, 0x07, 0x02, 0xBF],
            ),
            (
                "set_pip_mode",
                json!({"mode": "picture_by_picture"}),
                &[0x03, 0xB1, 0x00, 0x00, 0x03, 0xC5, 0x00, 0x01, 0x7D],
            ),
            (
                "set_pip_sub_input",
                json!({"sub": 1, "input": "hdbaset"}),
                &[0x03, 0xB1, 0x00, 0x00, 0x03, 0xC5, 0x02, 0x08, 0x86],
            ),
            (
                "set_edge_blending",
                json!({"on": true}),
                &[0x03, 0xB1, 0x00, 0x00, 0x03, 0xDF, 0x00, 0x01, 0x97],
            ),
            (
                "set_audio_select",
                json!({"input": "hdmi2", "source": "computer"}),
                &[0x03, 0xC9, 0x00, 0x00, 0x03, 0x09, 0x01, 0x02, 0xDB],
            ),
            (
                "get_lamp_usage",
                json!({"lamp": 1, "content": "remaining_percent"}),
                &[0x03, 0x96, 0x00, 0x00, 0x02, 0x00, 0x04, 0x9F],
            ),
            (
                "get_carbon_savings",
                json!({"scope": "operation"}),
                &[0x03, 0x9A, 0x00, 0x00, 0x01, 0x01, 0x9F],
            ),
            (
                "get_signal_frequency",
                json!({"which": "vertical"}),
                &[0x00, 0xD0, 0x00, 0x00, 0x03, 0x00, 0x04, 0x01, 0xD8],
            ),
            (
                "get_pip",
                json!({"target": "sub_input_2"}),
                &[0x03, 0xB0, 0x00, 0x00, 0x02, 0xC5, 0x09, 0x83],
            ),
        ];
        for (i, (name, params, wire)) in cases.iter().enumerate() {
            let mut m = module("np-pa1004ul", false);
            connected(&mut m);
            drain(&mut m);
            let a = run(&mut m, i as u64 + 1, name, params.clone());
            assert_eq!(sent(&a), [wire.to_vec()], "{name} {params}");
        }
        // The projector name is padded to 16 bytes and NUL-ended.
        let m = module("np-pa1004ul", false);
        let job = m
            .command_job(
                "set_projector_name",
                json!({"name": "Hall"}).as_object().unwrap(),
            )
            .unwrap();
        let bytes = job.bytes();
        assert_eq!(
            &bytes[..10],
            &[0x03, 0xB1, 0x00, 0x00, 0x12, 0x2C, b'H', b'a', b'l', b'l']
        );
        assert_eq!(bytes.len(), 5 + 18 + 1);
        assert_eq!(bytes[22], 0x00);
    }

    #[test]
    fn inputs_and_aspects_follow_the_models_tables() {
        // HDMI is 1Ah on an NP-PA600X and A1h on an NP-PA1004UL.
        let old = module("np-pa600x", true);
        let job = old
            .command_job("set_input", json!({"input": "hdmi"}).as_object().unwrap())
            .unwrap();
        assert_eq!(job.data, [0x01, 0x1A]);
        // A terminal the model lacks is refused before sending.
        let err = old
            .command_job("set_input", json!({"input": "sdi"}).as_object().unwrap())
            .unwrap_err();
        assert!(matches!(err, CommandError::InvalidParams { .. }));
        // 16:9 is 02h, FULL 06h on the PA622U family.
        let pa = module("np-pa622u", true);
        let job = pa
            .command_job("set_aspect", json!({"aspect": "16:9"}).as_object().unwrap())
            .unwrap();
        assert_eq!(job.data, [0x18, 0x00, 0x00, 0x02, 0x00]);
        // A command the family does not support is refused.
        let err = old
            .command_job(
                "drive_lens",
                json!({"target": "zoom", "drive": "stop"})
                    .as_object()
                    .unwrap(),
            )
            .unwrap_err();
        assert!(matches!(err, CommandError::UnsupportedForModel { .. }));
    }

    #[test]
    fn the_models_input_terminals_reach_state() {
        let mut m = module("np4100", true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert_eq!(
            state(&cx.take())["inputs"]["available"],
            json!([
                "computer",
                "computer2",
                "computer3",
                "video",
                "s_video",
                "component"
            ])
        );
        let mut m = module("np4100", false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert_eq!(state(&cx.take()), json!({}));
    }

    #[test]
    fn basic_information_and_errors_reach_state() {
        let mut m = module("np-pa1004ul", true);
        let a = connected(&mut m);
        // Identity first: the model name.
        assert_eq!(sent(&a), [model_name().bytes()]);
        let mut name = b"NP-PA1004UL".to_vec();
        name.resize(32, 0);
        let a = feed(&mut m, &answer(true, 0x00, 0x85, &name));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(state(&a)["info"]["model_name"], "NP-PA1004UL");
        // Answer the rest until the basic information request.
        while let Some((job, _)) = m.in_flight.clone() {
            if job.read == Read::Basic {
                break;
            }
            feed(&mut m, &answer(false, job.class, job.code, &[0x02, 0x0D]));
        }
        // On, HDMI 1, picture muted, frozen.
        let data = [
            0x02, 0x04, 0x00, 0x01, 0x21, 0xFF, 0x01, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0,
        ];
        let a = feed(&mut m, &answer(true, 0x00, 0xBF, &data));
        let s = state(&a);
        assert_eq!(s["power"], "on");
        assert_eq!(s["display"], "video");
        assert_eq!(s["input"]["type"], "hdmi");
        assert_eq!(s["input"]["number"], 1);
        assert_eq!(s["mute"]["picture"], true);
        assert_eq!(s["freeze"], true);
        // Then 078-2 running status, then the errors.
        while let Some((job, _)) = m.in_flight.clone() {
            if job.read == Read::Errors {
                break;
            }
            feed(&mut m, &answer(false, job.class, job.code, &[0x02, 0x0D]));
        }
        let mut e = [0u8; 12];
        e[0] = 0b0100_0001; // cover, lamp off
        e[1] = 0b1000_0000; // extended status follows
        e[8] = 0b0000_0010; // interlock open
        let a = feed(&mut m, &answer(true, 0x00, 0x88, &e));
        let s = state(&a);
        assert_eq!(s["errors"]["cover"], true);
        assert_eq!(s["errors"]["lamp_off"], true);
        assert_eq!(s["errors"]["interlock_open"], true);
        assert_eq!(s["errors"]["fan"], false);
    }

    #[test]
    fn gains_lens_and_lamps_are_read() {
        let m = module("np-pa1004ul", true);
        // 060-1: adjustable, max 100, min -100, default 0 (valid), value 10.
        let d = [
            0x02, 100, 0, 0x9C, 0xFF, 0, 0, 10, 0, 5, 0, 1, 0, 0x01, 0, 0,
        ];
        let (v, patch) = m.interpret(Read::Gain(0x00), &d).unwrap();
        assert_eq!(v["value"], 10);
        assert_eq!(v["min"], -100);
        assert_eq!(patch["picture"]["brightness"]["max"], 100);
        let (_, patch) = m.interpret(Read::Gain(0x05), &d).unwrap();
        assert_eq!(patch["volume"]["value"], 10);
        // 053-1: zoom, max 1000, min 0, position 512.
        let d = [0x00, 0x00, 0xE8, 0x03, 0x00, 0x00, 0x00, 0x02];
        let (_, patch) = m.interpret(Read::LensPosition(0x00), &d).unwrap();
        assert_eq!(patch["lens"]["zoom"]["position"], 512);
        assert_eq!(patch["lens"]["zoom"]["max"], 1000);
        // 037-4 example: 18000 s, 5 hours.
        let d = [0x00, 0x01, 0x50, 0x46, 0x00, 0x00];
        let (_, patch) = m
            .interpret(
                Read::Lamp {
                    lamp: 0,
                    content: 1,
                },
                &d,
            )
            .unwrap();
        assert_eq!(patch["lamps"]["1"]["usage_seconds"], 18000);
        assert_eq!(patch["lamps"]["1"]["usage_hours"], 5);
        // 037-6: 2460 kg and 63750 mg.
        let d = [0x00, 0x9C, 0x09, 0x00, 0x00, 0x06, 0xF9, 0x00, 0x00];
        let (v, _) = m.interpret(Read::Carbon(0), &d).unwrap();
        assert!((v.as_f64().unwrap() - 2460.06375).abs() < 1e-9);
        // 097-155 example MAC.
        let d = [0x9A, 0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB];
        let (v, _) = m.interpret(Read::Mac, &d).unwrap();
        assert_eq!(v, "01:23:45:67:89:ab");
        // 097-8: eco mode by the family's table.
        let (v, _) = m.interpret(Read::Eco, &[0x07, 0x05]).unwrap();
        assert_eq!(v, "boost");
    }

    #[test]
    fn an_error_answer_fails_the_command_with_its_meaning() {
        let mut m = module("np-pa1004ul", false);
        connected(&mut m);
        drain(&mut m);
        let a = run(&mut m, 9, "set_input", json!({"input": "hdbaset"}));
        assert_eq!(sent(&a).len(), 1);
        let a = feed(&mut m, &answer(false, 0x02, 0x03, &[0x01, 0x01]));
        match completed(&a, 9) {
            Some(Err(CommandError::DeviceError { code, message })) => {
                assert_eq!(code.as_deref(), Some("0101"));
                assert!(message.contains("input terminal is invalid"), "{message}");
            }
            other => panic!("{other:?}"),
        }
        // An execution result of FFh fails too.
        run(&mut m, 10, "set_input", json!({"input": "hdmi"}));
        let a = feed(&mut m, &answer(true, 0x02, 0x03, &[0xFF]));
        assert!(matches!(
            completed(&a, 10),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn a_reply_with_a_bad_checksum_or_split_in_pieces_is_handled() {
        let mut m = module("np-pa1004ul", false);
        connected(&mut m);
        drain(&mut m);
        run(&mut m, 4, "set_power", json!({"on": true}));
        let mut good = answer(true, 0x02, 0x00, &[]);
        let mut bad = good.clone();
        *bad.last_mut().unwrap() ^= 0xFF;
        let a = feed(&mut m, &bad);
        assert!(completed(&a, 4).is_none());
        // Split across two reads, after noise.
        let mut first = vec![0x55];
        first.extend(good.drain(..3));
        feed(&mut m, &first);
        let a = feed(&mut m, &good);
        assert_eq!(completed(&a, 4), Some(Ok(Outcome::Ack)));
    }

    #[test]
    fn opened_for_commands_only_it_sends_one_status_request_and_commands_still_work() {
        let mut m = module("np-pa1004ul", false);
        let a = connected(&mut m);
        // No identity, no polls: the liveness request only.
        assert_eq!(sent(&a), [basic().bytes()]);
        let a = feed(&mut m, &answer(true, 0x00, 0xBF, &[0x02; 16]));
        assert!(sent(&a).is_empty());
        let a = run(&mut m, 5, "set_power", json!({"on": false}));
        assert_eq!(sent(&a), [vec![0x02, 0x01, 0x00, 0x00, 0x00, 0x03]]);
        let a = feed(&mut m, &answer(true, 0x02, 0x01, &[]));
        assert_eq!(completed(&a, 5), Some(Ok(Outcome::Ack)));
        // No read-back follows a set.
        assert!(sent(&a).is_empty());
        // The next poll is the liveness request again.
        let mut cx = Cx::new(5_000);
        m.timer(&mut cx, POLL);
        assert_eq!(sent(&cx.take()), [basic().bytes()]);
    }

    #[test]
    fn a_round_trip_is_reported() {
        let mut m = module("np-pa1004ul", false);
        connected(&mut m);
        let mut cx = Cx::new(37);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(answer(true, 0x00, 0xBF, &[0x02; 16])),
        );
        assert!(cx.take().contains(&Action::RoundTrip(37)));
    }

    #[test]
    fn a_set_is_read_back_when_monitoring() {
        let mut m = module("np-pa1004ul", true);
        connected(&mut m);
        drain(&mut m);
        m.unsupported.clear();
        run(
            &mut m,
            3,
            "set_volume",
            json!({"value": 12, "relative": false}),
        );
        let a = feed(&mut m, &answer(true, 0x03, 0x10, &[0x00, 0x00]));
        assert_eq!(completed(&a, 3), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a), [gain(0x05).bytes()]);
    }

    #[test]
    fn unsupported_queries_are_not_asked_again() {
        let mut m = module("np-pa1004ul", true);
        connected(&mut m);
        // Every request answered "not supported".
        drain(&mut m);
        assert!(m.unsupported.contains(&("078-5", vec![0x04])));
        // The next cycle asks only the liveness request, which is kept.
        let mut cx = Cx::new(5_000);
        m.timer(&mut cx, POLL);
        assert_eq!(sent(&cx.take()), [basic().bytes()]);
        feed(&mut m, &answer(false, 0x00, 0xBF, &[0x00, 0x01]));
        assert!(m.in_flight.is_none());
    }

    #[test]
    fn three_unanswered_requests_reopen_the_link() {
        let mut m = module("np-pa1004ul", false);
        connected(&mut m);
        let mut cx = Cx::new(0);
        for _ in 0..3 {
            if m.in_flight.is_none() {
                m.timer(&mut cx, POLL);
            }
            m.timer(&mut cx, REPLY);
        }
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert_eq!(m.phase, Phase::Closed);
    }

    #[test]
    fn every_family_names_known_commands_and_unique_ids() {
        let numbers = [
            "009",
            "015",
            "016",
            "018",
            "020",
            "021",
            "022",
            "023",
            "024",
            "025",
            "030-1",
            "030-2",
            "030-12",
            "030-15",
            "037",
            "037-3",
            "037-4",
            "037-6",
            "050",
            "051",
            "052",
            "053",
            "053-1",
            "053-2",
            "053-3",
            "053-4",
            "053-5",
            "053-6",
            "053-7",
            "053-10",
            "053-11",
            "060-1",
            "078-1",
            "078-2",
            "078-3",
            "078-4",
            "078-5",
            "078-6",
            "079",
            "084",
            "097-8",
            "097-45",
            "097-155",
            "097-198",
            "097-243-1",
            "098-8",
            "098-45",
            "098-198",
            "098-243-1",
            "305-1",
            "305-2",
            "305-3",
            "319-10",
        ];
        let mut ids = HashSet::new();
        for f in FAMILIES {
            assert!(ids.insert(f.id), "{} twice", f.id);
            for n in f.commands {
                assert!(numbers.contains(n), "{}: {n}", f.id);
            }
        }
    }
}
