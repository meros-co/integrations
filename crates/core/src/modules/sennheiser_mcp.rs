//! Sennheiser evolution wireless G3/G4 over the Media Control Protocol.
//!
//! Wire format from Sennheiser TI 1254 v1.0. Lifecycle from a consumer's G3/G4
//! client, which was tested against real racks: subscribe with Push, renew
//! every 8 s, declare the device gone after 15 s of silence, and back off an
//! address that never answers. Readings that differ from a consumer's client were reviewed
//! with it in that consumer's integration review.
//!
//! Push with mode 3 also makes the device send every configuration attribute
//! at once and again whenever one changes, so the configuration needs no
//! polling; firmware and RF configuration are read once on connecting.
//!
//! Opened for commands only, it sends no Push and reads nothing on connecting.
//! Without Push a receiver sends nothing unasked, so it is asked its name at
//! the renewal cadence instead, as the liveness check.

use std::net::{IpAddr, SocketAddr};

use crate::{
    json,
    value::{Map, Value},
};

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
/// The liveness check of a device opened for commands only: one short reply,
/// and no subscription.
const LIVENESS_QUERY: &str = "Name";

/// Renew well inside the 60 s subscription: some firmware drops it early while
/// in RF-Mute (a consumer, observed on hardware).
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
/// The SR equalizer's five bands, in wire order after the on/off flag.
const IEM_BANDS: [&str; 5] = ["low", "low_mid", "mid", "mid_high", "high"];

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
    /// A request answered with the new value rather than an echo (relative
    /// changes, the SR equalizer): any reply with this keyword settles it.
    Keyword(&'static str),
    /// A get request: the reply starts with this keyword.
    FirmwareRevision,
    RfConfig,
    /// BankList for this bank: the reply names no bank, so it is tied to the
    /// request.
    BankList(i64),
    /// set_bank_channel, first step: the bank's list, to find the channel's
    /// frequency.
    BankForTune {
        bank: i64,
        channel: i64,
    },
    /// set_bank_channel, second step: Frequency with bank and channel, which
    /// the device answers with the bank and channel it took (0 0 when the
    /// frequency does not belong to them).
    Tuned {
        bank: i64,
        channel: i64,
    },
}

#[derive(Debug)]
struct Pending {
    id: CommandId,
    sent: String,
    expect: Expect,
    sent_at: Millis,
    deadline: Millis,
}

