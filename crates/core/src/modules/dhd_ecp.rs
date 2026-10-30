//! DHD's External Control Protocol (ECP) over TCP, for RM4200D frames and
//! Series 52 cores.
//!
//! Wire format from DHD's developer documentation (version 10.4, "ECP
//! Communication" and "ECP Commands"):
//!
//! - Every block is 16 bytes: the data length (0-8), an unused byte, the
//!   29-bit command ID in four bytes, eight data bytes and two unused bytes,
//!   big-endian. Unused bytes are sent as zero and ignored on receipt, and so
//!   is the low half of the ID (the controller module number on routing).
//! - Nothing is acknowledged over TCP. A request is the same block type with
//!   a shorter length, and its answer repeats the address it asked about, so
//!   a command's answer is matched by command ID, answer length and address
//!   bytes. A set the device does not know is silently dropped: commands with
//!   a request form send the set and then the request, and complete with the
//!   value answered (or time out after 2 s); the others report unverified.
//! - The same block types arrive unasked: crosspoint and fader value
//!   notifications, motor fader touches, meters, clean feed changes. Every
//!   block the device sends goes to state, whoever asked for it.
//!
//! When monitoring, the module switches on the crosspoint and fader value
//! notifications and the configured meters on connecting, and reads what the
//! `read_*` settings name, paced so the device's socket buffer is not
//! flooded. Opened for commands only it switches nothing on and reads
//! nothing: commands send only their own blocks, and what arrives is still
//! applied. The protocol has no request every configuration answers, so the
//! TCP connection is the only liveness signal.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::{DeviceSpec, Params};
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

/// ECP over IP listens on TCP 2008.
pub(crate) const DEFAULT_PORT: u16 = 2008;
const SOCKET: Key = "ecp";

const RETRY_TIMER: Key = "retry";
const PACE_TIMER: Key = "pace";
const DEADLINE_TIMER: Key = "deadline";

const BLOCK: usize = 16;
/// How long a command waits for the answer to its request.
const ANSWER_TIMEOUT: Millis = 2_000;
/// How often pending answers are checked against their deadline.
const DEADLINE_CHECK: Millis = 250;
/// Queued reads go out this many at a time, this often.
const PACE_BLOCKS: usize = 16;
const PACE_EVERY: Millis = 20;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

// Command IDs, the high 16 bits (the low 16 are zero when sent).
const ROUTE: u16 = 0x01F5;
const ARBITRATION: u16 = 0x01F9;
const MONITOR: u16 = 0x1100;
const FADER_ON: u16 = 0x1102;
const FADER_LEVEL: u16 = 0x1103;
const ACCESS: u16 = 0x1104;
const INPUT: u16 = 0x110D;
const LOGIC: u16 = 0x110E;
const POT: u16 = 0x1110;
const LABEL: u16 = 0x1114;
const PFL: u16 = 0x1116;
const LOAD_SAVE: u16 = 0x111C;
const METER: u16 = 0x111D;
const IO_GAIN: u16 = 0x112D;
const PARAMETER: u16 = 0x1181;
const TOUCH: u16 = 0x1182;

/// The clean feed parameter prefixes of 0x1181.
const CF_INPUT: u16 = 0x0024;
const CF_OUTPUT: u16 = 0x0025;
/// FaderHi addressing a fader channel rather than a logic fader.
const BY_CHANNEL: u8 = 0x10;

/// One 16-byte block as sent: length, unused, the ID, the data, zeros.
pub(crate) fn block(id: u16, data: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; BLOCK];
    b[0] = data.len() as u8;
    b[2..4].copy_from_slice(&id.to_be_bytes());
    b[6..6 + data.len()].copy_from_slice(data);
    b
}

/// A block as received.
#[derive(Debug, Clone, PartialEq)]
struct Block {
    id: u16,
    len: usize,
    data: [u8; 8],
}

impl Block {
    fn parse(raw: &[u8]) -> Block {
        let mut data = [0u8; 8];
        data.copy_from_slice(&raw[6..14]);
        Block {
            // ID28..ID24 are the low five bits of the first ID byte.
            id: u16::from_be_bytes([raw[2] & 0x1F, raw[3]]),
            len: usize::from(raw[0] & 0x0F).min(8),
            data,
        }
    }

    fn word(&self, at: usize) -> u16 {
        u16::from_be_bytes([self.data[at], self.data[at + 1]])
    }

    fn long(&self, at: usize) -> u32 {
        u32::from_be_bytes([
            self.data[at],
            self.data[at + 1],
            self.data[at + 2],
            self.data[at + 3],
        ])
    }
}

/// Hundredths of a dB, signed 16-bit, as a number of dB.
fn db(raw: u16) -> Value {
    json!(f64::from(raw as i16) / 100.0)
}

fn to_hundredths(level: f64) -> [u8; 2] {
    let v = (level * 100.0).round().clamp(-32768.0, 32767.0) as i16;
    v.to_be_bytes()
}

fn hex4(v: u16) -> String {
    format!("{v:04X}")
}

fn hex8(v: u32) -> String {
    format!("{v:08X}")
}

/// Where a fader address points: a logic fader or a fader channel.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Fader {
    Logic { mixer: u8, fader: u8 },
    Channel(u8),
}

impl Fader {
    fn decode(hi: u8, lo: u8) -> Option<Fader> {
        match hi {
            0..=3 => Some(Fader::Logic {
                mixer: hi + 1,
                fader: lo + 1,
            }),
            BY_CHANNEL => Some(Fader::Channel(lo)),
            _ => None,
        }
    }

    fn wire(self) -> [u8; 2] {
        match self {
            Fader::Logic { mixer, fader } => [mixer - 1, fader - 1],
            Fader::Channel(c) => [BY_CHANNEL, c],
        }
    }

    /// The state object for this fader, created when absent.
    fn node<'a>(&self, patch: &'a mut Map<String, Value>) -> &'a mut Map<String, Value> {
        match *self {
            Fader::Logic { mixer, fader } => {
                let m = object(object(patch, "mixers"), &mixer.to_string());
                object(object(m, "faders"), &fader.to_string())
            }
            Fader::Channel(c) => object(object(patch, "channels"), &c.to_string()),
        }
    }
}

