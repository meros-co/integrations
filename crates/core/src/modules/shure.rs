//! Shure networked wireless over the command strings protocol, TCP 2202.
//!
//! From Shure's Axient Digital and ULX-D command string documents, with the
//! SLX-D and PSM1000 dialects as RFDeck recorded them from Shure's documents
//! for those families (see the spec's sources):
//!
//! - Messages are ASCII in angle brackets, `< GET 1 CHAN_NAME >`, with no line
//!   breaks between them; string values are `{braced and padded}`.
//! - REP answers GET and SET, and is also sent unprompted whenever a value
//!   changes, so apart from metering nothing needs polling.
//! - Metering is a per-channel subscription, `SET n METER_RATE 01000`, that
//!   produces one SAMPLE per channel per interval, until set to 00000.
//! - Names, offsets and SAMPLE layouts differ by family.

use std::collections::VecDeque;
use std::net::SocketAddr;

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

const REPLY: Key = "reply";
const REFRESH: Key = "refresh";
const SILENCE: Key = "silence";
const RETRY: Key = "retry";
const FIRST_WORD: Key = "first-word";

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

    fn name_length(self) -> usize {
        match self {
            Family::Axient => 31,
            _ => 8,
        }
    }
}

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

/// SET commands wait for the REP of the same channel and parameter.
struct Pending {
    id: CommandId,
    channel: Option<u32>,
    param: String,
    deadline: Millis,
}

pub(crate) struct Shure {
    device: SocketAddr,
    family: Family,
    channels: u32,
    meter_ms: i64,
    framer: Framer,
    socket_open: bool,
    connected: bool,
    pending: VecDeque<Pending>,
    retry_after: Millis,
}

impl Shure {
    pub(crate) fn new(ctx: OpenContext) -> Shure {
        let meter_ms = ctx
            .settings
            .get("meter_interval_ms")
            .and_then(Value::as_i64)
            .unwrap_or(DEFAULT_METER_MS);
        Shure::for_device(
            SocketAddr::new(ctx.host, ctx.port.unwrap_or(PORT)),
            Family::of(&ctx.model),
            ctx.channels.unwrap_or(2),
            meter_ms,
        )
    }

