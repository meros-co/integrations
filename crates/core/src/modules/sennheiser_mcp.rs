//! Sennheiser evolution wireless G3/G4 over the Media Control Protocol.
//!
//! Wire format from Sennheiser TI 1254 v1.0. Lifecycle from RFDeck's G3/G4
//! client, which was tested against real racks: subscribe with Push, renew
//! every 8 s, declare the device gone after 15 s of silence, and back off an
//! address that never answers. Readings that differ from RFDeck were reviewed
//! with it in RFDeck `docs/INTEGRATIONS_CORE_REVIEW.md`.

use std::net::{IpAddr, SocketAddr};

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
};

/// "Devices of ew G4 series can be set and read via Ethernet at port 53212,
/// i.e. for sending and reception the same port number is used." (TI 1254 p.5)
const PORT: u16 = 53212;
const SOCKET: Key = "mcp";

/// Push parameters: a 60 s subscription, cyclic attributes every 500 ms, and
/// mode 3 (configuration attributes on change, cyclic attributes on warning
/// change). The cycle time has a 100 ms resolution (TI 1254 p.12).
const SUBSCRIBE: &str = "Push 60 500 3";

/// Renew well inside the 60 s subscription: some firmware drops it early while
/// in RF-Mute (RFDeck, observed on hardware).
const RESUBSCRIBE_EVERY: Millis = 8_000;
/// 30 missed 500 ms cycles.
const SILENCE_TIMEOUT: Millis = 15_000;
/// Silent offline cycles before an address is only probed occasionally.
const ATTEMPTS_BEFORE_BACKOFF: u32 = 4;
/// Probe interval once backed off. Two datagrams to one host every 30 s is
/// harmless; endlessly re-subscribing stale addresses locked up DMX nodes.
const SLOW_PROBE_EVERY: Millis = 30_000;
/// How long a set command waits for the device's echo before reporting
/// `unverified`, and a get before reporting a timeout.
const REPLY_WAIT: Millis = 1_000;

const RESUB: Key = "resubscribe";
const SILENCE: Key = "silence";
const SLOW_PROBE: Key = "slow-probe";
const REPLY: Key = "reply";

const EM_EQUALIZER: [&str; 4] = ["flat", "low_cut", "low_cut_high_boost", "high_boost"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// EM receivers: EM 300-500 G4 and G3.
    Receiver,
    /// SR IEM transmitters.
    Transmitter,
}

#[derive(Debug)]
enum Expect {
    /// A set request: the device echoes the instruction on success.
    Echo(String),
    /// A get request: the reply starts with this keyword.
    FirmwareRevision,
    RfConfig,
}

#[derive(Debug)]
struct Pending {
    id: CommandId,
    sent: String,
    expect: Expect,
    deadline: Millis,
}

pub(crate) struct Mcp {
    device: SocketAddr,
    kind: Kind,
    connected: bool,
    disconnect_reported: bool,
    unanswered_cycles: u32,
    backed_off: bool,
    pending: Vec<Pending>,
}

impl Mcp {
    pub(crate) fn new(ctx: OpenContext) -> Mcp {
        let kind = if ctx.model.starts_with("sr-") {
            Kind::Transmitter
        } else {
            Kind::Receiver
        };
        Mcp::for_host(ctx.host, kind)
    }

    fn for_host(host: IpAddr, kind: Kind) -> Mcp {
        Mcp {
            device: SocketAddr::new(host, PORT),
            kind,
            connected: false,
            disconnect_reported: false,
            unanswered_cycles: 0,
            backed_off: false,
            pending: Vec::new(),
        }
    }

    fn send(&self, cx: &mut Cx, instruction: &str) {
        cx.udp_send(SOCKET, self.device, format!("{instruction}\r"));
    }

    fn subscribe(&self, cx: &mut Cx) {
        self.send(cx, SUBSCRIBE);
    }

