//! Raspberry Pi GPIO through the pigpio daemon's socket interface (pigpiod,
//! TCP 8888), as the pigpio documentation (abyz.me.uk/rpi/pigpio, public
//! domain) gives it.
//!
//! - Each request is four little-endian 32-bit words, cmd, p1, p2 and p3 (p3
//!   the length of an extension that follows, for the few commands taking
//!   one), echoed back with the result in the fourth word: a value, or a
//!   negative error code. One request is outstanding at a time.
//! - A second connection is turned into a notification stream with NOIB
//!   (its answer is the handle); NB on the command connection starts
//!   reports for the monitored GPIO. Each report is 12 bytes: sequence
//!   number, flags (watchdog timeout, keep-alive, event, with the GPIO or
//!   event in bits 0-4), tick, and every GPIO's level. Levels reach state
//!   as they change; watchdog timeouts and events are message events.
//! - On connecting, the hardware revision, the pigpio version, and each
//!   monitored GPIO's mode and, for outputs, PWM duty cycle, range and
//!   frequency and servo pulse width are read; modes again every
//!   `poll_interval_ms`, since another program may change them. A command
//!   that sets something is followed by a read of it.
//!
//! Opened for commands only (`monitor` false), no notification stream is
//! opened and nothing is read; a TICK every 10 s of silence stays as the
//! liveness check. Every answered request reports its round trip.

use std::collections::VecDeque;
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    PollStamps, TcpInput,
};

pub(crate) const PORT: u16 = 8888;
const COMMAND: Key = "pigpio";
const NOTIFY: Key = "notify";
const REPLY_TIMER: Key = "reply";
const POLL_TIMER: Key = "poll";
const RETRY_TIMER: Key = "retry";
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const LIVENESS: Millis = 10_000;
const MISSES: u32 = 3;

pub(crate) const MODES: u32 = 0;
pub(crate) const MODEG: u32 = 1;
pub(crate) const PUD: u32 = 2;
pub(crate) const READ: u32 = 3;
pub(crate) const WRITE: u32 = 4;
pub(crate) const PWM: u32 = 5;
pub(crate) const PRS: u32 = 6;
pub(crate) const PFS: u32 = 7;
pub(crate) const SERVO: u32 = 8;
pub(crate) const WDOG: u32 = 9;
pub(crate) const BR1: u32 = 10;
pub(crate) const BR2: u32 = 11;
pub(crate) const BC1: u32 = 12;
pub(crate) const BC2: u32 = 13;
pub(crate) const BS1: u32 = 14;
pub(crate) const BS2: u32 = 15;
pub(crate) const TICK: u32 = 16;
pub(crate) const HWVER: u32 = 17;
pub(crate) const NB: u32 = 19;
pub(crate) const PRG: u32 = 22;
pub(crate) const PFG: u32 = 23;
pub(crate) const PRRG: u32 = 24;
pub(crate) const PIGPV: u32 = 26;
pub(crate) const TRIG: u32 = 37;
pub(crate) const GDC: u32 = 83;
pub(crate) const GPW: u32 = 84;
pub(crate) const HC: u32 = 85;
pub(crate) const HP: u32 = 86;
pub(crate) const FG: u32 = 97;
pub(crate) const FN: u32 = 98;
pub(crate) const NOIB: u32 = 99;
pub(crate) const PADS: u32 = 102;
pub(crate) const PADG: u32 = 103;
pub(crate) const EVT: u32 = 116;

const FLAG_WDOG: u16 = 1 << 5;
const FLAG_ALIVE: u16 = 1 << 6;
const FLAG_EVENT: u16 = 1 << 7;

/// A request: cmd, p1, p2, and an extension (p3 is its length).
pub(crate) fn request(cmd: u32, p1: u32, p2: u32, ext: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + ext.len());
    for w in [cmd, p1, p2, ext.len() as u32] {
        out.extend_from_slice(&w.to_le_bytes());
    }
    out.extend_from_slice(ext);
    out
}

