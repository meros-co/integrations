//! Middle Atlantic RackLink power products over their control protocol
//! (I-00472 Rev G, and the INFOCOMM 4:2012 additions): binary messages on
//! TCP 60000, or on a Premium unit's RS-232 port.
//!
//! - A message is 0xFE, the length of its envelope, the envelope
//!   (destination 0x00, command, subcommand, data), a checksum (the sum of
//!   every byte from the header to the envelope's end, its low 7 bits) and
//!   0xFF; 0xFE, 0xFF and 0xFD inside are sent as 0xFD and the byte
//!   inverted, and counted unescaped.
//! - On connecting the module logs in ("user|password"); a refused login is
//!   terminal (`unauthorized`) until the settings change. The unit pings
//!   the controller and drops it after three unanswered pings, so every
//!   ping is answered at once.
//! - Requests go one at a time and are answered in order: a response
//!   (subcommand 0x10) for the command in flight, or a NACK naming why it
//!   failed. Status changes (subcommand 0x12) and log updates (0x30) come
//!   unasked and only update state.
//! - Monitored, it reads which outlets and contacts exist, their states and
//!   names, and on Premium models the energy management states, sensors,
//!   thresholds, log count, alert and status registrations and product
//!   information; it registers for every status change the unit can push,
//!   then reads states and sensors again every `poll_interval_ms` (a Select
//!   pushes nothing). Commands-only, it logs in and answers pings; the
//!   outlet count is read as the liveness check.

use std::collections::VecDeque;

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Link, Millis, Module, OpenContext,
    Outcome, PollStamps, TcpInput,
};

const SOCKET: Key = "racklink";
const REPLY_TIMER: Key = "reply";
const POLL_TIMER: Key = "poll";
const RETRY_TIMER: Key = "retry";
const DEFAULT_PORT: u16 = 60000;
const REPLY_TIMEOUT: Millis = 3000;
const RETRY_MIN: Millis = 1000;
const RETRY_MAX: Millis = 30_000;
/// Unanswered requests in a row before the connection is opened again.
const MISSES: u32 = 3;

const HEADER: u8 = 0xFE;
const TAIL: u8 = 0xFF;
const ESCAPE: u8 = 0xFD;

const PING: u8 = 0x01;
const LOGIN: u8 = 0x02;
const NACK: u8 = 0x10;
const OUTLET: u8 = 0x20;
const OUTLET_NAME: u8 = 0x21;
const OUTLET_COUNT: u8 = 0x22;
const ENERGY_STATE: u8 = 0x23;
const CONTACT: u8 = 0x30;
const CONTACT_NAME: u8 = 0x31;
const CONTACT_COUNT: u8 = 0x32;
const SEQUENCE: u8 = 0x36;
const EPO: u8 = 0x37;
const LOG_ALERTS: u8 = 0x40;
const STATUS_REGISTRATION: u8 = 0x41;
const READ_LOG: u8 = 0x80;
const LOG_COUNT: u8 = 0x81;
const CLEAR_LOG: u8 = 0x82;

const SET: u8 = 0x01;
const GET: u8 = 0x02;
const RESPONSE: u8 = 0x10;
const CHANGE: u8 = 0x12;
const LOG_UPDATE: u8 = 0x30;

/// Sensors (0x50-0x61): command, state key and how the value reads.
const SENSORS: [(u8, &str, Reading); 12] = [
    (0x50, "energy", Reading::Float),
    (0x51, "peak_voltage", Reading::Int),
    (0x52, "rms_voltage", Reading::Int),
    (0x53, "peak_load", Reading::Float),
    (0x54, "rms_load", Reading::Float),
    (0x55, "temperature", Reading::Int),
    (0x56, "wattage", Reading::Int),
    (0x57, "power_factor", Reading::Float),
    (0x58, "thermal_load", Reading::Float),
    (0x59, "surge_protection", Reading::Surge),
    (0x60, "energy_management", Reading::Energy),
    (0x61, "occupancy", Reading::Occupancy),
];

/// Thresholds (0x70-0x77): command, state key and the digits before and
/// after the point the value is written with.
const THRESHOLDS: [(u8, &str, usize, usize); 6] = [
    (0x70, "low_voltage", 3, 0),
    (0x71, "high_voltage", 3, 0),
    (0x73, "max_load", 2, 1),
    (0x74, "min_load", 2, 1),
    (0x76, "max_temperature", 3, 0),
    (0x77, "min_temperature", 3, 0),
];

/// Product information (0x90-0x95): command and state key.
const PRODUCT: [(u8, &str); 5] = [
    (0x90, "part_number"),
    (0x91, "amp_rating"),
    (0x93, "surge"),
    (0x94, "ip_address"),
    (0x95, "mac_address"),
];

/// Every status change a unit can push (Register Status Change, 0x41): six
/// bytes, each bit an item the document names (bit 1 the lowest).
const REGISTER_ALL: [u8; 6] = [0x01, 0x0F, 0x5B, 0x01, 0x7F, 0x0F];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Reading {
    Int,
    Float,
    Surge,
    Energy,
    Occupancy,
}

const NACK_REASONS: [(u8, &str); 10] = [
    (0x01, "bad checksum on the previous command"),
    (0x02, "bad length on the previous command"),
    (0x03, "bad escape sequence on the previous command"),
    (0x04, "previous command invalid"),
    (0x05, "previous subcommand invalid"),
    (0x06, "previous command had the wrong byte count"),
    (0x07, "invalid data bytes in the previous command"),
    (0x08, "invalid credentials"),
    (0x10, "unknown error"),
    (0x11, "access denied (emergency power off)"),
];