    fn arm_reply_timer(&self, cx: &mut Cx) {
        match self.pending.iter().map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    fn request(&mut self, cx: &mut Cx, id: CommandId, instruction: String, expect: Expect) {
        self.send(cx, &instruction);
        self.pending.push(Pending {
            id,
            sent: instruction,
            expect,
            deadline: cx.now() + REPLY_WAIT,
        });
        self.arm_reply_timer(cx);
    }

    fn go_offline(&mut self, cx: &mut Cx) {
        self.connected = false;
        if !self.disconnect_reported {
            self.disconnect_reported = true;
            cx.log(
                Level::Warning,
                "no data for 15 s; if the Push echo arrives but no status follows, \
                 enable network control on the device",
            );
            cx.connection(Connection::Disconnected {
                reason: "no data for 15 s".into(),
            });
        }

        self.unanswered_cycles += 1;
        if self.unanswered_cycles <= ATTEMPTS_BEFORE_BACKOFF {
            self.subscribe(cx);
            cx.set_timer(SILENCE, SILENCE_TIMEOUT);
            return;
        }

        cx.cancel_timer(RESUB);
        if !self.backed_off {
            self.backed_off = true;
            cx.log(
                Level::Info,
                format!(
                    "no answer after {ATTEMPTS_BEFORE_BACKOFF} attempts; probing every 30 s \
                     until it comes back"
                ),
            );
            cx.set_timer(SLOW_PROBE, SLOW_PROBE_EVERY);
        }
    }

    /// Heard from the device: every packet proves it is there, not only the first.
    fn heard(&mut self, cx: &mut Cx) {
        self.unanswered_cycles = 0;
        if self.backed_off {
            self.backed_off = false;
            cx.cancel_timer(SLOW_PROBE);
            cx.set_timer(RESUB, RESUBSCRIBE_EVERY);
        }
        if !self.connected {
            self.connected = true;
            cx.connection(Connection::Connected);
        }
    }

    fn complete_error(&mut self, cx: &mut Cx, line: &str) -> bool {
        // "1020: Value out of range [ AfOut 125 ]"
        let Some((code, rest)) = line.split_once(':') else {
            return false;
        };
        if code.len() != 4 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        let instruction = rest
            .split_once('[')
            .and_then(|(_, r)| r.rsplit_once(']'))
            .map(|(inner, _)| inner.trim().to_string());
        let message = rest.split('[').next().unwrap_or("").trim().to_string();

        let index = instruction
            .as_deref()
            .and_then(|i| self.pending.iter().position(|p| p.sent == i));
        match index {
            Some(i) => {
                let p = self.pending.remove(i);
                cx.complete(
                    p.id,
                    Err(CommandError::DeviceError {
                        code: Some(code.into()),
                        message,
                    }),
                );
                self.arm_reply_timer(cx);
            }
            None => cx.log(Level::Warning, format!("device error: {line}")),
        }
        true
    }

    fn resolve(&mut self, cx: &mut Cx, line: &str) {
        let Some(index) = self.pending.iter().position(|p| matches_reply(p, line)) else {
            return;
        };
        let p = self.pending.remove(index);
        let result = match p.expect {
            Expect::Echo(_) => Ok(Outcome::Ack),
            Expect::FirmwareRevision => Ok(Outcome::Value {
                value: json!(line.strip_prefix("FirmwareRevision").unwrap_or("").trim()),
            }),
            Expect::RfConfig => Ok(Outcome::Value {
                value: parse_rf_config(line),
            }),
        };
        cx.complete(p.id, result);
        self.arm_reply_timer(cx);
    }

    fn parse_line(
        &self,
        line: &str,
        channel: &mut Map<String, Value>,
        device: &mut Map<String, Value>,
        tx: &mut TxMute,
    ) {
        let mut parts = line.split_whitespace();
        let Some(keyword) = parts.next() else { return };
        let args: Vec<&str> = parts.collect();
        let int = |i: usize| args.get(i).and_then(|v| v.parse::<i64>().ok());

        match (self.kind, keyword) {
            (_, "Name") => {
                if let Some(rest) = line.trim().strip_prefix("Name") {
                    let name = rest.trim();
                    if !name.is_empty() {
                        channel.insert("name".into(), json!(name));
                    }
                }
            }
            (_, "Frequency") => {
                if let Some(v) = args.first().and_then(|v| v.parse::<f64>().ok()) {
                    // kHz per TI 1254 p.15. RFDeck also accepts MHz below 1000,
                    // a fallback kept because it cannot misread a kHz value.
                    let khz = if v < 1000.0 {
                        (v * 1000.0).round()
                    } else {
                        v.round()
                    };
                    channel.insert("frequency_khz".into(), json!(khz as i64));
                }
                if let (Some(bank), Some(ch)) = (int(1), int(2)) {
                    channel.insert("bank".into(), json!(bank));
                    channel.insert("bank_channel".into(), json!(ch));
                }
            }
            (_, "Mute") => {
                if let Some(v) = int(0) {
                    channel.insert("mute".into(), json!(v == 1));
                }
            }
            (_, "Msg") => {
                let warnings: Vec<&str> = args.iter().copied().filter(|w| *w != "OK").collect();
                tx.msg = Some(warnings.contains(&"TX_Mute"));
                channel.insert("warnings".into(), json!(warnings));
            }
            (_, "Config") => {
                if let Some(v) = int(0) {
                    channel.insert("config_index".into(), json!(v));
                }
            }
            (_, "FirmwareRevision") => {
                if let Some(v) = args.first() {
                    device.insert("firmware".into(), json!(v));
                }
            }

            (Kind::Receiver, "Squelch") => insert_int(channel, "squelch_db", int(0)),
            (Kind::Receiver, "AfOut") => insert_int(channel, "af_out_db", int(0)),
            (Kind::Receiver, "Equalizer") => {
                if let Some(name) = int(0).and_then(|i| EM_EQUALIZER.get(i as usize)) {
                    channel.insert("equalizer".into(), json!(name));
                }
            }
            (Kind::Receiver, "RF1" | "RF2") => {
                if let (Some(min), Some(max), Some(active)) = (int(0), int(1), int(2)) {
                    let key = if keyword == "RF1" {
                        "antenna_a"
                    } else {
                        "antenna_b"
                    };
                    rf(channel).insert(
                        key.into(),
                        json!({"min": min, "max": max, "active": active == 1}),
                    );
                }
            }
            (Kind::Receiver, "RF") => {
                let rf = rf(channel);
                if let Some(level) = int(0) {
                    rf.insert("level".into(), json!(level));
                }
                if let Some(antenna) = int(1) {
                    rf.insert("active_antenna".into(), json!(antenna));
                }
                if let Some(pilot) = int(2) {
                    rf.insert("pilot".into(), json!(pilot == 1));
                }
            }
            (Kind::Receiver, "States") => {
                if let Some(flags) = int(0) {
                    channel.insert(
                        "mute_flags".into(),
                        json!({
                            "any": flags & 1 != 0,
                            "tx": flags & 2 != 0,
                            "rf": flags & 4 != 0,
                            "rx": flags & 8 != 0,
                        }),
                    );
                    tx.states = Some(flags & 2 != 0);
                }
                insert_int(channel, "pilot_flag", int(1));
            }
            (Kind::Receiver, "AF") => {
                let af = object(channel, "af");
                if let Some(peak) = int(0) {
                    af.insert("peak".into(), json!(peak));
                }
                if let Some(hold) = int(1) {
                    af.insert("peak_hold".into(), json!(hold));
                }
            }
            (Kind::Receiver, "Bat") => match args.first() {
                // "?": no battery telegram. Absent, never zero: no pack is not
                // a flat pack.
                Some(&"?") => {
                    channel.insert("battery_percent".into(), Value::Null);
                }
                Some(v) => {
                    if let Ok(pct) = v.parse::<i64>() {
                        channel.insert("battery_percent".into(), json!(pct));
                    }
                }
                None => {}
            },

            (Kind::Transmitter, "Af" | "AF") => {
                let levels: Vec<i64> = args.iter().filter_map(|v| v.parse().ok()).collect();
                object(channel, "af").insert("levels".into(), json!(levels));
            }
            (Kind::Transmitter, "States") => {
                if let Some(v) = int(0) {
                    channel.insert("rf_off".into(), json!(v == 1));
                }
            }
            (Kind::Transmitter, "Sensitivity") => insert_int(channel, "sensitivity_db", int(0)),
            (Kind::Transmitter, "Mode") => {
                if let Some(v) = int(0) {
                    channel.insert("mode".into(), json!(if v == 1 { "stereo" } else { "mono" }));
                }
            }
            _ => {}
        }
    }
}

/// TX mute as seen in one packet: from States bit 1, and from TX_Mute anywhere
/// in Msg. Either reporting it is enough.
#[derive(Default)]
struct TxMute {
    states: Option<bool>,
    msg: Option<bool>,
}

fn matches_reply(p: &Pending, line: &str) -> bool {
    match &p.expect {
        // A Frequency echo carries bank and channel after the value:
        // "Frequency 822000" is answered by "Frequency 822000 0 0".
        Expect::Echo(sent) => line == sent || line.starts_with(&format!("{sent} ")),
        Expect::FirmwareRevision => line.starts_with("FirmwareRevision "),
        Expect::RfConfig => line.starts_with("RfConfig "),
    }
}

/// "RfConfig 779125 787875 125 797125 805875 125" -> blocks of min, max, step.
fn parse_rf_config(line: &str) -> Value {
    let numbers: Vec<i64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    let blocks: Vec<Value> = numbers
        .chunks_exact(3)
        .map(|c| json!({"min_khz": c[0], "max_khz": c[1], "step_khz": c[2]}))
        .collect();
    json!(blocks)
}

fn insert_int(map: &mut Map<String, Value>, key: &str, value: Option<i64>) {
    if let Some(v) = value {
        map.insert(key.into(), json!(v));
    }
}

fn object<'a>(map: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    map.entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("object entry")
}