/// The child object at `key`, created when absent.
fn object<'a>(map: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    if !map.get(key).is_some_and(Value::is_object) {
        map.insert(key.into(), json!({}));
    }
    map.get_mut(key).and_then(Value::as_object_mut).unwrap()
}

const ACCESS_FUNCTIONS: [&str; 3] = ["gain", "pan_balance", "input_select"];

/// What a block says: the state it carries and the value a command reading
/// it returns. `None` for a block this module does not read.
fn decode(b: &Block) -> Option<(Map<String, Value>, Value)> {
    let d = &b.data;
    let mut patch = Map::new();
    let value = match (b.id, b.len) {
        (ROUTE, 8) => {
            let input = hex8(b.long(4));
            object(&mut patch, "routes").insert(hex8(b.long(0)), json!(input));
            json!(input)
        }
        (ROUTE, 4) => {
            let input = hex4(b.word(2));
            object(&mut patch, "routes_16").insert(hex4(b.word(0)), json!(input));
            json!(input)
        }
        (ARBITRATION, 5) => {
            object(&mut patch, "arbitration").insert(hex8(b.long(0)), json!(d[4]));
            json!(d[4])
        }
        (MONITOR, 6) => {
            let v = json!({"left": hex4(b.word(0)), "right": hex4(b.word(4))});
            object(&mut patch, "monitors").insert((u16::from(d[2]) + 1).to_string(), v.clone());
            v
        }
        (FADER_ON, 3) => {
            let fader = Fader::decode(d[0], d[1])?;
            fader.node(&mut patch).insert("on".into(), json!(d[2] != 0));
            json!(d[2] != 0)
        }
        (FADER_LEVEL, 4) => {
            let fader = Fader::decode(d[0], d[1])?;
            let level = db(b.word(2));
            fader.node(&mut patch).insert("level".into(), level.clone());
            level
        }
        (ACCESS, 7) => {
            let Fader::Logic { mixer, fader } = Fader::decode(d[0], d[1])? else {
                return None;
            };
            let v = json!({
                "mixer": mixer,
                "fader": fader,
                "enabled": d[2] != 0,
                "function": ACCESS_FUNCTIONS.get(usize::from(d[3])).copied()?,
                "channel": b.word(4),
                "group": d[6] + 1,
            });
            let mut entry = v.as_object().cloned().unwrap();
            entry.remove("group");
            object(&mut patch, "access").insert((d[6] + 1).to_string(), Value::Object(entry));
            v
        }
        (INPUT, 4) => {
            let fader = Fader::decode(d[0], d[1])?;
            fader
                .node(&mut patch)
                .insert("input".into(), json!(b.word(2)));
            json!(b.word(2))
        }
        (LOGIC, 3) => {
            object(&mut patch, "logics").insert(b.word(0).to_string(), json!(d[2] != 0));
            json!(d[2] != 0)
        }
        (POT, 4) => {
            let level = db(b.word(2));
            object(object(&mut patch, "pots"), &(b.word(0) + 1).to_string())
                .insert("level".into(), level.clone());
            level
        }
        (LABEL, 8) => {
            let text: String = d[4..8]
                .iter()
                .map(|&c| {
                    if c.is_ascii_graphic() || c == b' ' {
                        c as char
                    } else {
                        ' '
                    }
                })
                .collect();
            let text = text.trim_end().to_string();
            let target = b.word(0);
            let node = if target & 0x8000 != 0 {
                object(object(&mut patch, "panels"), &(target & 0xFF).to_string())
            } else {
                object(object(&mut patch, "channels"), &target.to_string())
            };
            object(node, "labels").insert(d[2].to_string(), json!(text));
            json!(text)
        }
        (PFL, 4) => {
            let fader = Fader::decode(d[0], d[1])?;
            let node = fader.node(&mut patch);
            node.insert("pfl".into(), json!(d[2] != 0));
            node.insert("pfl_auto_mute".into(), json!(d[3] != 0));
            json!({"pfl": d[2] != 0, "auto_mute": d[3] != 0})
        }
        (IO_GAIN, 4) => {
            let gain = db(b.word(2));
            object(&mut patch, "io_gains").insert(hex4(b.word(0)), gain.clone());
            gain
        }
        (IO_GAIN, 6) => {
            let gain = db(b.word(4));
            object(&mut patch, "io_gains").insert(hex8(b.long(0)), gain.clone());
            gain
        }
        (METER, 6) => {
            let v = json!({
                "correlation": f64::from(d[1] as i8) / 100.0,
                "left": db(b.word(2)),
                "right": db(b.word(4)),
            });
            object(&mut patch, "meters").insert(d[0].to_string(), v.clone());
            v
        }
        (PARAMETER, 8) => {
            let channel = b.word(0).to_string();
            match b.word(2) {
                CF_INPUT => {
                    let gain = d[7] as i8;
                    object(
                        object(object(&mut patch, "channels"), &channel),
                        "cleanfeed",
                    )
                    .insert("input_gain".into(), json!(gain));
                    json!(gain)
                }
                CF_OUTPUT => {
                    let v = cf_output(b.long(4));
                    let node = object(
                        object(object(&mut patch, "channels"), &channel),
                        "cleanfeed",
                    );
                    for (k, val) in v.as_object().unwrap() {
                        node.insert(k.clone(), val.clone());
                    }
                    v
                }
                _ => return None,
            }
        }
        (TOUCH, 2) => {
            object(object(&mut patch, "channels"), &d[0].to_string())
                .insert("touched".into(), json!(d[1] != 0));
            json!(d[1] != 0)
        }
        _ => return None,
    };
    Some((patch, value))
}

/// The clean feed output word: bit 18 n-mix, 17 alternative, 16 mute, 15-8
/// the alternative list position, 7-0 the gain as a signed byte.
fn cf_output(word: u32) -> Value {
    json!({
        "n_mix": word & (1 << 18) != 0,
        "alternative": word & (1 << 17) != 0,
        "muted": word & (1 << 16) != 0,
        "alternative_source": (word >> 8) & 0x7F,
        "gain": (word & 0xFF) as u8 as i8,
    })
}