fn checksum(bytes: &[u8]) -> u8 {
    (bytes.iter().map(|b| u32::from(*b)).sum::<u32>() & 0x7F) as u8
}

/// A message around an envelope (destination, command, subcommand, data).
pub(crate) fn frame(envelope: &[u8]) -> Vec<u8> {
    let mut raw = vec![HEADER, envelope.len() as u8];
    raw.extend_from_slice(envelope);
    raw.push(checksum(&raw));
    let mut out = vec![HEADER];
    for &b in &raw[1..] {
        if matches!(b, HEADER | TAIL | ESCAPE) {
            out.push(ESCAPE);
            out.push(!b);
        } else {
            out.push(b);
        }
    }
    out.push(TAIL);
    out
}

/// Splits the byte stream into envelopes, checking each one's length and
/// checksum; a damaged message is dropped.
#[derive(Default)]
struct Deframer {
    buf: Vec<u8>,
    inside: bool,
    escaped: bool,
}

impl Deframer {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for &b in bytes {
            if b == HEADER {
                self.buf.clear();
                self.inside = true;
                self.escaped = false;
                continue;
            }
            if !self.inside {
                continue;
            }
            if b == TAIL {
                self.inside = false;
                if let Some(env) = Self::check(&self.buf) {
                    out.push(env);
                }
                self.buf.clear();
                continue;
            }
            if self.escaped {
                self.buf.push(!b);
                self.escaped = false;
            } else if b == ESCAPE {
                self.escaped = true;
            } else {
                self.buf.push(b);
            }
            if self.buf.len() > 600 {
                self.inside = false;
                self.buf.clear();
            }
        }
        out
    }

    /// The envelope of [length, envelope, checksum], when they agree.
    fn check(body: &[u8]) -> Option<Vec<u8>> {
        let (&len, rest) = body.split_first()?;
        let (&sum, env) = rest.split_last()?;
        if env.len() != len as usize || env.len() < 3 {
            return None;
        }
        let mut raw = vec![HEADER, len];
        raw.extend_from_slice(env);
        (checksum(&raw) == sum).then(|| env.to_vec())
    }
}

/// Four ASCII digits, zero-padded.
fn digits4(n: u64) -> [u8; 4] {
    let s = format!("{:04}", n.min(9999));
    let b = s.as_bytes();
    [b[0], b[1], b[2], b[3]]
}

fn ascii(data: &[u8]) -> String {
    String::from_utf8_lossy(data)
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_string()
}

fn energy_state(letter: u8) -> &'static str {
    match letter {
        b'D' => "disconnected",
        b'S' => "standby",
        b'I' => "on",
        b'O' => "off",
        _ => "unknown",
    }
}

fn reading(kind: Reading, data: &[u8]) -> Option<Value> {
    let text = ascii(data);
    match kind {
        Reading::Int => text.parse::<i64>().ok().map(|n| json!(n)),
        Reading::Float => text.parse::<f64>().ok().map(|f| json!(f)),
        Reading::Surge => match data.first()? {
            0x00 => Some(json!("not_applicable")),
            0x01 => Some(json!("protected")),
            0x02 => Some(json!("compromised")),
            _ => None,
        },
        Reading::Occupancy => match data.first()? {
            b'O' => Some(json!("occupied")),
            b'U' => Some(json!("unoccupied")),
            _ => None,
        },
        // A letter per outlet, from outlet 1; the INFOCOMM 4:2012 response
        // puts the occupancy first.
        Reading::Energy => Some(json!(data
            .iter()
            .map(|&l| energy_state(l))
            .collect::<Vec<_>>())),
    }
}

fn nested(path: &[&str], value: Value) -> Value {
    let mut v = value;
    for key in path.iter().rev() {
        let mut m = Map::new();
        m.insert((*key).to_string(), v);
        v = Value::Object(m);
    }
    v
}

/// A request and what answers it.
#[derive(Debug, Clone, PartialEq)]
struct Request {
    envelope: Vec<u8>,
    /// The command (and, for per-outlet and per-contact commands, the
    /// number) a response must carry.
    answer: (u8, Option<u8>),
    /// A log read takes this many responses.
    responses: usize,
}

impl Request {
    fn new(command: u8, sub: u8, data: &[u8]) -> Request {
        let mut envelope = vec![0x00, command, sub];
        envelope.extend_from_slice(data);
        let numbered = matches!(
            command,
            OUTLET | OUTLET_NAME | ENERGY_STATE | CONTACT | CONTACT_NAME
        );
        Request {
            envelope,
            answer: (command, numbered.then(|| data.first().copied()).flatten()),
            responses: 1,
        }
    }
}

struct Job {
    id: Option<CommandId>,
    request: Request,
    returns_value: bool,
    /// What the responses gave, for a command returning several.
    collected: Vec<Value>,
}

pub(crate) struct RackLink {
    link: Link,
    monitor: bool,
    /// When each poll was last answered (`polled.<group>`, SPEC.md §8).
    stamps: PollStamps,
    premium: bool,
    username: String,
    password: String,
    poll_every: Millis,
    deframer: Deframer,
    open: bool,
    logged_in: bool,
    refused: bool,
    connected: bool,
    misses: u32,
    retry_after: Millis,
    queue: VecDeque<Job>,
    in_flight: Option<(Job, Millis)>,
    outlets: Vec<u8>,
    contacts: Vec<u8>,
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn int(params: &Params, name: &str) -> Result<u64, CommandError> {
    params
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("'{name}' is required")))
}

fn text<'a>(params: &'a Params, name: &str) -> &'a str {
    params.get(name).and_then(Value::as_str).unwrap_or("")
}

