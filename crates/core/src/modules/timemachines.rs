//! TimeMachines network clocks, timers and time servers: the Locator
//! Protocol API over UDP 7372.
//!
//! Written from TimeMachines' "Locator Protocol API version 2.3" (July 2025);
//! section numbers below are that document's. See the spec's sources.
//!
//! - Every request is one datagram to port 7372 starting with a command byte
//!   (A1h to BEh). A clock answers each control request with the single
//!   character 'A' (§1.3.9), and the locator query A1 04 B2 with a 40-byte
//!   status (clocks, §1.1.1) or an 80-byte one (TM1000A and TM2000A time
//!   servers, §1.1.2). Get Program and Get Active Program answer with the
//!   program's bytes.
//! - Requests go one at a time, each waiting for its answer, so an 'A' is
//!   never taken for another request's.
//! - Nothing is pushed: the locator status (mode, running, the displayed time
//!   or timer value, alarm, NTP sync count) is read every `poll_ms`. A control
//!   request is followed by a status read, so state follows it at once.
//!
//! Opened for commands only (`monitor` false), nothing is read beyond what
//! commands ask, except the status every 10 s while idle, the only sign the
//! clock is there. Every answered request reports its round trip.

use std::collections::VecDeque;
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext,
    Outcome, PollStamps,
};

const SOCKET: Key = "locator";
const REPLY_TIMER: Key = "reply";
const POLL_TIMER: Key = "poll";
const PROBE_TIMER: Key = "probe";

pub(crate) const DEFAULT_PORT: u16 = 7372;

const REPLY_TIMEOUT: Millis = 1_500;
const MISSES: u32 = 3;
const PROBE_EVERY: Millis = 10_000;

/// The locator query (§1.1.1, §1.1.2).
const QUERY: [u8; 3] = [0xA1, 0x04, 0xB2];
const ACK: u8 = b'A';

/// What a request's answer is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Answer {
    /// The 'A' acknowledge.
    Ack,
    /// The locator status.
    Status,
    /// A program's bytes, returned as hexadecimal text.
    Program,
}