/// The answer a command waits for: its ID, length and the data bytes it
/// repeats.
#[derive(Debug, Clone, PartialEq)]
struct Want {
    id: u16,
    len: usize,
    prefix: Vec<(usize, u8)>,
}

impl Want {
    fn new(id: u16, len: usize, prefix: &[u8]) -> Want {
        Want {
            id,
            len,
            prefix: prefix.iter().copied().enumerate().collect(),
        }
    }

    fn at(id: u16, len: usize, prefix: &[(usize, u8)]) -> Want {
        Want {
            id,
            len,
            prefix: prefix.to_vec(),
        }
    }

    fn matches(&self, b: &Block) -> bool {
        b.id == self.id && b.len == self.len && self.prefix.iter().all(|&(i, v)| b.data[i] == v)
    }
}

/// What a command sends and what completes it.
#[derive(Debug, Clone, PartialEq)]
struct Job {
    blocks: Vec<Vec<u8>>,
    /// The answer that completes it; `None` completes it once sent.
    want: Option<Want>,
    /// With no answer to wait for: unverified (a set) or ack (a request
    /// whose answers only update state).
    ack: bool,
}

impl Job {
    fn set(blocks: Vec<Vec<u8>>) -> Job {
        Job {
            blocks,
            want: None,
            ack: false,
        }
    }

    fn read(blocks: Vec<Vec<u8>>, want: Want) -> Job {
        Job {
            blocks,
            want: Some(want),
            ack: false,
        }
    }
}

struct Pending {
    command: CommandId,
    want: Want,
    sent: Millis,
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn int(p: &Params, key: &str) -> Result<i64, CommandError> {
    p.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn opt_int(p: &Params, key: &str) -> Option<i64> {
    p.get(key).and_then(Value::as_i64)
}

fn boolean(p: &Params, key: &str) -> Result<bool, CommandError> {
    p.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn float(p: &Params, key: &str) -> Result<f64, CommandError> {
    p.get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn text<'a>(p: &'a Params, key: &str) -> Result<&'a str, CommandError> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

/// A hex parameter of the given number of digits as its bytes.
fn hex(p: &Params, key: &str, digits: usize) -> Result<Vec<u8>, CommandError> {
    let s = text(p, key)?;
    if s.len() != digits || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid(format!("'{key}' is {digits} hex digits")));
    }
    Ok((0..digits)
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect())
}

pub(crate) struct DhdEcp {
    device: SocketAddr,
    /// Firmware 05: one mixer of 40 faders, no fader channels.
    fw5: bool,
    supports: HashSet<String>,
    monitor: bool,
    read_mixers: u8,
    read_faders: u8,
    read_monitors: u8,
    read_pots: u8,
    meters: u8,
    open: bool,
    connected: bool,
    buffer: Vec<u8>,
    /// Reads waiting to go out, paced.
    queue: VecDeque<Vec<u8>>,
    pending: Vec<Pending>,
    /// The last clean feed output word read per channel, so a command can
    /// change one field of it.
    cf_words: HashMap<u16, u32>,
    retry_after: Millis,
}

impl DhdEcp {
    pub(crate) fn new(spec: &DeviceSpec, ctx: OpenContext) -> Result<DhdEcp, String> {
        let model = spec
            .models
            .iter()
            .find(|m| m.id == ctx.model)
            .ok_or_else(|| format!("no DHD ECP model '{}'", ctx.model))?;
        let num = |key: &str, default: u64| {
            ctx.settings
                .get(key)
                .and_then(Value::as_u64)
                .unwrap_or(default)
                .min(255) as u8
        };
        Ok(DhdEcp {
            device: SocketAddr::new(ctx.host, ctx.port.unwrap_or(DEFAULT_PORT)),
            fw5: model.id == "rm4200d-fw5",
            supports: model.supports.iter().cloned().collect(),
            monitor: ctx.monitor,
            read_mixers: num("read_mixers", 1),
            read_faders: num("read_faders", 0),
            read_monitors: num("read_monitors", 0),
            read_pots: num("read_pots", 0),
            meters: num("meters", 0),
            open: false,
            connected: false,
            buffer: Vec::new(),
            queue: VecDeque::new(),
            pending: Vec::new(),
            cf_words: HashMap::new(),
            retry_after: RETRY_MIN,
        })
    }

    /// The fader a command addresses: a fader channel when given, else a
    /// logic fader of a virtual mixer.
    fn fader(&self, p: &Params) -> Result<Fader, CommandError> {
        if let Some(channel) = opt_int(p, "channel") {
            if self.fw5 {
                return Err(invalid("firmware 05 has no fader channels: give a fader"));
            }
            return Ok(Fader::Channel(channel as u8));
        }
        let fader = opt_int(p, "fader").ok_or_else(|| invalid("give a fader or a channel"))?;
        let mixer = opt_int(p, "mixer").unwrap_or(1);
        if self.fw5 && (mixer != 1 || fader > 40) {
            return Err(invalid("firmware 05 has mixer 1 with faders 1-40"));
        }
        Ok(Fader::Logic {
            mixer: mixer as u8,
            fader: fader as u8,
        })
    }

    /// The display a label command addresses: a fader channel or a central
    /// operating panel module (0x8001-0x800C).
    fn label_target(p: &Params) -> Result<[u8; 2], CommandError> {
        match (opt_int(p, "channel"), opt_int(p, "panel")) {
            (Some(c), None) => Ok((c as u16).to_be_bytes()),
            (None, Some(m)) => Ok((0x8000 | m as u16).to_be_bytes()),
            _ => Err(invalid("give either a channel or a panel")),
        }
    }

    fn level_block(id: u16, target: [u8; 2], p: &Params) -> Result<Vec<u8>, CommandError> {
        let level = to_hundredths(float(p, "level")?);
        let fade = opt_int(p, "fade_ms").unwrap_or(0) as u16;
        let curve = match p.get("curve").and_then(Value::as_str) {
            Some("level_linear") => 0,
            _ => 1,
        };
        Ok(if fade == 0 {
            block(id, &[target[0], target[1], level[0], level[1]])
        } else {
            let t = fade.to_be_bytes();
            block(
                id,
                &[target[0], target[1], level[0], level[1], t[0], t[1], curve],
            )
        })
    }