    fn for_device(device: SocketAddr, family: Family, channels: u32, meter_ms: i64) -> Shure {
        Shure {
            device,
            family,
            channels,
            // Shure documents 100 ms as the minimum interval.
            meter_ms: if meter_ms == 0 { 0 } else { meter_ms.max(100) },
            framer: Framer::default(),
            socket_open: false,
            connected: false,
            pending: VecDeque::new(),
            retry_after: RETRY_MIN,
        }
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

    /// Everything, once: device identity, then each channel.
    fn query_all(&self, cx: &mut Cx) {
        match self.family {
            Family::Axient => {
                for p in [
                    "MODEL",
                    "DEVICE_ID",
                    "FW_VER",
                    "RF_BAND",
                    "TRANSMISSION_MODE",
                ] {
                    self.send(cx, &format!("GET {p}"));
                }
            }
            Family::Slxd => {
                for p in ["MODEL", "DEVICE_ID", "FW_VER", "RF_BAND"] {
                    self.send(cx, &format!("GET {p}"));
                }
            }
            Family::Ulxd => {
                for p in ["DEVICE_ID", "FW_VER", "HIGH_DENSITY"] {
                    self.send(cx, &format!("GET {p}"));
                }
            }
            Family::Psm1000 => self.send(cx, "GET DEVICE_NAME"),
        }
        for ch in 1..=self.channels {
            if self.family == Family::Psm1000 {
                // No GET ALL on the PSM1000.
                for p in ["CHAN_NAME", "FREQUENCY", "RF_MUTE"] {
                    self.send(cx, &format!("GET {ch} {p}"));
                }
            } else {
                self.send(cx, &format!("GET {ch} ALL"));
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
        self.framer = Framer::default();
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn arm_reply_timer(&self, cx: &mut Cx) {
        match self.pending.iter().map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    fn set(&mut self, cx: &mut Cx, id: CommandId, channel: Option<u32>, param: &str, value: &str) {
        let body = match channel {
            Some(ch) => format!("SET {ch} {param} {value}"),
            None => format!("SET {param} {value}"),
        };
        self.send(cx, &body);
        self.pending.push_back(Pending {
            id,
            channel,
            param: param.to_string(),
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
            "REP" | "REPORT" => {
                if let Some(i) = self
                    .pending
                    .iter()
                    .position(|p| p.channel == msg.channel && p.param == msg.param)
                {
                    let p = self.pending.remove(i).unwrap();
                    cx.complete(p.id, Ok(Outcome::Ack));
                    self.arm_reply_timer(cx);
                }
                if let Some(patch) = self.report(&msg) {
                    cx.state(patch);
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

    /// A REP's effect on the state.
    fn report(&self, msg: &Message) -> Option<Value> {
        let v = msg.value();
        let on = v == "ON";
        let Some(ch) = msg.channel else {
            let device = match msg.param.as_str() {
                "MODEL" => json!({"model": v}),
                "DEVICE_ID" | "DEVICE_NAME" => json!({"id": v}),
                "FW_VER" => json!({"firmware": v.trim_end_matches('*').trim()}),
                "RF_BAND" => json!({"rf_band": v}),
                "TRANSMISSION_MODE" => json!({"high_density": v == "HIGH_DENSITY"}),
                "HIGH_DENSITY" => json!({"high_density": on}),
                _ => return None,
            };
            return Some(json!({"device": device}));
        };
        let number = |s: &str| s.trim().parse::<i64>().ok();
        let channel = match (msg.param.as_str(), self.family) {
            ("CHAN_NAME", _) => json!({"name": v}),
            ("FREQUENCY", _) => json!({"frequency_khz": number(v)}),
            ("AUDIO_MUTE", _) => json!({"mute": on}),
            ("RF_MUTE", Family::Psm1000) => json!({"rf_mute": v == "1"}),
            ("AUDIO_GAIN", _) => json!({"gain_db": number(v).map(|g| g - 18)}),
            ("CHAN_QUALITY", Family::Axient) => json!({"quality": gauge(v)}),
            ("TX_BATT_BARS", Family::Axient | Family::Slxd) | ("BATT_BARS", Family::Ulxd) => {
                json!({"transmitter": {"battery_bars": gauge(v)}})
            }
            ("TX_BATT_CHARGE_PERCENT", Family::Axient) | ("BATT_CHARGE", Family::Ulxd) => {
                json!({"transmitter": {"battery_percent": gauge(v)}})
            }
            ("TX_BATT_MINS", Family::Axient | Family::Slxd) | ("BATT_RUN_TIME", Family::Ulxd) => {
                // 65533 communication warning, 65534 calculating, 65535 unknown.
                json!({"transmitter": {"battery_minutes": number(v).filter(|&m| m <= 65_532)}})
            }
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
            ("AUDIO_IN_LVL_L", Family::Psm1000) => json!({"af": {"input_meter_left": number(v)}}),
            ("AUDIO_IN_LVL_R", Family::Psm1000) => {
                json!({"af": {"input_meter_right": number(v)}})
            }
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
            // < SAMPLE x ALL nn aaa eee >: antenna LEDs lit, RF, audio.
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
                    "af": {"level": n(audio)},
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
                cx.complete(
                    id,
                    Err(CommandError::InvalidParams {
                        message: format!("this model has {} channels", self.channels),
                    }),
                );
                return;
            }
        }
        let flag = |key: &str| params.get(key).and_then(Value::as_bool).unwrap_or(true);
        let int = |key: &str| params.get(key).and_then(Value::as_i64).unwrap_or(0);
        match name {
            "mute" => {
                let value = if flag("muted") { "ON" } else { "OFF" };
                self.set(cx, id, channel, "AUDIO_MUTE", value);
            }
            "rf_mute" => {
                let value = if flag("muted") { "1" } else { "0" };
                self.set(cx, id, channel, "RF_MUTE", value);
            }
            "set_gain" => {
                // "The values REPorted and SET are offset by 18."
                let value = format!("{:03}", int("gain_db") + 18);
                self.set(cx, id, channel, "AUDIO_GAIN", &value);
            }
            "set_frequency" => {
                let value = int("frequency_khz").to_string();
                self.set(cx, id, channel, "FREQUENCY", &value);
            }
            "set_channel_name" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let limit = self.family.name_length();
                if name.chars().count() > limit {
                    cx.complete(
                        id,
                        Err(CommandError::InvalidParams {
                            message: format!("names are at most {limit} characters on this model"),
                        }),
                    );
                    return;
                }
                let value = if self.family == Family::Psm1000 {
                    name.to_string()
                } else {
                    format!("{{{name}}}")
                };
                self.set(cx, id, channel, "CHAN_NAME", &value);
            }
            "flash" => {
                let value = if flag("enabled") { "ON" } else { "OFF" };
                self.set(cx, id, None, "FLASH", value);
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
                self.query_all(cx);
                if self.meter_ms > 0 {
                    for ch in 1..=self.channels {
                        self.send(cx, &self.meter_rate(ch, self.meter_ms));
                    }
                }
                cx.set_timer(SILENCE, SILENCE_TIMEOUT);
                cx.set_timer(REFRESH, REFRESH_EVERY);
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
                self.query_all(cx);
                cx.set_timer(REFRESH, REFRESH_EVERY);
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
        // long as it stays powered.
        if self.socket_open && self.meter_ms > 0 {
            for ch in 1..=self.channels {
                self.send(cx, &self.meter_rate(ch, 0));
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

    fn shure(family: Family, channels: u32) -> Shure {
        Shure::for_device(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 30)), PORT),
            family,
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

    fn connected(family: Family, channels: u32) -> (Shure, Vec<Action>) {
        let mut m = shure(family, channels);
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
        let (_, a) = connected(Family::Axient, 2);
        assert_eq!(
            sent(&a),
            [
                "< GET MODEL >",
                "< GET DEVICE_ID >",
                "< GET FW_VER >",
                "< GET RF_BAND >",
                "< GET TRANSMISSION_MODE >",
                "< GET 1 ALL >",
                "< GET 2 ALL >",
                "< SET 1 METER_RATE 01000 >",
                "< SET 2 METER_RATE 01000 >",
            ]
        );
        // Connected only once the receiver says something.
        assert!(!a.contains(&Action::Connection(Connection::Connected)));
    }

    #[test]
    fn the_psm1000_dialect() {
        let (mut m, a) = connected(Family::Psm1000, 2);
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
        let (mut m, _) = connected(Family::Axient, 2);
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
        let (mut m, _) = connected(Family::Ulxd, 1);
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
        let (mut m, _) = connected(Family::Axient, 2);
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
        let (mut m, _) = connected(Family::Ulxd, 2);
        let s = state(&feed(&mut m, 10, "< SAMPLE 2 ALL AX 078 032 >"));
        let ch = &s["channels"]["2"];
        assert_eq!(ch["rf"]["rssi_dbm"], json!({"a": -50}));
        assert_eq!(ch["rf"]["antennas"], json!({"a": "on", "b": "off"}));
        assert_eq!(ch["af"], json!({"level": 32}));

        let (mut m, _) = connected(Family::Slxd, 2);
        let s = state(&feed(&mut m, 10, "< SAMPLE 1 ALL 102 100 086 >"));
        let ch = &s["channels"]["1"];
        assert_eq!(ch["af"], json!({"peak_dbfs": -18, "rms_dbfs": -20}));
        assert_eq!(ch["rf"]["rssi_dbm"], json!({"a": -34}));
    }

    #[test]
    fn sets_are_encoded_per_family_and_acknowledged_by_the_matching_rep() {
        let (mut m, _) = connected(Family::Ulxd, 2);
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
        let (mut m, _) = connected(Family::Ulxd, 1);
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
    fn a_silent_receiver_is_disconnected_and_metering_stops_on_close() {
        let (mut m, _) = connected(Family::Slxd, 2);
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

        let (mut m, _) = connected(Family::Axient, 2);
        let mut cx = Cx::new(100);
        m.stop(&mut cx);
        assert_eq!(
            sent(&cx.take()),
            ["< SET 1 METER_RATE 00000 >", "< SET 2 METER_RATE 00000 >"]
        );
    }
}