/// The error codes the GPIO functions return (pigpio.h, Error Codes).
fn error_text(code: i32) -> &'static str {
    match code {
        -2 => "GPIO not 0-31 (PI_BAD_USER_GPIO)",
        -3 => "GPIO not 0-53 (PI_BAD_GPIO)",
        -4 => "mode not 0-7 (PI_BAD_MODE)",
        -5 => "level not 0-1 (PI_BAD_LEVEL)",
        -6 => "pull not 0-2 (PI_BAD_PUD)",
        -7 => "pulse width not 0 or 500-2500 (PI_BAD_PULSEWIDTH)",
        -8 => "duty cycle outside the range set (PI_BAD_DUTYCYCLE)",
        -15 => "watchdog timeout not 0-60000 (PI_BAD_WDOG_TIMEOUT)",
        -21 => "duty cycle range not 25-40000 (PI_BAD_DUTYRANGE)",
        -24 => "no handle available (PI_NO_HANDLE)",
        -25 => "unknown handle (PI_BAD_HANDLE)",
        -41 => "GPIO operation not permitted (PI_NOT_PERMITTED)",
        -42 => "one or more GPIO not permitted (PI_SOME_PERMITTED)",
        -46 => "trigger pulse length not 1-100 (PI_BAD_PULSELEN)",
        -92 => "GPIO is not in use for PWM (PI_NOT_PWM_GPIO)",
        -93 => "GPIO is not in use for servo pulses (PI_NOT_SERVO_GPIO)",
        -94 => "GPIO has no hardware clock (PI_NOT_HCLK_GPIO)",
        -95 => "GPIO has no hardware PWM (PI_NOT_HPWM_GPIO)",
        -96 => "invalid hardware PWM frequency (PI_BAD_HPWM_FREQ)",
        -97 => "hardware PWM duty cycle not 0-1M (PI_BAD_HPWM_DUTY)",
        -98 => "invalid hardware clock frequency (PI_BAD_HCLK_FREQ)",
        -99 => "need password to use hardware clock 1 (PI_BAD_HCLK_PASS)",
        -100 => "illegal, PWM in use for main clock (PI_HPWM_ILLEGAL)",
        -125 => "filter steady time not 0-300000 (PI_BAD_FILTER)",
        -126 => "bad pad number (PI_BAD_PAD)",
        -127 => "bad pad drive strength (PI_BAD_STRENGTH)",
        -143 => "event id not 0-31 (PI_BAD_EVENT_ID)",
        _ => "pigpio error",
    }
}

fn mode_name(mode: i32) -> &'static str {
    match mode {
        0 => "input",
        1 => "output",
        4 => "alt0",
        5 => "alt1",
        6 => "alt2",
        7 => "alt3",
        3 => "alt4",
        2 => "alt5",
        _ => "unknown",
    }
}

fn mode_code(name: &str) -> Option<u32> {
    Some(match name {
        "input" => 0,
        "output" => 1,
        "alt0" => 4,
        "alt1" => 5,
        "alt2" => 6,
        "alt3" => 7,
        "alt4" => 3,
        "alt5" => 2,
        _ => return None,
    })
}

/// The `gpios` setting: numbers and ranges, such as 2-27 or 4,17,22.
pub(crate) fn parse_gpios(text: &str) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for item in text
        .split([',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let (a, b) = match item.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (item, item),
        };
        let (a, b): (u32, u32) = match (a.parse(), b.parse()) {
            (Ok(a), Ok(b)) if a <= b && b <= 31 => (a, b),
            _ => return Err(format!("'{item}' is not a GPIO 0-31 or a range of them")),
        };
        for g in a..=b {
            if !out.contains(&g) {
                out.push(g);
            }
        }
    }
    out.sort_unstable();
    Ok(out)
}

/// What a request's answer is for.
#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    Command(CommandId, Answer),
    /// A read into state.
    Read,
    Liveness,
    /// NB: the stream's reports start.
    Begin,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Answer {
    /// Success is a result of 0.
    Ack,
    /// The result is the value (unsigned for the commands that cannot fail).
    Value { unsigned: bool },
    /// The result is a mode code.
    Mode,
}

#[derive(Debug, Clone)]
struct Request {
    bytes: Vec<u8>,
    purpose: Purpose,
}