    fn command_job(&self, name: &str, p: &Params) -> Result<Job, CommandError> {
        Ok(match name {
            "set_route" => {
                let (out, input) = (hex(p, "output", 8)?, hex(p, "input", 8)?);
                Job::read(
                    vec![
                        block(ROUTE, &[out.clone(), input].concat()),
                        block(ROUTE, &[&out[..], &[0, 0, 0, 0, 0]].concat()[..5]),
                    ],
                    Want::new(ROUTE, 8, &out),
                )
            }
            "get_route" => {
                let out = hex(p, "output", 8)?;
                Job::read(
                    vec![block(ROUTE, &[&out[..], &[0]].concat())],
                    Want::new(ROUTE, 8, &out),
                )
            }
            "set_route_16" => {
                let (out, input) = (hex(p, "output", 4)?, hex(p, "input", 4)?);
                Job::read(
                    vec![
                        block(ROUTE, &[out.clone(), input].concat()),
                        block(ROUTE, &out),
                    ],
                    Want::new(ROUTE, 4, &out),
                )
            }
            "get_route_16" => {
                let out = hex(p, "output", 4)?;
                Job::read(vec![block(ROUTE, &out)], Want::new(ROUTE, 4, &out))
            }
            "get_all_routes_16" => Job {
                blocks: vec![block(ROUTE, &[])],
                want: None,
                ack: true,
            },
            "set_crosspoint_notification" => {
                Job::set(vec![block(ROUTE, &[u8::from(boolean(p, "enabled")?)])])
            }
            "set_arbitration" => {
                let id = hex(p, "audio_id", 8)?;
                let owner = int(p, "owner")? as u8;
                Job::read(
                    vec![
                        block(ARBITRATION, &[&id[..], &[owner]].concat()),
                        block(ARBITRATION, &id),
                    ],
                    Want::new(ARBITRATION, 5, &id),
                )
            }
            "get_arbitration" => {
                let id = hex(p, "audio_id", 8)?;
                Job::read(
                    vec![block(ARBITRATION, &id)],
                    Want::new(ARBITRATION, 5, &id),
                )
            }
            "set_monitor_channel" => {
                let n = (int(p, "monitor")? - 1) as u8;
                let (l, r) = (hex(p, "left", 4)?, hex(p, "right", 4)?);
                Job::read(
                    vec![
                        block(MONITOR, &[l[0], l[1], n, 0, r[0], r[1]]),
                        block(MONITOR, &[0, 0, n]),
                    ],
                    Want::at(MONITOR, 6, &[(2, n)]),
                )
            }
            "get_monitor_channel" => {
                let n = (int(p, "monitor")? - 1) as u8;
                Job::read(
                    vec![block(MONITOR, &[0, 0, n])],
                    Want::at(MONITOR, 6, &[(2, n)]),
                )
            }
            "set_fader_on" => {
                let f = self.fader(p)?.wire();
                Job::set(vec![block(
                    FADER_ON,
                    &[f[0], f[1], u8::from(boolean(p, "enabled")?)],
                )])
            }
            "set_fader_level" => {
                let f = self.fader(p)?.wire();
                Job::read(
                    vec![
                        Self::level_block(FADER_LEVEL, f, p)?,
                        block(FADER_LEVEL, &f),
                    ],
                    Want::new(FADER_LEVEL, 4, &f),
                )
            }
            "get_fader_level" => {
                let f = self.fader(p)?.wire();
                Job::read(vec![block(FADER_LEVEL, &f)], Want::new(FADER_LEVEL, 4, &f))
            }
            "set_fader_notification" => Job::set(vec![block(
                FADER_LEVEL,
                &[u8::from(boolean(p, "enabled")?)],
            )]),
            "set_fader_access" => {
                let f = self.fader(p)?.wire();
                let function = match text(p, "function")? {
                    "gain" => 0,
                    "pan_balance" => 1,
                    "input_select" => 2,
                    other => return Err(invalid(format!("unknown function '{other}'"))),
                };
                Job::set(vec![block(
                    ACCESS,
                    &[
                        f[0],
                        f[1],
                        u8::from(boolean(p, "enabled")?),
                        function,
                        0,
                        0,
                        0,
                    ],
                )])
            }
            "get_fader_access" => Job::read(vec![block(ACCESS, &[])], Want::new(ACCESS, 7, &[])),
            "set_fader_input" | "get_fader_input" => {
                let mixer = opt_int(p, "mixer").unwrap_or(1);
                let fader = int(p, "fader")?;
                if self.fw5 && (mixer != 1 || fader > 40) {
                    return Err(invalid("firmware 05 has mixer 1 with faders 1-40"));
                }
                let f = [(mixer - 1) as u8, (fader - 1) as u8];
                let read = block(INPUT, &f);
                let blocks = if name == "set_fader_input" {
                    let c = (int(p, "input")? as u16).to_be_bytes();
                    vec![block(INPUT, &[f[0], f[1], c[0], c[1]]), read]
                } else {
                    vec![read]
                };
                Job::read(blocks, Want::new(INPUT, 4, &f))
            }
            "get_all_fader_inputs" => Job {
                blocks: vec![block(INPUT, &[])],
                want: None,
                ack: true,
            },
            "set_logic" | "get_logic" => {
                let id = (int(p, "logic")? as u16).to_be_bytes();
                let mut blocks = Vec::new();
                if name == "set_logic" {
                    blocks.push(block(
                        LOGIC,
                        &[id[0], id[1], u8::from(boolean(p, "value")?)],
                    ));
                }
                blocks.push(block(LOGIC, &id));
                Job::read(blocks, Want::new(LOGIC, 3, &id))
            }
            "set_pot_level" => {
                let pot = ((int(p, "pot")? - 1) as u16).to_be_bytes();
                let set = Self::level_block(POT, pot, p)?;
                if self.supports.contains("get_pot_level") {
                    Job::read(vec![set, block(POT, &pot)], Want::new(POT, 4, &pot))
                } else {
                    Job::set(vec![set])
                }
            }
            "get_pot_level" => {
                let pot = ((int(p, "pot")? - 1) as u16).to_be_bytes();
                Job::read(vec![block(POT, &pot)], Want::new(POT, 4, &pot))
            }
            "set_label" => {
                let t = Self::label_target(p)?;
                let display = int(p, "display")? as u8;
                let s = text(p, "text")?;
                if s.len() > 4 || !s.bytes().all(|c| (0x20..0x7F).contains(&c)) {
                    return Err(invalid("text is up to four printable ASCII characters"));
                }
                let mut chars = [b' '; 4];
                chars[..s.len()].copy_from_slice(s.as_bytes());
                Job::read(
                    vec![
                        block(LABEL, &[&[t[0], t[1], display, 0][..], &chars].concat()),
                        block(LABEL, &[t[0], t[1], display]),
                    ],
                    Want::new(LABEL, 8, &[t[0], t[1], display]),
                )
            }
            "reset_label" => {
                let t = Self::label_target(p)?;
                Job::set(vec![block(LABEL, &t)])
            }
            "get_label" => {
                let t = Self::label_target(p)?;
                let display = int(p, "display")? as u8;
                Job::read(
                    vec![block(LABEL, &[t[0], t[1], display])],
                    Want::new(LABEL, 8, &[t[0], t[1], display]),
                )
            }
            "set_pfl" => {
                let f = self.fader(p)?.wire();
                let set = block(
                    PFL,
                    &[
                        f[0],
                        f[1],
                        u8::from(boolean(p, "enabled")?),
                        u8::from(p.get("exclusive").and_then(Value::as_bool) == Some(true)),
                    ],
                );
                if self.supports.contains("get_pfl") {
                    Job::read(vec![set, block(PFL, &f)], Want::new(PFL, 4, &f))
                } else {
                    Job::set(vec![set])
                }
            }
            "get_pfl" => {
                let f = self.fader(p)?.wire();
                Job::read(vec![block(PFL, &f)], Want::new(PFL, 4, &f))
            }
            "clear_pfl" => {
                if self.fw5 {
                    Job::set(vec![block(PFL, &[])])
                } else {
                    let mixer = opt_int(p, "mixer").unwrap_or(1) as u8;
                    Job::set(vec![block(PFL, &[mixer - 1])])
                }
            }
            "load_channel_snapshot" | "save_channel_snapshot" => {
                let f = self.fader(p)?.wire();
                let action = if name.starts_with("load") { 0x00 } else { 0x01 };
                let snapshot = int(p, "snapshot")? as u8;
                Job::set(vec![block(LOAD_SAVE, &[action, snapshot, f[0], f[1]])])
            }
            "load_mixer_snapshot" | "save_mixer_snapshot" => {
                let action = if name.starts_with("load") { 0x0A } else { 0x0B };
                let snapshot = int(p, "snapshot")? as u8;
                let mixer = opt_int(p, "mixer").unwrap_or(1) as u8;
                Job::set(vec![block(LOAD_SAVE, &[action, snapshot, mixer - 1])])
            }
            "set_io_gain_16" | "get_io_gain_16" => {
                let io = hex(p, "io", 4)?;
                let mut blocks = Vec::new();
                if name == "set_io_gain_16" {
                    let g = to_hundredths(float(p, "gain")?);
                    blocks.push(block(IO_GAIN, &[io[0], io[1], g[0], g[1]]));
                }
                blocks.push(block(IO_GAIN, &io));
                Job::read(blocks, Want::new(IO_GAIN, 4, &io))
            }
            "set_io_gain" | "get_io_gain" => {
                let id = hex(p, "audio_id", 8)?;
                let mut blocks = Vec::new();
                if name == "set_io_gain" {
                    let g = to_hundredths(float(p, "gain")?);
                    blocks.push(block(IO_GAIN, &[&id[..], &g].concat()));
                }
                blocks.push(block(IO_GAIN, &id));
                Job::read(blocks, Want::new(IO_GAIN, 6, &id))
            }
            "set_meter_notification" => Job::set(vec![block(
                METER,
                &[int(p, "meter")? as u8, u8::from(boolean(p, "enabled")?)],
            )]),
            "set_cleanfeed_input_gain" | "get_cleanfeed_input_gain" => {
                let c = (int(p, "channel")? as u16).to_be_bytes();
                let head = [c[0], c[1], 0x00, CF_INPUT as u8];
                let mut blocks = Vec::new();
                if name == "set_cleanfeed_input_gain" {
                    let gain = int(p, "gain")? as i8 as u8;
                    blocks.push(block(PARAMETER, &[&head[..], &[0, 0, 0, gain]].concat()));
                }
                blocks.push(block(PARAMETER, &head));
                Job::read(blocks, Want::new(PARAMETER, 8, &head))
            }
            "set_cleanfeed_output" | "get_cleanfeed_output" => {
                let channel = int(p, "channel")? as u16;
                let c = channel.to_be_bytes();
                let head = [c[0], c[1], 0x00, CF_OUTPUT as u8];
                let mut blocks = Vec::new();
                if name == "set_cleanfeed_output" {
                    let word = self.cf_word(channel, p)?.to_be_bytes();
                    blocks.push(block(PARAMETER, &[&head[..], &word].concat()));
                }
                blocks.push(block(PARAMETER, &head));
                Job::read(blocks, Want::new(PARAMETER, 8, &head))
            }
            other => {
                return Err(CommandError::UnknownCommand {
                    command: other.into(),
                })
            }
        })
    }