#[derive(Debug)]
struct Job {
    id: Option<CommandId>,
    data: Vec<u8>,
    answer: Answer,
    /// Read the status once the request is answered.
    read_back: bool,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// A null-terminated ASCII string from `bytes`.
fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn ip(bytes: &[u8]) -> String {
    format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
}

fn mac(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

const DEVICE_TYPES: &[(u8, &str)] = &[
    (0x01, "poe_display"),
    (0x02, "wifi_display"),
    (0x03, "dot_matrix"),
    (0x04, "tm1000a"),
    (0x05, "tm2000a"),
];

const MODES: &[&str] = &[
    "time",
    "up_timer",
    "down_timer",
    "interval_count_up",
    "interval_count_down",
];

/// The locator status as state (§1.1.1 for clocks, §1.1.2 for servers).
fn decode_status(d: &[u8]) -> Option<Value> {
    let kind = *d.first()?;
    let device_type = DEVICE_TYPES
        .iter()
        .find(|(c, _)| *c == kind)
        .map_or_else(|| format!("unknown_{kind:02x}"), |(_, n)| (*n).to_string());
    let mut info = Map::new();
    info.insert("device_type".into(), json!(device_type));
    info.insert("ip_address".into(), json!(ip(d.get(1..5)?)));
    info.insert("mac_address".into(), json!(mac(d.get(5..11)?)));
    info.insert(
        "firmware".into(),
        json!(format!("{}.{}", d.get(11)?, d.get(12)?)),
    );
    if kind == 0x04 || kind == 0x05 {
        if d.len() < 80 {
            return None;
        }
        let lock = match d[13] {
            0 => "no_lock",
            1 => "2d",
            2 => "3d",
            _ => "unknown",
        };
        let sync = u32::from_be_bytes([d[14], d[15], d[16], d[17]]);
        return Some(json!({
            "info": info,
            "gps_lock": lock,
            "ntp_sync_count": sync,
            "utc_time": format!("{:02}:{:02}:{:02}", d[18], d[19], d[20]),
            "location": cstr(&d[21..46]),
            "time_server_name": cstr(&d[46..80]),
        }));
    }
    if d.len() < 40 {
        return None;
    }
    let mode_byte = d[19];
    let mode = MODES
        .get(usize::from(mode_byte & 0x07))
        .copied()
        .unwrap_or("unknown");
    let days = u16::from(d[21]) | (u16::from(d[22] >> 5) << 8);
    let digits = match d[22] & 0x1F {
        0 => "4_6_digits",
        1 => "d_h_m_s",
        2 => "h_m_s_tenths",
        _ => "unknown",
    };
    let format = if mode_byte & 0x20 != 0 {
        "d_h_m"
    } else if mode_byte & 0x80 != 0 {
        "m_s_tenths"
    } else {
        "h_m_s"
    };
    info.insert("name".into(), json!(cstr(&d[24..40])));
    info.insert("wifi_signal_dbm".into(), json!(-i64::from(d[23])));
    Some(json!({
        "info": info,
        "ntp_sync_count": u16::from_be_bytes([d[13], d[14]]),
        "display": {
            "time": format!("{:02}:{:02}:{:02}", d[15], d[16], d[17]),
            "tenths": d[18],
            "days": days,
            "mode": mode,
            "format": format,
            "running": mode_byte & 0x40 != 0,
            "digits": digits,
        },
        "alarm": {
            "seconds": d[20] & 0x7F,
            "enabled": d[20] & 0x80 != 0,
        },
    }))
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

fn byte(params: &Params, name: &str, min: i64, max: i64) -> Result<u8, CommandError> {
    Ok(int(params, name, min, max)? as u8)
}

fn flag(params: &Params, name: &str) -> Result<u8, CommandError> {
    params
        .get(name)
        .and_then(Value::as_bool)
        .map(u8::from)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

fn text<'a>(params: &'a Params, name: &str) -> Result<&'a str, CommandError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(format!("'{name}' is required")))
}

/// A timer display format byte: 00h MIN:SEC.Tenths, 01h HH:MM:SS.
fn timer_format(params: &Params) -> Result<u8, CommandError> {
    match text(params, "format")? {
        "m_s_tenths" => Ok(0x00),
        "h_m_s" => Ok(0x01),
        other => Err(bad(format!("'{other}' is not a timer format"))),
    }
}

/// ASCII text with its terminating null, refusing what the clock cannot show.
fn ascii_z(params: &Params, name: &str, max: usize) -> Result<Vec<u8>, CommandError> {
    let t = text(params, name)?;
    if !t.is_ascii() || t.bytes().any(|b| b == 0) {
        return Err(bad(format!("'{name}' must be ASCII text")));
    }
    if t.len() > max {
        return Err(bad(format!("'{name}' is longer than {max} characters")));
    }
    let mut out = t.as_bytes().to_vec();
    out.push(0);
    Ok(out)
}

/// A program name: exactly 12 bytes, null terminated (§1.3.4).
fn program_name(params: &Params) -> Result<Vec<u8>, CommandError> {
    let mut name = ascii_z(params, "name", 11)?;
    name.resize(12, 0);
    Ok(name)
}

/// The ten 32-byte program steps (§1.3.1), given as hexadecimal text.
fn program_steps(params: &Params) -> Result<Vec<u8>, CommandError> {
    let steps =
        from_hex(text(params, "steps")?).ok_or_else(|| bad("'steps' must be hexadecimal bytes"))?;
    if steps.len() > 320 || !steps.len().is_multiple_of(32) {
        return Err(bad(
            "'steps' must be whole 32-byte steps, at most ten (640 hex digits)",
        ));
    }
    let mut out = steps;
    out.resize(320, 0);
    Ok(out)
}

/// The countdown bytes shared by Use DownTimer and DownTimer Reset (§1.2.4).
fn countdown(params: &Params) -> Result<Vec<u8>, CommandError> {
    let format = timer_format(params)? | (flag(params, "bargraph")? << 7);
    let days = int(params, "days", 0, 65_535)? as u16;
    let mut out = vec![
        format,
        byte(params, "hours", 0, 23)?,
        byte(params, "minutes", 0, 59)?,
        byte(params, "seconds", 0, 59)?,
        byte(params, "tenths", 0, 9)?,
        flag(params, "alarm")?,
        byte(params, "alarm_seconds", 0, 255)?,
    ];
    out.extend(days.to_le_bytes());
    Ok(out)
}

/// A new timer value (§1.2.11, §1.2.12).
fn timer_value(params: &Params) -> Result<Vec<u8>, CommandError> {
    let days = int(params, "days", 0, 65_535)? as u16;
    let mut out = vec![
        byte(params, "hours", 0, 23)?,
        byte(params, "minutes", 0, 59)?,
        byte(params, "seconds", 0, 59)?,
        byte(params, "tenths", 0, 9)?,
        byte(params, "hundredths", 0, 9)?,
    ];
    out.extend(days.to_le_bytes());
    Ok(out)
}

/// What a command sends and how it is answered; `true` reads the status back.
fn plan(name: &str, params: &Params) -> Result<(Vec<u8>, Answer, bool), CommandError> {
    let ack = |mut head: Vec<u8>, rest: Vec<u8>| {
        head.extend(rest);
        Ok((head, Answer::Ack, true))
    };
    let run = |params: &Params| flag(params, "running");
    match name {
        "get_status" => Ok((QUERY.to_vec(), Answer::Status, false)),
        "use_up_timer" => ack(vec![0xA2, timer_format(params)?, 0x00], vec![]),
        "set_up_timer_running" => ack(vec![0xA3, run(params)?, 0x00], vec![]),
        "reset_up_timer" => ack(vec![0xA4, timer_format(params)?, 0x00], vec![]),
        "use_down_timer" => ack(vec![0xA5], countdown(params)?),
        "set_down_timer_running" => ack(vec![0xA6, run(params)?, 0x00], vec![]),
        "reset_down_timer" => ack(vec![0xA7], countdown(params)?),
        "time_mode" => ack(vec![0xA8, 0x01, 0x00], vec![]),
        "set_matrix_text" => {
            let scroll = match text(params, "scroll")? {
                "none" => 0,
                "right_to_left" => 1,
                "bottom_to_top" => 2,
                other => return Err(bad(format!("'{other}' is not a scroll direction"))),
            };
            let justify = justification(params)?;
            ack(
                vec![
                    0xA9,
                    0x1B,
                    (scroll << 2) | justify,
                    byte(params, "speed", 0, 255)?,
                ],
                ascii_z(params, "text", 249)?,
            )
        }
        "set_line_text" => {
            let scroll = match text(params, "scroll")? {
                "none" => 0,
                "right_to_left" => 1,
                "bottom_to_top" => 2,
                other => return Err(bad(format!("'{other}' is not a scroll direction"))),
            };
            let attr = (flag(params, "blink")? << 4) | (scroll << 2) | justification(params)?;
            let font = match text(params, "font")? {
                "standard" => 1,
                "bold" => 4,
                other => return Err(bad(format!("'{other}' is not a font"))),
            };
            let t = text(params, "text")?.replace("\\n", "\n");
            let mut p = params.clone();
            p.insert("text".into(), json!(t));
            ack(
                vec![
                    0xA9,
                    0x00,
                    attr,
                    byte(params, "speed", 0, 255)?,
                    font,
                    byte(params, "color", 0, 9)?,
                ],
                ascii_z_multiline(&p)?,
            )
        }
        "release_line_text" => ack(vec![0xBD, 0x01, 0x00], vec![]),
        "set_up_timer_value" => ack(vec![0xAA], timer_value(params)?),
        "set_down_timer_value" => ack(vec![0xAB], timer_value(params)?),
        "reset_countdown_to_date" => {
            let format = match text(params, "format")? {
                "h_m_s_tenths" => 0x00,
                "d_h_m_s" => 0x01,
                "d_h_m" => 0x02,
                other => return Err(bad(format!("'{other}' is not a date timer format"))),
            };
            let year = int(params, "year", 2000, 2255)? as u16;
            let mut data = vec![0xB9, format];
            data.extend(year.to_le_bytes());
            data.extend([
                byte(params, "month", 1, 12)?,
                byte(params, "day", 1, 31)?,
                byte(params, "hour", 0, 23)?,
                byte(params, "minute", 0, 59)?,
                byte(params, "second", 0, 59)?,
                flag(params, "alarm")?,
                byte(params, "alarm_seconds", 0, 255)?,
            ]);
            ack(data, vec![])
        }
        "set_countdown_to_date_running" => ack(vec![0xBA, run(params)?, 0x00], vec![]),
        "get_program" => Ok((
            vec![0xB2, byte(params, "program", 0, 9)?],
            Answer::Program,
            false,
        )),
        "store_program" => {
            let mut data = vec![0xB3, byte(params, "program", 0, 9)?];
            data.extend(program_name(params)?);
            data.extend(program_steps(params)?);
            Ok((data, Answer::Ack, false))
        }
        "get_active_program" => Ok((vec![0xAF], Answer::Program, false)),
        "load_program" => {
            let mut data = vec![0xAC, 0x4F, byte(params, "program", 0, 9)?];
            data.extend(program_name(params)?);
            data.extend(program_steps(params)?);
            ack(data, vec![])
        }
        "set_program_running" => ack(vec![0xAE, run(params)?], vec![]),
        "execute_program" => ack(vec![0xB8, byte(params, "program", 0, 9)?], vec![]),
        "pulse_relay" => ack(vec![0xB4, byte(params, "seconds", 1, 255)?, 0x00], vec![]),
        "set_relay" => ack(vec![0xBC, flag(params, "closed")?, 0x00], vec![]),
        "set_revert_timeout" => {
            let minutes = int(params, "minutes", 0, 65_535)? as u16;
            let mut data = vec![0xBB];
            data.extend(minutes.to_le_bytes());
            ack(data, vec![])
        }
        "set_dimmer" => ack(
            vec![
                0xB5,
                byte(params, "digits", 0, 100)?,
                byte(params, "dots", 0, 100)?,
            ],
            vec![],
        ),
        "set_variable" => ack(
            vec![0xBE, byte(params, "variable", 0, 4)?],
            ascii_z(params, "text", 100)?,
        ),
        "set_colors" => {
            let rgb = |key: &str| -> Result<[u8; 3], CommandError> {
                let t = text(params, key)?.trim_start_matches('#');
                let b = from_hex(t)
                    .filter(|b| b.len() == 3)
                    .ok_or_else(|| bad(format!("'{key}' must be RRGGBB")))?;
                Ok([b[0], b[1], b[2]])
            };
            let mut data = vec![0xB6];
            data.extend(rgb("minutes_seconds")?);
            data.extend(rgb("hours")?);
            ack(data, vec![])
        }
        other => Err(CommandError::UnknownCommand {
            command: other.into(),
        }),
    }
}

fn justification(params: &Params) -> Result<u8, CommandError> {
    match text(params, "justify")? {
        "left" => Ok(1),
        "center" => Ok(2),
        "right" => Ok(3),
        other => Err(bad(format!("'{other}' is not a justification"))),
    }
}

/// Text that may hold line breaks (§1.2.9 lines), null terminated.
fn ascii_z_multiline(params: &Params) -> Result<Vec<u8>, CommandError> {
    let t = text(params, "text")?;
    if !t.is_ascii() || t.bytes().any(|b| b == 0 || (b < 0x20 && b != b'\n')) {
        return Err(bad("'text' must be ASCII text"));
    }
    if t.len() > 249 {
        return Err(bad("'text' is longer than 249 characters"));
    }
    let mut out = t.as_bytes().to_vec();
    out.push(0);
    Ok(out)
}

pub(crate) struct TimeMachines {
    to: SocketAddr,
    monitor: bool,
    /// When each poll was last answered (`polled.<group>`, SPEC.md §8).
    stamps: PollStamps,
    poll_ms: Millis,
    reachable: bool,
    misses: u32,
    queue: VecDeque<Job>,
    in_flight: Option<(Job, Millis)>,
}

impl TimeMachines {
    pub(crate) fn new(ctx: OpenContext) -> Result<TimeMachines, String> {
        if ctx.host.is_unspecified() {
            return Err("give the clock's address as host".into());
        }
        let poll_ms = ctx
            .settings
            .get("poll_ms")
            .and_then(Value::as_u64)
            .unwrap_or(1_000)
            .max(250);
        Ok(TimeMachines {
            to: SocketAddr::new(ctx.host, ctx.port.unwrap_or(DEFAULT_PORT)),
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            poll_ms,
            reachable: false,
            misses: 0,
            queue: VecDeque::new(),
            in_flight: None,
        })
    }

    fn queue_status(&mut self) {
        let pending = |j: &Job| j.id.is_none() && j.answer == Answer::Status;
        if self.queue.iter().any(pending)
            || self.in_flight.as_ref().is_some_and(|(j, _)| pending(j))
        {
            return;
        }
        self.queue.push_back(Job {
            id: None,
            data: QUERY.to_vec(),
            answer: Answer::Status,
            read_back: false,
        });
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() {
            return;
        }
        let Some(job) = self.queue.pop_front() else {
            return;
        };
        cx.udp_send(SOCKET, self.to, job.data.clone());
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        cx.set_timer(PROBE_TIMER, PROBE_EVERY);
        self.in_flight = Some((job, cx.now()));
    }

    fn answered(&mut self, cx: &mut Cx, data: &[u8]) {
        let Some((job, _)) = &self.in_flight else {
            return;
        };
        let fits = match job.answer {
            Answer::Ack => data == [ACK],
            Answer::Status => data.len() >= 40,
            Answer::Program => data != [ACK],
        };
        if !fits {
            return;
        }
        let (job, sent_at) = self.in_flight.take().unwrap();
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.misses = 0;
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
        let outcome = match job.answer {
            Answer::Ack => Outcome::Ack,
            Answer::Status => match decode_status(data) {
                Some(state) => {
                    cx.state(state.clone());
                    if job.id.is_none() && self.monitor {
                        self.stamps.answered(cx, "status");
                    }
                    Outcome::Value { value: state }
                }
                None => Outcome::Value { value: Value::Null },
            },
            Answer::Program => Outcome::Value {
                value: json!(hex(data)),
            },
        };
        if let Some(id) = job.id {
            cx.complete(id, Ok(outcome));
        }
        if job.read_back {
            self.queue_status();
        }
        self.pump(cx);
    }
}

impl Module for TimeMachines {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.udp_open(SOCKET, Bind::Ephemeral);
        self.queue_status();
        if self.monitor {
            cx.set_timer(POLL_TIMER, self.poll_ms);
        }
        cx.set_timer(PROBE_TIMER, PROBE_EVERY);
        self.pump(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match plan(name, params) {
            Ok((data, answer, read_back)) => {
                let job = Job {
                    id: Some(id),
                    data,
                    answer,
                    read_back,
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

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        if from.ip() != self.to.ip() {
            return;
        }
        self.answered(cx, data);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            REPLY_TIMER => {
                if let Some((job, _)) = self.in_flight.take() {
                    if let Some(id) = job.id {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES && self.reachable {
                        self.reachable = false;
                        cx.log(Level::Warning, "the clock answered none of three requests");
                        cx.connection(Connection::Disconnected {
                            reason: "no reply from the clock".into(),
                        });
                    }
                }
                self.pump(cx);
            }
            POLL_TIMER if self.monitor => {
                self.queue_status();
                self.pump(cx);
                cx.set_timer(POLL_TIMER, self.poll_ms);
            }
            PROBE_TIMER => {
                if self.in_flight.is_none() && self.queue.is_empty() {
                    self.queue_status();
                    self.pump(cx);
                }
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.udp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use std::net::{IpAddr, Ipv4Addr};

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 40));

    fn module(monitor: bool) -> TimeMachines {
        let spec = Catalog::source_tree().devices["timemachines-clock"].clone();
        let settings = validate(&spec.settings, &Params::new()).unwrap();
        TimeMachines::new(OpenContext {
            host: HOST,
            host_name: None,
            port: None,
            model: "poe-display".into(),
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
                Action::UdpSend { data, to, .. } => {
                    assert_eq!(to.port(), DEFAULT_PORT);
                    Some(data.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn reply(m: &mut TimeMachines, now: Millis, data: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.datagram(&mut cx, SOCKET, SocketAddr::new(HOST, DEFAULT_PORT), data);
        cx.take()
    }

    fn run(m: &mut TimeMachines, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    /// A POE clock's status: an up timer at 00:01:30.5, running, HH:MM:SS.
    fn clock_status() -> Vec<u8> {
        let mut d = vec![
            0x01, 192, 0, 2, 40, 0x00, 0x1B, 0x2C, 0x01, 0x02, 0x03, 5, 4,
        ];
        d.extend([0x00, 0x2A, 0, 1, 30, 5, 0x41, 0x85, 0x02, 0x00, 0x00]);
        let mut name = b"Studio A".to_vec();
        name.resize(16, 0);
        d.extend(name);
        assert_eq!(d.len(), 40);
        d
    }

    #[test]
    fn requests_are_the_documents_bytes() {
        let p = |name: &str, v: Value| plan(name, v.as_object().unwrap()).unwrap().0;
        assert_eq!(p("get_status", json!({})), QUERY);
        assert_eq!(
            p("use_up_timer", json!({"format": "h_m_s"})),
            [0xA2, 0x01, 0x00]
        );
        assert_eq!(
            p("set_up_timer_running", json!({"running": true})),
            [0xA3, 0x01, 0x00]
        );
        assert_eq!(
            p(
                "use_down_timer",
                json!({"format": "m_s_tenths", "bargraph": true, "days": 0, "hours": 0,
                                        "minutes": 5, "seconds": 0, "tenths": 0, "alarm": true,
                                        "alarm_seconds": 3})
            ),
            [0xA5, 0x80, 0, 5, 0, 0, 1, 3, 0, 0]
        );
        assert_eq!(p("time_mode", json!({})), [0xA8, 0x01, 0x00]);
        assert_eq!(
            p(
                "set_matrix_text",
                json!({"text": "ON AIR", "scroll": "right_to_left",
                                         "justify": "center", "speed": 100})
            ),
            [&[0xA9, 0x1B, 0x06, 100][..], b"ON AIR\0"].concat()
        );
        assert_eq!(
            p(
                "set_line_text",
                json!({"text": "A\\nB", "scroll": "none", "justify": "left",
                                       "speed": 60, "blink": true, "font": "bold", "color": 2})
            ),
            [&[0xA9, 0x00, 0x11, 60, 4, 2][..], b"A\nB\0"].concat()
        );
        assert_eq!(
            p(
                "reset_countdown_to_date",
                json!({"format": "d_h_m", "year": 2026, "month": 12,
                "day": 31, "hour": 23, "minute": 59, "second": 0, "alarm": false, "alarm_seconds": 0})
            ),
            [0xB9, 0x02, 0xEA, 0x07, 12, 31, 23, 59, 0, 0, 0]
        );
        assert_eq!(p("execute_program", json!({"program": 6})), [0xB8, 0x06]);
        assert_eq!(
            p("set_revert_timeout", json!({"minutes": 300})),
            [0xBB, 0x2C, 0x01]
        );
        assert_eq!(
            p("set_dimmer", json!({"digits": 80, "dots": 40})),
            [0xB5, 80, 40]
        );
        assert_eq!(
            p(
                "set_colors",
                json!({"minutes_seconds": "#00FF00", "hours": "ff0000"})
            ),
            [0xB6, 0, 255, 0, 255, 0, 0]
        );
        let stored = p(
            "store_program",
            json!({"program": 6, "name": "ExampleProg",
                                               "steps": format!("06{}", "00".repeat(31))}),
        );
        // §1.3.9: B3 06, the 12-byte name, ten 32-byte steps: 334 bytes.
        assert_eq!(stored.len(), 334);
        assert_eq!(&stored[..14], b"\xB3\x06ExampleProg\0");
        assert_eq!(stored[14], 0x06);
        assert!(plan(
            "store_program",
            json!({"program": 6, "name": "x", "steps": "060000"})
                .as_object()
                .unwrap()
        )
        .is_err());
    }

    #[test]
    fn the_status_is_read_into_state() {
        let state = decode_status(&clock_status()).unwrap();
        assert_eq!(state["info"]["device_type"], json!("poe_display"));
        assert_eq!(state["info"]["mac_address"], json!("00:1B:2C:01:02:03"));
        assert_eq!(state["info"]["firmware"], json!("5.4"));
        assert_eq!(state["info"]["name"], json!("Studio A"));
        assert_eq!(state["ntp_sync_count"], json!(42));
        assert_eq!(state["display"]["time"], json!("00:01:30"));
        assert_eq!(state["display"]["mode"], json!("up_timer"));
        assert_eq!(state["display"]["running"], json!(true));
        assert_eq!(state["display"]["format"], json!("h_m_s"));
        assert_eq!(state["alarm"], json!({"seconds": 5, "enabled": true}));
        let mut server = vec![
            0x05, 10, 0, 0, 9, 1, 2, 3, 4, 5, 6, 0, 6, 2, 0, 0, 1, 0, 12, 0, 0,
        ];
        server.extend({
            let mut l = b"N40.0 W105.0".to_vec();
            l.resize(25, 0);
            l
        });
        server.extend({
            let mut n = b"TM2000A".to_vec();
            n.resize(34, 0);
            n
        });
        let state = decode_status(&server).unwrap();
        assert_eq!(state["info"]["device_type"], json!("tm2000a"));
        assert_eq!(state["gps_lock"], json!("3d"));
        assert_eq!(state["ntp_sync_count"], json!(256));
        assert_eq!(state["utc_time"], json!("12:00:00"));
        assert_eq!(state["time_server_name"], json!("TM2000A"));
    }

    #[test]
    fn a_command_is_acknowledged_then_read_back() {
        let mut m = module(true);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert_eq!(sent(&a), [QUERY.to_vec()]);
        let a = reply(&mut m, 3, &clock_status());
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(3)));
        let a = run(&mut m, 1, "time_mode", json!({}));
        assert_eq!(sent(&a), [vec![0xA8, 0x01, 0x00]]);
        // A status that arrives late is not the acknowledge.
        let a = reply(&mut m, 4, &clock_status());
        assert!(completed(&a, 1).is_none());
        let a = reply(&mut m, 5, b"A");
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a), [QUERY.to_vec()]);
    }

    #[test]
    fn opened_for_commands_only_it_polls_only_while_idle() {
        let mut m = module(false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(!a.iter().any(|x| matches!(
            x,
            Action::SetTimer {
                key: POLL_TIMER,
                ..
            }
        )));
        reply(&mut m, 1, &clock_status());
        let a = run(&mut m, 2, "get_program", json!({"program": 0}));
        assert_eq!(sent(&a), [vec![0xB2, 0x00]]);
        let a = reply(&mut m, 2, &[0x06, 0x00, 0x00, 0x00]);
        assert_eq!(
            completed(&a, 2),
            Some(Ok(Outcome::Value {
                value: json!("06000000")
            }))
        );
    }

    #[test]
    fn every_command_of_the_spec_is_planned() {
        let spec = Catalog::source_tree().devices["timemachines-clock"].clone();
        for (name, command) in &spec.commands {
            let mut params = Params::new();
            for (p, decl) in &command.params {
                let value = match (&decl.values, decl.kind) {
                    (Some(values), _) => json!(values[0]),
                    (None, crate::catalog::ParamType::Bool) => json!(false),
                    (None, crate::catalog::ParamType::String) if p == "steps" => {
                        json!("00".repeat(32))
                    }
                    (None, crate::catalog::ParamType::String)
                        if p.contains("seconds") || p == "hours" =>
                    {
                        json!("#102030")
                    }
                    (None, crate::catalog::ParamType::String) => json!("Text"),
                    _ => json!(decl.min.unwrap_or(0.0) as i64),
                };
                params.insert(p.clone(), value);
            }
            let planned = plan(name, &params);
            assert!(planned.is_ok(), "{name}: {planned:?}");
        }
    }

    #[test]
    fn unanswered_requests_report_the_clock_gone() {
        let mut m = module(false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        reply(&mut m, 1, &clock_status());
        let mut last = Vec::new();
        for id in 1..=3 {
            run(&mut m, id, "time_mode", json!({}));
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