/// Bytes from hex digits, spaces and colons allowed between them.
fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    let digits: String = text.chars().filter(|c| !matches!(c, ' ' | ':')).collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(digits.get(i..i + 2)?, 16).ok())
        .collect()
}

/// The request a command sends, and whether it returns a value.
fn plan(name: &str, params: &Params) -> Result<(Request, bool), CommandError> {
    let number = |key: &str| -> Result<u8, CommandError> { Ok(int(params, key)? as u8) };
    let action = |state: &str| -> Result<u8, CommandError> {
        match state {
            "off" => Ok(0x00),
            "on" => Ok(0x01),
            "cycle" => Ok(0x02),
            other => Err(invalid(format!("unknown action '{other}'"))),
        }
    };
    let switch = |command: u8, key: &str| -> Result<Request, CommandError> {
        let mut data = vec![number(key)?, action(text(params, "action"))?];
        let cycle = params.get("cycle_s").and_then(Value::as_u64).unwrap_or(0);
        data.extend_from_slice(&digits4(cycle));
        Ok(Request::new(command, SET, &data))
    };
    let named = |command: u8, key: &str| -> Result<Request, CommandError> {
        let mut data = vec![number(key)?];
        data.extend_from_slice(text(params, "name").as_bytes());
        Ok(Request::new(command, SET, &data))
    };
    Ok(match name {
        "set_outlet" => (switch(OUTLET, "outlet")?, false),
        "get_outlet" => (Request::new(OUTLET, GET, &[number("outlet")?]), true),
        "set_outlet_name" => (named(OUTLET_NAME, "outlet")?, false),
        "get_outlet_name" => (Request::new(OUTLET_NAME, GET, &[number("outlet")?]), true),
        "get_outlet_count" => (Request::new(OUTLET_COUNT, GET, &[]), true),
        "set_contact" => (switch(CONTACT, "contact")?, false),
        "get_contact" => (Request::new(CONTACT, GET, &[number("contact")?]), true),
        "set_contact_name" => (named(CONTACT_NAME, "contact")?, false),
        "get_contact_name" => (Request::new(CONTACT_NAME, GET, &[number("contact")?]), true),
        "get_contact_count" => (Request::new(CONTACT_COUNT, GET, &[]), true),
        "sequence_outlets" => {
            let direction = match text(params, "direction") {
                "up" => 0x01,
                "down" => 0x03,
                other => return Err(invalid(format!("unknown direction '{other}'"))),
            };
            let delay = params.get("delay_s").and_then(Value::as_u64).unwrap_or(0);
            let mut data = vec![direction];
            data.extend_from_slice(&digits4(delay));
            (Request::new(SEQUENCE, SET, &data), false)
        }
        "set_energy_state" => {
            let letter = match text(params, "state") {
                "disconnected" => b'D',
                "standby" => b'S',
                "on" => b'I',
                "off" => b'O',
                other => return Err(invalid(format!("unknown state '{other}'"))),
            };
            (
                Request::new(ENERGY_STATE, SET, &[number("outlet")?, letter]),
                false,
            )
        }
        "get_energy_state" => (Request::new(ENERGY_STATE, GET, &[number("outlet")?]), true),
        "emergency_power_off" => {
            let on = params
                .get("active")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            (Request::new(EPO, SET, &[u8::from(on)]), false)
        }
        "set_log_alerts" => {
            let flags = int(params, "flags")? as u16;
            (
                Request::new(LOG_ALERTS, SET, &[(flags >> 8) as u8, flags as u8]),
                false,
            )
        }
        "get_log_alerts" => (Request::new(LOG_ALERTS, GET, &[]), true),
        "set_status_registration" => {
            let bytes = hex_bytes(text(params, "flags")).ok_or_else(|| {
                invalid("flags: give six bytes in hex, such as 01 0f 5b 01 7f 0f")
            })?;
            if bytes.len() != 6 {
                return Err(invalid("flags: give six bytes in hex"));
            }
            (Request::new(STATUS_REGISTRATION, SET, &bytes), false)
        }
        "get_status_registration" => (Request::new(STATUS_REGISTRATION, GET, &[]), true),
        "get_sensor" => {
            let key = text(params, "sensor");
            let (code, _, _) = SENSORS
                .iter()
                .find(|(_, k, _)| *k == key)
                .ok_or_else(|| invalid(format!("unknown sensor '{key}'")))?;
            (Request::new(*code, GET, &[]), true)
        }
        "set_occupancy" => {
            let occupied = params
                .get("occupied")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            (
                Request::new(0x61, SET, &[if occupied { b'O' } else { b'U' }]),
                false,
            )
        }
        "get_threshold" => {
            let key = text(params, "threshold");
            let (code, ..) = THRESHOLDS
                .iter()
                .find(|(_, k, ..)| *k == key)
                .ok_or_else(|| invalid(format!("unknown threshold '{key}'")))?;
            (Request::new(*code, GET, &[]), true)
        }
        "set_threshold" => {
            let key = text(params, "threshold");
            let (code, _, whole, decimals) = THRESHOLDS
                .iter()
                .find(|(_, k, ..)| *k == key)
                .ok_or_else(|| invalid(format!("unknown threshold '{key}'")))?;
            let value = params
                .get("value")
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid("'value' is required"))?;
            let width = whole + if *decimals > 0 { decimals + 1 } else { 0 };
            let s = format!("{value:0width$.decimals$}", decimals = *decimals);
            if s.len() != width || value < 0.0 {
                return Err(invalid(format!(
                    "{key} takes {whole} digits before the point and {decimals} after"
                )));
            }
            (Request::new(*code, SET, s.as_bytes()), false)
        }
        "read_log" => {
            let start = int(params, "start")?;
            let count = int(params, "count")?;
            let mut data = digits4(start).to_vec();
            data.push(b'|');
            data.extend_from_slice(format!("{:02}", count.min(99)).as_bytes());
            let mut r = Request::new(READ_LOG, GET, &data);
            r.responses = count as usize;
            (r, true)
        }
        "get_log_count" => (Request::new(LOG_COUNT, GET, &[]), true),
        "clear_log" => (Request::new(CLEAR_LOG, SET, &[]), false),
        "get_product_info" => {
            let key = text(params, "item");
            let (code, _) = PRODUCT
                .iter()
                .find(|(_, k)| *k == key)
                .ok_or_else(|| invalid(format!("unknown item '{key}'")))?;
            (Request::new(*code, GET, &[]), true)
        }
        other => {
            return Err(CommandError::UnknownCommand {
                command: other.into(),
            })
        }
    })
}

