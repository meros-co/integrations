//! Shure networked wireless over the command strings protocol, TCP 2202.
//!
//! From Shure's command string documents for Axient Digital, ULX-D, SLX-D and
//! the PSM1000 (see the spec's sources); QLX-D speaks the ULX-D strings:
//!
//! - Messages are ASCII in angle brackets, `< GET 1 CHAN_NAME >`, with no line
//!   breaks between them; string values are `{braced and padded}`.
//! - REP answers GET and SET, and is also sent unprompted whenever a value
//!   changes, so apart from metering nothing needs polling. `< REP ERR >`
//!   refuses a malformed or out-of-range SET.
//! - Metering is a per-channel subscription, `SET n METER_RATE 01000`, that
//!   produces one SAMPLE per channel per interval, until set to 00000.
//! - Names, offsets and SAMPLE layouts differ by family. Axient Digital also
//!   addresses the eight transmitter slots of each channel, `SET 1 SLOT_OFFSET
//!   2 15`, through which a linked ShowLink transmitter is controlled.
//!
//! Opened for commands only, it starts no metering and reads nothing on
//! connecting or later. Changes are still pushed as REP and applied. A
//! receiver that is not metering is otherwise silent, so its device id is asked
//! every 5 s as the liveness check.

use std::collections::{BTreeMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddr};

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

const PORT: u16 = 2202;
const SOCKET: Key = "shure";

const REPLY_TIMEOUT: Millis = 2_000;
/// Changes are pushed; this only recovers ones made while disconnected.
const REFRESH_EVERY: Millis = 30_000;
/// A receiver that has said nothing for this long is not there, whatever TCP
/// believes: a half-open socket survives a power cut.
const SILENCE_TIMEOUT: Millis = 15_000;
/// Connected but silent this long after connecting: tell the operator why that
/// usually happens.
const FIRST_WORD_WARNING: Millis = 5_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const DEFAULT_METER_MS: i64 = 1_000;
/// Opened for commands only, how often the device id is asked to prove the
/// receiver is there: well inside the silence timeout.
const LIVENESS_EVERY: Millis = 5_000;
/// Axient Digital: transmitter slots per channel.
const SLOTS: u32 = 8;

const REPLY: Key = "reply";
const REFRESH: Key = "refresh";
const SILENCE: Key = "silence";
const RETRY: Key = "retry";
const FIRST_WORD: Key = "first-word";

/// The Axient Digital slot properties read once on connecting, for every slot
/// (index 0).
const SLOT_PARAMS: [&str; 15] = [
    "SLOT_STATUS",
    "SLOT_TX_MODEL",
    "SLOT_TX_DEVICE_ID",
    "SLOT_RF_OUTPUT",
    "SLOT_RF_POWER_MODE",
    "SLOT_RF_POWER",
    "SLOT_OFFSET",
    "SLOT_INPUT_PAD",
    "SLOT_POLARITY",
    "SLOT_SHOWLINK_STATUS",
    "SLOT_BATT_TYPE",
    "SLOT_BATT_BARS",
    "SLOT_BATT_CHARGE_PERCENT",
    "SLOT_BATT_HEALTH_PERCENT",
    "SLOT_BATT_MINS",
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Family {
    Axient,
    Ulxd,
    Slxd,
    Psm1000,
}

impl Family {
    fn of(model: &str) -> Family {
        match model {
            "ad4d" | "ad4q" => Family::Axient,
            "slxd4" | "slxd4d" => Family::Slxd,
            "p10t" => Family::Psm1000,
            // ulxd4, ulxd4d, ulxd4q, qlxd4: QLX-D speaks the ULX-D strings.
            _ => Family::Ulxd,
        }
    }

    /// Offset from the reported RF value to dBm.
    fn rssi_offset(self) -> i64 {
        match self {
            Family::Ulxd => 128,
            _ => 120,
        }
    }

    /// The device-level name parameter, also asked as the liveness check:
    /// short, and answered by every family.
    fn identity_param(self) -> &'static str {
        match self {
            Family::Psm1000 => "DEVICE_NAME",
            _ => "DEVICE_ID",
        }
    }

    /// Axient Digital and SLX-D brace the group and channel and spell the
    /// parameter out; ULX-D, QLX-D and the PSM1000 abbreviate it.
    fn group_param(self) -> &'static str {
        match self {
            Family::Axient | Family::Slxd => "GROUP_CHANNEL",
            _ => "GROUP_CHAN",
        }
    }

    /// The longest meter interval each family documents.
    fn meter_max(self) -> i64 {
        match self {
            Family::Ulxd => 99_999,
            _ => 65_535,
        }
    }

    /// String values go in braces, except on the PSM1000.
    fn text(self, s: &str) -> String {
        if self == Family::Psm1000 {
            s.to_string()
        } else {
            format!("{{{s}}}")
        }
    }
}

/// Names a SET accepts: every family's current document allows 8 characters
/// (Axient Digital and SLX-D report 31, padded).
const NAME_LENGTH: usize = 8;

/// One `< ... >` message, parsed.
#[derive(Debug, PartialEq)]
struct Message {
    kind: String,
    channel: Option<u32>,
    param: String,
    /// Fields after the parameter name, a braced value kept whole and trimmed.
    args: Vec<String>,
}

impl Message {
    fn value(&self) -> &str {
        self.args.last().map(String::as_str).unwrap_or("")
    }
}

fn parse(raw: &str) -> Option<Message> {
    let body = raw
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim();
    // A braced value may contain spaces: take it out before splitting.
    let (head, braced) = match (body.find('{'), body.rfind('}')) {
        (Some(open), Some(close)) if close > open => (
            format!("{} {}", &body[..open], &body[close + 1..]),
            Some(body[open + 1..close].trim().to_string()),
        ),
        _ => (body.to_string(), None),
    };
    let mut tokens = head.split_whitespace();
    let kind = tokens.next()?.to_string();
    let mut rest: Vec<&str> = tokens.collect();
    let channel = match rest.first().map(|t| t.parse::<u32>()) {
        Some(Ok(n)) => {
            rest.remove(0);
            Some(n)
        }
        _ => None,
    };
    if rest.is_empty() {
        return None;
    }
    let param = rest.remove(0).to_string();
    let mut args: Vec<String> = rest.into_iter().map(str::to_string).collect();
    if let Some(b) = braced {
        args.push(b);
    }
    Some(Message {
        kind,
        channel,
        param,
        args,
    })
}

/// Complete `< ... >` messages from the stream, keeping a partial tail.
#[derive(Default)]
struct Framer {
    buffer: String,
}

impl Framer {
    fn feed(&mut self, data: &[u8]) -> Vec<String> {
        self.buffer.push_str(&String::from_utf8_lossy(data));
        let mut out = Vec::new();
        while let Some(end) = self.buffer.find('>') {
            if let Some(start) = self.buffer[..end].rfind('<') {
                out.push(self.buffer[start..=end].to_string());
            }
            self.buffer.drain(..=end);
        }
        // A '<' that never closes must not grow the buffer without limit.
        if self.buffer.len() > 4096 {
            let keep = self.buffer.split_off(self.buffer.len() - 1024);
            self.buffer = keep;
        }
        out
    }
}

/// A 3-digit gauge, or None for the 255 "unknown" sentinel.
fn gauge(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok().filter(|&n| n != 255)
}

/// A 5-digit count, or None for the 65535 "unknown" sentinel.
fn count(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok().filter(|&n| n != 65_535)
}

/// Run time in minutes; 65533 (communication warning), 65534 (calculating)
/// and 65535 (unknown) are not readings.
fn minutes(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok().filter(|&m| m <= 65_532)
}

/// An enumerated value in lower case, or None for the families' unknowns.
fn word(s: &str) -> Value {
    match s.trim() {
        "" | "UNKN" | "UNKNOWN" => Value::Null,
        v => json!(v.to_ascii_lowercase().replace('.', "_")),
    }
}

/// ON / OFF style flags; unknown is None.
fn flag(s: &str, on: &str, off: &str) -> Value {
    match s.trim() {
        v if v == on => json!(true),
        v if v == off => json!(false),
        _ => Value::Null,
    }
}

/// A string value, or None when blank (no transmitter).
fn text(s: &str) -> Value {
    match s.trim() {
        "" => Value::Null,
        v => json!(v),
    }
}

/// `6,100`, `06,06` or `06 06`: the group and its channel, None for `--`.
fn group_channel(args: &[String]) -> (Value, Value) {
    let joined = args.join(",");
    let mut parts = joined
        .split([',', ' '])
        .map(str::trim)
        .filter(|p| !p.is_empty());
    let part = |p: Option<&str>| match p {
        Some(p) if !p.starts_with('-') => {
            let trimmed = p.trim_start_matches('0');
            json!(if trimmed.is_empty() { p } else { trimmed })
        }
        _ => Value::Null,
    };
    let group = part(parts.next());
    let channel = part(parts.next());
    (group, channel)
}

/// `192.168.001.025` as `192.168.1.25`.
fn address(s: &str) -> Value {
    match s.parse::<Ipv4Addr>() {
        Ok(a) => json!(a.to_string()),
        Err(_) => {
            let octets: Option<Vec<u8>> = s.split('.').map(|o| o.parse::<u8>().ok()).collect();
            match octets.filter(|o| o.len() == 4) {
                Some(o) => json!(Ipv4Addr::new(o[0], o[1], o[2], o[3]).to_string()),
                None => Value::Null,
            }
        }
    }
}

/// SET commands wait for the REP of the same channel and parameter (and slot
/// or interface, where the parameter has one).
struct Pending {
    id: CommandId,
    channel: Option<u32>,
    param: String,
    index: Option<String>,
    /// For settings a receiver ignores when they do not apply to it, the value
    /// the REP must carry for the SET to have worked.
    expect: Option<String>,
    sent_at: Millis,
    deadline: Millis,
}

pub(crate) struct Shure {
    device: SocketAddr,
    family: Family,
    model: String,
    channels: u32,
    meter_ms: i64,
    /// Read state and meter; false for commands only.
    monitor: bool,
    /// When the unanswered identity GET went.
    probe: Option<Millis>,
    framer: Framer,
    socket_open: bool,
    connected: bool,
    pending: VecDeque<Pending>,
    /// Each channel's meter interval as last reported, so metering started by
    /// a command is stopped on closing too.
    metering: BTreeMap<u32, i64>,
    retry_after: Millis,
}