pub(crate) struct Pigpio {
    to: SocketAddr,
    gpios: Vec<u32>,
    monitor: bool,
    /// When each poll was last answered (`polled.<group>`, SPEC.md §8).
    stamps: PollStamps,
    poll_every: Millis,
    timeout: Millis,
    open: bool,
    closed: bool,
    reachable: bool,
    misses: u32,
    retry_after: Millis,
    buf: Vec<u8>,
    notify_buf: Vec<u8>,
    /// The notification handle, once NOIB answered.
    handle: Option<u32>,
    notify_open: bool,
    last_levels: Option<u32>,
    queue: VecDeque<Request>,
    in_flight: Option<(Request, Millis)>,
    heard_at: Millis,
}

impl Pigpio {
    pub(crate) fn new(ctx: OpenContext) -> Result<Pigpio, String> {
        if ctx.host.is_unspecified() {
            return Err("give the Raspberry Pi's address as host".into());
        }
        let s = &ctx.settings;
        Ok(Pigpio {
            to: SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)),
            gpios: parse_gpios(s.get("gpios").and_then(Value::as_str).unwrap_or("2-27"))?,
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            poll_every: s
                .get("poll_interval_ms")
                .and_then(Value::as_u64)
                .unwrap_or(5000),
            timeout: s
                .get("timeout_ms")
                .and_then(Value::as_u64)
                .unwrap_or(2000)
                .max(1),
            open: false,
            closed: false,
            reachable: false,
            misses: 0,
            retry_after: RETRY_MIN,
            buf: Vec::new(),
            notify_buf: Vec::new(),
            handle: None,
            notify_open: false,
            last_levels: None,
            queue: VecDeque::new(),
            in_flight: None,
            heard_at: 0,
        })
    }

    fn connect(&mut self, cx: &mut Cx) {
        self.closed = false;
        cx.connection(Connection::Connecting);
        cx.tcp_open(COMMAND, self.to);
        if self.monitor {
            cx.tcp_open(NOTIFY, self.to);
        }
    }

    fn read(&mut self, cmd: u32, p1: u32) {
        let bytes = request(cmd, p1, 0, &[]);
        let waiting = self.queue.iter().any(|r| r.bytes == bytes)
            || self
                .in_flight
                .as_ref()
                .is_some_and(|(r, _)| r.bytes == bytes);
        if !waiting {
            self.queue.push_back(Request {
                bytes,
                purpose: Purpose::Read,
            });
        }
    }

    fn read_modes(&mut self) {
        for g in self.gpios.clone() {
            self.read(MODEG, g);
        }
        self.read(BR1, 0);
    }

    /// The settings of an output: its PWM and servo state.
    fn read_output(&mut self, gpio: u32) {
        for cmd in [GDC, GPW, PRG, PRRG, PFG] {
            self.read(cmd, gpio);
        }
    }

    fn pump(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() || !self.open {
            return;
        }
        let Some(r) = self.queue.pop_front() else {
            return;
        };
        cx.tcp_send(COMMAND, r.bytes.clone());
        cx.set_timer(REPLY_TIMER, self.timeout);
        self.in_flight = Some((r, cx.now()));
    }

    fn enqueue(&mut self, cx: &mut Cx, r: Request) {
        let at = self
            .queue
            .iter()
            .position(|q| !matches!(q.purpose, Purpose::Command(..)));
        match at {
            Some(at) => self.queue.insert(at, r),
            None => self.queue.push_back(r),
        }
        self.pump(cx);
    }

    fn gpio_patch(gpio: u32, value: Value) -> Value {
        json!({"gpio": { gpio.to_string(): value }})
    }

    /// An answer on the command connection.
    fn answered(&mut self, cx: &mut Cx, words: [u32; 4]) {
        let Some((r, sent_at)) = self.in_flight.take() else {
            return;
        };
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.misses = 0;
        self.heard_at = cx.now();
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
        let [cmd, p1, p2, res] = words;
        let signed = res as i32;
        match &r.purpose {
            Purpose::Read | Purpose::Command(..)
                if signed >= 0 || matches!(cmd, BR1 | BR2 | TICK | HWVER | PIGPV) =>
            {
                self.reflect(cx, cmd, p1, p2, res);
            }
            _ => {}
        }
        if matches!(r.purpose, Purpose::Read) && signed >= 0 && self.monitor {
            self.stamps.answered(cx, "status");
        }
        match r.purpose {
            Purpose::Command(id, answer) => {
                let unsigned = matches!(answer, Answer::Value { unsigned: true });
                let result = if signed < 0 && !unsigned {
                    Err(CommandError::DeviceError {
                        code: Some(signed.to_string()),
                        message: error_text(signed).into(),
                    })
                } else {
                    Ok(match answer {
                        Answer::Ack => Outcome::Ack,
                        Answer::Value { unsigned: true } => Outcome::Value { value: json!(res) },
                        Answer::Value { unsigned: false } => Outcome::Value {
                            value: json!(signed),
                        },
                        Answer::Mode => Outcome::Value {
                            value: json!(mode_name(signed)),
                        },
                    })
                };
                // A setting is read back.
                if result.is_ok() && self.monitor {
                    match cmd {
                        MODES => {
                            self.read(MODEG, p1);
                            if p2 == 1 {
                                self.read_output(p1);
                            }
                        }
                        PWM | PRS | PFS | SERVO | WRITE | HP => self.read_output(p1),
                        PADS => self.read(PADG, p1),
                        _ => {}
                    }
                }
                cx.complete(id, result);
            }
            Purpose::Begin if signed < 0 => {
                cx.log(
                    Level::Warning,
                    format!("pigpio refused notifications: {}", error_text(signed)),
                );
            }
            _ => {}
        }
        self.pump(cx);
    }

    /// What an answer says of the Pi's state.
    fn reflect(&mut self, cx: &mut Cx, cmd: u32, p1: u32, p2: u32, res: u32) {
        let signed = res as i32;
        match cmd {
            MODEG => {
                cx.state(Pigpio::gpio_patch(p1, json!({"mode": mode_name(signed)})));
                if signed == 1 && self.monitor {
                    self.read_output(p1);
                }
            }
            MODES => cx.state(Pigpio::gpio_patch(
                p1,
                json!({"mode": mode_name(p2 as i32)}),
            )),
            PUD => {
                let pull = match p2 {
                    0 => "off",
                    1 => "down",
                    _ => "up",
                };
                cx.state(Pigpio::gpio_patch(p1, json!({"pull": pull})));
            }
            READ => cx.state(Pigpio::gpio_patch(p1, json!({"level": res != 0}))),
            WRITE => cx.state(Pigpio::gpio_patch(p1, json!({"level": p2 != 0}))),
            BR1 => self.levels(cx, res),
            GDC => cx.state(Pigpio::gpio_patch(p1, json!({"pwm_dutycycle": signed}))),
            GPW => cx.state(Pigpio::gpio_patch(p1, json!({"servo_pulsewidth": signed}))),
            PRG => cx.state(Pigpio::gpio_patch(p1, json!({"pwm_range": signed}))),
            PRRG => cx.state(Pigpio::gpio_patch(p1, json!({"pwm_real_range": signed}))),
            PFG => cx.state(Pigpio::gpio_patch(p1, json!({"pwm_frequency": signed}))),
            WDOG => cx.state(Pigpio::gpio_patch(p1, json!({"watchdog_ms": p2}))),
            HWVER => cx.state(json!({"hardware_revision": format!("{res:x}")})),
            PIGPV => cx.state(json!({"pigpio_version": res})),
            PADG => cx.state(json!({"pads": { p1.to_string(): {"strength_ma": signed}}})),
            _ => {}
        }
    }

    /// Every monitored GPIO's level from a bank 1 word.
    fn levels(&mut self, cx: &mut Cx, bits: u32) {
        let mut gpio = Map::new();
        for g in &self.gpios {
            let level = bits & (1 << g) != 0;
            let changed = self
                .last_levels
                .is_none_or(|l| (l & (1 << g) != 0) != level);
            if changed {
                gpio.insert(g.to_string(), json!({"level": level}));
            }
        }
        self.last_levels = Some(bits);
        if !gpio.is_empty() {
            cx.state(json!({ "gpio": gpio }));
        }
    }

    /// Reports on the notification stream.
    fn reports(&mut self, cx: &mut Cx) {
        while self.notify_buf.len() >= 12 {
            let r: Vec<u8> = self.notify_buf.drain(..12).collect();
            let flags = u16::from_le_bytes([r[2], r[3]]);
            let tick = u32::from_le_bytes([r[4], r[5], r[6], r[7]]);
            let level = u32::from_le_bytes([r[8], r[9], r[10], r[11]]);
            cx.alive();
            let source = self.to;
            if flags & FLAG_WDOG != 0 {
                let gpio = u32::from(flags & 0x1F);
                cx.message(
                    "watchdog",
                    None,
                    json!({"gpio": gpio, "tick": tick}),
                    source,
                );
            } else if flags & FLAG_EVENT != 0 {
                let event = u32::from(flags & 0x1F);
                cx.message("event", None, json!({"event": event, "tick": tick}), source);
            } else if flags & FLAG_ALIVE == 0 {
                self.levels(cx, level);
            }
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        if self.closed {
            return;
        }
        self.open = false;
        self.closed = true;
        self.reachable = false;
        self.handle = None;
        self.notify_open = false;
        self.buf.clear();
        self.notify_buf.clear();
        self.last_levels = None;
        for r in self
            .in_flight
            .take()
            .map(|(r, _)| r)
            .into_iter()
            .chain(self.queue.drain(..))
        {
            if let Purpose::Command(id, _) = r.purpose {
                cx.complete(
                    id,
                    Err(CommandError::Transport {
                        message: reason.clone(),
                    }),
                );
            }
        }
        cx.tcp_close(COMMAND);
        cx.tcp_close(NOTIFY);
        cx.cancel_timer(REPLY_TIMER);
        cx.cancel_timer(POLL_TIMER);
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY_TIMER, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn begin_notifications(&mut self, cx: &mut Cx) {
        if let (Some(handle), true) = (self.handle, self.open) {
            let bits = self.gpios.iter().fold(0u32, |a, g| a | (1 << g));
            self.queue.push_front(Request {
                bytes: request(NB, handle, bits, &[]),
                purpose: Purpose::Begin,
            });
            self.pump(cx);
        }
    }
}

fn u32_param(params: &Params, key: &str) -> u32 {
    params.get(key).and_then(Value::as_u64).unwrap_or(0) as u32
}

impl Module for Pigpio {
    fn start(&mut self, cx: &mut Cx) {
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if self.closed || !self.open {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let g = u32_param(params, "gpio");
        let v = |k: &str| u32_param(params, k);
        let (bytes, answer) = match name {
            "set_mode" => {
                let Some(mode) = params
                    .get("mode")
                    .and_then(Value::as_str)
                    .and_then(mode_code)
                else {
                    cx.complete(
                        id,
                        Err(CommandError::InvalidParams {
                            message: "unknown mode".into(),
                        }),
                    );
                    return;
                };
                (request(MODES, g, mode, &[]), Answer::Ack)
            }
            "get_mode" => (request(MODEG, g, 0, &[]), Answer::Mode),
            "set_pull" => {
                let pud = match params.get("pull").and_then(Value::as_str) {
                    Some("down") => 1,
                    Some("up") => 2,
                    _ => 0,
                };
                (request(PUD, g, pud, &[]), Answer::Ack)
            }
            "read" => (request(READ, g, 0, &[]), Answer::Value { unsigned: false }),
            "write" => {
                let level = u32::from(
                    params
                        .get("level")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                );
                (request(WRITE, g, level, &[]), Answer::Ack)
            }
            "set_pwm" => (request(PWM, g, v("dutycycle"), &[]), Answer::Ack),
            "set_pwm_range" => (
                request(PRS, g, v("range"), &[]),
                Answer::Value { unsigned: false },
            ),
            "set_pwm_frequency" => (
                request(PFS, g, v("frequency"), &[]),
                Answer::Value { unsigned: false },
            ),
            "get_pwm" => (request(GDC, g, 0, &[]), Answer::Value { unsigned: false }),
            "get_pwm_range" => (request(PRG, g, 0, &[]), Answer::Value { unsigned: false }),
            "get_pwm_real_range" => (request(PRRG, g, 0, &[]), Answer::Value { unsigned: false }),
            "get_pwm_frequency" => (request(PFG, g, 0, &[]), Answer::Value { unsigned: false }),
            "set_servo" => (request(SERVO, g, v("pulsewidth"), &[]), Answer::Ack),
            "get_servo" => (request(GPW, g, 0, &[]), Answer::Value { unsigned: false }),
            "set_watchdog" => (request(WDOG, g, v("timeout_ms"), &[]), Answer::Ack),
            "trigger" => {
                let level = u32::from(params.get("level").and_then(Value::as_bool).unwrap_or(true));
                (
                    request(TRIG, g, v("pulse_us"), &level.to_le_bytes()),
                    Answer::Ack,
                )
            }
            "set_glitch_filter" => (request(FG, g, v("steady_us"), &[]), Answer::Ack),
            "set_noise_filter" => (
                request(FN, g, v("steady_us"), &v("active_us").to_le_bytes()),
                Answer::Ack,
            ),
            "hardware_clock" => (request(HC, g, v("frequency"), &[]), Answer::Ack),
            "hardware_pwm" => (
                request(HP, g, v("frequency"), &v("dutycycle").to_le_bytes()),
                Answer::Ack,
            ),
            "read_bank_1" => (request(BR1, 0, 0, &[]), Answer::Value { unsigned: true }),
            "read_bank_2" => (request(BR2, 0, 0, &[]), Answer::Value { unsigned: true }),
            "set_bits_bank_1" => (request(BS1, v("bits"), 0, &[]), Answer::Ack),
            "clear_bits_bank_1" => (request(BC1, v("bits"), 0, &[]), Answer::Ack),
            "set_bits_bank_2" => (request(BS2, v("bits"), 0, &[]), Answer::Ack),
            "clear_bits_bank_2" => (request(BC2, v("bits"), 0, &[]), Answer::Ack),
            "set_pad_strength" => (request(PADS, v("pad"), v("strength_ma"), &[]), Answer::Ack),
            "get_pad_strength" => (
                request(PADG, v("pad"), 0, &[]),
                Answer::Value { unsigned: false },
            ),
            "trigger_event" => (request(EVT, v("event"), 0, &[]), Answer::Ack),
            "get_tick" => (request(TICK, 0, 0, &[]), Answer::Value { unsigned: true }),
            "get_hardware_revision" => {
                (request(HWVER, 0, 0, &[]), Answer::Value { unsigned: true })
            }
            "get_version" => (request(PIGPV, 0, 0, &[]), Answer::Value { unsigned: true }),
            "refresh" => {
                self.read_modes();
                self.pump(cx);
                cx.complete(id, Ok(Outcome::Ack));
                return;
            }
            other => {
                cx.complete(
                    id,
                    Err(CommandError::UnknownCommand {
                        command: other.into(),
                    }),
                );
                return;
            }
        };
        self.enqueue(
            cx,
            Request {
                bytes,
                purpose: Purpose::Command(id, answer),
            },
        );
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        match (socket, input) {
            (COMMAND, TcpInput::Connected) => {
                self.open = true;
                self.retry_after = RETRY_MIN;
                self.buf.clear();
                self.heard_at = cx.now();
                if self.monitor {
                    self.read(HWVER, 0);
                    self.read(PIGPV, 0);
                    self.read_modes();
                    for pad in 0..3 {
                        self.read(PADG, pad);
                    }
                    cx.set_timer(POLL_TIMER, self.poll_every.max(LIVENESS / 10));
                } else {
                    self.queue.push_back(Request {
                        bytes: request(TICK, 0, 0, &[]),
                        purpose: Purpose::Liveness,
                    });
                    cx.set_timer(POLL_TIMER, LIVENESS);
                }
                self.begin_notifications(cx);
                self.pump(cx);
            }
            (COMMAND, TcpInput::Data(bytes)) => {
                self.buf.extend_from_slice(&bytes);
                while self.buf.len() >= 16 {
                    let w: Vec<u32> = self.buf[..16]
                        .chunks(4)
                        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    self.buf.drain(..16);
                    self.answered(cx, [w[0], w[1], w[2], w[3]]);
                }
            }
            (NOTIFY, TcpInput::Connected) => {
                self.notify_open = true;
                self.notify_buf.clear();
                cx.tcp_send(NOTIFY, request(NOIB, 0, 0, &[]));
            }
            (NOTIFY, TcpInput::Data(bytes)) => {
                self.notify_buf.extend_from_slice(&bytes);
                if self.handle.is_none() {
                    if self.notify_buf.len() < 16 {
                        return;
                    }
                    let res = i32::from_le_bytes([
                        self.notify_buf[12],
                        self.notify_buf[13],
                        self.notify_buf[14],
                        self.notify_buf[15],
                    ]);
                    self.notify_buf.drain(..16);
                    if res < 0 {
                        cx.log(
                            Level::Warning,
                            format!("pigpio gave no notification handle: {}", error_text(res)),
                        );
                        return;
                    }
                    self.handle = Some(res as u32);
                    self.begin_notifications(cx);
                }
                self.reports(cx);
            }
            (_, TcpInput::Closed { reason }) => self.lost(cx, reason),
            _ => {}
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY_TIMER if self.closed => self.connect(cx),
            REPLY_TIMER => {
                if let Some((r, _)) = self.in_flight.take() {
                    if let Purpose::Command(id, _) = r.purpose {
                        cx.complete(id, Err(CommandError::Timeout));
                    }
                    self.misses += 1;
                    if self.misses >= MISSES {
                        self.lost(cx, "no answer from pigpio".into());
                        return;
                    }
                }
                self.pump(cx);
            }
            POLL_TIMER if self.open => {
                if self.monitor && self.poll_every > 0 {
                    self.read_modes();
                    cx.set_timer(POLL_TIMER, self.poll_every);
                } else {
                    if self.in_flight.is_none()
                        && self.queue.is_empty()
                        && cx.now().saturating_sub(self.heard_at) >= LIVENESS
                    {
                        self.queue.push_back(Request {
                            bytes: request(TICK, 0, 0, &[]),
                            purpose: Purpose::Liveness,
                        });
                    }
                    cx.set_timer(POLL_TIMER, LIVENESS);
                }
                self.pump(cx);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.tcp_close(NOTIFY);
        cx.tcp_close(COMMAND);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn module(monitor: bool, settings: Value) -> Pigpio {
        let spec = Catalog::source_tree().devices["pigpio"].clone();
        let settings = validate(&spec.settings, settings.as_object().unwrap()).unwrap();
        Pigpio::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 31)),
            host_name: None,
            port: None,
            model: "pigpiod".into(),
            channels: None,
            settings,
            monitor,
        })
        .unwrap()
    }

    fn sent(actions: &[Action], socket: Key) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { socket: s, data } if *s == socket => Some(data.clone()),
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

    /// The answer to the request in flight: its words with the result.
    fn answer(m: &mut Pigpio, now: Millis, res: i32) -> Vec<Action> {
        let (r, _) = m.in_flight.clone().expect("a request in flight");
        let mut bytes = r.bytes[..12].to_vec();
        bytes.extend(res.to_le_bytes());
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, COMMAND, TcpInput::Data(bytes));
        cx.take()
    }

    fn report(seq: u16, flags: u16, tick: u32, level: u32) -> Vec<u8> {
        let mut r = seq.to_le_bytes().to_vec();
        r.extend(flags.to_le_bytes());
        r.extend(tick.to_le_bytes());
        r.extend(level.to_le_bytes());
        r
    }

    #[test]
    fn requests_are_four_little_endian_words() {
        // pigs w 4 1: WRITE GPIO 4 high.
        assert_eq!(
            request(WRITE, 4, 1, &[]),
            [4, 0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]
        );
        // TRIG carries the level in a 4-byte extension.
        assert_eq!(
            request(TRIG, 17, 10, &1u32.to_le_bytes()),
            [37, 0, 0, 0, 17, 0, 0, 0, 10, 0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0]
        );
        assert_eq!(parse_gpios("2-4, 17").unwrap(), [2, 3, 4, 17]);
        assert!(parse_gpios("40").is_err());
    }

    #[test]
    fn monitored_it_opens_notifications_and_reads_the_gpio() {
        let mut m = module(true, json!({"gpios": "4,17"}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let a = cx.take();
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::TcpOpen { socket: NOTIFY, .. })));
        // The notification connection asks for its handle.
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, NOTIFY, TcpInput::Connected);
        assert_eq!(sent(&cx.take(), NOTIFY), [request(NOIB, 0, 0, &[])]);
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, COMMAND, TcpInput::Connected);
        assert_eq!(sent(&cx.take(), COMMAND), [request(HWVER, 0, 0, &[])]);
        // The handle arrives: NB for GPIO 4 and 17 goes next.
        let mut handle = request(NOIB, 0, 0, &[])[..12].to_vec();
        handle.extend(3i32.to_le_bytes());
        let mut cx = Cx::new(1);
        m.tcp(&mut cx, NOTIFY, TcpInput::Data(handle));
        cx.take();
        let a = answer(&mut m, 2, 0xa02082);
        assert_eq!(state(&a)["hardware_revision"], "a02082");
        assert!(a.contains(&Action::RoundTrip(2)));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(
            sent(&a, COMMAND),
            [request(NB, 3, (1 << 4) | (1 << 17), &[])]
        );
        answer(&mut m, 3, 0);
        // PIGPV, then GPIO 4's mode: an output, whose PWM is read.
        answer(&mut m, 4, 79);
        let a = answer(&mut m, 5, 1);
        assert_eq!(state(&a)["gpio"]["4"]["mode"], "output");
        answer(&mut m, 6, 0);
        // The bank: GPIO 17 high.
        let a = answer(&mut m, 7, 1 << 17);
        assert_eq!(
            state(&a)["gpio"],
            json!({"4": {"level": false}, "17": {"level": true}})
        );
        // A report: GPIO 4 goes high.
        let mut cx = Cx::new(8);
        m.tcp(
            &mut cx,
            NOTIFY,
            TcpInput::Data(report(0, 0, 1000, (1 << 4) | (1 << 17))),
        );
        assert_eq!(state(&cx.take())["gpio"], json!({"4": {"level": true}}));
        // A watchdog timeout on GPIO 17 is a message.
        let mut cx = Cx::new(9);
        m.tcp(
            &mut cx,
            NOTIFY,
            TcpInput::Data(report(1, 0x20 | 17, 2000, 0)),
        );
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::Message { address, args, .. }
            if address == "watchdog" && args["gpio"] == 17)));
    }

    #[test]
    fn commands_are_answered_and_errors_named() {
        let mut m = module(false, json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(!cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::TcpOpen { socket: NOTIFY, .. })));
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, COMMAND, TcpInput::Connected);
        // Commands only: one TICK, the liveness check.
        assert_eq!(sent(&cx.take(), COMMAND), [request(TICK, 0, 0, &[])]);
        answer(&mut m, 1, -5);
        let mut cx = Cx::new(2);
        m.command(
            &mut cx,
            1,
            "write",
            json!({"gpio": 4, "level": true}).as_object().unwrap(),
        );
        assert_eq!(sent(&cx.take(), COMMAND), [request(WRITE, 4, 1, &[])]);
        let a = answer(&mut m, 3, 0);
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(state(&a)["gpio"]["4"]["level"], true);
        let mut cx = Cx::new(4);
        m.command(
            &mut cx,
            2,
            "set_servo",
            json!({"gpio": 18, "pulsewidth": 100}).as_object().unwrap(),
        );
        let a = answer(&mut m, 5, -7);
        assert_eq!(
            completed(&a, 2),
            Some(Err(CommandError::DeviceError {
                code: Some("-7".into()),
                message: "pulse width not 0 or 500-2500 (PI_BAD_PULSEWIDTH)".into()
            }))
        );
        let mut cx = Cx::new(6);
        m.command(
            &mut cx,
            3,
            "get_mode",
            json!({"gpio": 22}).as_object().unwrap(),
        );
        let a = answer(&mut m, 7, 4);
        assert_eq!(
            completed(&a, 3),
            Some(Ok(Outcome::Value {
                value: json!("alt0")
            }))
        );
        // A bank read is unsigned.
        let mut cx = Cx::new(8);
        m.command(&mut cx, 4, "read_bank_1", &Params::new());
        let a = answer(&mut m, 9, -1);
        assert_eq!(
            completed(&a, 4),
            Some(Ok(Outcome::Value {
                value: json!(4294967295u32)
            }))
        );
    }
}