/// What a message from the unit says: a state patch, and the value a get
/// command returns.
fn decode(command: u8, data: &[u8]) -> Option<(Value, Value)> {
    let cycle = |d: &[u8]| ascii(d).parse::<i64>().ok();
    match command {
        OUTLET | CONTACT => {
            let (&n, rest) = data.split_first()?;
            let state = match rest.first()? {
                0x00 => "off",
                0x01 => "on",
                0x03 => "not_controllable",
                _ => return None,
            };
            let mut item = json!({ "state": state });
            if let Some(c) = rest.get(1..5).and_then(cycle) {
                item["cycle_time"] = json!(c);
            }
            let group = if command == OUTLET {
                "outlets"
            } else {
                "contacts"
            };
            Some((nested(&[group, &n.to_string()], item.clone()), item))
        }
        OUTLET_NAME | CONTACT_NAME => {
            let (&n, rest) = data.split_first()?;
            let name = ascii(rest);
            let group = if command == OUTLET_NAME {
                "outlets"
            } else {
                "contacts"
            };
            Some((
                nested(&[group, &n.to_string(), "name"], json!(name)),
                json!(name),
            ))
        }
        OUTLET_COUNT | CONTACT_COUNT => {
            let group = if command == OUTLET_COUNT {
                "outlets"
            } else {
                "contacts"
            };
            let mut items = Map::new();
            let mut list = Vec::new();
            for (i, letter) in data.iter().enumerate() {
                let n = (i + 1).to_string();
                match letter {
                    b'C' | b'N' => {
                        items.insert(n, json!({ "controllable": *letter == b'C' }));
                    }
                    // Absent: the row leaves state.
                    _ => {
                        items.insert(n, Value::Null);
                    }
                }
                list.push(json!(match letter {
                    b'C' => "controllable",
                    b'N' => "not_controllable",
                    _ => "absent",
                }));
            }
            Some((json!({ group: items }), json!(list)))
        }
        ENERGY_STATE => {
            let (&n, rest) = data.split_first()?;
            let state = energy_state(*rest.first()?);
            Some((
                nested(&["outlets", &n.to_string(), "energy_state"], json!(state)),
                json!(state),
            ))
        }
        SEQUENCE => {
            let state = match data.first()? {
                0x00 => "not_sequencing",
                0x01 => "sequencing_up",
                0x02 => "up_complete",
                0x03 => "sequencing_down",
                0x04 => "down_complete",
                _ => return None,
            };
            let mut seq = json!({ "state": state });
            if let Some(d) = data.get(1..5).and_then(cycle) {
                seq["delay"] = json!(d);
            }
            Some((json!({ "sequence": seq.clone() }), seq))
        }
        EPO => {
            let on = match data.first()? {
                0x00 => false,
                0x01 => true,
                _ => return None,
            };
            Some((json!({ "emergency_power_off": on }), json!(on)))
        }
        LOG_ALERTS => {
            let flags = (u64::from(*data.first()?) << 8) | u64::from(*data.get(1)?);
            Some((json!({ "log_alerts": flags }), json!(flags)))
        }
        STATUS_REGISTRATION => {
            let hex: Vec<String> = data.iter().map(|b| format!("{b:02x}")).collect();
            let hex = hex.join(" ");
            Some((json!({ "status_registration": hex }), json!(hex)))
        }
        LOG_COUNT => {
            let n = ascii(data).parse::<i64>().ok()?;
            Some((json!({ "log": { "count": n } }), json!(n)))
        }
        CLEAR_LOG => Some((json!({ "log": { "count": 0 } }), Value::Null)),
        READ_LOG => {
            // "rr|NN|eeee|tt|data": item, items still to come, entry, type.
            let text = String::from_utf8_lossy(data).to_string();
            let mut parts = text.splitn(5, '|');
            let (_, pending, entry, kind, rest) = (
                parts.next()?,
                parts.next()?,
                parts.next()?,
                parts.next()?,
                parts.next().unwrap_or(""),
            );
            let entry = json!({
                "entry": entry.trim().parse::<i64>().ok()?,
                "type": log_type(kind.trim()),
                "pending": pending.trim().parse::<i64>().ok()?,
                "data": rest.trim_end_matches(['\0', ',']).to_string(),
            });
            Some((Value::Null, entry))
        }
        0x50..=0x61 => {
            let (_, key, kind) = SENSORS.iter().find(|(c, ..)| *c == command)?;
            if *kind == Reading::Energy {
                // The INFOCOMM 4:2012 response puts the occupancy first.
                let (occupancy, letters) = match data.first() {
                    Some(b'U' | b'O') if !data.len().is_multiple_of(2) => {
                        (data.first(), &data[1..])
                    }
                    _ => (None, data),
                };
                let mut patch = json!({});
                let mut outlets = Map::new();
                for (i, l) in letters.iter().enumerate() {
                    outlets.insert(
                        (i + 1).to_string(),
                        json!({ "energy_state": energy_state(*l) }),
                    );
                }
                patch["outlets"] = Value::Object(outlets);
                if let Some(o) = occupancy {
                    patch["sensors"] = json!({ "occupancy": reading(Reading::Occupancy, &[*o])? });
                }
                let value = reading(Reading::Energy, letters)?;
                return Some((patch, value));
            }
            let value = reading(*kind, data)?;
            Some((nested(&["sensors", key], value.clone()), value))
        }
        0x70..=0x77 => {
            let (_, key, ..) = THRESHOLDS.iter().find(|(c, ..)| *c == command)?;
            let value = ascii(data).parse::<f64>().ok()?;
            Some((nested(&["thresholds", key], json!(value)), json!(value)))
        }
        0x90..=0x95 => {
            let (_, key) = PRODUCT.iter().find(|(c, _)| *c == command)?;
            let text = ascii(data);
            let value = match *key {
                "amp_rating" => json!(text.parse::<i64>().ok()?),
                "surge" => json!(text == "Y"),
                _ => json!(text),
            };
            Some((nested(&["product", key], value.clone()), value))
        }
        _ => None,
    }
}