    /// The clean feed output word a set sends: the fields given, the others
    /// as last read for the channel.
    fn cf_word(&self, channel: u16, p: &Params) -> Result<u32, CommandError> {
        let last = self.cf_words.get(&channel).copied();
        let flag = |key: &str, bit: u32| -> Result<u32, CommandError> {
            match (p.get(key).and_then(Value::as_bool), last) {
                (Some(v), _) => Ok(u32::from(v) << bit),
                (None, Some(w)) => Ok(w & (1 << bit)),
                (None, None) => Err(invalid(format!(
                    "'{key}' is needed until the channel's clean feed output has been read (get_cleanfeed_output)"
                ))),
            }
        };
        let source = match (opt_int(p, "alternative_source"), last) {
            (Some(v), _) => (v as u32 & 0x7F) << 8,
            (None, Some(w)) => w & 0x7F00,
            (None, None) => {
                return Err(invalid(
                    "'alternative_source' is needed until the channel's clean feed output has been read (get_cleanfeed_output)",
                ))
            }
        };
        let gain = match (opt_int(p, "gain"), last) {
            (Some(v), _) => u32::from(v as i8 as u8),
            (None, Some(w)) => w & 0xFF,
            (None, None) => {
                return Err(invalid(
                    "'gain' is needed until the channel's clean feed output has been read (get_cleanfeed_output)",
                ))
            }
        };
        Ok(flag("n_mix", 18)? | flag("alternative", 17)? | flag("muted", 16)? | source | gain)
    }