fn rf(channel: &mut Map<String, Value>) -> &mut Map<String, Value> {
    object(channel, "rf")
}

fn bool_param(params: &Params, name: &str) -> bool {
    params.get(name).and_then(Value::as_bool).unwrap_or(false)
}

fn int_param(params: &Params, name: &str) -> i64 {
    params.get(name).and_then(Value::as_i64).unwrap_or(0)
}

fn str_param<'a>(params: &'a Params, name: &str) -> &'a str {
    params.get(name).and_then(Value::as_str).unwrap_or("")
}

impl Module for Mcp {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.udp_open(SOCKET, Bind::Shared(PORT));
        self.subscribe(cx);
        self.send(cx, "Name");
        self.send(cx, "Frequency");
        cx.set_timer(RESUB, RESUBSCRIBE_EVERY);
        cx.set_timer(SILENCE, SILENCE_TIMEOUT);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        let instruction = match name {
            "mute" | "rf_mute" => format!("Mute {}", u8::from(bool_param(params, "muted"))),
            "set_frequency" => format!("Frequency {}", int_param(params, "frequency_khz")),
            "set_name" => format!("Name {}", str_param(params, "name")),
            "set_squelch" => format!("Squelch {}", int_param(params, "squelch_db")),
            "set_af_out" => format!("AfOut {}", int_param(params, "level_db")),
            "set_equalizer" => {
                let preset = str_param(params, "preset");
                let index = EM_EQUALIZER.iter().position(|p| *p == preset).unwrap_or(0);
                format!("Equalizer {index}")
            }
            "set_sensitivity" => format!("Sensitivity {}", int_param(params, "sensitivity_db")),
            "set_mode" => format!("Mode {}", u8::from(str_param(params, "mode") == "stereo")),
            "get_firmware" => {
                return self.request(cx, id, "FirmwareRevision".into(), Expect::FirmwareRevision);
            }
            "get_rf_config" => {
                return self.request(cx, id, "RfConfig".into(), Expect::RfConfig);
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
        let expect = Expect::Echo(instruction.clone());
        self.request(cx, id, instruction, expect);
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, _from: SocketAddr, data: &[u8]) {
        cx.set_timer(SILENCE, SILENCE_TIMEOUT);
        self.disconnect_reported = false;
        cx.alive();

        let text = String::from_utf8_lossy(data);
        let lines: Vec<&str> = text
            .split('\r')
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();

        // An error reply means the device is reachable but rejected something.
        // It carries no telemetry, so it does not count as connecting.
        let mut telemetry = false;
        let mut channel = Map::new();
        let mut device = Map::new();
        let mut tx = TxMute::default();
        for line in &lines {
            if self.complete_error(cx, line) {
                continue;
            }
            telemetry = true;
            self.resolve(cx, line);
            self.parse_line(line, &mut channel, &mut device, &mut tx);
        }
        if !telemetry {
            return;
        }
        self.heard(cx);

        if tx.states.is_some() || tx.msg.is_some() {
            let muted = tx.states == Some(true) || tx.msg == Some(true);
            channel.insert("tx_mute".into(), json!(muted));
        }
        let mut patch = Map::new();
        if !channel.is_empty() {
            patch.insert("channels".into(), json!({ "1": channel }));
        }
        if !device.is_empty() {
            patch.insert("device".into(), Value::Object(device));
        }
        if !patch.is_empty() {
            cx.state(Value::Object(patch));
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RESUB => {
                self.subscribe(cx);
                cx.set_timer(RESUB, RESUBSCRIBE_EVERY);
            }
            SILENCE => self.go_offline(cx),
            SLOW_PROBE => {
                self.subscribe(cx);
                self.send(cx, "Name");
                cx.set_timer(SLOW_PROBE, SLOW_PROBE_EVERY);
            }
            REPLY => {
                let now = cx.now();
                let (expired, waiting): (Vec<_>, Vec<_>) =
                    self.pending.drain(..).partition(|p| p.deadline <= now);
                self.pending = waiting;
                for p in expired {
                    let result = match p.expect {
                        Expect::Echo(_) => Ok(Outcome::Unverified),
                        _ => Err(CommandError::Timeout),
                    };
                    cx.complete(p.id, result);
                }
                self.arm_reply_timer(cx);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::Ipv4Addr;

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 40));

    fn receiver() -> Mcp {
        Mcp::for_host(HOST, Kind::Receiver)
    }

    fn from() -> SocketAddr {
        SocketAddr::new(HOST, PORT)
    }

    fn sent(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } => Some(String::from_utf8_lossy(data).into_owned()),
                _ => None,
            })
            .collect()
    }

    fn state(actions: &[Action]) -> Value {
        let mut merged = Value::Object(Map::new());
        for a in actions {
            if let Action::State(p) = a {
                crate::session::merge_patch(&mut merged, p);
            }
        }
        merged
    }

    fn completed(actions: &[Action]) -> Vec<(CommandId, crate::module::CommandResult)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Complete { id, result } => Some((*id, result.clone())),
                _ => None,
            })
            .collect()
    }

    fn feed(m: &mut Mcp, now: Millis, packet: &str) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.datagram(&mut cx, SOCKET, from(), packet.as_bytes());
        cx.take()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn start_subscribes_on_the_shared_port() {
        let mut m = receiver();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let actions = cx.take();
        assert!(actions.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Shared(53212)
        }));
        assert_eq!(sent(&actions), ["Push 60 500 3\r", "Name\r", "Frequency\r"]);
    }

    #[test]
    fn parses_the_documented_em_cycle() {
        // TI 1254 p.9, "Response of cyclic attributes".
        let mut m = receiver();
        let actions = feed(
            &mut m,
            0,
            "RF1 25 65 1\rRF2 28 78 0\rStates 3 2\rRF 50 1 1\rAF 40 65 3\rBat 70\r\
             Msg Low_RF_Signal Low_Battery\rConfig 234\r",
        );
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        let ch = &state(&actions)["channels"]["1"];
        assert_eq!(
            ch["rf"]["antenna_a"],
            json!({"min": 25, "max": 65, "active": true})
        );
        assert_eq!(
            ch["rf"]["antenna_b"],
            json!({"min": 28, "max": 78, "active": false})
        );
        assert_eq!(ch["rf"]["level"], 50);
        assert_eq!(ch["rf"]["pilot"], true);
        assert_eq!(ch["af"]["peak"], 40);
        assert_eq!(ch["battery_percent"], 70);
        assert_eq!(ch["warnings"], json!(["Low_RF_Signal", "Low_Battery"]));
        assert_eq!(ch["config_index"], 234);
        assert_eq!(
            ch["mute_flags"],
            json!({"any": true, "tx": true, "rf": false, "rx": false})
        );
        assert_eq!(ch["tx_mute"], true);
    }

    #[test]
    fn receiver_mute_is_not_tx_mute() {
        // Bit 3 alone is Rx-Mute: an operator's mute, not the performer's.
        let mut m = receiver();
        let ch = &state(&feed(&mut m, 0, "States 8 1\rMsg RX_Mute\r"))["channels"]["1"];
        assert_eq!(ch["tx_mute"], false);
        assert_eq!(ch["mute_flags"]["rx"], true);
    }

    #[test]
    fn tx_mute_is_found_anywhere_in_msg() {
        let mut m = receiver();
        let ch = &state(&feed(&mut m, 0, "Msg Low_Battery TX_Mute\r"))["channels"]["1"];
        assert_eq!(ch["tx_mute"], true);
        let ch = &state(&feed(&mut m, 10, "Msg OK\r"))["channels"]["1"];
        assert_eq!(ch["warnings"], json!([]));
        assert_eq!(ch["tx_mute"], false);
    }

    #[test]
    fn missing_battery_telegram_removes_the_value() {
        let mut m = receiver();
        let actions = feed(&mut m, 0, "Bat ?\r");
        let patch = actions.iter().find_map(|a| match a {
            Action::State(p) => Some(p.clone()),
            _ => None,
        });
        assert_eq!(
            patch.unwrap()["channels"]["1"]["battery_percent"],
            Value::Null
        );
    }

    #[test]
    fn set_command_acks_on_echo() {
        let mut m = receiver();
        let mut cx = Cx::new(0);
        m.command(
            &mut cx,
            7,
            "set_frequency",
            &params(json!({"frequency_khz": 822000})),
        );
        assert_eq!(sent(&cx.take()), ["Frequency 822000\r"]);

        let actions = feed(&mut m, 100, "Frequency 822000 2 10\r");
        assert_eq!(completed(&actions), [(7, Ok(Outcome::Ack))]);
        let ch = &state(&actions)["channels"]["1"];
        assert_eq!(ch["frequency_khz"], 822000);
        assert_eq!(ch["bank"], 2);
    }

    #[test]
    fn set_command_reports_device_errors() {
        let mut m = receiver();
        let mut cx = Cx::new(0);
        m.command(&mut cx, 3, "set_af_out", &params(json!({"level_db": 5})));
        let actions = feed(&mut m, 50, "1020: Value out of range [ AfOut 5 ] \r");
        assert_eq!(
            completed(&actions),
            [(
                3,
                Err(CommandError::DeviceError {
                    code: Some("1020".into()),
                    message: "Value out of range".into(),
                })
            )]
        );
        assert!(!actions.contains(&Action::Connection(Connection::Connected)));
    }

    #[test]
    fn unanswered_set_is_unverified_and_unanswered_get_times_out() {
        let mut m = receiver();
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "mute", &params(json!({"muted": true})));
        m.command(&mut cx, 2, "get_firmware", &params(json!({})));
        let mut cx = Cx::new(REPLY_WAIT);
        m.timer(&mut cx, REPLY);
        assert_eq!(
            completed(&cx.take()),
            [
                (1, Ok(Outcome::Unverified)),
                (2, Err(CommandError::Timeout))
            ]
        );
    }

    #[test]
    fn gets_return_values() {
        let mut m = receiver();
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "get_rf_config", &params(json!({})));
        let actions = feed(&mut m, 10, "RfConfig 566000 608000 25\r");
        assert_eq!(
            completed(&actions),
            [(
                1,
                Ok(Outcome::Value {
                    value: json!([{"min_khz": 566000, "max_khz": 608000, "step_khz": 25}]),
                })
            )]
        );
    }

    #[test]
    fn silence_disconnects_then_backs_off_then_recovers() {
        let mut m = receiver();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        feed(&mut m, 100, "Msg OK\r");

        // First silent cycle: disconnected, resubscribed.
        let mut cx = Cx::new(15_100);
        m.timer(&mut cx, SILENCE);
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert_eq!(sent(&a), ["Push 60 500 3\r"]);

        // Cycles two to four: resubscribe, no repeated disconnect.
        for n in 2..=4 {
            let mut cx = Cx::new(15_100 * n);
            m.timer(&mut cx, SILENCE);
            let a = cx.take();
            assert!(!a.iter().any(|x| matches!(x, Action::Connection(_))));
            assert_eq!(sent(&a), ["Push 60 500 3\r"]);
        }

        // Fifth: stop renewing, probe slowly.
        let mut cx = Cx::new(80_000);
        m.timer(&mut cx, SILENCE);
        let a = cx.take();
        assert!(a.contains(&Action::CancelTimer { key: RESUB }));
        assert!(a.contains(&Action::SetTimer {
            key: SLOW_PROBE,
            after: SLOW_PROBE_EVERY
        }));
        assert!(sent(&a).is_empty());

        // Any packet restores normal renewal.
        let a = feed(&mut m, 90_000, "Msg OK\r");
        assert!(a.contains(&Action::CancelTimer { key: SLOW_PROBE }));
        assert!(a.contains(&Action::SetTimer {
            key: RESUB,
            after: RESUBSCRIBE_EVERY
        }));
        assert!(a.contains(&Action::Connection(Connection::Connected)));
    }

    #[test]
    fn transmitter_mute_and_levels() {
        let mut m = Mcp::for_host(HOST, Kind::Transmitter);
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "rf_mute", &params(json!({"muted": true})));
        assert_eq!(sent(&cx.take()), ["Mute 1\r"]);
        // TI 1254 p.9 example for an SR.
        let ch = &state(&feed(
            &mut m,
            10,
            "Af 15 25 40 38 5\rStates 0 2\rMsg OK\rConfig 555\r",
        ))["channels"]["1"];
        assert_eq!(ch["af"]["levels"], json!([15, 25, 40, 38, 5]));
        assert_eq!(ch["rf_off"], false);
    }
}