fn log_type(code: &str) -> &'static str {
    match code {
        "00" => "normal",
        "01" => "over_voltage",
        "02" => "under_voltage",
        "03" => "over_current",
        "04" => "under_current",
        "05" => "over_temperature",
        "06" => "under_temperature",
        "07" => "surge_fault",
        "08" => "auto_ping_fault",
        "09" => "rs232_ping_fail",
        "10" => "epo_initiate",
        "11" => "epo_recovery",
        _ => "unknown",
    }
}

impl RackLink {
    pub(crate) fn new(ctx: OpenContext) -> Result<RackLink, String> {
        let s = &ctx.settings;
        let text = |k: &str, d: &str| s.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
        let link = Link::from_settings(&ctx, DEFAULT_PORT)?;
        let premium = ctx.model != "racklink-select";
        if matches!(link, Link::Serial(_)) && ctx.model != "racklink-premium" {
            return Err("only RackLink Premium has an RS-232 control port".into());
        }
        let username = text("username", "user");
        let password = text("password", "");
        if username.contains('|') {
            return Err(
                "the user name cannot hold '|', which separates it from the password".into(),
            );
        }
        Ok(RackLink {
            link,
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            premium,
            username,
            password,
            poll_every: s
                .get("poll_interval_ms")
                .and_then(Value::as_u64)
                .unwrap_or(5000)
                .max(500),
            deframer: Deframer::default(),
            open: false,
            logged_in: false,
            refused: false,
            connected: false,
            misses: 0,
            retry_after: RETRY_MIN,
            queue: VecDeque::new(),
            in_flight: None,
            outlets: Vec::new(),
            contacts: Vec::new(),
        })
    }

    fn send(&self, cx: &mut Cx, envelope: &[u8]) {
        cx.tcp_send(SOCKET, frame(envelope));
    }

    fn login(&mut self, cx: &mut Cx) {
        let mut env = vec![0x00, LOGIN, SET];
        env.extend_from_slice(format!("{}|{}", self.username, self.password).as_bytes());
        self.send(cx, &env);
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
    }

    /// Queue a background read unless the same one waits already.
    fn read(&mut self, request: Request) {
        let queued = self
            .queue
            .iter()
            .chain(self.in_flight.as_ref().map(|(j, _)| j))
            .any(|j| j.id.is_none() && j.request == request);
        if !queued {
            self.queue.push_back(Job {
                id: None,
                request,
                returns_value: false,
                collected: Vec::new(),
            });
        }
    }