    /// The reads the settings name, queued to go out paced.
    fn queue_reads(&mut self, cx: &mut Cx) {
        let has = |c: &str| self.supports.contains(c);
        let mut reads = Vec::new();
        for m in 0..self.read_mixers.min(if self.fw5 { 1 } else { 4 }) {
            for f in 0..self.read_faders.min(if self.fw5 { 40 } else { 48 }) {
                reads.push(block(FADER_LEVEL, &[m, f]));
                reads.push(block(INPUT, &[m, f]));
                if has("get_pfl") {
                    reads.push(block(PFL, &[m, f]));
                }
            }
        }
        for n in 0..self.read_monitors {
            reads.push(block(MONITOR, &[0, 0, n]));
        }
        if has("get_pot_level") {
            for n in 0..u16::from(self.read_pots) {
                reads.push(block(POT, &n.to_be_bytes()));
            }
        }
        let idle = self.queue.is_empty();
        self.queue.extend(reads);
        if idle && !self.queue.is_empty() {
            self.pace(cx);
        }
    }

    fn pace(&mut self, cx: &mut Cx) {
        if !self.open {
            return;
        }
        for _ in 0..PACE_BLOCKS {
            match self.queue.pop_front() {
                Some(b) => cx.tcp_send(SOCKET, b),
                None => return,
            }
        }
        if !self.queue.is_empty() {
            cx.set_timer(PACE_TIMER, PACE_EVERY);
        }
    }