impl Shure {
    pub(crate) fn new(ctx: OpenContext) -> Shure {
        let meter_ms = ctx
            .settings
            .get("meter_interval_ms")
            .and_then(Value::as_i64)
            .unwrap_or(DEFAULT_METER_MS);
        let mut shure = Shure::for_device(
            SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)),
            &ctx.model,
            ctx.channels.unwrap_or(2),
            // Commands only: metering is never started, so there is none to
            // leave running either.
            if ctx.monitor { meter_ms } else { 0 },
        );
        shure.monitor = ctx.monitor;
        shure
    }

    fn for_device(device: SocketAddr, model: &str, channels: u32, meter_ms: i64) -> Shure {
        Shure {
            device,
            family: Family::of(model),
            model: model.to_string(),
            channels,
            // Shure documents 100 ms as the minimum interval.
            meter_ms: if meter_ms == 0 { 0 } else { meter_ms.max(100) },
            monitor: true,
            probe: None,
            framer: Framer::default(),
            socket_open: false,
            connected: false,
            pending: VecDeque::new(),
            metering: BTreeMap::new(),
            retry_after: RETRY_MIN,
        }
    }

    fn is_qlxd(&self) -> bool {
        self.model == "qlxd4"
    }

    fn send(&self, cx: &mut Cx, body: &str) {
        let terminator = if self.family == Family::Psm1000 {
            "\r\n"
        } else {
            ""
        };
        cx.tcp_send(SOCKET, format!("< {body} >{terminator}"));
    }

    fn meter_rate(&self, channel: u32, ms: i64) -> String {
        if self.family == Family::Psm1000 {
            format!("SET {channel} METER_RATE {ms}")
        } else {
            format!("SET {channel} METER_RATE {ms:05}")
        }
    }

    /// The identity GET alone: the liveness check of a device opened for
    /// commands only, whose answer is timed.
    fn ask_identity(&mut self, cx: &mut Cx) {
        self.send(cx, &format!("GET {}", self.family.identity_param()));
        self.probe = Some(cx.now());
    }

    /// Everything, once: device identity, then each channel. `connecting` adds
    /// what is read only on connecting: settings that are never reported on
    /// change (network), Axient Digital's transmitter slots, and device
    /// settings some firmware may not know, whose `REP ERR` must not land
    /// while a SET is waiting.
    fn query_all(&mut self, cx: &mut Cx, connecting: bool) {
        self.probe = Some(cx.now());
        let mut device: Vec<&str> = match self.family {
            Family::Axient => vec![
                "MODEL",
                "DEVICE_ID",
                "FW_VER",
                "RF_BAND",
                "TRANSMISSION_MODE",
            ],
            Family::Slxd => vec!["MODEL", "DEVICE_ID", "FW_VER", "RF_BAND"],
            Family::Ulxd if self.is_qlxd() => vec!["DEVICE_ID", "FW_VER"],
            Family::Ulxd => vec!["DEVICE_ID", "FW_VER", "HIGH_DENSITY"],
            Family::Psm1000 => vec!["DEVICE_NAME"],
        };
        let multi = self.channels > 1;
        if connecting {
            match self.family {
                Family::Axient => {
                    device.extend(["ENCRYPTION_MODE", "QUADVERSITY_MODE"]);
                    device.extend(["NET_SETTINGS SC", "NET_SETTINGS D1", "NET_SETTINGS D2"]);
                }
                Family::Slxd => device.push("LOCK_STATUS"),
                Family::Ulxd if !self.is_qlxd() => {
                    device.extend(["MODEL", "ENCRYPTION", "SCAN_LOCK", "SYNC_LOCK"]);
                    device.push("NET_SETTINGS SC");
                    if multi {
                        device.extend([
                            "AUDIO_SUMMING_MODE",
                            "FREQUENCY_DIVERSITY_MODE",
                            "NA_DEVICE_NAME",
                            "NET_SETTINGS D1",
                            "NET_SETTINGS D2",
                        ]);
                    }
                }
                _ => {}
            }
        }
        for p in device {
            self.send(cx, &format!("GET {p}"));
        }
        for ch in 1..=self.channels {
            if self.family == Family::Psm1000 {
                // No GET ALL on the PSM1000.
                for p in [
                    "CHAN_NAME",
                    "FREQUENCY",
                    "GROUP_CHAN",
                    "RF_MUTE",
                    "RF_TX_LVL",
                    "AUDIO_IN_LVL",
                    "AUDIO_TX_MODE",
                    "AUDIO_IN_LINE_LVL",
                ] {
                    self.send(cx, &format!("GET {ch} {p}"));
                }
            } else {
                self.send(cx, &format!("GET {ch} ALL"));
            }
            if connecting && self.family == Family::Axient {
                for p in SLOT_PARAMS {
                    self.send(cx, &format!("GET {ch} {p} 0"));
                }
            }
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        for p in self.pending.drain(..) {
            cx.complete(
                p.id,
                Err(CommandError::Transport {
                    message: reason.clone(),
                }),
            );
        }
        for key in [REPLY, REFRESH, SILENCE, FIRST_WORD] {
            cx.cancel_timer(key);
        }
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.connected = false;
        self.metering.clear();
        self.framer = Framer::default();
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// The full refresh when monitoring; for commands only, the liveness check.
    fn refresh_every(&self) -> Millis {
        if self.monitor {
            REFRESH_EVERY
        } else {
            LIVENESS_EVERY
        }
    }

    fn arm_reply_timer(&self, cx: &mut Cx) {
        match self.pending.iter().map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    fn set(&mut self, cx: &mut Cx, id: CommandId, channel: Option<u32>, param: &str, value: &str) {
        self.request(cx, id, channel, param, None, value, None);
    }

    /// `SET [channel] PARAM [index] value`, answered by the REP of the same
    /// channel, parameter and index.
    #[allow(clippy::too_many_arguments)]
    fn request(
        &mut self,
        cx: &mut Cx,
        id: CommandId,
        channel: Option<u32>,
        param: &str,
        index: Option<String>,
        value: &str,
        expect: Option<String>,
    ) {
        let mut body = String::from("SET");
        if let Some(ch) = channel {
            body.push_str(&format!(" {ch}"));
        }
        body.push_str(&format!(" {param}"));
        if let Some(i) = &index {
            body.push_str(&format!(" {i}"));
        }
        body.push_str(&format!(" {value}"));
        self.send(cx, &body);
        self.pending.push_back(Pending {
            id,
            channel,
            param: param.to_string(),
            index,
            expect,
            sent_at: cx.now(),
            deadline: cx.now() + REPLY_TIMEOUT,
        });
        self.arm_reply_timer(cx);
    }

    fn message(&mut self, cx: &mut Cx, raw: &str) {
        let Some(msg) = parse(raw) else {
            return;
        };
        if !self.connected {
            self.connected = true;
            self.retry_after = RETRY_MIN;
            cx.cancel_timer(FIRST_WORD);
            cx.connection(Connection::Connected);
        }
        match msg.kind.as_str() {
            "REP" | "REPORT" if msg.channel.is_none() && msg.param == "ERR" => {
                // Commands are answered in order, so the refusal is the oldest
                // SET still waiting.
                if let Some(p) = self.pending.pop_front() {
                    cx.round_trip(cx.now().saturating_sub(p.sent_at));
                    cx.complete(
                        p.id,
                        Err(CommandError::DeviceError {
                            code: Some("ERR".into()),
                            message: format!(
                                "the receiver refused {}: malformed, out of range, or not \
                                 possible now",
                                p.param
                            ),
                        }),
                    );
                    self.arm_reply_timer(cx);
                }
            }
            "REP" | "REPORT" => {
                if msg.param == "METER_RATE" {
                    if let (Some(ch), Ok(ms)) = (msg.channel, msg.value().trim().parse::<i64>()) {
                        self.metering.insert(ch, ms);
                    }
                }
                // State first, so a caller reading the snapshot after its SET
                // completes sees the reported value.
                if let Some(patch) = self.report(&msg) {
                    cx.state(patch);
                }
                if msg.channel.is_none() && msg.param == self.family.identity_param() {
                    if let Some(sent) = self.probe.take() {
                        cx.round_trip(cx.now().saturating_sub(sent));
                    }
                }
                let index = msg.args.first().and_then(|a| a.parse::<u32>().ok());
                if let Some(i) = self.pending.iter().position(|p| {
                    p.channel == msg.channel
                        && p.param == msg.param
                        && p.index
                            .as_ref()
                            .is_none_or(|want| want.parse::<u32>().ok() == index)
                }) {
                    let p = self.pending.remove(i).unwrap();
                    cx.round_trip(cx.now().saturating_sub(p.sent_at));
                    let got = msg.value().trim().to_string();
                    let result = match p.expect {
                        Some(want) if want != got => Err(CommandError::DeviceError {
                            code: None,
                            message: format!(
                                "the receiver kept {} at {got}: {want} does not apply to it",
                                p.param
                            ),
                        }),
                        _ => Ok(Outcome::Ack),
                    };
                    cx.complete(p.id, result);
                    self.arm_reply_timer(cx);
                }
            }
            "SAMPLE" => {
                if let Some(patch) = self.sample(&msg) {
                    cx.state(patch);
                }
            }
            _ => {}
        }
    }

    /// A device-level REP's effect on the state.
    fn device_report(&self, msg: &Message) -> Option<Value> {
        let v = msg.value();
        let on = v == "ON";
        let device = match msg.param.as_str() {
            "MODEL" => json!({"model": v}),
            "DEVICE_ID" | "DEVICE_NAME" => json!({"id": v}),
            "FW_VER" => json!({"firmware": v.trim_end_matches('*').trim()}),
            "RF_BAND" => json!({"rf_band": v}),
            "TRANSMISSION_MODE" => json!({"high_density": v == "HIGH_DENSITY"}),
            "HIGH_DENSITY" => json!({"high_density": on}),
            "QUADVERSITY_MODE" => json!({"quadversity": on}),
            "ENCRYPTION" | "ENCRYPTION_MODE" => json!({"encryption": word(v)}),
            "ENCRYPTION_REGENERATE_KEY" => json!({"encryption_key_regeneration": word(v)}),
            "AUDIO_SUMMING_MODE" => json!({"audio_summing_mode": word(v)}),
            "FREQUENCY_DIVERSITY_MODE" => json!({"frequency_diversity_mode": word(v)}),
            "SCAN_LOCK" => json!({"scan_lock": on}),
            "SYNC_LOCK" => json!({"sync_lock": on}),
            "LOCK_STATUS" => json!({"transmitter_lock": word(v)}),
            "NA_DEVICE_NAME" => json!({"dante_name": text(v)}),
            "FLASH" => json!({"flash": on}),
            "NET_SETTINGS" => {
                let [interface, mode, ip, mask, gateway, rest @ ..] = msg.args.as_slice() else {
                    return None;
                };
                let key = match interface.as_str() {
                    "SC" => "control",
                    "D1" => "dante_primary",
                    "D2" => "dante_secondary",
                    _ => return None,
                };
                let gateway = address(gateway).as_str().map(str::to_string);
                json!({"network": {key: {
                    "mode": word(mode),
                    "address": address(ip),
                    "subnet_mask": address(mask),
                    "gateway": gateway.filter(|g| g != "0.0.0.0"),
                    "mac": rest.first().map(|m| m.to_ascii_lowercase()),
                }}})
            }
            _ => return None,
        };
        Some(json!({ "device": device }))
    }

    /// The state an Axient Digital slot REP sets: `< REP 1 SLOT_OFFSET 2 015 >`.
    fn slot_report(msg: &Message, ch: u32) -> Option<Value> {
        let [slot, value] = msg.args.as_slice() else {
            return None;
        };
        let slot: u32 = slot.parse().ok().filter(|s| (1..=SLOTS).contains(s))?;
        let v = value.as_str();
        let field = match msg.param.as_str() {
            "SLOT_STATUS" => json!({"status": word(v)}),
            "SLOT_TX_MODEL" => json!({"model": word(v)}),
            "SLOT_TX_DEVICE_ID" => json!({"device_id": text(v)}),
            "SLOT_BATT_BARS" => json!({"battery_bars": gauge(v)}),
            "SLOT_BATT_CHARGE_PERCENT" => json!({"battery_percent": gauge(v)}),
            "SLOT_BATT_CYCLE_COUNT" => json!({"battery_cycles": count(v)}),
            "SLOT_BATT_HEALTH_PERCENT" => json!({"battery_health_percent": gauge(v)}),
            "SLOT_BATT_MINS" => json!({"battery_minutes": minutes(v)}),
            "SLOT_BATT_TYPE" => json!({"battery_type": word(v)}),
            "SLOT_INPUT_PAD" => json!({"input_pad": gauge(v).map(|n| n == 0)}),
            "SLOT_OFFSET" => json!({"offset_db": gauge(v).map(|n| n - 12)}),
            "SLOT_POLARITY" => json!({"polarity": word(v)}),
            "SLOT_RF_OUTPUT" => json!({"rf_mute": flag(v, "RF_MUTE", "RF_ON")}),
            "SLOT_RF_POWER" => json!({"rf_power_mw": gauge(v)}),
            "SLOT_RF_POWER_MODE" => json!({"rf_power": word(v)}),
            "SLOT_SHOWLINK_STATUS" => json!({"showlink_quality": gauge(v)}),
            _ => return None,
        };
        Some(json!({"channels": {ch.to_string(): {"slots": {slot.to_string(): field}}}}))
    }

    /// A REP's effect on the state.
    fn report(&self, msg: &Message) -> Option<Value> {
        let v = msg.value();
        let on = v == "ON";
        let Some(ch) = msg.channel else {
            return self.device_report(msg);
        };
        if msg.param.starts_with("SLOT_") {
            return Self::slot_report(msg, ch);
        }
        // Older ULX-D firmware reports encryption per channel, ON or OFF.
        if msg.param == "ENCRYPTION" {
            return Some(json!({"device": {"encryption": word(v)}}));
        }
        let number = |s: &str| s.trim().parse::<i64>().ok();
        let tx = |field: Value| json!({ "transmitter": field });
        let channel = match (msg.param.as_str(), self.family) {
            ("CHAN_NAME", _) => json!({"name": v}),
            ("FREQUENCY", _) => json!({"frequency_khz": number(v)}),
            ("FREQUENCY2", Family::Axient) => json!({"frequency2_khz": number(v)}),
            ("GROUP_CHANNEL" | "GROUP_CHAN", _) => {
                let (group, channel) = group_channel(&msg.args);
                json!({"group": group, "group_channel": channel})
            }
            ("GROUP_CHANNEL2", Family::Axient) => {
                let (group, channel) = group_channel(&msg.args);
                json!({"group2": group, "group_channel2": channel})
            }
            ("METER_RATE", _) => json!({"meter_rate_ms": number(v)}),
            ("FLASH", _) => json!({"flash": on}),
            ("AUDIO_MUTE", _) => json!({"mute": on}),
            ("RF_MUTE", Family::Psm1000) => json!({"rf_mute": v == "1"}),
            ("AUDIO_GAIN", _) => json!({"gain_db": number(v).map(|g| g - 18)}),
            ("CHAN_QUALITY", Family::Axient) => json!({"quality": gauge(v)}),
            ("FD_MODE", Family::Axient) => json!({"fd_mode": word(v)}),
            ("ENCRYPTION_WARNING", _) => json!({"encryption_warning": flag(v, "ON", "OFF")}),
            ("ENCRYPTION_STATUS", _) => {
                json!({"encryption_warning": flag(v, "ERROR", "OK")})
            }
            ("RF_INT_DET", _) => json!({"interference": flag(v, "CRITICAL", "NONE")}),
            ("INTERFERENCE_STATUS", _) => {
                json!({"interference": flag(v, "DETECTED", "NONE")})
            }
            ("INTERFERENCE_STATUS2", _) => {
                json!({"interference2": flag(v, "DETECTED", "NONE")})
            }
            ("UNREGISTERED_TX_STATUS", _) => {
                json!({"unregistered_transmitter": flag(v, "ERROR", "OK")})
            }
            ("AUDIO_OUT_LVL_SWITCH", _) => json!({"audio_output_level": word(v)}),
            ("NA_CHAN_NAME", _) => json!({"dante_name": text(v)}),
            ("AUDIO_LEVEL_RMS", Family::Axient | Family::Slxd) => {
                json!({"af": {"rms_dbfs": number(v).map(|n| n - 120)}})
            }
            ("AUDIO_LEVEL_PEAK", Family::Axient | Family::Slxd) => {
                json!({"af": {"peak_dbfs": number(v).map(|n| n - 120)}})
            }
            ("RSSI", Family::Axient | Family::Slxd) => {
                // < REP 1 RSSI 1 083 >: antenna index, then the level.
                let (antenna, level) = match msg.args.as_slice() {
                    [a, l] => (a.as_str(), l.as_str()),
                    [l] => ("1", l.as_str()),
                    _ => return None,
                };
                let letter = antenna_letter(antenna.parse().ok()?)?;
                json!({"rf": {"rssi_dbm": {letter: number(level).map(|n| n - 120)}}})
            }
            // PSM1000 channel settings.
            ("AUDIO_IN_LVL", Family::Psm1000) => json!({"audio_input_level_db": number(v)}),
            ("RF_TX_LVL" | "RF_TX_LEVEL", Family::Psm1000) => json!({"rf_power_mw": number(v)}),
            ("AUDIO_TX_MODE", Family::Psm1000) => {
                let mode = match v.trim() {
                    "1" => json!("mono"),
                    "2" => json!("point_to_point"),
                    "3" => json!("stereo"),
                    _ => Value::Null,
                };
                json!({ "audio_tx_mode": mode })
            }
            ("AUDIO_IN_LINE_LVL", Family::Psm1000) => {
                json!({"input_line_level": flag(v, "1", "0")})
            }
            ("AUDIO_IN_LVL_L", Family::Psm1000) => json!({"af": {"input_meter_left": number(v)}}),
            ("AUDIO_IN_LVL_R", Family::Psm1000) => {
                json!({"af": {"input_meter_right": number(v)}})
            }
            // The transmitter, as the receiver hears it.
            ("TX_BATT_BARS", Family::Axient | Family::Slxd) | ("BATT_BARS", Family::Ulxd) => {
                tx(json!({"battery_bars": gauge(v)}))
            }
            ("TX_BATT_CHARGE_PERCENT", Family::Axient) | ("BATT_CHARGE", Family::Ulxd) => {
                tx(json!({"battery_percent": gauge(v)}))
            }
            ("TX_BATT_MINS", Family::Axient | Family::Slxd) | ("BATT_RUN_TIME", Family::Ulxd) => {
                tx(json!({"battery_minutes": minutes(v)}))
            }
            ("TX_BATT_HEALTH_PERCENT", Family::Axient) | ("BATT_HEALTH", Family::Ulxd) => {
                tx(json!({"battery_health_percent": gauge(v)}))
            }
            ("TX_BATT_CYCLE_COUNT", Family::Axient) | ("BATT_CYCLE", Family::Ulxd) => {
                tx(json!({"battery_cycles": count(v)}))
            }
            ("TX_BATT_TEMP_C", Family::Axient) | ("BATT_TEMP_C", Family::Ulxd) => {
                tx(json!({"battery_temp_c": gauge(v).map(|t| t - 40)}))
            }
            ("TX_BATT_TYPE", Family::Axient) | ("BATT_TYPE", Family::Ulxd) => {
                tx(json!({"battery_type": word(v)}))
            }
            ("TX_MODEL", Family::Axient | Family::Slxd) | ("TX_TYPE", Family::Ulxd) => {
                tx(json!({"model": word(v)}))
            }
            ("TX_DEVICE_ID", _) => tx(json!({"device_id": text(v)})),
            ("TX_FW_VER", _) => tx(json!({"firmware": text(v.trim_end_matches('*'))})),
            ("TX_OFFSET", _) => tx(json!({"offset_db": gauge(v).map(|o| o - 12)})),
            ("TX_RF_PWR", Family::Ulxd) => tx(json!({"rf_power": word(v)})),
            ("TX_POWER_LEVEL", Family::Axient) => tx(json!({"rf_power_mw": gauge(v)})),
            ("TX_MENU_LOCK", Family::Ulxd) => tx(json!({"menu_lock": flag(v, "ON", "OFF")})),
            ("TX_PWR_LOCK", Family::Ulxd) => tx(json!({"power_lock": flag(v, "ON", "OFF")})),
            ("TX_LOCK", Family::Axient) => {
                // NONE, POWER, MENU or ALL: both locks from one value.
                let (menu, power) = match v.trim() {
                    "NONE" => (json!(false), json!(false)),
                    "MENU" => (json!(true), json!(false)),
                    "POWER" => (json!(false), json!(true)),
                    "ALL" => (json!(true), json!(true)),
                    _ => (Value::Null, Value::Null),
                };
                tx(json!({"lock": word(v), "menu_lock": menu, "power_lock": power}))
            }
            ("TX_MUTE_STATUS", Family::Ulxd) => tx(json!({"muted": flag(v, "ON", "OFF")})),
            ("TX_MUTE_MODE_STATUS", Family::Axient) => tx(json!({"muted": flag(v, "MUTE", "ON")})),
            ("TX_MUTE_BUTTON_STATUS", Family::Ulxd) => {
                tx(json!({"mute_button_pressed": flag(v, "PRESSED", "RELEASED")}))
            }
            ("TX_POWER_SOURCE", Family::Ulxd) => tx(json!({"power_source": word(v)})),
            ("TX_POLARITY", Family::Axient) => tx(json!({"polarity": word(v)})),
            ("TX_TALK_SWITCH", Family::Axient) => tx(json!({"talk_switch": flag(v, "ON", "OFF")})),
            ("TX_INPUT_PAD", Family::Axient) => tx(json!({"input_pad": gauge(v).map(|n| n == 0)})),
            _ => return None,
        };
        Some(json!({"channels": {ch.to_string(): channel}}))
    }

    /// A SAMPLE's effect on the state. The layout depends on the family.
    fn sample(&self, msg: &Message) -> Option<Value> {
        let ch = msg.channel?;
        if msg.param != "ALL" {
            return None;
        }
        let f: Vec<&str> = msg.args.iter().map(String::as_str).collect();
        let n = |s: &str| s.trim().parse::<i64>().ok();
        let offset = self.family.rssi_offset();
        let channel = match self.family {
            // < SAMPLE ch ALL audPeak audRms rfRssi >
            Family::Slxd => {
                let [peak, rms, rssi, ..] = f.as_slice() else {
                    return None;
                };
                json!({
                    "af": {"peak_dbfs": n(peak).map(|v| v - 120), "rms_dbfs": n(rms).map(|v| v - 120)},
                    "rf": {"rssi_dbm": {"a": n(rssi).map(|v| v - offset)}},
                })
            }
            // < SAMPLE x ALL nn aaa eee >: antenna LEDs lit, RF, audio. The
            // current document gives the audio level in dBFS as eee - 50.
            Family::Ulxd => {
                let [leds, rf, audio, ..] = f.as_slice() else {
                    return None;
                };
                let mut antennas = Map::new();
                for (i, c) in leds.chars().enumerate() {
                    let letter = antenna_letter(i as u32 + 1)?;
                    antennas.insert(letter.into(), json!(if c == 'X' { "off" } else { "on" }));
                }
                json!({
                    "af": {"level": n(audio), "rms_dbfs": n(audio).map(|v| v - 50)},
                    "rf": {"rssi_dbm": {"a": n(rf).map(|v| v - offset)}, "antennas": antennas},
                })
            }
            // < SAMPLE ch ALL qual audBitmap audPeak audRms antStats
            //   (rfBitmap rfRssi) per antenna [second FD-C section] >
            Family::Axient => {
                let [qual, _bitmap, peak, rms, stats, rest @ ..] = f.as_slice() else {
                    return None;
                };
                let mut antennas = Map::new();
                let mut rssi = Map::new();
                for (i, c) in stats.chars().enumerate() {
                    let letter = antenna_letter(i as u32 + 1)?;
                    let led = match c {
                        'R' => "red",
                        'B' => "blue",
                        _ => "off",
                    };
                    antennas.insert(letter.into(), json!(led));
                    if let Some(level) = rest.get(i * 2 + 1) {
                        rssi.insert(letter.into(), json!(n(level).map(|v| v - offset)));
                    }
                }
                json!({
                    "quality": gauge(qual),
                    "af": {"peak_dbfs": n(peak).map(|v| v - 120), "rms_dbfs": n(rms).map(|v| v - 120)},
                    "rf": {"rssi_dbm": rssi, "antennas": antennas},
                })
            }
            Family::Psm1000 => return None,
        };
        Some(json!({"channels": {ch.to_string(): channel}}))
    }

    fn refuse(cx: &mut Cx, id: CommandId, message: String) {
        cx.complete(id, Err(CommandError::InvalidParams { message }));
    }

    fn unsupported(&self, cx: &mut Cx, id: CommandId, command: &str) {
        cx.complete(
            id,
            Err(CommandError::UnsupportedForModel {
                command: command.into(),
                model: self.model.clone(),
            }),
        );
    }

    /// A name for a SET: at most 8 characters from Shure's printable set.
    fn name_param(cx: &mut Cx, id: CommandId, params: &Params, key: &str) -> Option<String> {
        let name = params.get(key).and_then(Value::as_str).unwrap_or("");
        if name.chars().count() > NAME_LENGTH {
            Self::refuse(
                cx,
                id,
                format!("names are at most {NAME_LENGTH} characters"),
            );
            return None;
        }
        Some(name.to_string())
    }
}

fn antenna_letter(index: u32) -> Option<&'static str> {
    ["a", "b", "c", "d"]
        .get(index.checked_sub(1)? as usize)
        .copied()
}

impl Module for Shure {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let channel = params
            .get("channel")
            .and_then(Value::as_u64)
            .map(|c| c as u32);
        if let Some(ch) = channel {
            if ch > self.channels {
                Self::refuse(cx, id, format!("this model has {} channels", self.channels));
                return;
            }
        }
        let on = |key: &str| params.get(key).and_then(Value::as_bool).unwrap_or(true);
        let int = |key: &str| params.get(key).and_then(Value::as_i64).unwrap_or(0);
        let choice = |key: &str| params.get(key).and_then(Value::as_str).unwrap_or("");
        let slot = || {
            params
                .get("slot")
                .and_then(Value::as_u64)
                .map(|s| s.to_string())
        };
        let family = self.family;
        let ulxd_only = family == Family::Ulxd && !self.is_qlxd();
        match name {
            "mute" => {
                let value = if on("muted") { "ON" } else { "OFF" };
                self.set(cx, id, channel, "AUDIO_MUTE", value);
            }
            "rf_mute" => {
                let value = if on("muted") { "1" } else { "0" };
                self.set(cx, id, channel, "RF_MUTE", value);
            }
            "set_gain" => {
                // "The values REPorted and SET are offset by 18."
                let value = format!("{:03}", int("gain_db") + 18);
                self.set(cx, id, channel, "AUDIO_GAIN", &value);
            }
            "set_frequency" => {
                let second = params.get("second").and_then(Value::as_bool) == Some(true);
                if second && family != Family::Axient {
                    return self.unsupported(cx, id, name);
                }
                let param = if second { "FREQUENCY2" } else { "FREQUENCY" };
                let value = int("frequency_khz").to_string();
                self.set(cx, id, channel, param, &value);
            }
            "set_group_channel" => {
                let second = params.get("second").and_then(Value::as_bool) == Some(true);
                if second && family != Family::Axient {
                    return self.unsupported(cx, id, name);
                }
                let (group, chan) = (choice("group"), choice("group_channel"));
                let param = format!("{}{}", family.group_param(), if second { "2" } else { "" });
                let value = match family {
                    Family::Axient | Family::Slxd => format!("{{{group},{chan}}}"),
                    // Two characters each, as the documents write gg,cc.
                    _ => format!("{group:0>2},{chan:0>2}"),
                };
                self.set(cx, id, channel, &param, &value);
            }
            "set_channel_name" => {
                let Some(name) = Self::name_param(cx, id, params, "name") else {
                    return;
                };
                self.set(cx, id, channel, "CHAN_NAME", &family.text(&name));
            }
            "set_device_id" => {
                let Some(name) = Self::name_param(cx, id, params, "name") else {
                    return;
                };
                self.set(cx, id, None, family.identity_param(), &family.text(&name));
            }
            "flash" => {
                let value = if on("enabled") { "ON" } else { "OFF" };
                self.set(cx, id, channel, "FLASH", value);
            }
            "set_meter_rate" => {
                let ms = int("interval_ms");
                if (1..100).contains(&ms) || ms > family.meter_max() {
                    return Self::refuse(
                        cx,
                        id,
                        format!(
                            "the interval is 0 (off) or 100 to {} ms",
                            family.meter_max()
                        ),
                    );
                }
                let Some(ch) = channel else {
                    return Self::refuse(cx, id, "a channel is required".into());
                };
                let value = if family == Family::Psm1000 {
                    ms.to_string()
                } else {
                    format!("{ms:05}")
                };
                self.set(cx, id, Some(ch), "METER_RATE", &value);
            }
            "set_encryption"
            | "regenerate_encryption_key"
            | "set_high_density"
            | "set_scan_lock"
            | "set_sync_lock"
                if !ulxd_only =>
            {
                self.unsupported(cx, id, name)
            }
            "set_encryption" => {
                let mode = choice("mode").to_ascii_uppercase();
                self.set(cx, id, None, "ENCRYPTION", &mode);
            }
            "regenerate_encryption_key" => {
                self.set(cx, id, None, "ENCRYPTION_REGENERATE_KEY", "REQUESTED");
            }
            "set_high_density" => {
                let value = if on("enabled") { "ON" } else { "OFF" };
                self.request(
                    cx,
                    id,
                    None,
                    "HIGH_DENSITY",
                    None,
                    value,
                    Some(value.into()),
                );
            }
            "set_scan_lock" => {
                let value = if on("locked") { "ON" } else { "OFF" };
                self.set(cx, id, None, "SCAN_LOCK", value);
            }
            "set_sync_lock" => {
                let value = if on("locked") { "ON" } else { "OFF" };
                self.set(cx, id, None, "SYNC_LOCK", value);
            }
            "set_audio_summing_mode" | "set_frequency_diversity_mode" => {
                if !ulxd_only || self.channels < 2 {
                    return self.unsupported(cx, id, name);
                }
                let param = if name == "set_audio_summing_mode" {
                    "AUDIO_SUMMING_MODE"
                } else {
                    "FREQUENCY_DIVERSITY_MODE"
                };
                let mode = choice("mode").to_ascii_uppercase();
                // A mode that does not apply is ignored and the current one
                // reported, so the answer is checked against the request.
                self.request(cx, id, None, param, None, &mode, Some(mode.clone()));
            }
            "set_network" => {
                if !matches!(family, Family::Axient | Family::Ulxd) || self.is_qlxd() {
                    return self.unsupported(cx, id, name);
                }
                let interface = match choice("interface") {
                    "dante_primary" => "D1",
                    "dante_secondary" => "D2",
                    _ => "SC",
                };
                if interface != "SC" && family == Family::Ulxd && self.channels < 2 {
                    return Self::refuse(cx, id, "the ULXD4 has the control interface only".into());
                }
                let value = if choice("mode") == "auto" {
                    "AUTO na na na".to_string()
                } else {
                    let mut fields = Vec::new();
                    for key in ["address", "subnet_mask", "gateway"] {
                        let given = params.get(key).and_then(Value::as_str);
                        let given = given.or((key == "gateway").then_some("0.0.0.0"));
                        match given.map(str::parse::<Ipv4Addr>) {
                            Some(Ok(a)) => fields.push(a.to_string()),
                            _ => {
                                return Self::refuse(
                                    cx,
                                    id,
                                    format!("manual addressing needs a dotted IPv4 {key}"),
                                )
                            }
                        }
                    }
                    format!("MANUAL {}", fields.join(" "))
                };
                // The receiver moves to the new address (or restarts, for
                // Dante) instead of answering, so this is done once sent.
                self.send(cx, &format!("SET NET_SETTINGS {interface} {value}"));
                cx.complete(id, Ok(Outcome::Ack));
            }
            "set_transmitter_rf_mute"
            | "set_transmitter_rf_power"
            | "set_transmitter_offset"
            | "set_transmitter_input_pad"
            | "set_transmitter_polarity"
            | "set_transmitter_name" => {
                if family != Family::Axient {
                    return self.unsupported(cx, id, name);
                }
                let (param, value) = match name {
                    "set_transmitter_rf_mute" => (
                        "SLOT_RF_OUTPUT",
                        if on("muted") { "RF_MUTE" } else { "RF_ON" }.to_string(),
                    ),
                    "set_transmitter_rf_power" => {
                        ("SLOT_RF_POWER_MODE", choice("power").to_ascii_uppercase())
                    }
                    "set_transmitter_offset" => {
                        ("SLOT_OFFSET", (int("offset_db") + 12).to_string())
                    }
                    // 000 engages the -12 dB pad, 012 leaves it out.
                    "set_transmitter_input_pad" => (
                        "SLOT_INPUT_PAD",
                        if on("enabled") { "0" } else { "12" }.to_string(),
                    ),
                    "set_transmitter_polarity" => {
                        ("SLOT_POLARITY", choice("polarity").to_ascii_uppercase())
                    }
                    _ => {
                        let Some(name) = Self::name_param(cx, id, params, "name") else {
                            return;
                        };
                        ("SLOT_TX_DEVICE_ID", format!("{{{name}}}"))
                    }
                };
                self.request(cx, id, channel, param, slot(), &value, None);
            }
            "set_audio_input_level"
            | "set_rf_power"
            | "set_audio_tx_mode"
            | "set_input_line_level" => {
                if family != Family::Psm1000 {
                    return self.unsupported(cx, id, name);
                }
                let (param, value) = match name {
                    "set_audio_input_level" => ("AUDIO_IN_LVL", int("level_db").to_string()),
                    "set_rf_power" => ("RF_TX_LVL", choice("power_mw").to_string()),
                    "set_audio_tx_mode" => (
                        "AUDIO_TX_MODE",
                        match choice("mode") {
                            "mono" => "1",
                            "point_to_point" => "2",
                            _ => "3",
                        }
                        .to_string(),
                    ),
                    _ => (
                        "AUDIO_IN_LINE_LVL",
                        if on("line") { "1" } else { "0" }.to_string(),
                    ),
                };
                self.set(cx, id, channel, param, &value);
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.socket_open = true;
                if self.monitor {
                    self.query_all(cx, true);
                } else {
                    self.ask_identity(cx);
                }
                if self.meter_ms > 0 {
                    for ch in 1..=self.channels {
                        self.send(cx, &self.meter_rate(ch, self.meter_ms));
                    }
                }
                cx.set_timer(SILENCE, SILENCE_TIMEOUT);
                cx.set_timer(REFRESH, self.refresh_every());
                cx.set_timer(FIRST_WORD, FIRST_WORD_WARNING);
            }
            TcpInput::Data(data) => {
                cx.alive();
                cx.set_timer(SILENCE, SILENCE_TIMEOUT);
                for raw in self.framer.feed(&data) {
                    self.message(cx, &raw);
                }
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
            FIRST_WORD => cx.log(
                Level::Warning,
                "connected but the receiver has not answered; on SLX-D, network control \
                 (command strings) must be enabled on the receiver",
            ),
            SILENCE => self.lost(
                cx,
                if self.connected {
                    "no data from the receiver for 15 s".into()
                } else {
                    "the receiver accepted the connection but never answered".into()
                },
            ),
            REFRESH => {
                if self.monitor {
                    self.query_all(cx, false);
                } else {
                    self.ask_identity(cx);
                }
                cx.set_timer(REFRESH, self.refresh_every());
            }
            REPLY => {
                let now = cx.now();
                while let Some(i) = self.pending.iter().position(|p| p.deadline <= now) {
                    let p = self.pending.remove(i).unwrap();
                    cx.complete(p.id, Err(CommandError::Timeout));
                }
                self.arm_reply_timer(cx);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        // A receiver left metering keeps doing it into a dead connection for as
        // long as it stays powered: stop every channel metered by the setting
        // or by a command.
        if self.socket_open {
            for ch in 1..=self.channels {
                let commanded = self.metering.get(&ch).is_some_and(|&ms| ms > 0);
                if self.meter_ms > 0 || commanded {
                    self.send(cx, &self.meter_rate(ch, 0));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn shure(model: &str, channels: u32) -> Shure {
        Shure::for_device(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 30)), PORT),
            model,
            channels,
            1_000,
        )
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

    fn feed(m: &mut Shure, now: Millis, data: &str) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(data.as_bytes().to_vec()));
        cx.take()
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

    fn connected(model: &str, channels: u32) -> (Shure, Vec<Action>) {
        let mut m = shure(model, channels);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        (m, cx.take())
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn framing_splits_on_brackets_and_keeps_partial_messages() {
        let mut f = Framer::default();
        assert_eq!(
            f.feed(b"< REP 1 AUDIO_MUTE ON >< REP 1 CHAN"),
            ["< REP 1 AUDIO_MUTE ON >"]
        );
        assert_eq!(
            f.feed(b"_NAME {Lead Vox          } >"),
            ["< REP 1 CHAN_NAME {Lead Vox          } >"]
        );
    }

    #[test]
    fn braced_values_keep_their_spaces() {
        let m = parse("< REP 1 CHAN_NAME {Lead Vox          } >").unwrap();
        assert_eq!(m.channel, Some(1));
        assert_eq!(m.param, "CHAN_NAME");
        assert_eq!(m.value(), "Lead Vox");
        let m = parse("< REP DEVICE_ID {Rack 1  } >").unwrap();
        assert_eq!(m.channel, None);
        assert_eq!(m.value(), "Rack 1");
    }

    #[test]
    fn connecting_asks_for_everything_and_starts_metering() {
        let (mut m, a) = connected("ad4d", 2);
        let s = sent(&a);
        assert_eq!(
            s[..10],
            [
                "< GET MODEL >",
                "< GET DEVICE_ID >",
                "< GET FW_VER >",
                "< GET RF_BAND >",
                "< GET TRANSMISSION_MODE >",
                "< GET ENCRYPTION_MODE >",
                "< GET QUADVERSITY_MODE >",
                "< GET NET_SETTINGS SC >",
                "< GET NET_SETTINGS D1 >",
                "< GET NET_SETTINGS D2 >",
            ]
        );
        assert_eq!(s[10], "< GET 1 ALL >");
        // Every slot of each channel, once, on connecting.
        assert_eq!(s[11], "< GET 1 SLOT_STATUS 0 >");
        assert_eq!(s.iter().filter(|x| x.contains(" SLOT_")).count(), 30);
        assert!(s.contains(&"< GET 2 ALL >".to_string()));
        assert_eq!(
            s[s.len() - 2..],
            ["< SET 1 METER_RATE 01000 >", "< SET 2 METER_RATE 01000 >"]
        );
        // Connected only once the receiver says something.
        assert!(!a.contains(&Action::Connection(Connection::Connected)));

        // The periodic refresh asks again for what changes are reported on,
        // not for the slots or the network.
        let mut cx = Cx::new(REFRESH_EVERY);
        m.timer(&mut cx, REFRESH);
        let s = sent(&cx.take());
        assert_eq!(s.len(), 7);
        assert!(!s.iter().any(|x| x.contains("SLOT_") || x.contains("NET_")));

        // ULX-D: device settings on connecting, the Dante ones on a dual.
        let (_, a) = connected("ulxd4d", 2);
        let s = sent(&a);
        for p in [
            "< GET ENCRYPTION >",
            "< GET SCAN_LOCK >",
            "< GET AUDIO_SUMMING_MODE >",
            "< GET FREQUENCY_DIVERSITY_MODE >",
            "< GET NET_SETTINGS D1 >",
        ] {
            assert!(s.contains(&p.to_string()), "{p}");
        }
        let (_, a) = connected("ulxd4", 1);
        assert!(!sent(&a)
            .iter()
            .any(|x| x.contains("SUMMING") || x.contains("D1")));
        // QLX-D: only what its strings are known to have.
        let (_, a) = connected("qlxd4", 1);
        assert_eq!(
            sent(&a),
            [
                "< GET DEVICE_ID >",
                "< GET FW_VER >",
                "< GET 1 ALL >",
                "< SET 1 METER_RATE 01000 >"
            ]
        );
    }

    #[test]
    fn the_psm1000_dialect() {
        let (mut m, a) = connected("p10t", 2);
        let s = sent(&a);
        assert!(s.contains(&"< GET 1 CHAN_NAME >\r\n".to_string()));
        assert!(s.contains(&"< SET 1 METER_RATE 1000 >\r\n".to_string()));
        assert!(!s.iter().any(|x| x.contains("ALL")));
        let a = feed(
            &mut m,
            10,
            "< REPORT 1 CHAN_NAME IEM1 >\r\n< REPORT 2 RF_MUTE 1 >\r\n",
        );
        let st = state(&a);
        assert_eq!(st["channels"]["1"]["name"], "IEM1");
        assert_eq!(st["channels"]["2"]["rf_mute"], true);
    }

    #[test]
    fn reports_become_state_with_family_names_offsets_and_sentinels() {
        let (mut m, _) = connected("ad4d", 2);
        let a = feed(
            &mut m,
            10,
            "< REP 1 CHAN_NAME {Lead Vox       } >< REP 1 FREQUENCY 0578350 >\
             < REP 1 AUDIO_MUTE OFF >< REP 1 AUDIO_GAIN 030 >< REP 1 TX_BATT_BARS 255 >\
             < REP 1 TX_BATT_CHARGE_PERCENT 088 >< REP 1 TX_BATT_MINS 65534 >\
             < REP MODEL {AD4D-A    } >< REP FW_VER {2.0.15.2* } >< REP TRANSMISSION_MODE HIGH_DENSITY >",
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        let s = state(&a);
        let ch = &s["channels"]["1"];
        assert_eq!(ch["name"], "Lead Vox");
        assert_eq!(ch["frequency_khz"], 578350);
        assert_eq!(ch["mute"], false);
        assert_eq!(ch["gain_db"], 12);
        assert_eq!(ch["transmitter"], json!({"battery_percent": 88}));
        assert_eq!(
            s["device"],
            json!({"model": "AD4D-A", "firmware": "2.0.15.2", "high_density": true})
        );

        // ULX-D names: BATT_BARS, not TX_BATT_BARS.
        let (mut m, _) = connected("ulxd4", 1);
        let s = state(&feed(
            &mut m,
            10,
            "< REP 1 BATT_BARS 004 >< REP 1 TX_BATT_BARS 002 >",
        ));
        assert_eq!(
            s["channels"]["1"]["transmitter"],
            json!({"battery_bars": 4})
        );
    }

    #[test]
    fn axient_samples_read_the_antenna_count_from_the_status_field() {
        let (mut m, _) = connected("ad4d", 2);
        let s = state(&feed(
            &mut m,
            10,
            "< SAMPLE 1 ALL 005 031 102 098 BB 31 086 31 065 >",
        ));
        let ch = &s["channels"]["1"];
        assert_eq!(ch["quality"], 5);
        assert_eq!(ch["af"], json!({"peak_dbfs": -18, "rms_dbfs": -22}));
        assert_eq!(ch["rf"]["rssi_dbm"], json!({"a": -34, "b": -55}));
        assert_eq!(ch["rf"]["antennas"], json!({"a": "blue", "b": "blue"}));

        // Quadversity: four antennas.
        let s = state(&feed(
            &mut m,
            20,
            "< SAMPLE 2 ALL 255 031 102 102 BRXB 31 083 31 068 00 069 31 072 >",
        ));
        let ch = &s["channels"]["2"];
        assert_eq!(ch.get("quality"), None, "255 is unknown, not a reading");
        assert_eq!(
            ch["rf"]["rssi_dbm"],
            json!({"a": -37, "b": -52, "c": -51, "d": -48})
        );
        assert_eq!(ch["rf"]["antennas"]["c"], "off");

        // FD-C: the second section is not read as more antennas.
        let s = state(&feed(
            &mut m,
            30,
            "< SAMPLE 1 ALL 005 031 102 102 BB 31 082 31 060 BB 31 082 31 060 >",
        ));
        assert_eq!(
            s["channels"]["1"]["rf"]["rssi_dbm"],
            json!({"a": -38, "b": -60})
        );
    }

    #[test]
    fn ulxd_and_slxd_samples() {
        let (mut m, _) = connected("ulxd4d", 2);
        let s = state(&feed(&mut m, 10, "< SAMPLE 2 ALL AX 078 032 >"));
        let ch = &s["channels"]["2"];
        assert_eq!(ch["rf"]["rssi_dbm"], json!({"a": -50}));
        assert_eq!(ch["rf"]["antennas"], json!({"a": "on", "b": "off"}));
        assert_eq!(ch["af"], json!({"level": 32, "rms_dbfs": -18}));

        let (mut m, _) = connected("slxd4d", 2);
        let s = state(&feed(&mut m, 10, "< SAMPLE 1 ALL 102 100 086 >"));
        let ch = &s["channels"]["1"];
        assert_eq!(ch["af"], json!({"peak_dbfs": -18, "rms_dbfs": -20}));
        assert_eq!(ch["rf"]["rssi_dbm"], json!({"a": -34}));
    }

    #[test]
    fn sets_are_encoded_per_family_and_acknowledged_by_the_matching_rep() {
        let (mut m, _) = connected("ulxd4d", 2);
        feed(&mut m, 5, "< REP DEVICE_ID {Rack1   } >");
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "set_gain",
            &params(json!({"channel": 2, "gain_db": -3})),
        );
        m.command(
            &mut cx,
            2,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        m.command(
            &mut cx,
            3,
            "set_channel_name",
            &params(json!({"channel": 1, "name": "Pulpit"})),
        );
        m.command(&mut cx, 4, "flash", &params(json!({"enabled": true})));
        assert_eq!(
            sent(&cx.take()),
            [
                "< SET 2 AUDIO_GAIN 015 >",
                "< SET 1 AUDIO_MUTE ON >",
                "< SET 1 CHAN_NAME {Pulpit} >",
                "< SET FLASH ON >",
            ]
        );
        let a = feed(
            &mut m,
            20,
            "< REP 1 AUDIO_MUTE ON >< REP 2 AUDIO_GAIN 015 >",
        );
        assert!(a.contains(&Action::Complete {
            id: 2,
            result: Ok(Outcome::Ack)
        }));
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));
        assert_eq!(state(&a)["channels"]["2"]["gain_db"], -3);
        // Each REP's state lands before the SET it answers completes.
        let at = |f: fn(&Action) -> bool| a.iter().position(f).unwrap();
        assert!(
            at(|x| matches!(x, Action::State(_))) < at(|x| matches!(x, Action::Complete { .. }))
        );

        let mut cx = Cx::new(2_100);
        m.timer(&mut cx, REPLY);
        let a = cx.take();
        assert!(a.contains(&Action::Complete {
            id: 3,
            result: Err(CommandError::Timeout)
        }));
        assert!(a.contains(&Action::Complete {
            id: 4,
            result: Err(CommandError::Timeout)
        }));
    }

    #[test]
    fn names_longer_than_the_family_allows_are_refused() {
        let (mut m, _) = connected("ulxd4", 1);
        feed(&mut m, 5, "< REP DEVICE_ID {Rack1   } >");
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            1,
            "set_channel_name",
            &params(json!({"channel": 1, "name": "Lead Vocal"})),
        );
        let a = cx.take();
        assert!(sent(&a).is_empty());
        assert!(matches!(
            &a[..],
            [Action::Complete {
                result: Err(CommandError::InvalidParams { .. }),
                ..
            }]
        ));
    }

    #[test]
    fn opened_for_commands_only_it_neither_meters_nor_reads() {
        let mut m = Shure::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 30)),
            host_name: None,
            port: None,
            model: "ad4d".into(),
            channels: Some(2),
            settings: params(json!({"meter_interval_ms": 500})),
            monitor: false,
        });
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = cx.take();
        // No METER_RATE, no GET ALL: the device id alone, as the liveness check.
        assert_eq!(sent(&a), ["< GET DEVICE_ID >"]);
        assert!(a.contains(&Action::SetTimer {
            key: REFRESH,
            after: LIVENESS_EVERY
        }));

        let a = feed(&mut m, 12, "< REP DEVICE_ID {Rack1   } >");
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(12)));

        // The refresh only asks the device id again.
        let mut cx = Cx::new(LIVENESS_EVERY);
        m.timer(&mut cx, REFRESH);
        assert_eq!(sent(&cx.take()), ["< GET DEVICE_ID >"]);

        // Pushed changes are applied, and commands work.
        let a = feed(&mut m, 5_100, "< REP 1 CHAN_NAME {Lead Vox} >");
        assert_eq!(state(&a)["channels"]["1"]["name"], "Lead Vox");
        let mut cx = Cx::new(5_200);
        m.command(
            &mut cx,
            1,
            "mute",
            &params(json!({"channel": 2, "muted": true})),
        );
        assert_eq!(sent(&cx.take()), ["< SET 2 AUDIO_MUTE ON >"]);
        let a = feed(&mut m, 5_230, "< REP 2 AUDIO_MUTE ON >");
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));

        // Metering was never started, so closing has none to stop.
        let mut cx = Cx::new(6_000);
        m.stop(&mut cx);
        assert!(sent(&cx.take()).is_empty());
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let (mut m, _) = connected("ulxd4d", 2);
        // The identity GET sent on connecting is timed.
        let a = feed(&mut m, 20, "< REP DEVICE_ID {Rack1   } >");
        assert!(a.contains(&Action::RoundTrip(20)));
        // Samples and unprompted REPs are pushed, not answers.
        let a = feed(
            &mut m,
            30,
            "< SAMPLE 2 ALL AX 078 032 >< REP 1 CHAN_NAME {A} >",
        );
        assert!(!a.iter().any(|x| matches!(x, Action::RoundTrip(_))));

        let mut cx = Cx::new(100);
        m.command(
            &mut cx,
            1,
            "set_gain",
            &params(json!({"channel": 1, "gain_db": 0})),
        );
        let a = feed(&mut m, 117, "< REP 1 AUDIO_GAIN 018 >");
        assert!(a.contains(&Action::RoundTrip(17)));
    }

    #[test]
    fn a_silent_receiver_is_disconnected_and_metering_stops_on_close() {
        let (mut m, _) = connected("slxd4d", 2);
        let mut cx = Cx::new(5_000);
        m.timer(&mut cx, FIRST_WORD);
        assert!(cx.take().iter().any(|a| matches!(a, Action::Log { .. })));
        let mut cx = Cx::new(15_000);
        m.timer(&mut cx, SILENCE);
        let a = cx.take();
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Connection(Connection::Disconnected { reason }) if reason.contains("never answered")
        )));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));

        let (mut m, _) = connected("ad4d", 2);
        let mut cx = Cx::new(100);
        m.stop(&mut cx);
        assert_eq!(
            sent(&cx.take()),
            ["< SET 1 METER_RATE 00000 >", "< SET 2 METER_RATE 00000 >"]
        );
    }

    /// A receiver that has answered once, so commands are accepted.
    fn ready(model: &str, channels: u32) -> Shure {
        let (mut m, _) = connected(model, channels);
        feed(&mut m, 5, "< REP DEVICE_ID {Rack1   } >");
        m
    }

    fn run(m: &mut Shure, now: Millis, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, &params(p));
        cx.take()
    }

    fn acked(a: &[Action], id: CommandId) -> bool {
        a.contains(&Action::Complete {
            id,
            result: Ok(Outcome::Ack),
        })
    }

    fn refused(a: &[Action], id: CommandId) -> bool {
        a.iter().any(|x| {
            matches!(x, Action::Complete { id: i, result: Err(CommandError::DeviceError { .. }) } if *i == id)
        })
    }

    #[test]
    fn group_channel_device_id_flash_and_meter_rate_are_encoded_per_family() {
        let mut m = ready("ad4d", 2);
        let a = run(
            &mut m,
            10,
            1,
            "set_group_channel",
            json!({"channel": 1, "group": "6", "group_channel": "100"}),
        );
        assert_eq!(sent(&a), ["< SET 1 GROUP_CHANNEL {6,100} >"]);
        // The frequency comes first; the group and channel REP answers.
        let a = feed(
            &mut m,
            20,
            "< REP 1 FREQUENCY 0652875 >< REP 1 GROUP_CHANNEL {6,100     } >",
        );
        assert!(acked(&a, 1));
        let s = state(&a);
        assert_eq!(s["channels"]["1"]["group"], "6");
        assert_eq!(s["channels"]["1"]["group_channel"], "100");
        assert_eq!(s["channels"]["1"]["frequency_khz"], 652875);
        // A frequency off the group plan reports no group or channel.
        let s = state(&feed(&mut m, 30, "< REP 1 GROUP_CHANNEL {--,--     } >"));
        assert_eq!(s["channels"]["1"], json!({}));

        // FD-C's second carrier.
        let a = run(
            &mut m,
            40,
            2,
            "set_frequency",
            json!({"channel": 2, "frequency_khz": 578850, "second": true}),
        );
        assert_eq!(sent(&a), ["< SET 2 FREQUENCY2 578850 >"]);

        let a = run(&mut m, 50, 3, "set_device_id", json!({"name": "Rack 2"}));
        assert_eq!(sent(&a), ["< SET DEVICE_ID {Rack 2} >"]);
        let a = run(
            &mut m,
            50,
            4,
            "flash",
            json!({"channel": 2, "enabled": true}),
        );
        assert_eq!(sent(&a), ["< SET 2 FLASH ON >"]);
        let a = run(
            &mut m,
            50,
            5,
            "set_meter_rate",
            json!({"channel": 1, "interval_ms": 250}),
        );
        assert_eq!(sent(&a), ["< SET 1 METER_RATE 00250 >"]);
        let a = run(
            &mut m,
            50,
            6,
            "set_meter_rate",
            json!({"channel": 1, "interval_ms": 50}),
        );
        assert!(sent(&a).is_empty());

        // ULX-D abbreviates and pads to two characters, unbraced.
        let mut m = ready("ulxd4d", 2);
        let a = run(
            &mut m,
            10,
            1,
            "set_group_channel",
            json!({"channel": 2, "group": "3", "group_channel": "7"}),
        );
        assert_eq!(sent(&a), ["< SET 2 GROUP_CHAN 03,07 >"]);
        let a = feed(
            &mut m,
            20,
            "< REP 2 FREQUENCY 578350 >< REP 2 GROUP_CHAN 03,07 >",
        );
        assert!(acked(&a, 1));
        assert_eq!(state(&a)["channels"]["2"]["group_channel"], "7");
        // The second carrier is Axient Digital's alone.
        let a = run(
            &mut m,
            30,
            2,
            "set_frequency",
            json!({"channel": 1, "frequency_khz": 578850, "second": true}),
        );
        assert!(matches!(
            &a[..],
            [Action::Complete {
                result: Err(CommandError::UnsupportedForModel { .. }),
                ..
            }]
        ));

        // PSM1000: bare names, CRLF.
        let mut m = ready("p10t", 2);
        let a = run(&mut m, 10, 1, "set_device_id", json!({"name": "IEM"}));
        assert_eq!(sent(&a), ["< SET DEVICE_NAME IEM >\r\n"]);
        let a = feed(&mut m, 20, "< REPORT DEVICE_NAME IEM >\r\n");
        assert!(acked(&a, 1));
        assert_eq!(state(&a)["device"]["id"], "IEM");
    }

    #[test]
    fn ulxd_device_settings_and_the_modes_a_receiver_ignores() {
        let mut m = ready("ulxd4q", 4);
        let a = run(&mut m, 10, 1, "set_encryption", json!({"mode": "manual"}));
        assert_eq!(sent(&a), ["< SET ENCRYPTION MANUAL >"]);
        let a = feed(&mut m, 20, "< REP ENCRYPTION MANUAL >");
        assert!(acked(&a, 1));
        assert_eq!(state(&a)["device"]["encryption"], "manual");

        let a = run(&mut m, 30, 2, "regenerate_encryption_key", json!({}));
        assert_eq!(sent(&a), ["< SET ENCRYPTION_REGENERATE_KEY REQUESTED >"]);
        let a = feed(
            &mut m,
            40,
            "< REP ENCRYPTION_REGENERATE_KEY REQUESTED >< REP ENCRYPTION_REGENERATE_KEY COMPLETED >",
        );
        assert!(acked(&a, 2));
        assert_eq!(
            state(&a)["device"]["encryption_key_regeneration"],
            "completed"
        );

        for (id, name, p, wire) in [
            (
                3,
                "set_high_density",
                json!({"enabled": true}),
                "< SET HIGH_DENSITY ON >",
            ),
            (
                4,
                "set_scan_lock",
                json!({"locked": true}),
                "< SET SCAN_LOCK ON >",
            ),
            (
                5,
                "set_sync_lock",
                json!({"locked": false}),
                "< SET SYNC_LOCK OFF >",
            ),
            (
                6,
                "set_audio_summing_mode",
                json!({"mode": "1+2/3+4"}),
                "< SET AUDIO_SUMMING_MODE 1+2/3+4 >",
            ),
        ] {
            assert_eq!(sent(&run(&mut m, 50, id, name, p)), [wire]);
        }
        let a = feed(
            &mut m,
            60,
            "< REP HIGH_DENSITY ON >< REP SCAN_LOCK ON >< REP SYNC_LOCK OFF >< REP AUDIO_SUMMING_MODE 1+2/3+4 >",
        );
        for id in 3..=6 {
            assert!(acked(&a, id), "{id}");
        }
        let s = state(&a);
        assert_eq!(s["device"]["high_density"], true);
        assert_eq!(s["device"]["scan_lock"], true);
        assert_eq!(s["device"]["sync_lock"], false);
        assert_eq!(s["device"]["audio_summing_mode"], "1+2/3+4");

        // A mode that does not apply is ignored and the current one reported.
        run(
            &mut m,
            70,
            7,
            "set_frequency_diversity_mode",
            json!({"mode": "3+4"}),
        );
        let a = feed(&mut m, 80, "< REP FREQUENCY_DIVERSITY_MODE OFF >");
        assert!(refused(&a, 7));
        assert_eq!(state(&a)["device"]["frequency_diversity_mode"], "off");

        // Not on a single-channel receiver, and none of it on Axient Digital.
        let mut m = ready("ulxd4", 1);
        let a = run(
            &mut m,
            10,
            1,
            "set_audio_summing_mode",
            json!({"mode": "1+2"}),
        );
        assert!(sent(&a).is_empty());
        let mut m = ready("ad4d", 2);
        let a = run(&mut m, 10, 1, "set_encryption", json!({"mode": "auto"}));
        assert!(matches!(
            &a[..],
            [Action::Complete {
                result: Err(CommandError::UnsupportedForModel { .. }),
                ..
            }]
        ));
    }

    #[test]
    fn rep_err_refuses_the_oldest_waiting_set() {
        let mut m = ready("ad4d", 2);
        run(
            &mut m,
            10,
            1,
            "set_transmitter_rf_mute",
            json!({"channel": 1, "slot": 3, "muted": true}),
        );
        run(&mut m, 11, 2, "mute", json!({"channel": 2, "muted": true}));
        let a = feed(&mut m, 30, "< REP ERR >");
        assert!(a.contains(&Action::RoundTrip(20)));
        assert!(refused(&a, 1));
        let a = feed(&mut m, 40, "< REP 2 AUDIO_MUTE ON >");
        assert!(acked(&a, 2));
        // With nothing waiting, an ERR (a GET the receiver does not know) is
        // ignored.
        assert!(!feed(&mut m, 50, "< REP ERR >")
            .iter()
            .any(|x| matches!(x, Action::Complete { .. })));
    }

    #[test]
    fn showlink_transmitters_are_set_and_kept_per_slot() {
        let mut m = ready("ad4q", 4);
        for (id, name, p, wire) in [
            (
                1,
                "set_transmitter_rf_mute",
                json!({"channel": 1, "slot": 2, "muted": true}),
                "< SET 1 SLOT_RF_OUTPUT 2 RF_MUTE >",
            ),
            (
                2,
                "set_transmitter_rf_power",
                json!({"channel": 1, "slot": 2, "power": "low"}),
                "< SET 1 SLOT_RF_POWER_MODE 2 LOW >",
            ),
            (
                3,
                "set_transmitter_offset",
                json!({"channel": 1, "slot": 2, "offset_db": -7}),
                "< SET 1 SLOT_OFFSET 2 5 >",
            ),
            (
                4,
                "set_transmitter_input_pad",
                json!({"channel": 1, "slot": 2, "enabled": true}),
                "< SET 1 SLOT_INPUT_PAD 2 0 >",
            ),
            (
                5,
                "set_transmitter_polarity",
                json!({"channel": 1, "slot": 2, "polarity": "negative"}),
                "< SET 1 SLOT_POLARITY 2 NEGATIVE >",
            ),
            (
                6,
                "set_transmitter_name",
                json!({"channel": 1, "slot": 2, "name": "Vox"}),
                "< SET 1 SLOT_TX_DEVICE_ID 2 {Vox} >",
            ),
            (
                7,
                "set_transmitter_offset",
                json!({"channel": 1, "slot": 3, "offset_db": 0}),
                "< SET 1 SLOT_OFFSET 3 12 >",
            ),
        ] {
            assert_eq!(sent(&run(&mut m, 10, id, name, p)), [wire]);
        }
        // Slot 3's answer completes slot 3's SET, not slot 2's.
        let a = feed(&mut m, 20, "< REP 1 SLOT_OFFSET 3 012 >");
        assert!(acked(&a, 7));
        assert!(!acked(&a, 3));
        let a = feed(
            &mut m,
            30,
            "< REP 1 SLOT_RF_OUTPUT 2 RF_MUTE >< REP 1 SLOT_RF_POWER_MODE 2 LOW >\
             < REP 1 SLOT_RF_POWER 2 002 >< REP 1 SLOT_OFFSET 2 005 >< REP 1 SLOT_INPUT_PAD 2 000 >\
             < REP 1 SLOT_POLARITY 2 NEGATIVE >< REP 1 SLOT_TX_DEVICE_ID 2 {Vox                            } >\
             < REP 1 SLOT_STATUS 2 LINKED.ACTIVE >< REP 1 SLOT_TX_MODEL 2 ADX1 >\
             < REP 1 SLOT_SHOWLINK_STATUS 2 004 >< REP 1 SLOT_BATT_MINS 2 65534 >\
             < REP 1 SLOT_BATT_CHARGE_PERCENT 2 087 >< REP 1 SLOT_BATT_TYPE 2 LION >\
             < REP 1 SLOT_TX_DEVICE_ID 5 {                               } >",
        );
        for id in 1..=6 {
            assert!(acked(&a, id), "{id}");
        }
        let s = state(&a);
        assert_eq!(
            s["channels"]["1"]["slots"]["2"],
            json!({
                "rf_mute": true, "rf_power": "low", "rf_power_mw": 2, "offset_db": -7,
                "input_pad": true, "polarity": "negative", "device_id": "Vox",
                "status": "linked_active", "model": "adx1", "showlink_quality": 4,
                "battery_percent": 87, "battery_type": "lion",
            })
        );
        assert_eq!(s["channels"]["1"]["slots"].get("5"), Some(&json!({})));

        // Slots are Axient Digital's.
        let mut m = ready("ulxd4d", 2);
        let a = run(
            &mut m,
            10,
            1,
            "set_transmitter_polarity",
            json!({"channel": 1, "slot": 1, "polarity": "positive"}),
        );
        assert!(sent(&a).is_empty());
    }

    #[test]
    fn transmitter_side_channel_state_is_kept_for_each_family() {
        let mut m = ready("ulxd4d", 2);
        let s = state(&feed(
            &mut m,
            10,
            "< REP 1 TX_TYPE ULXD6 >< REP 1 TX_OFFSET 015 >< REP 1 TX_RF_PWR NORMAL >\
             < REP 1 TX_MENU_LOCK ON >< REP 1 TX_PWR_LOCK OFF >< REP 1 TX_MUTE_STATUS ON >\
             < REP 1 TX_MUTE_BUTTON_STATUS PRESSED >< REP 1 TX_POWER_SOURCE BATTERY >\
             < REP 1 BATT_TYPE LION >< REP 1 BATT_HEALTH 097 >< REP 1 BATT_CYCLE 00019 >\
             < REP 1 BATT_TEMP_C 062 >< REP 1 TX_DEVICE_ID {Lead    } >< REP 1 RF_INT_DET CRITICAL >\
             < REP 1 ENCRYPTION_WARNING ON >< REP 2 TX_TYPE UNKN >< REP 2 BATT_CYCLE 65535 >",
        ));
        assert_eq!(
            s["channels"]["1"]["transmitter"],
            json!({
                "model": "ulxd6", "offset_db": 3, "rf_power": "normal", "menu_lock": true,
                "power_lock": false, "muted": true, "mute_button_pressed": true,
                "power_source": "battery", "battery_type": "lion", "battery_health_percent": 97,
                "battery_cycles": 19, "battery_temp_c": 22, "device_id": "Lead",
            })
        );
        assert_eq!(s["channels"]["1"]["interference"], true);
        assert_eq!(s["channels"]["1"]["encryption_warning"], true);
        assert_eq!(s["channels"]["2"]["transmitter"], json!({}));

        let mut m = ready("ad4d", 2);
        let s = state(&feed(
            &mut m,
            10,
            "< REP 1 TX_MODEL AD2 >< REP 1 TX_LOCK MENU >< REP 1 TX_POWER_LEVEL 010 >\
             < REP 1 TX_MUTE_MODE_STATUS MUTE >< REP 1 TX_TALK_SWITCH OFF >< REP 1 TX_INPUT_PAD 012 >\
             < REP 1 TX_POLARITY POSITIVE >< REP 1 ENCRYPTION_STATUS OK >< REP 1 INTERFERENCE_STATUS2 DETECTED >\
             < REP 1 UNREGISTERED_TX_STATUS ERROR >< REP 1 FD_MODE FD-C >< REP 1 FREQUENCY2 0578850 >\
             < REP ENCRYPTION_MODE ON >< REP QUADVERSITY_MODE OFF >\
             < REP NET_SETTINGS SC AUTO 192.168.001.025 255.255.255.000 000.000.000.000 00:0E:DD:45:60:EB >",
        ));
        let ch = &s["channels"]["1"];
        assert_eq!(
            ch["transmitter"],
            json!({
                "model": "ad2", "lock": "menu", "menu_lock": true, "power_lock": false,
                "rf_power_mw": 10, "muted": true, "talk_switch": false, "input_pad": false,
                "polarity": "positive",
            })
        );
        assert_eq!(ch["encryption_warning"], false);
        assert_eq!(ch["interference2"], true);
        assert_eq!(ch["unregistered_transmitter"], true);
        assert_eq!(ch["fd_mode"], "fd-c");
        assert_eq!(ch["frequency2_khz"], 578850);
        assert_eq!(s["device"]["encryption"], "on");
        assert_eq!(s["device"]["quadversity"], false);
        assert_eq!(
            s["device"]["network"]["control"],
            json!({"mode": "auto", "address": "192.168.1.25", "subnet_mask": "255.255.255.0",
                   "mac": "00:0e:dd:45:60:eb"})
        );

        let mut m = ready("slxd4d", 2);
        let s = state(&feed(
            &mut m,
            10,
            "< REP 2 AUDIO_OUT_LVL_SWITCH LINE >< REP LOCK_STATUS MENU >< REP 2 TX_MODEL SLXD2 >",
        ));
        assert_eq!(s["channels"]["2"]["audio_output_level"], "line");
        assert_eq!(s["channels"]["2"]["transmitter"]["model"], "slxd2");
        assert_eq!(s["device"]["transmitter_lock"], "menu");

        // ULX-D's audio level in dBFS, as the current document gives it.
        let mut m = ready("ulxd4", 1);
        let s = state(&feed(&mut m, 10, "< SAMPLE 1 ALL AX 078 032 >"));
        assert_eq!(
            s["channels"]["1"]["af"],
            json!({"level": 32, "rms_dbfs": -18})
        );
    }

    #[test]
    fn psm1000_transmitter_settings() {
        let mut m = ready("p10t", 2);
        for (id, name, p, wire) in [
            (
                1,
                "set_audio_input_level",
                json!({"channel": 1, "level_db": -16}),
                "< SET 1 AUDIO_IN_LVL -16 >\r\n",
            ),
            (
                2,
                "set_rf_power",
                json!({"channel": 1, "power_mw": "50"}),
                "< SET 1 RF_TX_LVL 50 >\r\n",
            ),
            (
                3,
                "set_audio_tx_mode",
                json!({"channel": 2, "mode": "point_to_point"}),
                "< SET 2 AUDIO_TX_MODE 2 >\r\n",
            ),
            (
                4,
                "set_input_line_level",
                json!({"channel": 2, "line": true}),
                "< SET 2 AUDIO_IN_LINE_LVL 1 >\r\n",
            ),
            (
                5,
                "set_group_channel",
                json!({"channel": 2, "group": "1", "group_channel": "12"}),
                "< SET 2 GROUP_CHAN 01,12 >\r\n",
            ),
        ] {
            assert_eq!(sent(&run(&mut m, 10, id, name, p)), [wire]);
        }
        let a = feed(
            &mut m,
            20,
            "< REPORT 1 AUDIO_IN_LVL -16 >\r\n< REPORT 1 RF_TX_LVL 50 >\r\n\
             < REPORT 2 AUDIO_TX_MODE 2 >\r\n< REPORT 2 AUDIO_IN_LINE_LVL 1 >\r\n\
             < REPORT 2 FREQUENCY 578000 >\r\n< REPORT 2 GROUP_CHAN 01,12 >\r\n",
        );
        for id in 1..=5 {
            assert!(acked(&a, id), "{id}");
        }
        let s = state(&a);
        assert_eq!(s["channels"]["1"]["audio_input_level_db"], -16);
        assert_eq!(s["channels"]["1"]["rf_power_mw"], 50);
        assert_eq!(s["channels"]["2"]["audio_tx_mode"], "point_to_point");
        assert_eq!(s["channels"]["2"]["input_line_level"], true);
        assert_eq!(s["channels"]["2"]["group"], "1");
    }

    #[test]
    fn network_settings_are_sent_and_done_once_sent() {
        let mut m = ready("ad4d", 2);
        let a = run(
            &mut m,
            10,
            1,
            "set_network",
            json!({"interface": "dante_primary", "mode": "manual", "address": "10.10.1.15",
                   "subnet_mask": "255.255.255.0", "gateway": "10.10.1.1"}),
        );
        assert_eq!(
            sent(&a),
            ["< SET NET_SETTINGS D1 MANUAL 10.10.1.15 255.255.255.0 10.10.1.1 >"]
        );
        assert!(acked(&a, 1));
        let a = run(
            &mut m,
            10,
            2,
            "set_network",
            json!({"interface": "control", "mode": "auto"}),
        );
        assert_eq!(sent(&a), ["< SET NET_SETTINGS SC AUTO na na na >"]);
        let a = run(
            &mut m,
            10,
            3,
            "set_network",
            json!({"interface": "control", "mode": "manual", "address": "10.0.0.400"}),
        );
        assert!(sent(&a).is_empty());
        // The ULXD4 has no Dante interface.
        let mut m = ready("ulxd4", 1);
        let a = run(
            &mut m,
            10,
            1,
            "set_network",
            json!({"interface": "dante_primary", "mode": "auto"}),
        );
        assert!(sent(&a).is_empty());
    }

    #[test]
    fn metering_started_by_a_command_is_stopped_on_closing() {
        let mut m = Shure::for_device(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 30)), PORT),
            "ad4d",
            2,
            0,
        );
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        feed(&mut m, 5, "< REP DEVICE_ID {Rack1} >");
        run(
            &mut m,
            10,
            1,
            "set_meter_rate",
            json!({"channel": 2, "interval_ms": 500}),
        );
        let a = feed(&mut m, 20, "< REP 2 METER_RATE 00500 >");
        assert!(acked(&a, 1));
        assert_eq!(state(&a)["channels"]["2"]["meter_rate_ms"], 500);
        let mut cx = Cx::new(30);
        m.stop(&mut cx);
        assert_eq!(sent(&cx.take()), ["< SET 2 METER_RATE 00000 >"]);
    }
}
