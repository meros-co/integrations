//! Obsidian ONYX and ONYX Manager over Telnet (TCP, CRLF lines).
//!
//! Commands are text lines (`GQL 14`, `GTQ 14,4.1`, `QLActive`). Replies take
//! one of two forms, as ONYX Manager's server sends them (see the spec's
//! sources):
//!
//! - continuation lines `NNN-text`, ended by a line `NNN` or `NNN text`: the
//!   connection banner and errors (`400-I never heard that command before...`);
//! - a status line `NNN text` (`200 Ok`), data lines, and a line holding only
//!   `.`. A status line that no `.` follows is taken as complete after
//!   [`QUIET`] without more lines.
//!
//! One request is in flight at a time, so each reply is matched to its
//! request, which is what lets QLList and QLActive (whose data lines look the
//! same) keep cuelist names and active flags. Nothing is sent until the banner
//! has ended or [`BANNER_WAIT`] has passed. A query the server refuses with a
//! 4xx is not polled again on that connection, so ONYX's own server, which has
//! fewer commands than ONYX Manager's, is not asked the same thing every 2 s.
//!
//! Opened for commands only, it polls no state: it asks `WhoIAm` every 10 s
//! as the liveness check (any reply, success or refusal, proves the server is
//! there). Replies to commands that carry state (a QLList asked for by the
//! host) still update it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Millis, Module, OpenContext, Outcome, TcpInput,
};

const PORT: u16 = 2323;
const SOCKET: Key = "onyx";

/// A reply's deadline.
const REPLY_TIMEOUT: Millis = 3_000;
/// A status line with no "." after it is complete after this long.
const QUIET: Millis = 300;
/// The banner has this long to end before the first request goes anyway.
const BANNER_WAIT: Millis = 1_000;
const ACTIVE_EVERY: Millis = 2_000;
const LIST_EVERY: Millis = 30_000;
const FLAGS_EVERY: Millis = 10_000;
const LIVENESS_EVERY: Millis = 10_000;
/// After a cuelist command, the active list is asked this soon.
const AFTER_CUELIST: Millis = 300;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

const REPLY: Key = "reply";
const QUIET_TIMER: Key = "quiet";
const BANNER: Key = "banner";
const ACTIVE: Key = "active";
const LIST: Key = "list";
const FLAGS: Key = "flags";
const LIVENESS: Key = "liveness";
const RETRY: Key = "retry";

/// The queries the module asks of its own accord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Poll {
    List,
    Active,
    MxRun,
    SchRun,
    WhoAmI,
}

impl Poll {
    fn line(self) -> &'static str {
        match self {
            Poll::List => "QLList",
            Poll::Active => "QLActive",
            Poll::MxRun => "IsMxRun",
            Poll::SchRun => "IsSchRun",
            Poll::WhoAmI => "WhoIAm",
        }
    }
}

/// How a reply becomes a command's result.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    Ack,
    /// `NNNNN - Name` lines: [{number, name}]. Cuelists also update state.
    Numbered {
        cuelists: bool,
        active: bool,
    },
    YesNo,
    FirstLine,
    Text,
}

#[derive(Debug)]
enum Origin {
    Command { id: CommandId, shape: Shape },
    Poll(Poll),
}

#[derive(Debug)]
struct Request {
    line: String,
    origin: Origin,
}

/// A reply as it was read.
#[derive(Debug, Default, PartialEq)]
struct Reply {
    code: u16,
    /// The status line's text, or the continuation lines' texts.
    message: Vec<String>,
    data: Vec<String>,
}

/// Where the reply in progress is.
#[derive(Debug, Default, PartialEq)]
enum Reading {
    #[default]
    Idle,
    /// `NNN-` lines seen; a line without the dash ends them.
    Continuation(Reply),
    /// A status line seen; data lines until ".".
    Data(Reply),
}