    /// What monitoring reads after logging in and at each poll.
    fn poll(&mut self, first: bool) {
        if !self.monitor {
            self.read(Request::new(OUTLET_COUNT, GET, &[]));
            return;
        }
        if first {
            if self.premium {
                self.queue.push_back(Job {
                    id: None,
                    request: Request::new(STATUS_REGISTRATION, SET, &REGISTER_ALL),
                    returns_value: false,
                    collected: Vec::new(),
                });
            }
            self.read(Request::new(OUTLET_COUNT, GET, &[]));
            self.read(Request::new(CONTACT_COUNT, GET, &[]));
            if self.premium {
                for (code, _) in PRODUCT {
                    self.read(Request::new(code, GET, &[]));
                }
                for (code, ..) in THRESHOLDS {
                    self.read(Request::new(code, GET, &[]));
                }
                for c in [LOG_COUNT, LOG_ALERTS, STATUS_REGISTRATION] {
                    self.read(Request::new(c, GET, &[]));
                }
            }
        } else {
            for n in self.outlets.clone() {
                self.read(Request::new(OUTLET, GET, &[n]));
            }
            for n in self.contacts.clone() {
                self.read(Request::new(CONTACT, GET, &[n]));
            }
        }
        if self.premium {
            for (code, ..) in SENSORS {
                self.read(Request::new(code, GET, &[]));
            }
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || !self.logged_in {
            return;
        }
        let Some(job) = self.queue.pop_front() else {
            return;
        };
        self.send(cx, &job.request.envelope);
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        self.in_flight = Some((job, cx.now()));
    }

    fn heard(&mut self, cx: &mut Cx) {
        self.misses = 0;
        cx.alive();
    }

    fn received(&mut self, cx: &mut Cx, env: &[u8]) {
        let (command, sub, data) = (env[1], env[2], &env[3..]);
        match (command, sub) {
            (PING, SET) => {
                self.send(cx, &[0x00, PING, RESPONSE]);
                self.heard(cx);
                return;
            }
            (LOGIN, RESPONSE) => {
                cx.cancel_timer(REPLY_TIMER);
                self.heard(cx);
                if data.first() == Some(&0x01) {
                    self.logged_in = true;
                    self.connected = true;
                    cx.connection(Connection::Connected);
                    self.poll(true);
                    cx.set_timer(POLL_TIMER, self.poll_every);
                    self.pump(cx);
                } else {
                    self.refused = true;
                    cx.connection(Connection::Unauthorized {
                        reason: "the unit rejected the login".into(),
                    });
                    self.fail_all(
                        cx,
                        CommandError::Auth {
                            message: "the unit rejected the login".into(),
                        },
                    );
                    cx.tcp_close(SOCKET);
                }
                return;
            }
            (NACK, _) => {
                self.heard(cx);
                let code = data.first().copied().unwrap_or(0);
                let reason = NACK_REASONS
                    .iter()
                    .find(|(c, _)| *c == code)
                    .map_or("unknown error", |(_, r)| r);
                if let Some((job, _)) = self.in_flight.take() {
                    cx.cancel_timer(REPLY_TIMER);
                    match job.id {
                        Some(id) => cx.complete(
                            id,
                            Err(CommandError::DeviceError {
                                code: Some(format!("{code:02x}")),
                                message: reason.into(),
                            }),
                        ),
                        None => cx.log(
                            Level::Debug,
                            format!(
                                "read {:02x} refused: {reason}",
                                job.request.envelope.get(1).copied().unwrap_or(0)
                            ),
                        ),
                    }
                }
                if code == 0x08 {
                    // Logged out: log in again.
                    self.logged_in = false;
                    self.login(cx);
                    return;
                }
                self.pump(cx);
                return;
            }
            _ => {}
        }
        self.heard(cx);
        let decoded = decode(command, data);
        if let Some((patch, _)) = &decoded {
            if patch.as_object().is_some_and(|m| !m.is_empty()) {
                cx.state(patch.clone());
            }
            match command {
                OUTLET_COUNT => self.outlets = present(data),
                CONTACT_COUNT => self.contacts = present(data),
                _ => {}
            }
            if matches!(command, OUTLET_COUNT | CONTACT_COUNT) && self.monitor {
                let (state, name) = if command == OUTLET_COUNT {
                    (OUTLET, OUTLET_NAME)
                } else {
                    (CONTACT, CONTACT_NAME)
                };
                let numbers = if command == OUTLET_COUNT {
                    self.outlets.clone()
                } else {
                    self.contacts.clone()
                };
                for n in numbers {
                    self.read(Request::new(state, GET, &[n]));
                    self.read(Request::new(name, GET, &[n]));
                    if command == OUTLET_COUNT && self.premium {
                        self.read(Request::new(ENERGY_STATE, GET, &[n]));
                    }
                }
            }
        }
        // Status changes and log updates are the unit's own.
        if sub == CHANGE || sub == LOG_UPDATE {
            return;
        }
        let answers = self.in_flight.as_ref().is_some_and(|(job, _)| {
            let (c, n) = job.request.answer;
            c == command && n.is_none_or(|n| data.first() == Some(&n))
        });
        if !answers {
            return;
        }
        let (mut job, sent_at) = self.in_flight.take().expect("in flight");
        let mut value = decoded.map(|(_, v)| v).unwrap_or(Value::Null);
        if command == READ_LOG {
            let pending = value.get("pending").and_then(Value::as_i64).unwrap_or(0);
            if !value.is_null() {
                job.collected.push(std::mem::take(&mut value));
            }
            if pending > 0 && job.collected.len() < job.request.responses {
                // More entries follow for the same request.
                self.in_flight = Some((job, sent_at));
                cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
                return;
            }
        }
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        if job.id.is_none() && self.monitor {
            self.stamps.answered(cx, "status");
        }
        if let Some(id) = job.id {
            let outcome = if command == READ_LOG {
                Outcome::Value {
                    value: json!({ "entries": job.collected }),
                }
            } else if job.returns_value {
                Outcome::Value { value }
            } else {
                Outcome::Ack
            };
            cx.complete(id, Ok(outcome));
            // Clearing the log reads its count back.
            if command == CLEAR_LOG {
                self.read(Request::new(LOG_COUNT, GET, &[]));
            }
        }
        self.pump(cx);
    }

    fn fail_all(&mut self, cx: &mut Cx, error: CommandError) {
        for job in self
            .in_flight
            .take()
            .map(|(j, _)| j)
            .into_iter()
            .chain(self.queue.drain(..))
        {
            if let Some(id) = job.id {
                cx.complete(id, Err(error.clone()));
            }
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.open = false;
        self.logged_in = false;
        self.deframer = Deframer::default();
        self.fail_all(
            cx,
            CommandError::Transport {
                message: reason.clone(),
            },
        );
        for key in [REPLY_TIMER, POLL_TIMER] {
            cx.cancel_timer(key);
        }
        if self.refused {
            return;
        }
        self.connected = false;
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn open_link(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.link.open(cx, SOCKET);
    }
}

/// The numbers a count response lists as present (controllable or not).
fn present(data: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .filter(|(_, l)| matches!(l, b'C' | b'N'))
        .map(|(i, _)| (i + 1) as u8)
        .collect()
}

impl Module for RackLink {
    fn start(&mut self, cx: &mut Cx) {
        self.open_link(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.refused {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: "the unit rejected the login".into(),
                }),
            );
            return;
        }
        if !self.open {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        if name == "refresh" {
            self.poll(true);
            self.pump(cx);
            cx.complete(id, Ok(Outcome::Ack));
            return;
        }
        match plan(name, params) {
            Ok((request, returns_value)) => {
                let job = Job {
                    id: Some(id),
                    request,
                    returns_value,
                    collected: Vec::new(),
                };
                // Commands go ahead of background reads.
                let at = self.queue.iter().position(|j| j.id.is_none());
                match at {
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
                self.login(cx);
            }
            TcpInput::Data(bytes) => {
                for env in self.deframer.feed(&bytes) {
                    self.received(cx, &env);
                }
            }
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => self.open_link(cx),
            REPLY_TIMER => {
                if !self.logged_in {
                    // The login went unanswered: start again.
                    cx.tcp_close(SOCKET);
                    self.lost(cx, "the unit did not answer the login".into());
                    return;
                }
                if let Some((job, _)) = self.in_flight.take() {
                    if let Some(id) = job.id {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES {
                        cx.log(Level::Warning, "the unit answered none of three requests");
                        cx.tcp_close(SOCKET);
                        self.lost(cx, "no reply from the unit".into());
                        return;
                    }
                }
                self.pump(cx);
            }
            POLL_TIMER if self.logged_in => {
                self.poll(false);
                self.pump(cx);
                cx.set_timer(POLL_TIMER, self.poll_every);
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
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    fn module(model: &str, monitor: bool) -> RackLink {
        RackLink::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 9)),
            host_name: None,
            port: None,
            model: model.into(),
            channels: None,
            settings: json!({ "username": "user", "password": "password" })
                .as_object()
                .unwrap()
                .clone(),
            monitor,
        })
        .unwrap()
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
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
        let mut out = json!({});
        for a in actions {
            if let Action::State(p) = a {
                crate::session::merge_patch(&mut out, p);
            }
        }
        out
    }

    fn feed(m: &mut RackLink, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(10);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn logged_in(m: &mut RackLink) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        feed(m, &hex("FE040002100115FF"))
    }

    #[test]
    fn frames_match_the_documents_examples() {
        // Login, outlet 1 on, outlet 2 cycle 5 s, a ping response, a name.
        assert_eq!(
            frame(&[
                0x00, LOGIN, SET, b'u', b's', b'e', b'r', b'|', b'p', b'a', b's', b's', b'w', b'o',
                b'r', b'd'
            ]),
            hex("FE10000201757365727C70617373776F72643FFF")
        );
        assert_eq!(
            frame(&hex("002001010130303030")),
            hex("fe090020010101303030306aff")
        );
        assert_eq!(
            frame(&hex("002001020230303035")),
            hex("fe0900200102023030303571ff")
        );
        assert_eq!(frame(&[0x00, PING, RESPONSE]), hex("FE0300011012FF"));
        assert_eq!(
            frame(&hex("002101016E616D65")),
            hex("FE08002101016E616D654AFF")
        );
        assert_eq!(
            frame(&hex("0036010130303033")),
            hex("FE08003601013030303301FF")
        );
        // A protected byte is escaped and not counted (the document's
        // example: 0x00 0x06 0x00 0xff).
        assert_eq!(frame(&hex("000600FF")), hex("FE04000600FD0007FF"));
        let mut d = Deframer::default();
        assert_eq!(d.feed(&hex("FE04000600FD0007FF")), vec![hex("000600FF")]);
        // A bad checksum is dropped.
        assert!(d.feed(&hex("FE040010100124FF")).is_empty());
    }

    #[test]
    fn it_logs_in_answers_pings_and_reads_the_unit() {
        let mut m = module("racklink-premium", true);
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        assert_eq!(
            sent(&cx.take()),
            vec![hex("FE10000201757365727C70617373776F72643FFF")]
        );
        let actions = feed(&mut m, &hex("FE040002100115FF"));
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        // First the status registration, one request at a time.
        assert_eq!(
            sent(&actions),
            vec![frame(&[
                0x00, 0x41, 0x01, 0x01, 0x0F, 0x5B, 0x01, 0x7F, 0x0F
            ])]
        );
        // A ping is answered at once, whatever waits.
        assert_eq!(
            sent(&feed(&mut m, &hex("FE0300010103FF"))),
            vec![hex("FE0300011012FF")]
        );
        // The registration's answer, then the outlet count.
        let a = feed(
            &mut m,
            &frame(&[0x00, 0x41, 0x10, 0x01, 0x0F, 0x5B, 0x01, 0x7F, 0x0F]),
        );
        assert_eq!(state(&a)["status_registration"], "01 0f 5b 01 7f 0f");
        assert_eq!(sent(&a), vec![frame(&[0x00, OUTLET_COUNT, GET])]);
        let mut count = vec![0x00, OUTLET_COUNT, RESPONSE];
        count.extend_from_slice(b"CCNXXXXXXXXXXXXX");
        let a = feed(&mut m, &frame(&count));
        let s = state(&a);
        assert_eq!(s["outlets"]["1"]["controllable"], true);
        assert_eq!(s["outlets"]["3"]["controllable"], false);
        assert!(s["outlets"]["4"].is_null());
        assert_eq!(m.outlets, vec![1, 2, 3]);
        // An outlet's state, and a pushed change.
        let a = feed(&mut m, &frame(&hex("002010010130303035")));
        assert_eq!(
            state(&a)["outlets"]["1"],
            json!({ "state": "on", "cycle_time": 5 })
        );
        let pushed = frame(&hex("002012020030303035"));
        let a = feed(&mut m, &pushed);
        assert_eq!(state(&a)["outlets"]["2"]["state"], "off");
    }

    #[test]
    fn a_rejected_login_is_unauthorized_and_not_retried() {
        let mut m = module("racklink-select", true);
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take();
        let a = feed(&mut m, &frame(&[0x00, LOGIN, RESPONSE, 0x00]));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        let mut cx = Cx::new(20);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "closed".into(),
            },
        );
        assert!(!cx.take().iter().any(|x| matches!(
            x,
            Action::SetTimer {
                key: RETRY_TIMER,
                ..
            }
        )));
    }

    #[test]
    fn commands_are_sent_and_answered() {
        let mut m = module("racklink-premium", false);
        logged_in(&mut m);
        // The liveness read goes first; answer it.
        let mut count = vec![0x00, OUTLET_COUNT, RESPONSE];
        count.extend_from_slice(b"CCCCCCCCXXXXXXXX");
        feed(&mut m, &frame(&count));
        let mut cx = Cx::new(30);
        let p = json!({ "outlet": 2, "action": "cycle", "cycle_s": 5 });
        m.command(&mut cx, 1, "set_outlet", p.as_object().unwrap());
        assert_eq!(sent(&cx.take()), vec![hex("fe0900200102023030303571ff")]);
        let a = feed(&mut m, &frame(&hex("002010020130303035")));
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));
        // A NACK fails the command with its reason.
        let mut cx = Cx::new(40);
        let p = json!({ "threshold": "max_load", "value": 15.5 });
        m.command(&mut cx, 2, "set_threshold", p.as_object().unwrap());
        assert_eq!(
            sent(&cx.take()),
            vec![frame(&[0x00, 0x73, 0x01, b'1', b'5', b'.', b'5'])]
        );
        let a = feed(&mut m, &frame(&[0x00, NACK, RESPONSE, 0x07]));
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Complete {
                id: 2,
                result: Err(CommandError::DeviceError { .. })
            }
        )));
        // Sequencing, energy state, a sensor read.
        let mut cx = Cx::new(50);
        let p = json!({ "direction": "down", "delay_s": 5 });
        m.command(&mut cx, 3, "sequence_outlets", p.as_object().unwrap());
        assert_eq!(sent(&cx.take()), vec![hex("FE08003601033030303505FF")]);
        let a = feed(&mut m, &frame(&hex("0036010330303035")));
        assert_eq!(
            state(&a)["sequence"],
            json!({ "state": "sequencing_down", "delay": 5 })
        );
        let mut cx = Cx::new(60);
        m.command(
            &mut cx,
            4,
            "get_sensor",
            json!({ "sensor": "rms_load" }).as_object().unwrap(),
        );
        assert_eq!(sent(&cx.take()), vec![frame(&[0x00, 0x54, GET])]);
        let a = feed(
            &mut m,
            &frame(&[0x00, 0x54, RESPONSE, b'0', b'7', b'.', b'5']),
        );
        assert!(a.contains(&Action::Complete {
            id: 4,
            result: Ok(Outcome::Value { value: json!(7.5) })
        }));
        assert_eq!(state(&a)["sensors"]["rms_load"], 7.5);
    }

    #[test]
    fn a_log_read_collects_every_entry() {
        let mut m = module("racklink-premium-plus", false);
        logged_in(&mut m);
        feed(&mut m, &frame(&[0x00, OUTLET_COUNT, RESPONSE, b'C']));
        let mut cx = Cx::new(30);
        m.command(
            &mut cx,
            9,
            "read_log",
            json!({ "start": 2, "count": 2 }).as_object().unwrap(),
        );
        let mut get = vec![0x00, READ_LOG, GET];
        get.extend_from_slice(b"0002|02");
        assert_eq!(sent(&cx.take()), vec![frame(&get)]);
        let entry = |text: &str| {
            let mut e = vec![0x00, READ_LOG, RESPONSE];
            e.extend_from_slice(text.as_bytes());
            frame(&e)
        };
        // One entry is still to come: nothing completes yet.
        assert!(!feed(
            &mut m,
            &entry("01|01|0002|05|10/02/2024 10:00:00,091,0120,1.0,120,01.0,0409.5,O,")
        )
        .iter()
        .any(|x| matches!(x, Action::Complete { .. })));
        let a = feed(
            &mut m,
            &entry("02|00|0003|00|10/02/2024 11:00:00,072,0100,1.0,121,00.8,0341.2,U,"),
        );
        let value = a.iter().find_map(|x| match x {
            Action::Complete {
                id: 9,
                result: Ok(Outcome::Value { value }),
            } => Some(value.clone()),
            _ => None,
        });
        let value = value.unwrap();
        assert_eq!(value["entries"].as_array().unwrap().len(), 2);
        assert_eq!(value["entries"][0]["type"], "over_temperature");
    }

    #[test]
    fn decoding_reads_each_kind() {
        let (p, _) = decode(0x60, b"OIIOSDU").unwrap();
        assert_eq!(p["sensors"]["occupancy"], "occupied");
        assert_eq!(p["outlets"]["1"]["energy_state"], "on");
        assert_eq!(p["outlets"]["6"]["energy_state"], "unknown");
        let (p, _) = decode(0x59, &[0x02]).unwrap();
        assert_eq!(p["sensors"]["surge_protection"], "compromised");
        let (p, _) = decode(0x93, b"Y").unwrap();
        assert_eq!(p["product"]["surge"], true);
        let (p, _) = decode(EPO, &[0x01]).unwrap();
        assert_eq!(p["emergency_power_off"], true);
        assert_eq!(hex_bytes("01 0f:5B"), Some(vec![1, 15, 0x5b]));
    }
}