pub(crate) struct Mcp {
    device: SocketAddr,
    kind: Kind,
    /// Subscribe and read on connecting; false for commands only.
    monitor: bool,
    /// When the last unanswered Push (or, unmonitored, liveness query) went.
    keep_alive_sent: Option<Millis>,
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
        let mut mcp = Mcp::for_host(ctx.host, kind);
        mcp.monitor = ctx.monitor;
        mcp
    }

    fn for_host(host: IpAddr, kind: Kind) -> Mcp {
        Mcp {
            device: SocketAddr::new(host, PORT),
            kind,
            monitor: true,
            keep_alive_sent: None,
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

    /// What keeps the device talking: Push when monitoring, otherwise the
    /// liveness query, since without Push a receiver says nothing unasked.
    fn keep_alive(&mut self, cx: &mut Cx) {
        let instruction = if self.monitor {
            SUBSCRIBE
        } else {
            LIVENESS_QUERY
        };
        self.send(cx, instruction);
        self.keep_alive_sent = Some(cx.now());
    }

    /// The device's answer to the keep-alive: Push is echoed, the liveness
    /// query answered with the name.
    fn answers_keep_alive(&self, line: &str) -> bool {
        let keyword = line.split_whitespace().next();
        if self.monitor {
            keyword == Some("Push")
        } else {
            keyword == Some(LIVENESS_QUERY)
        }
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
            sent_at: cx.now(),
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
            self.keep_alive(cx);
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
        self.disconnect_reported = false;
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
                cx.round_trip(cx.now().saturating_sub(p.sent_at));
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
        cx.round_trip(cx.now().saturating_sub(p.sent_at));
        let result = match p.expect {
            Expect::Echo(_) | Expect::Keyword(_) => Ok(Outcome::Ack),
            Expect::FirmwareRevision => Ok(Outcome::Value {
                value: json!(line.strip_prefix("FirmwareRevision").unwrap_or("").trim()),
            }),
            Expect::RfConfig => Ok(Outcome::Value {
                value: parse_rf_config(line),
            }),
            Expect::BankList(bank) => {
                let list = parse_bank_list(line);
                cx.state(json!({"channels": {"1": {"banks": {bank.to_string(): list.clone()}}}}));
                Ok(Outcome::Value { value: json!(list) })
            }
            Expect::BankForTune { bank, channel } => {
                let list = parse_bank_list(line);
                cx.state(json!({"channels": {"1": {"banks": {bank.to_string(): list.clone()}}}}));
                let khz = usize::try_from(channel - 1)
                    .ok()
                    .and_then(|i| list.get(i))
                    .and_then(Value::as_i64)
                    .filter(|&f| f > 0);
                match khz {
                    Some(khz) => {
                        // The same command goes on to its second step.
                        let instruction = format!("Frequency {khz} {bank} {channel}");
                        self.send(cx, &instruction);
                        self.pending.push(Pending {
                            id: p.id,
                            sent: instruction,
                            expect: Expect::Tuned { bank, channel },
                            sent_at: cx.now(),
                            deadline: cx.now() + REPLY_WAIT,
                        });
                        self.arm_reply_timer(cx);
                        return;
                    }
                    None => Err(CommandError::DeviceError {
                        code: None,
                        message: format!("bank {bank} has no frequency on channel {channel}"),
                    }),
                }
            }
            Expect::Tuned { bank, channel } => {
                let mut fields = line
                    .split_whitespace()
                    .skip(2)
                    .map(|v| v.parse::<i64>().ok());
                let took = (fields.next().flatten(), fields.next().flatten());
                if took == (Some(bank), Some(channel)) {
                    Ok(Outcome::Ack)
                } else {
                    Err(CommandError::DeviceError {
                        code: None,
                        message: format!(
                            "the device tuned the frequency but did not take bank {bank} \
                             channel {channel}"
                        ),
                    })
                }
            }
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
                    // kHz per TI 1254 p.15. That consumer also accepts MHz below 1000,
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
                    channel.insert("mute_flags".into(), mute_bits(flags));
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
                // The current mute state, as opposed to States' flags since
                // the last cycle.
                if let Some(flags) = int(2) {
                    channel.insert("mute_now".into(), mute_bits(flags));
                }
            }
            (_, "RfConfig") => {
                device.insert("rf_config".into(), parse_rf_config(line));
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
                // 0 on all cycle, 1 off all cycle, 2 changed during it.
                insert_int(channel, "rf_mute_flag", int(1));
            }
            (Kind::Transmitter, "Equalizer") => {
                if let Some(on) = int(0) {
                    let mut eq = Map::new();
                    eq.insert("enabled".into(), json!(on == 1));
                    for (i, band) in IEM_BANDS.iter().enumerate() {
                        insert_int(&mut eq, band, int(i + 1));
                    }
                    channel.insert("iem_equalizer".into(), Value::Object(eq));
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
        Expect::Keyword(keyword) => line.split_whitespace().next() == Some(*keyword),
        Expect::FirmwareRevision => line.starts_with("FirmwareRevision "),
        Expect::RfConfig => line.starts_with("RfConfig "),
        Expect::BankList(_) | Expect::BankForTune { .. } => {
            line.split_whitespace().next() == Some("BankList")
        }
        Expect::Tuned { .. } => line.starts_with("Frequency "),
    }
}

/// "BankList 0 822000 0 819375" -> each channel's frequency in kHz, from
/// channel 1; 0 is an unused channel. Trailing unused channels are left out by
/// the device.
fn parse_bank_list(line: &str) -> Vec<Value> {
    line.split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse::<i64>().ok())
        .map(|f| json!(f))
        .collect()
}

/// The four mute bits of the EM's States and AF attributes.
fn mute_bits(flags: i64) -> Value {
    json!({
        "any": flags & 1 != 0,
        "tx": flags & 2 != 0,
        "rf": flags & 4 != 0,
        "rx": flags & 8 != 0,
    })
}

/// "RfConfig 779125 787875 125 797125 805875 125" -> blocks of min, max, step.
fn parse_rf_config(line: &str) -> Value {
    let numbers: Vec<i64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    let blocks: Vec<Value> = numbers
        .as_chunks::<3>()
        .0
        .iter()
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
        self.keep_alive(cx);
        if self.monitor {
            for read in ["Name", "Frequency", "FirmwareRevision", "RfConfig"] {
                self.send(cx, read);
            }
        }
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
            "get_bank_list" => {
                let bank = int_param(params, "bank");
                return self.request(cx, id, format!("BankList {bank}"), Expect::BankList(bank));
            }
            "set_bank_channel" => {
                // Frequency must name the channel's own frequency for the bank
                // and channel to be taken, so the bank is listed first.
                let bank = int_param(params, "bank");
                let channel = int_param(params, "bank_channel");
                return self.request(
                    cx,
                    id,
                    format!("BankList {bank}"),
                    Expect::BankForTune { bank, channel },
                );
            }
            "step_frequency" | "step_squelch" | "step_af_out" | "step_sensitivity" => {
                let keyword = match name {
                    "step_frequency" => "Frequency",
                    "step_squelch" => "Squelch",
                    "step_af_out" => "AfOut",
                    _ => "Sensitivity",
                };
                let steps = int_param(params, "steps");
                if steps == 0 {
                    cx.complete(
                        id,
                        Err(CommandError::InvalidParams {
                            message: "steps must not be 0".into(),
                        }),
                    );
                    return;
                }
                return self.request(
                    cx,
                    id,
                    format!("{keyword} #{steps}"),
                    Expect::Keyword(keyword),
                );
            }
            "set_iem_equalizer" => {
                let on = u8::from(bool_param(params, "enabled"));
                let bands: Vec<Option<i64>> = IEM_BANDS
                    .iter()
                    .map(|b| params.get(*b).and_then(Value::as_i64))
                    .collect();
                let instruction = if bands.iter().all(Option::is_none) {
                    format!("Equalizer {on}")
                } else if let Some(values) = bands.iter().copied().collect::<Option<Vec<i64>>>() {
                    let values: Vec<String> = values.iter().map(i64::to_string).collect();
                    format!("Equalizer {on} {}", values.join(" "))
                } else {
                    cx.complete(
                        id,
                        Err(CommandError::InvalidParams {
                            message: "give all five bands, or none to switch the equalizer only"
                                .into(),
                        }),
                    );
                    return;
                };
                // Answered with the whole equalizer, not an echo.
                return self.request(cx, id, instruction, Expect::Keyword("Equalizer"));
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
        cx.alive();

        let text = String::from_utf8_lossy(data);
        let lines: Vec<&str> = text
            .split('\r')
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();

        // An error reply means the device is reachable but rejected something.
        // It carries no telemetry, so it does not count as connecting.
        let mut replies = Vec::new();
        let mut channel = Map::new();
        let mut device = Map::new();
        let mut tx = TxMute::default();
        for line in &lines {
            if self.complete_error(cx, line) {
                continue;
            }
            if self.answers_keep_alive(line) {
                if let Some(sent) = self.keep_alive_sent.take() {
                    cx.round_trip(cx.now().saturating_sub(sent));
                }
            }
            replies.push(*line);
            self.parse_line(line, &mut channel, &mut device, &mut tx);
        }
        if replies.is_empty() {
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
        // After the state, so a caller reading the snapshot after its command
        // completes sees what the reply carried.
        for line in replies {
            self.resolve(cx, line);
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RESUB => {
                self.keep_alive(cx);
                cx.set_timer(RESUB, RESUBSCRIBE_EVERY);
            }
            SILENCE => self.go_offline(cx),
            SLOW_PROBE => {
                self.keep_alive(cx);
                if self.monitor {
                    self.send(cx, "Name");
                }
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
        assert_eq!(
            sent(&actions),
            [
                "Push 60 500 3\r",
                "Name\r",
                "Frequency\r",
                "FirmwareRevision\r",
                "RfConfig\r"
            ]
        );
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
        let at = |f: fn(&Action) -> bool| actions.iter().position(f).unwrap();
        assert!(
            at(|x| matches!(x, Action::State(_))) < at(|x| matches!(x, Action::Complete { .. })),
            "the echo's state lands before the command completes"
        );
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

    fn disconnects(actions: &[Action]) -> usize {
        actions
            .iter()
            .filter(|x| matches!(x, Action::Connection(Connection::Disconnected { .. })))
            .count()
    }

    /// Every G3/G4 fault a consumer found on a rig lived in a transition, so the
    /// whole cycle is walked twice as one sequence (consumer review item N.4):
    /// connect, loss, retries, backoff, recovery, and loss again. Each of these
    /// fails if its fix is removed: disconnect only reported before the first
    /// connection; recovery that runs once per lifetime; a retry counter that
    /// stops advancing because the silence timer is not re-armed.
    #[test]
    fn connect_loss_backoff_recovery_and_loss_again() {
        let mut m = receiver();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let mut now = 100;
        let a = feed(&mut m, now, "Msg OK\r");
        assert!(a.contains(&Action::Connection(Connection::Connected)));

        for round in 1..=2 {
            // Loss: reported once, then the subscription is retried with the
            // silence timer re-armed each time.
            let mut reported = 0;
            for attempt in 1..=ATTEMPTS_BEFORE_BACKOFF {
                now += SILENCE_TIMEOUT;
                let mut cx = Cx::new(now);
                m.timer(&mut cx, SILENCE);
                let a = cx.take();
                reported += disconnects(&a);
                assert_eq!(
                    sent(&a),
                    ["Push 60 500 3\r"],
                    "round {round}, attempt {attempt}"
                );
                assert!(
                    a.contains(&Action::SetTimer {
                        key: SILENCE,
                        after: SILENCE_TIMEOUT
                    }),
                    "round {round}, attempt {attempt}: silence timer re-armed"
                );

                // A device error while offline is not telemetry: it neither
                // recovers the device nor reports the loss a second time.
                let a = feed(&mut m, now + 1, "1020: Value out of range [ AfOut 125 ]\r");
                assert!(!a.iter().any(|x| matches!(x, Action::Connection(_))));
            }
            assert_eq!(reported, 1, "round {round}: one disconnect per loss");

            // Backoff: renewal stops, slow probing starts.
            now += SILENCE_TIMEOUT;
            let mut cx = Cx::new(now);
            m.timer(&mut cx, SILENCE);
            let a = cx.take();
            assert_eq!(disconnects(&a), 0);
            assert!(a.contains(&Action::CancelTimer { key: RESUB }));
            assert!(
                a.contains(&Action::SetTimer {
                    key: SLOW_PROBE,
                    after: SLOW_PROBE_EVERY
                }),
                "round {round}: backed off"
            );

            now += SLOW_PROBE_EVERY;
            let mut cx = Cx::new(now);
            m.timer(&mut cx, SLOW_PROBE);
            assert_eq!(sent(&cx.take()), ["Push 60 500 3\r", "Name\r"]);

            // Recovery on the next packet, in every round.
            now += 100;
            let a = feed(&mut m, now, "Name Lead\r");
            assert!(
                a.contains(&Action::Connection(Connection::Connected)),
                "round {round}: recovered"
            );
            assert!(a.contains(&Action::SetTimer {
                key: SILENCE,
                after: SILENCE_TIMEOUT
            }));
            assert!(a.contains(&Action::CancelTimer { key: SLOW_PROBE }));
            assert!(a.contains(&Action::SetTimer {
                key: RESUB,
                after: RESUBSCRIBE_EVERY
            }));
            now += 100;
        }
    }

    #[test]
    fn opened_for_commands_only_it_asks_only_for_its_name() {
        let mut m = receiver();
        m.monitor = false;
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        // No Push, no Frequency read: the name query is the liveness check.
        assert_eq!(sent(&cx.take()), ["Name\r"]);

        let a = feed(&mut m, 30, "Name Vocal 1\r");
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(30)));
        assert_eq!(state(&a)["channels"]["1"]["name"], "Vocal 1");

        // Renewal, silence and slow probing all ask the name, never Push.
        let mut cx = Cx::new(RESUBSCRIBE_EVERY);
        m.timer(&mut cx, RESUB);
        assert_eq!(sent(&cx.take()), ["Name\r"]);
        let mut cx = Cx::new(20_000);
        m.timer(&mut cx, SILENCE);
        assert_eq!(sent(&cx.take()), ["Name\r"]);
        let mut cx = Cx::new(60_000);
        m.timer(&mut cx, SLOW_PROBE);
        assert_eq!(sent(&cx.take()), ["Name\r"]);

        // Commands work as ever, and what the device sends unasked is kept.
        let mut cx = Cx::new(61_000);
        m.command(&mut cx, 1, "mute", &params(json!({"muted": true})));
        assert_eq!(sent(&cx.take()), ["Mute 1\r"]);
        let a = feed(&mut m, 61_040, "Mute 1\r");
        assert_eq!(completed(&a), [(1, Ok(Outcome::Ack))]);
        assert_eq!(state(&a)["channels"]["1"]["mute"], true);
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let mut m = receiver();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        // The Push echo answers the subscription.
        let a = feed(&mut m, 25, "Push 60 500 3\r");
        assert!(a.contains(&Action::RoundTrip(25)));
        // Cyclic attributes are pushed, not answers: nothing is measured.
        let a = feed(&mut m, 500, "Msg OK\r");
        assert!(!a.iter().any(|x| matches!(x, Action::RoundTrip(_))));

        let mut cx = Cx::new(1_000);
        m.command(&mut cx, 2, "set_af_out", &params(json!({"level_db": 6})));
        let a = feed(&mut m, 1_012, "AfOut 6\r");
        assert!(a.contains(&Action::RoundTrip(12)));
        let mut cx = Cx::new(2_000);
        m.command(&mut cx, 3, "set_af_out", &params(json!({"level_db": 5})));
        let a = feed(&mut m, 2_009, "1020: Value out of range [ AfOut 5 ]\r");
        assert!(a.contains(&Action::RoundTrip(9)));
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

    fn run(m: &mut Mcp, now: Millis, id: CommandId, name: &str, p: Value) -> Vec<String> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, &params(p));
        sent(&cx.take())
    }

    #[test]
    fn bank_lists_are_returned_and_kept() {
        let mut m = receiver();
        assert_eq!(
            run(&mut m, 0, 1, "get_bank_list", json!({"bank": 21})),
            ["BankList 21\r"]
        );
        // TI 1254 p.14: unused channels are 0, trailing ones left out.
        let a = feed(&mut m, 10, "BankList 0 822000 0 819375\r");
        assert_eq!(
            completed(&a),
            [(
                1,
                Ok(Outcome::Value {
                    value: json!([0, 822000, 0, 819375])
                })
            )]
        );
        assert_eq!(
            state(&a)["channels"]["1"]["banks"]["21"],
            json!([0, 822000, 0, 819375])
        );
    }

    #[test]
    fn bank_and_channel_are_tuned_through_the_bank_list() {
        let mut m = receiver();
        assert_eq!(
            run(
                &mut m,
                0,
                1,
                "set_bank_channel",
                json!({"bank": 2, "bank_channel": 3})
            ),
            ["BankList 2\r"]
        );
        let mut cx = Cx::new(10);
        m.datagram(&mut cx, SOCKET, from(), b"BankList 820000 821000 822000\r");
        let a = cx.take();
        // The channel's own frequency, with its bank and channel.
        assert_eq!(sent(&a), ["Frequency 822000 2 3\r"]);
        assert!(completed(&a).is_empty());
        let a = feed(&mut m, 20, "Frequency 822000 2 3\r");
        assert_eq!(completed(&a), [(1, Ok(Outcome::Ack))]);
        assert_eq!(state(&a)["channels"]["1"]["bank"], 2);

        // An unused channel is refused without tuning.
        run(
            &mut m,
            30,
            2,
            "set_bank_channel",
            json!({"bank": 21, "bank_channel": 1}),
        );
        let a = feed(&mut m, 40, "BankList 0 822000\r");
        assert!(sent(&a).is_empty());
        assert!(matches!(
            &completed(&a)[..],
            [(2, Err(CommandError::DeviceError { .. }))]
        ));

        // Taken into U1.0 instead of the bank: the command fails.
        run(
            &mut m,
            50,
            3,
            "set_bank_channel",
            json!({"bank": 2, "bank_channel": 1}),
        );
        feed(&mut m, 60, "BankList 820000\r");
        let a = feed(&mut m, 70, "Frequency 820000 0 0\r");
        assert!(matches!(
            &completed(&a)[..],
            [(3, Err(CommandError::DeviceError { .. }))]
        ));
    }

    #[test]
    fn relative_steps_are_settled_by_the_new_value() {
        let mut m = receiver();
        assert_eq!(
            run(&mut m, 0, 1, "step_squelch", json!({"steps": -2})),
            ["Squelch #-2\r"]
        );
        let a = feed(&mut m, 10, "Squelch 5\r");
        assert_eq!(completed(&a), [(1, Ok(Outcome::Ack))]);
        assert_eq!(state(&a)["channels"]["1"]["squelch_db"], 5);
        assert_eq!(
            run(&mut m, 20, 2, "step_frequency", json!({"steps": 1})),
            ["Frequency #1\r"]
        );
        let a = feed(&mut m, 30, "Frequency 821025 0 0\r");
        assert_eq!(completed(&a), [(2, Ok(Outcome::Ack))]);
        assert!(run(&mut m, 40, 3, "step_af_out", json!({"steps": 0})).is_empty());
    }

    #[test]
    fn the_iem_equalizer_is_set_and_read() {
        let mut m = Mcp::for_host(HOST, Kind::Transmitter);
        assert_eq!(
            run(
                &mut m,
                0,
                1,
                "set_iem_equalizer",
                json!({"enabled": false, "low": -5, "low_mid": 3, "mid": 0, "mid_high": -3, "high": 5})
            ),
            ["Equalizer 0 -5 3 0 -3 5\r"]
        );
        assert_eq!(
            run(&mut m, 10, 2, "set_iem_equalizer", json!({"enabled": true})),
            ["Equalizer 1\r"]
        );
        // Answered with the whole equalizer (TI 1254 p.33).
        let a = feed(
            &mut m,
            20,
            "Equalizer 0 -5 3 0 -3 5\rEqualizer 1 -5 3 0 -3 5\r",
        );
        assert_eq!(
            completed(&a),
            [(1, Ok(Outcome::Ack)), (2, Ok(Outcome::Ack))]
        );
        assert_eq!(
            state(&a)["channels"]["1"]["iem_equalizer"],
            json!({"enabled": true, "low": -5, "low_mid": 3, "mid": 0, "mid_high": -3, "high": 5})
        );
        // Some bands but not all is refused.
        assert!(run(
            &mut m,
            30,
            3,
            "set_iem_equalizer",
            json!({"enabled": true, "low": 1})
        )
        .is_empty());
        // SR States: the RF-mute flag since the last cycle.
        let a = feed(&mut m, 40, "States 1 2\r");
        assert_eq!(state(&a)["channels"]["1"]["rf_mute_flag"], 2);
    }

    #[test]
    fn the_em_current_mute_state_and_rf_config() {
        let mut m = receiver();
        // TI 1254 p.30: 13 is RX-Mute and RF-Mute.
        let a = feed(&mut m, 0, "AF 37 42 13\rRfConfig 566000 608000 25\r");
        let s = state(&a);
        assert_eq!(
            s["channels"]["1"]["mute_now"],
            json!({"any": true, "tx": false, "rf": true, "rx": true})
        );
        assert_eq!(
            s["device"]["rf_config"],
            json!([{"min_khz": 566000, "max_khz": 608000, "step_khz": 25}])
        );
    }
}