    fn on_connected(&mut self, cx: &mut Cx) {
        self.open = true;
        self.connected = true;
        self.retry_after = RETRY_MIN;
        cx.connection(Connection::Connected);
        if !self.monitor {
            return;
        }
        if self.supports.contains("set_crosspoint_notification") {
            cx.tcp_send(SOCKET, block(ROUTE, &[1]));
        }
        if self.supports.contains("set_fader_notification") {
            cx.tcp_send(SOCKET, block(FADER_LEVEL, &[1]));
        }
        if self.supports.contains("set_meter_notification") {
            for meter in 1..=self.meters.min(48) {
                cx.tcp_send(SOCKET, block(METER, &[meter, 1]));
            }
        }
        self.queue_reads(cx);
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        for p in self.pending.drain(..) {
            cx.complete(
                p.command,
                Err(CommandError::Transport {
                    message: reason.clone(),
                }),
            );
        }
        cx.cancel_timer(PACE_TIMER);
        cx.cancel_timer(DEADLINE_TIMER);
        self.queue.clear();
        self.buffer.clear();
        self.open = false;
        self.connected = false;
        cx.connection(Connection::Disconnected {
            reason: reason.clone(),
        });
        cx.log(Level::Debug, format!("DHD ECP: {reason}"));
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn on_block(&mut self, cx: &mut Cx, b: &Block) {
        if b.id == PARAMETER && b.len == 8 && b.word(2) == CF_OUTPUT {
            self.cf_words.insert(b.word(0), b.long(4));
        }
        let Some((patch, value)) = decode(b) else {
            cx.log(
                Level::Debug,
                format!("DHD ECP: ignored block {:04X}0000 length {}", b.id, b.len),
            );
            return;
        };
        cx.state(Value::Object(patch));
        let now = cx.now();
        let (done, waiting): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|p| p.want.matches(b));
        self.pending = waiting;
        for p in done {
            cx.round_trip(now.saturating_sub(p.sent));
            cx.complete(
                p.command,
                Ok(Outcome::Value {
                    value: value.clone(),
                }),
            );
        }
    }
}

impl Module for DhdEcp {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.open {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        if name == "refresh" {
            self.queue_reads(cx);
            cx.complete(id, Ok(Outcome::Ack));
            return;
        }
        let job = match self.command_job(name, params) {
            Ok(job) => job,
            Err(e) => {
                cx.complete(id, Err(e));
                return;
            }
        };
        for b in job.blocks {
            cx.tcp_send(SOCKET, b);
        }
        match job.want {
            Some(want) => {
                if self.pending.is_empty() {
                    cx.set_timer(DEADLINE_TIMER, DEADLINE_CHECK);
                }
                self.pending.push(Pending {
                    command: id,
                    want,
                    sent: cx.now(),
                });
            }
            None if job.ack => cx.complete(id, Ok(Outcome::Ack)),
            None => cx.complete(id, Ok(Outcome::Unverified)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => self.on_connected(cx),
            TcpInput::Data(data) => {
                if !self.open {
                    return;
                }
                cx.alive();
                self.buffer.extend_from_slice(&data);
                let whole = self.buffer.len() / BLOCK * BLOCK;
                let raw: Vec<u8> = self.buffer.drain(..whole).collect();
                for chunk in raw.chunks(BLOCK) {
                    self.on_block(cx, &Block::parse(chunk));
                }
            }
            // Dropped, or never made: either way, retry with backoff.
            TcpInput::Closed { reason } => self.lost(cx, reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER => {
                if !self.open {
                    cx.connection(Connection::Connecting);
                    cx.tcp_open(SOCKET, self.device);
                }
            }
            PACE_TIMER => self.pace(cx),
            DEADLINE_TIMER => {
                let now = cx.now();
                let (expired, waiting): (Vec<Pending>, Vec<Pending>) =
                    std::mem::take(&mut self.pending)
                        .into_iter()
                        .partition(|p| now.saturating_sub(p.sent) >= ANSWER_TIMEOUT);
                self.pending = waiting;
                for p in expired {
                    cx.complete(p.command, Err(CommandError::Timeout));
                }
                if !self.pending.is_empty() {
                    cx.set_timer(DEADLINE_TIMER, DEADLINE_CHECK);
                }
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.open {
            cx.tcp_close(SOCKET);
        }
        self.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn module(model: &str, monitor: bool, settings: Value) -> DhdEcp {
        let spec = Catalog::source_tree().devices["dhd-ecp"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        DhdEcp::new(
            &spec,
            OpenContext {
                host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 52)),
                host_name: None,
                port: None,
                model: model.into(),
                channels: None,
                settings,
                monitor,
            },
        )
        .unwrap()
    }

    /// A block as DHD's examples write it: ID, length, data bytes.
    fn ex(id: u32, data: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 16];
        b[0] = data.len() as u8;
        b[2..6].copy_from_slice(&id.to_be_bytes());
        b[6..6 + data.len()].copy_from_slice(data);
        b
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

    fn connected(m: &mut DhdEcp) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    fn run_at(m: &mut DhdEcp, now: Millis, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    fn run(m: &mut DhdEcp, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        run_at(m, 0, id, name, p)
    }

    fn feed_at(m: &mut DhdEcp, now: Millis, bytes: &[u8]) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(bytes.to_vec()));
        cx.take()
    }

    fn feed(m: &mut DhdEcp, bytes: &[u8]) -> Vec<Action> {
        feed_at(m, 0, bytes)
    }

    fn idle(model: &str) -> DhdEcp {
        let mut m = module(model, false, json!({}));
        connected(&mut m);
        m
    }

    #[test]
    fn blocks_match_the_documented_examples() {
        let mut m = idle("52-xc2");
        // "01F50000,8,40,08,08,03,40,01,00,02 sets a crosspoint on output
        // 0x40080803 with source 0x40010002", then the length-5 request.
        let a = run(
            &mut m,
            1,
            "set_route",
            json!({"output": "40080803", "input": "40010002"}),
        );
        assert_eq!(
            sent(&a),
            vec![
                ex(
                    0x01F50000,
                    &[0x40, 0x08, 0x08, 0x03, 0x40, 0x01, 0x00, 0x02]
                ),
                ex(0x01F50000, &[0x40, 0x08, 0x08, 0x03, 0x00]),
            ]
        );
        // "0x11020000,3,10,4D,00 will switch channel no. 77 to OFF".
        let a = run(
            &mut m,
            2,
            "set_fader_on",
            json!({"channel": 77, "enabled": false}),
        );
        assert_eq!(sent(&a), vec![ex(0x11020000, &[0x10, 0x4D, 0x00])]);
        assert_eq!(completed(&a, 2), Some(Ok(Outcome::Unverified)));
        // "0x11020000,3,02,16,00 will switch fader no. 23 at Virtual Mixer 3 to OFF".
        let a = run(
            &mut m,
            3,
            "set_fader_on",
            json!({"mixer": 3, "fader": 23, "enabled": false}),
        );
        assert_eq!(sent(&a), vec![ex(0x11020000, &[0x02, 0x16, 0x00])]);
        // "11000000,6,03,00,05,00,03,01 routes input signal 1/2 from slot 3
        // onto monitor channel no. 6".
        let a = run(
            &mut m,
            4,
            "set_monitor_channel",
            json!({"monitor": 6, "left": "0300", "right": "0301"}),
        );
        assert_eq!(
            sent(&a)[0],
            ex(0x11000000, &[0x03, 0x00, 0x05, 0x00, 0x03, 0x01])
        );
        // "01F50000,4,05,80,01,02 will route slot 1, input port 3 to slot 5,
        // output port 1".
        let a = run(
            &mut m,
            5,
            "set_route_16",
            json!({"output": "0580", "input": "0102"}),
        );
        assert_eq!(
            sent(&a),
            vec![
                ex(0x01F50000, &[0x05, 0x80, 0x01, 0x02]),
                ex(0x01F50000, &[0x05, 0x80]),
            ]
        );
        // "01F90000,4,40,03,18,06 request owner of Audio ID 0x40031806".
        let a = run(
            &mut m,
            6,
            "get_arbitration",
            json!({"audio_id": "40031806"}),
        );
        assert_eq!(sent(&a), vec![ex(0x01F90000, &[0x40, 0x03, 0x18, 0x06])]);
        // The layout is length, unused, the ID, data, two unused bytes.
        assert_eq!(
            block(0x1103, &[0x00, 0x05, 0xFC, 0x18]),
            vec![4, 0, 0x11, 0x03, 0, 0, 0x00, 0x05, 0xFC, 0x18, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn a_read_back_completes_the_command_and_updates_state() {
        let mut m = idle("52-xc2");
        let a = run_at(
            &mut m,
            100,
            1,
            "set_fader_level",
            json!({"mixer": 1, "fader": 6, "level": -10.0, "fade_ms": 1500, "curve": "level_linear"}),
        );
        assert_eq!(
            sent(&a),
            vec![
                // -10.00 dB is -1000 = 0xFC18; 1500 ms is 0x05DC; type 0.
                ex(0x11030000, &[0x00, 0x05, 0xFC, 0x18, 0x05, 0xDC, 0x00]),
                ex(0x11030000, &[0x00, 0x05]),
            ]
        );
        assert_eq!(completed(&a, 1), None);
        // An answer for another fader does not complete it.
        let a = feed_at(&mut m, 120, &ex(0x11030000, &[0x00, 0x04, 0x00, 0x00]));
        assert_eq!(completed(&a, 1), None);
        let a = feed_at(&mut m, 140, &ex(0x11030000, &[0x00, 0x05, 0xFE, 0x0C]));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(-5.0) }))
        );
        assert!(a.contains(&Action::RoundTrip(40)));
        assert_eq!(
            state(&a)["mixers"]["1"]["faders"]["6"]["level"],
            json!(-5.0)
        );
    }

    #[test]
    fn an_unanswered_read_times_out() {
        let mut m = idle("52-xc2");
        run_at(&mut m, 0, 1, "get_logic", json!({"logic": 719}));
        let mut cx = Cx::new(ANSWER_TIMEOUT);
        m.timer(&mut cx, DEADLINE_TIMER);
        let a = cx.take();
        assert_eq!(completed(&a, 1), Some(Err(CommandError::Timeout)));
    }

    #[test]
    fn pushed_blocks_reach_state() {
        let mut m = idle("52-xc2");
        let mut bytes = Vec::new();
        // A crosspoint notification, a motor fader touch, a meter, a clean
        // feed output change and a label, in one read and split mid-block.
        bytes.extend(ex(
            0x01F50003,
            &[0x40, 0x08, 0x08, 0x03, 0x40, 0x01, 0x00, 0x02],
        ));
        bytes.extend(ex(0x11820000, &[12, 1]));
        bytes.extend(ex(0x111D0000, &[3, 0xCE, 0xFC, 0x18, 0xF8, 0x30]));
        bytes.extend(ex(
            0x11810000,
            &[0x00, 0x07, 0x00, 0x25, 0x00, 0x05, 0x03, 0xFB],
        ));
        bytes.extend(ex(
            0x11140000,
            &[0x00, 0x07, 0x01, 0x00, b'M', b'I', b'C', b' '],
        ));
        bytes.extend(ex(
            0x11140000,
            &[0x80, 0x02, 0x00, 0x00, b'P', b'G', b'M', b'1'],
        ));
        let (first, rest) = bytes.split_at(20);
        let mut a = feed(&mut m, first);
        a.extend(feed(&mut m, rest));
        let s = state(&a);
        assert_eq!(s["routes"]["40080803"], "40010002");
        assert_eq!(s["channels"]["12"]["touched"], true);
        assert_eq!(
            s["meters"]["3"],
            json!({"correlation": -0.5, "left": -10.0, "right": -20.0})
        );
        assert_eq!(
            s["channels"]["7"]["cleanfeed"],
            json!({"n_mix": true, "alternative": false, "muted": true, "alternative_source": 3, "gain": -5})
        );
        assert_eq!(s["channels"]["7"]["labels"]["1"], "MIC");
        assert_eq!(s["panels"]["2"]["labels"]["0"], "PGM1");
    }

    #[test]
    fn a_clean_feed_output_field_keeps_the_others_as_read() {
        let mut m = idle("52-xc2");
        let a = run(
            &mut m,
            1,
            "set_cleanfeed_output",
            json!({"channel": 7, "muted": false}),
        );
        assert!(matches!(
            completed(&a, 1),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        feed(
            &mut m,
            &ex(
                0x11810000,
                &[0x00, 0x07, 0x00, 0x25, 0x00, 0x05, 0x03, 0xFB],
            ),
        );
        let a = run(
            &mut m,
            2,
            "set_cleanfeed_output",
            json!({"channel": 7, "muted": false}),
        );
        assert_eq!(
            sent(&a),
            vec![
                ex(
                    0x11810000,
                    &[0x00, 0x07, 0x00, 0x25, 0x00, 0x04, 0x03, 0xFB]
                ),
                ex(0x11810000, &[0x00, 0x07, 0x00, 0x25]),
            ]
        );
    }

    #[test]
    fn monitored_it_switches_notifications_on_and_reads_the_settings() {
        let mut m = module(
            "52-xc2",
            true,
            json!({"read_mixers": 1, "read_faders": 2, "read_monitors": 1, "read_pots": 1, "meters": 2}),
        );
        let a = connected(&mut m);
        assert_eq!(
            sent(&a),
            vec![
                ex(0x01F50000, &[1]),
                ex(0x11030000, &[1]),
                ex(0x111D0000, &[1, 1]),
                ex(0x111D0000, &[2, 1]),
                ex(0x11030000, &[0, 0]),
                ex(0x110D0000, &[0, 0]),
                ex(0x11160000, &[0, 0]),
                ex(0x11030000, &[0, 1]),
                ex(0x110D0000, &[0, 1]),
                ex(0x11160000, &[0, 1]),
                ex(0x11000000, &[0, 0, 0]),
                ex(0x11100000, &[0, 0]),
            ]
        );
    }

    #[test]
    fn opened_for_commands_only_nothing_is_sent_unasked() {
        let mut m = module("52-xc2", false, json!({"read_faders": 4, "meters": 2}));
        let a = connected(&mut m);
        assert!(sent(&a).is_empty());
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // A command still goes out, and what arrives is still applied.
        let a = run(&mut m, 1, "get_logic", json!({"logic": 719}));
        assert_eq!(sent(&a), vec![ex(0x110E0000, &[0x02, 0xCF])]);
        let a = feed(&mut m, &ex(0x110E0000, &[0x02, 0xCF, 1]));
        assert_eq!(
            completed(&a, 1),
            Some(Ok(Outcome::Value { value: json!(true) }))
        );
        assert_eq!(state(&a)["logics"]["719"], true);
    }

    #[test]
    fn firmware_5_has_no_fader_channels() {
        let mut m = idle("rm4200d-fw5");
        let a = run(&mut m, 1, "get_fader_level", json!({"channel": 3}));
        assert!(matches!(
            completed(&a, 1),
            Some(Err(CommandError::InvalidParams { .. }))
        ));
        let a = run(&mut m, 2, "clear_pfl", json!({}));
        assert_eq!(sent(&a), vec![ex(0x11160000, &[])]);
        // Without the PFL request, set_pfl is not read back.
        let a = run(&mut m, 3, "set_pfl", json!({"fader": 3, "enabled": true}));
        assert_eq!(completed(&a, 3), Some(Ok(Outcome::Unverified)));
    }

    #[test]
    fn a_dropped_connection_fails_what_waits_and_reconnects() {
        let mut m = idle("52-xc2");
        run(&mut m, 1, "get_route", json!({"output": "40080803"}));
        let mut cx = Cx::new(0);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        let a = cx.take();
        assert!(matches!(
            completed(&a, 1),
            Some(Err(CommandError::Transport { .. }))
        ));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key, .. } if *key == RETRY_TIMER)));
    }
}