fn status(line: &str) -> Option<(u16, Option<char>, &str)> {
    let digits = line.get(..3)?;
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let code = digits.parse().ok()?;
    let rest = &line[3..];
    match rest.chars().next() {
        None => Some((code, None, "")),
        Some(c @ ('-' | ' ')) => Some((code, Some(c), &rest[1..])),
        Some(_) => None,
    }
}

/// `00002 - House Lights` as (2, "House Lights").
fn numbered(line: &str) -> Option<(u64, String)> {
    let (number, name) = line.split_once(" - ")?;
    let number = number.trim().parse().ok()?;
    Some((number, name.trim().to_string()))
}

fn yes_no(reply: &Reply) -> Option<bool> {
    let line = reply.data.first().or(reply.message.first())?;
    match line.trim().to_ascii_lowercase().as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

pub(crate) struct Onyx {
    device: SocketAddr,
    monitor: bool,
    socket_open: bool,
    /// The banner has ended (or its wait has); requests may go.
    ready: bool,
    buffer: Vec<u8>,
    reading: Reading,
    in_flight: Option<(Request, Millis)>,
    queue: VecDeque<Request>,
    /// Queries refused on this connection.
    refused: BTreeSet<Poll>,
    /// Cuelists from the last QLList, for marking the inactive ones.
    known: BTreeMap<u64, String>,
    replies_seen: bool,
    retry_after: Millis,
}

impl Onyx {
    pub(crate) fn new(ctx: OpenContext) -> Onyx {
        let mut onyx = Onyx::for_device(SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)));
        onyx.monitor = ctx.monitor;
        onyx
    }

    fn for_device(device: SocketAddr) -> Onyx {
        Onyx {
            device,
            monitor: true,
            socket_open: false,
            ready: false,
            buffer: Vec::new(),
            reading: Reading::Idle,
            in_flight: None,
            queue: VecDeque::new(),
            refused: BTreeSet::new(),
            known: BTreeMap::new(),
            replies_seen: false,
            retry_after: RETRY_MIN,
        }
    }

    /// Queue a request: commands ahead of queued polls, polls once each.
    fn enqueue(&mut self, cx: &mut Cx, request: Request) {
        match request.origin {
            Origin::Command { .. } => {
                let at = self
                    .queue
                    .iter()
                    .position(|r| matches!(r.origin, Origin::Poll(_)))
                    .unwrap_or(self.queue.len());
                self.queue.insert(at, request);
            }
            Origin::Poll(p) => {
                if self.refused.contains(&p) {
                    return;
                }
                let queued = self
                    .queue
                    .iter()
                    .chain(self.in_flight.as_ref().map(|(r, _)| r))
                    .any(|r| matches!(r.origin, Origin::Poll(q) if q == p));
                if queued {
                    return;
                }
                self.queue.push_back(request);
            }
        }
        self.pump(cx);
    }

    fn poll(&mut self, cx: &mut Cx, p: Poll) {
        self.enqueue(
            cx,
            Request {
                line: p.line().into(),
                origin: Origin::Poll(p),
            },
        );
    }

    fn pump(&mut self, cx: &mut Cx) {
        if !self.socket_open || !self.ready || self.in_flight.is_some() {
            return;
        }
        let Some(request) = self.queue.pop_front() else {
            return;
        };
        cx.tcp_send(SOCKET, format!("{}\r\n", request.line));
        self.in_flight = Some((request, cx.now()));
        self.reading = Reading::Idle;
        cx.set_timer(REPLY, REPLY_TIMEOUT);
    }

    fn feed(&mut self, cx: &mut Cx, data: &[u8]) {
        self.buffer.extend_from_slice(data);
        while let Some(end) = self.buffer.iter().position(|&b| b == b'\n' || b == b'\r') {
            let line: Vec<u8> = self.buffer.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
            if line.is_empty() {
                continue;
            }
            self.line(cx, &line);
        }
        if self.buffer.len() > 65_536 {
            self.buffer.clear();
        }
    }

    fn line(&mut self, cx: &mut Cx, line: &str) {
        match std::mem::take(&mut self.reading) {
            Reading::Continuation(mut reply) => match status(line) {
                Some((code, Some('-'), text)) => {
                    reply.code = code;
                    reply.message.push(text.to_string());
                    self.reading = Reading::Continuation(reply);
                }
                Some((code, _, text)) => {
                    reply.code = code;
                    if !text.is_empty() {
                        reply.message.push(text.to_string());
                    }
                    self.finish(cx, reply);
                }
                // A line that is not a status line inside a continuation:
                // keep it as text.
                None => {
                    reply.message.push(line.to_string());
                    self.reading = Reading::Continuation(reply);
                }
            },
            Reading::Data(mut reply) => {
                if line == "." {
                    cx.cancel_timer(QUIET_TIMER);
                    self.finish(cx, reply);
                } else {
                    reply.data.push(line.to_string());
                    self.reading = Reading::Data(reply);
                    cx.set_timer(QUIET_TIMER, QUIET);
                }
            }
            Reading::Idle => match status(line) {
                Some((code, Some('-'), text)) => {
                    self.reading = Reading::Continuation(Reply {
                        code,
                        message: vec![text.to_string()],
                        data: Vec::new(),
                    });
                }
                Some((code, _, text)) => {
                    self.reading = Reading::Data(Reply {
                        code,
                        message: if text.is_empty() {
                            Vec::new()
                        } else {
                            vec![text.to_string()]
                        },
                        data: Vec::new(),
                    });
                    cx.set_timer(QUIET_TIMER, QUIET);
                }
                // Not part of any reply.
                None => {}
            },
        }
    }

    /// A whole reply: the banner, or the answer to the request in flight.
    fn finish(&mut self, cx: &mut Cx, reply: Reply) {
        cx.cancel_timer(QUIET_TIMER);
        if !self.ready {
            // The banner.
            self.ready = true;
            cx.cancel_timer(BANNER);
            self.pump(cx);
            return;
        }
        let Some((request, sent_at)) = self.in_flight.take() else {
            return;
        };
        cx.cancel_timer(REPLY);
        self.replies_seen = true;
        cx.round_trip(cx.now().saturating_sub(sent_at));
        let ok = (200..300).contains(&reply.code);
        match request.origin {
            Origin::Poll(p) => {
                if (400..600).contains(&reply.code) {
                    self.refused.insert(p);
                } else if ok {
                    self.poll_reply(cx, p, &reply);
                }
            }
            Origin::Command { id, shape } => {
                if !ok {
                    let message = if reply.message.is_empty() {
                        format!("refused with code {}", reply.code)
                    } else {
                        reply.message.join(" ")
                    };
                    cx.complete(
                        id,
                        Err(CommandError::DeviceError {
                            code: Some(reply.code.to_string()),
                            message,
                        }),
                    );
                } else {
                    let result = self.result(cx, shape, &reply);
                    cx.complete(id, result);
                }
            }
        }
        self.pump(cx);
    }

    fn poll_reply(&mut self, cx: &mut Cx, p: Poll, reply: &Reply) {
        match p {
            Poll::List => {
                self.apply_list(cx, reply);
            }
            Poll::Active => {
                self.apply_active(cx, reply);
            }
            Poll::MxRun => {
                if let Some(b) = yes_no(reply) {
                    cx.state(json!({"onyx_running": b}));
                }
            }
            Poll::SchRun => {
                if let Some(b) = yes_no(reply) {
                    cx.state(json!({"scheduler_running": b}));
                }
            }
            Poll::WhoAmI => {}
        }
    }

    fn entries(reply: &Reply) -> Vec<(u64, String)> {
        reply.data.iter().filter_map(|l| numbered(l)).collect()
    }

    fn listed(entries: &[(u64, String)]) -> Value {
        Value::Array(
            entries
                .iter()
                .map(|(n, name)| json!({"number": n, "name": name}))
                .collect(),
        )
    }

    fn apply_list(&mut self, cx: &mut Cx, reply: &Reply) -> Value {
        let entries = Onyx::entries(reply);
        let mut cuelists = Map::new();
        for (n, name) in &entries {
            cuelists.insert(n.to_string(), json!({"name": name}));
        }
        self.known = entries.iter().cloned().collect();
        if !cuelists.is_empty() {
            cx.state(json!({"cuelists": cuelists}));
        }
        Onyx::listed(&entries)
    }

    fn apply_active(&mut self, cx: &mut Cx, reply: &Reply) -> Value {
        let entries = Onyx::entries(reply);
        let active: BTreeSet<u64> = entries.iter().map(|(n, _)| *n).collect();
        let mut cuelists = Map::new();
        for n in self.known.keys() {
            cuelists.insert(n.to_string(), json!({"active": active.contains(n)}));
        }
        for (n, name) in &entries {
            cuelists.insert(n.to_string(), json!({"active": true, "name": name}));
        }
        if !cuelists.is_empty() {
            cx.state(json!({"cuelists": cuelists}));
        }
        Onyx::listed(&entries)
    }

    fn result(
        &mut self,
        cx: &mut Cx,
        shape: Shape,
        reply: &Reply,
    ) -> Result<Outcome, CommandError> {
        let unexpected = |what: &str| CommandError::DeviceError {
            code: None,
            message: format!("unexpected reply to a {what} query"),
        };
        Ok(match shape {
            Shape::Ack => Outcome::Ack,
            Shape::Numbered {
                cuelists: true,
                active: false,
            } => Outcome::Value {
                value: self.apply_list(cx, reply),
            },
            Shape::Numbered {
                cuelists: true,
                active: true,
            } => Outcome::Value {
                value: self.apply_active(cx, reply),
            },
            Shape::Numbered { .. } => {
                let entries = Onyx::entries(reply);
                let value = if entries.is_empty() {
                    Value::Array(reply.data.iter().map(|l| json!(l)).collect())
                } else {
                    Onyx::listed(&entries)
                };
                Outcome::Value { value }
            }
            Shape::YesNo => Outcome::Value {
                value: json!(yes_no(reply).ok_or_else(|| unexpected("yes/no"))?),
            },
            Shape::FirstLine => Outcome::Value {
                value: json!(reply
                    .data
                    .first()
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default()),
            },
            Shape::Text => {
                let lines = if reply.data.is_empty() {
                    &reply.message
                } else {
                    &reply.data
                };
                Outcome::Value {
                    value: json!(lines.join("\n")),
                }
            }
        })
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if let Some((request, _)) = self.in_flight.take() {
            if let Origin::Command { id, .. } = request.origin {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        for request in self.queue.drain(..) {
            if let Origin::Command { id, .. } = request.origin {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        for key in [REPLY, QUIET_TIMER, BANNER, ACTIVE, LIST, FLAGS, LIVENESS] {
            cx.cancel_timer(key);
        }
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.ready = false;
        self.buffer.clear();
        self.reading = Reading::Idle;
        self.refused.clear();
        self.replies_seen = false;
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn start_polling(&mut self, cx: &mut Cx) {
        if self.monitor {
            self.poll(cx, Poll::List);
            self.poll(cx, Poll::Active);
            self.poll(cx, Poll::MxRun);
            self.poll(cx, Poll::SchRun);
            cx.set_timer(ACTIVE, ACTIVE_EVERY);
            cx.set_timer(LIST, LIST_EVERY);
            cx.set_timer(FLAGS, FLAGS_EVERY);
        } else {
            cx.set_timer(LIVENESS, LIVENESS_EVERY);
        }
    }
}

/// A command's line and how its reply is read; `Err` for invalid parameters.
fn line_for(name: &str, params: &Params) -> Option<(String, Shape)> {
    let int = |k: &str| params.get(k).and_then(Value::as_i64);
    let num = |k: &str| params.get(k).and_then(Value::as_f64);
    let text = |k: &str| params.get(k).and_then(Value::as_str);
    let flag = |k: &str| params.get(k).and_then(Value::as_bool);
    let one = |cmd: &str, k: &str| int(k).map(|n| format!("{cmd} {n}"));
    let numbered = Shape::Numbered {
        cuelists: false,
        active: false,
    };
    Some(match name {
        "clear_programmer" => ("CLRCLR".into(), Shape::Ack),
        "go_cuelist" => (one("GQL", "cuelist")?, Shape::Ack),
        "pause_cuelist" => (one("PQL", "cuelist")?, Shape::Ack),
        "release_cuelist" => (one("RQL", "cuelist")?, Shape::Ack),
        "go_to_cue" => (
            format!("GTQ {},{}", int("cuelist")?, text("cue")?),
            Shape::Ack,
        ),
        "set_cuelist_level" => (
            format!("SQL {},{}", int("cuelist")?, int("level")?),
            Shape::Ack,
        ),
        "set_cuelist_level_manager" => (
            format!("SetQLLevel {},{}", int("cuelist")?, int("level")?),
            Shape::Ack,
        ),
        "release_all_cuelists" => ("RAQL".into(), Shape::Ack),
        "release_all_cuelists_dimmer_first" => ("RAQLDF".into(), Shape::Ack),
        "release_all_overrides" => ("RAO".into(), Shape::Ack),
        "release_all_cuelists_and_overrides" => ("RAQLO".into(), Shape::Ack),
        "release_all_cuelists_and_overrides_dimmer_first" => ("RAQLODF".into(), Shape::Ack),
        "list_cuelists" => (
            "QLList".into(),
            Shape::Numbered {
                cuelists: true,
                active: false,
            },
        ),
        "list_active_cuelists" => (
            "QLActive".into(),
            Shape::Numbered {
                cuelists: true,
                active: true,
            },
        ),
        "is_cuelist_active" => (one("IsQLActive", "cuelist")?, Shape::YesNo),
        "cuelist_name" => (one("QLName", "cuelist")?, Shape::FirstLine),
        "is_onyx_running" => ("IsMxRun".into(), Shape::YesNo),
        "action_group" => (one("ACT", "group")?, Shape::Ack),
        "list_action_groups" => ("ActList".into(), numbered),
        "action_group_name" => (one("ActName", "group")?, Shape::FirstLine),
        "run_command" => (one("CMD", "command")?, Shape::Ack),
        "list_commands" => ("CmdList".into(), numbered),
        "command_name" => (one("CmdName", "command")?, Shape::FirstLine),
        "go_schedule" => (one("GSC", "schedule")?, Shape::Ack),
        "use_calendar_rules" => ("SchUseCalendar".into(), Shape::Ack),
        "list_schedules" => ("SchList".into(), numbered),
        "schedule_name" => (one("SchName", "schedule")?, Shape::FirstLine),
        "is_scheduler_running" => ("IsSchRun".into(), Shape::YesNo),
        "list_time_presets" => ("TimePresetList".into(), numbered),
        "status" => ("Status".into(), Shape::Text),
        "last_log" => (one("LastLog", "lines")?, Shape::Text),
        "who_am_i" => ("WhoIAm".into(), Shape::FirstLine),
        "set_date" => (
            format!(
                "SetDate {},{:02},{:02}",
                int("year")?,
                int("month")?,
                int("day")?
            ),
            Shape::Ack,
        ),
        "set_time" => (
            format!(
                "SetTime {:02},{:02},{:02}",
                int("hour")?,
                int("minute")?,
                int("second")?
            ),
            Shape::Ack,
        ),
        "set_position" => (
            format!(
                "SetPosDec {},{},{},{}",
                trim_float(num("latitude")?),
                if flag("north")? { "N" } else { "S" },
                trim_float(num("longitude")?),
                if flag("east")? { "E" } else { "W" }
            ),
            Shape::Ack,
        ),
        "help" => ("Help".into(), Shape::Text),
        "send_raw" => (text("text")?.to_string(), Shape::Text),
        _ => return None,
    })
}

/// A coordinate with up to four decimals and no trailing zeros: 45.5, 34.
fn trim_float(x: f64) -> String {
    let s = format!("{x:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

impl Module for Onyx {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.socket_open {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let Some((line, shape)) = line_for(name, params) else {
            cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            );
            return;
        };
        if self.monitor
            && matches!(
                name,
                "go_cuelist"
                    | "pause_cuelist"
                    | "release_cuelist"
                    | "go_to_cue"
                    | "release_all_cuelists"
                    | "release_all_cuelists_dimmer_first"
                    | "release_all_overrides"
                    | "release_all_cuelists_and_overrides"
                    | "release_all_cuelists_and_overrides_dimmer_first"
            )
        {
            cx.set_timer(ACTIVE, AFTER_CUELIST);
        }
        self.enqueue(
            cx,
            Request {
                line,
                origin: Origin::Command { id, shape },
            },
        );
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.socket_open = true;
                self.retry_after = RETRY_MIN;
                cx.connection(Connection::Connected);
                cx.set_timer(BANNER, BANNER_WAIT);
                self.start_polling(cx);
            }
            TcpInput::Data(data) => {
                cx.alive();
                self.feed(cx, &data);
            }
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => {
                cx.connection(Connection::Connecting);
                cx.tcp_open(SOCKET, self.device);
            }
            BANNER => {
                if !self.ready {
                    self.ready = true;
                    self.reading = Reading::Idle;
                    self.pump(cx);
                }
            }
            QUIET_TIMER => {
                if let Reading::Data(reply) = std::mem::take(&mut self.reading) {
                    self.finish(cx, reply);
                }
            }
            REPLY => {
                let Some((request, _)) = self.in_flight.take() else {
                    return;
                };
                self.reading = Reading::Idle;
                cx.cancel_timer(QUIET_TIMER);
                match request.origin {
                    Origin::Command { id, .. } => cx.complete(id, Err(CommandError::Timeout)),
                    Origin::Poll(_) if self.replies_seen => {
                        // The server answered before and has stopped: a
                        // half-open connection.
                        self.lost(cx, "no reply from the ONYX server".into());
                        return;
                    }
                    Origin::Poll(_) => {}
                }
                self.pump(cx);
            }
            ACTIVE => {
                self.poll(cx, Poll::Active);
                cx.set_timer(ACTIVE, ACTIVE_EVERY);
            }
            LIST => {
                self.poll(cx, Poll::List);
                cx.set_timer(LIST, LIST_EVERY);
            }
            FLAGS => {
                self.poll(cx, Poll::MxRun);
                self.poll(cx, Poll::SchRun);
                cx.set_timer(FLAGS, FLAGS_EVERY);
            }
            LIVENESS => {
                self.poll(cx, Poll::WhoAmI);
                cx.set_timer(LIVENESS, LIVENESS_EVERY);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn onyx(monitor: bool) -> Onyx {
        let mut m = Onyx::for_device(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 40)),
            PORT,
        ));
        m.monitor = monitor;
        m
    }

    fn sent(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(String::from_utf8(data.clone()).unwrap()),
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

    fn feed(m: &mut Onyx, now: Millis, data: &str) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(data.as_bytes().to_vec()));
        cx.take()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    const BANNER_TEXT: &str =
        "200-*************Welcome to Onyx Manager v4.0.1010 build 0!*************\r\n\
                               200-Type HELP for a list of available commands\r\n200\r\n";

    /// Connected, banner read; returns what was sent at that point.
    fn connected(monitor: bool) -> (Onyx, Vec<Action>) {
        let mut m = onyx(monitor);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let mut actions = cx.take();
        assert!(sent(&actions).is_empty(), "nothing before the banner");
        actions.extend(feed(&mut m, 10, BANNER_TEXT));
        (m, actions)
    }

    #[test]
    fn status_lines() {
        assert_eq!(status("200 Ok"), Some((200, Some(' '), "Ok")));
        assert_eq!(status("200-Type HELP"), Some((200, Some('-'), "Type HELP")));
        assert_eq!(status("200"), Some((200, None, "")));
        assert_eq!(status("00002 - House Lights"), None);
        assert_eq!(status("Yes"), None);
        assert_eq!(
            numbered("00002 - House Lights"),
            Some((2, "House Lights".into()))
        );
    }

    #[test]
    fn waits_for_the_banner_then_polls_lists_one_at_a_time() {
        let (mut m, a) = connected(true);
        assert_eq!(sent(&a), ["QLList\r\n"]);
        let a = feed(
            &mut m,
            20,
            "200 Ok\r\n00002 - House Lights\r\n00003 - SlimPar\r\n.\r\n",
        );
        assert_eq!(sent(&a), ["QLActive\r\n"]);
        assert!(a.contains(&Action::RoundTrip(10)));
        assert_eq!(
            state(&a),
            json!({"cuelists": {"2": {"name": "House Lights"}, "3": {"name": "SlimPar"}}})
        );
        let a = feed(&mut m, 30, "200 Ok\r\n00003 - SlimPar\r\n.\r\n");
        assert_eq!(
            state(&a),
            json!({"cuelists": {"2": {"active": false}, "3": {"active": true, "name": "SlimPar"}}})
        );
        assert_eq!(sent(&a), ["IsMxRun\r\n"]);
        let a = feed(&mut m, 40, "200 Ok\r\nYes\r\n.\r\n");
        assert_eq!(state(&a), json!({"onyx_running": true}));
        // Nothing active.
        let mut cx = Cx::new(50);
        m.timer(&mut cx, ACTIVE);
        let a = feed(&mut m, 60, "200 Ok\r\nNo\r\n.\r\n"); // IsSchRun answered first
        assert_eq!(state(&a), json!({"scheduler_running": false}));
        assert_eq!(sent(&a), ["QLActive\r\n"]);
        let a = feed(&mut m, 70, "200 Ok\r\nNo Active Qlist in List\r\n.\r\n");
        assert_eq!(
            state(&a),
            json!({"cuelists": {"2": {"active": false}, "3": {"active": false}}})
        );
    }

    #[test]
    fn a_refused_query_is_not_polled_again() {
        let (mut m, _) = connected(true);
        let a = feed(
            &mut m,
            20,
            "400-I never heard that command before... are you sure?\r\n400-Type HELP for a list of commands\r\n400 QLList\r\n",
        );
        assert_eq!(sent(&a), ["QLActive\r\n"]);
        assert!(m.refused.contains(&Poll::List));
        let mut cx = Cx::new(30);
        m.timer(&mut cx, LIST);
        assert!(m
            .queue
            .iter()
            .all(|r| !matches!(r.origin, Origin::Poll(Poll::List))));
    }

    #[test]
    fn commands_go_ahead_of_polls_and_are_acknowledged() {
        let (mut m, _) = connected(true);
        // QLList in flight; QLActive, IsMxRun, IsSchRun queued.
        let mut cx = Cx::new(15);
        m.command(&mut cx, 1, "go_cuelist", &params(json!({"cuelist": 14})));
        m.command(
            &mut cx,
            2,
            "go_to_cue",
            &params(json!({"cuelist": 14, "cue": "4.1"})),
        );
        assert!(sent(&cx.take()).is_empty());
        let a = feed(&mut m, 20, "200 Ok\r\n.\r\n");
        assert_eq!(sent(&a), ["GQL 14\r\n"]);
        let a = feed(&mut m, 30, "200 Ok\r\n.\r\n");
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));
        assert_eq!(sent(&a), ["GTQ 14,4.1\r\n"]);
        // A reply without "." completes after the quiet time.
        let a = feed(&mut m, 40, "200 Ok\r\n");
        assert!(!a.iter().any(|x| matches!(x, Action::Complete { .. })));
        let mut cx = Cx::new(340);
        m.timer(&mut cx, QUIET_TIMER);
        let a = cx.take();
        assert!(a.contains(&Action::Complete {
            id: 2,
            result: Ok(Outcome::Ack)
        }));
        assert_eq!(sent(&a), ["QLActive\r\n"]);
    }

    #[test]
    fn an_error_reply_fails_the_command() {
        let (mut m, _) = connected(false);
        let mut cx = Cx::new(20);
        m.command(&mut cx, 7, "send_raw", &params(json!({"text": "hello"})));
        assert_eq!(sent(&cx.take()), ["hello\r\n"]);
        let a = feed(
            &mut m,
            30,
            "400-I never heard that command before... are you sure?\r\n400-Type HELP for a list of commands\r\n400 hello\r\n",
        );
        let failed = a.iter().any(|x| {
            matches!(x, Action::Complete { id: 7, result: Err(CommandError::DeviceError { code: Some(c), .. }) } if c == "400")
        });
        assert!(failed, "{a:?}");
    }

    #[test]
    fn query_commands_return_values_and_state() {
        let (mut m, _) = connected(false);
        let mut cx = Cx::new(20);
        m.command(&mut cx, 1, "list_cuelists", &Params::new());
        m.command(
            &mut cx,
            2,
            "is_cuelist_active",
            &params(json!({"cuelist": 3})),
        );
        m.command(
            &mut cx,
            3,
            "set_position",
            &params(json!({"latitude": 45.5, "north": true, "longitude": 34.0, "east": false})),
        );
        let a = feed(&mut m, 30, "200 Ok\r\n00002 - House Lights\r\n.\r\n");
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Value {
                value: json!([{"number": 2, "name": "House Lights"}])
            })
        }));
        assert_eq!(
            state(&a),
            json!({"cuelists": {"2": {"name": "House Lights"}}})
        );
        assert_eq!(sent(&a), ["IsQLActive 3\r\n"]);
        let a = feed(&mut m, 40, "200 Ok\r\nNo\r\n.\r\n");
        assert!(a.contains(&Action::Complete {
            id: 2,
            result: Ok(Outcome::Value {
                value: json!(false)
            })
        }));
        assert_eq!(sent(&a), ["SetPosDec 45.5,N,34,W\r\n"]);
    }

    #[test]
    fn commands_only_polls_no_state_and_checks_liveness() {
        let (mut m, a) = connected(false);
        assert!(sent(&a).is_empty(), "no state read on connecting");
        let mut cx = Cx::new(10_000);
        m.timer(&mut cx, LIVENESS);
        let a = cx.take();
        assert_eq!(sent(&a), ["WhoIAm\r\n"]);
        let a = feed(&mut m, 10_005, "200 Ok\r\n10.0.0.2\r\n.\r\n");
        assert!(a.contains(&Action::RoundTrip(5)));
        assert_eq!(state(&a), json!({}));
        // Commands still work.
        let mut cx = Cx::new(10_010);
        m.command(&mut cx, 1, "clear_programmer", &Params::new());
        assert_eq!(sent(&cx.take()), ["CLRCLR\r\n"]);
    }

    #[test]
    fn a_silent_server_that_has_answered_before_is_dropped() {
        let (mut m, _) = connected(true);
        feed(&mut m, 20, "200 Ok\r\n.\r\n");
        let mut cx = Cx::new(3_100);
        m.timer(&mut cx, REPLY);
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
    }

    #[test]
    fn the_banner_wait_ends_without_a_banner() {
        let mut m = onyx(false);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        m.command(&mut cx, 1, "go_cuelist", &params(json!({"cuelist": 2})));
        assert!(sent(&cx.take()).is_empty());
        let mut cx = Cx::new(1_000);
        m.timer(&mut cx, BANNER);
        assert_eq!(sent(&cx.take()), ["GQL 2\r\n"]);
    }
}
