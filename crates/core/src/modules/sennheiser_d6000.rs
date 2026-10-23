//! Sennheiser Digital 6000 (EM 6000, EM 6000 Dante) over SSC: JSON over UDP.
//!
//! Protocol from Sennheiser TI 1109 v2.2. Lifecycle from a consumer's Digital 6000
//! client: two subscriptions (a fixed-rate metering array and an on-change
//! channel tree), renewed at a third of their lifetime, with a silence timeout
//! underneath because a lapsed subscription is otherwise indistinguishable from
//! an idle receiver.
//!
//! Settings that cannot be subscribed (bank and channel, encryption, sync
//! settings, user banks, AF out, booster, brightness, clock, network) are read
//! on connecting, and read again when the receiver's on-change counters
//! (`/rxN/wsm_master_cnt`, `/sys/wsm_master_cnt`), which are subscribed, say
//! they were changed on the receiver.
//!
//! Opened for commands only, it subscribes to nothing and reads no identity or
//! settings. A receiver sends nothing unsubscribed, so its name is asked at the
//! renewal cadence instead, as the liveness check.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, CommandResult, Connection, Cx, Key, Level, Millis, Module,
    OpenContext, Outcome,
};

const SOCKET: Key = "ssc";

/// The specification's own example for the metering node.
const METER_INTERVAL_MS: u64 = 480;
const LIFETIME_S: u64 = 20;
/// Renew at a third of the lifetime, so two lost datagrams are survivable.
const RENEW_EVERY: Millis = LIFETIME_S * 1000 / 3;
const SILENCE_TIMEOUT: Millis = 15_000;
const RECONNECT_AFTER: Millis = 5_000;
const REPLY_WAIT: Millis = 1_000;

/// "310 – subscription terminated", sent when a lifetime or count expires.
const SUBSCRIPTION_TERMINATED: i64 = 310;

const RENEW: Key = "renew";
const SILENCE: Key = "silence";
const RECONNECT: Key = "reconnect";
const REPLY: Key = "reply";

/// User banks (§8.10-8.15), read on connecting; the system banks are fixed
/// tables, read on request.
const USER_BANKS: [&str; 6] = ["u1", "u2", "u3", "u4", "u5", "u6"];
/// §8.29 low cut options, by index.
const LOW_CUT_HZ: [i64; 5] = [30, 60, 80, 100, 120];
/// §8.35 cable emulation options, by index.
const CABLE_EMULATION: [&str; 4] = ["line", "type1", "type2", "type3"];
/// §8.97 clock options, numbered from 1.
const CLOCK: [&str; 4] = [
    "internal_48k",
    "internal_96k",
    "external",
    "internal_manual",
];

#[derive(Debug)]
enum Expect {
    /// A setter whose reply states the resulting value, which should be this.
    Equals(Value),
    /// A numeric setter: the reply is the applied value, possibly adapted.
    Applied,
    /// Any reply at the path settles it (identify; several leaves at once).
    Present,
    Limits,
}

enum Reply {
    Done(CommandResult),
    Wait,
}

#[derive(Debug)]
struct Pending {
    id: CommandId,
    xid: u64,
    path: Vec<String>,
    expect: Expect,
    sent_at: Millis,
    deadline: Millis,
}

pub(crate) struct D6000 {
    device: SocketAddr,
    channels: u32,
    /// The EM 6000 Dante, which has the Dante methods.
    dante: bool,
    /// Subscribe and read identity; false for commands only.
    monitor: bool,
    /// The unanswered liveness query of a commands-only device: its xid and
    /// when it went.
    liveness: Option<(u64, Millis)>,
    connected: bool,
    open: bool,
    next_xid: u64,
    pending: Vec<Pending>,
    /// The last on-change counter seen, per receiver channel (0 for /sys).
    counters: BTreeMap<u32, i64>,
}

impl D6000 {
    pub(crate) fn new(ctx: OpenContext) -> D6000 {
        let port = ctx.port.unwrap_or(45);
        let mut d = D6000::for_device(SocketAddr::new(ctx.host, port), ctx.channels.unwrap_or(2));
        d.monitor = ctx.monitor;
        d.dante = ctx.model == "em-6000-dante";
        d
    }

    fn for_device(device: SocketAddr, channels: u32) -> D6000 {
        D6000 {
            device,
            channels,
            dante: false,
            monitor: true,
            liveness: None,
            connected: false,
            open: false,
            next_xid: 1,
            pending: Vec::new(),
            counters: BTreeMap::new(),
        }
    }

    fn send(&self, cx: &mut Cx, message: &Value) {
        cx.udp_send(SOCKET, self.device, message.to_string());
    }

    fn open(&mut self, cx: &mut Cx) {
        cx.udp_open(SOCKET, Bind::Ephemeral);
        self.open = true;
        self.subscribe(cx);
        cx.set_timer(SILENCE, SILENCE_TIMEOUT);
        cx.set_timer(RENEW, RENEW_EVERY);
    }

    fn close(&mut self, cx: &mut Cx, reason: &str) {
        if self.open {
            cx.udp_close(SOCKET);
            self.open = false;
        }
        cx.cancel_timer(RENEW);
        cx.cancel_timer(SILENCE);
        self.connected = false;
        self.counters.clear();
        cx.connection(Connection::Disconnected {
            reason: reason.into(),
        });
        cx.set_timer(RECONNECT, RECONNECT_AFTER);
    }

    /// Both subscriptions, and identity and settings until connected. Opened
    /// for commands only, the liveness query instead: an unsubscribed receiver
    /// sends nothing, so without it a lost device could not be told from an
    /// idle one.
    fn subscribe(&mut self, cx: &mut Cx) {
        if !self.monitor {
            let xid = self.next_xid;
            self.next_xid += 1;
            self.send(cx, &json!({"osc": {"xid": xid}, "device": {"name": null}}));
            self.liveness = Some((xid, cx.now()));
            return;
        }
        for message in subscription_messages(self.channels) {
            self.send(cx, &message);
        }
        if !self.connected {
            for message in identity_messages() {
                self.send(cx, &message);
            }
            self.read_system(cx);
            for ch in 1..=self.channels {
                self.read_channel(cx, ch);
            }
        }
    }

    /// The receiver-wide settings no subscription reports.
    fn read_system(&self, cx: &mut Cx) {
        let mut messages = vec![
            json!({"device": {"name": null}}),
            json!({"sys": {"booster": null, "brightness": null, "clock": null,
                           "clock_frequency_measured": null}}),
            json!({"device": {"network": {"ipv4": {
                "auto": null, "mdns": null, "static_ipaddr": null, "static_netmask": null,
                "static_gateway": null, "ipaddr": null, "netmask": null, "gateway": null,
            }}}}),
        ];
        if self.dante {
            messages.push(json!({"sys": {"dante": {"name": null, "version": null}}}));
            messages.push(json!({"device": {"network": {"ipv4_dante": {
                "auto": null, "ipaddr": null, "netmask": null, "gateway": null,
            }}}}));
        }
        for m in messages {
            self.send(cx, &m);
        }
    }

    /// One receiver channel's settings no subscription reports, and its user
    /// banks, one bank per datagram.
    fn read_channel(&self, cx: &mut Cx, ch: u32) {
        let rx = format!("rx{ch}");
        let mut sync = Map::new();
        for key in SYNC_KEYS {
            sync.insert(key.into(), Value::Null);
        }
        self.send(
            cx,
            &json!({ &rx: {"name": null, "carrier": null, "active_bank_channel": null,
                           "encryption": null, "testtone": null, "sync_settings": sync} }),
        );
        self.send(
            cx,
            &json!({"audio": { format!("out{ch}"): {"level_db": null} }}),
        );
        for bank in USER_BANKS {
            self.send(cx, &json!({ &rx: {"freq": { bank: bank_query() }} }));
        }
    }

    /// An on-change counter moved: something was changed on the receiver
    /// itself, so read again what it covers. The first value seen is only
    /// remembered, since everything was read on connecting.
    fn counter(&mut self, cx: &mut Cx, key: u32, value: i64) {
        let before = self.counters.insert(key, value);
        if before.is_none_or(|b| b == value) || !self.monitor {
            return;
        }
        if key == 0 {
            self.read_system(cx);
        } else if key <= self.channels {
            self.read_channel(cx, key);
        }
    }

    fn arm_reply_timer(&self, cx: &mut Cx) {
        match self.pending.iter().map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    fn request(&mut self, cx: &mut Cx, id: CommandId, path: &[&str], value: Value, expect: Expect) {
        let xid = self.next_xid;
        self.next_xid += 1;
        let mut message = nest(path, value);
        message
            .as_object_mut()
            .unwrap()
            .entry("osc")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap()
            .insert("xid".into(), json!(xid));
        self.send(cx, &message);
        self.pending.push(Pending {
            id,
            xid,
            path: path.iter().map(|s| s.to_string()).collect(),
            expect,
            sent_at: cx.now(),
            deadline: cx.now() + REPLY_WAIT,
        });
        self.arm_reply_timer(cx);
    }

    fn handle_errors(&mut self, cx: &mut Cx, message: &Value) {
        let Some(errors) = message.pointer("/osc/error").and_then(Value::as_array) else {
            return;
        };
        let xid = message.pointer("/osc/xid").and_then(Value::as_u64);
        for entry in errors {
            let mut leaves = Vec::new();
            error_leaves(entry, &mut Vec::new(), &mut leaves);
            for (path, code, desc) in leaves {
                if code == Some(SUBSCRIPTION_TERMINATED) {
                    cx.log(Level::Debug, "subscription terminated; renewing");
                    self.subscribe(cx);
                    continue;
                }
                let index = self.pending.iter().position(|p| {
                    xid == Some(p.xid) || (!path.is_empty() && path.starts_with(&p.path))
                });
                match index {
                    Some(i) => {
                        let p = self.pending.remove(i);
                        cx.round_trip(cx.now().saturating_sub(p.sent_at));
                        cx.complete(
                            p.id,
                            Err(CommandError::DeviceError {
                                code: code.map(|c| c.to_string()),
                                message: desc,
                            }),
                        );
                        self.arm_reply_timer(cx);
                    }
                    None => cx.log(
                        Level::Warning,
                        format!("device rejected a request: {entry}"),
                    ),
                }
            }
        }
    }

    fn resolve_replies(&mut self, cx: &mut Cx, message: &Value) {
        let xid = message.pointer("/osc/xid").and_then(Value::as_u64);
        if let Some((probe, sent)) = self.liveness {
            let answers = match xid {
                Some(x) => x == probe,
                None => message.pointer("/device/name").is_some(),
            };
            if answers {
                self.liveness = None;
                cx.round_trip(cx.now().saturating_sub(sent));
            }
        }
        let mut i = 0;
        while i < self.pending.len() {
            let p = &self.pending[i];
            if xid.is_some_and(|x| x != p.xid) {
                i += 1;
                continue;
            }
            let by_xid = xid == Some(p.xid);
            let reply = match &p.expect {
                Expect::Limits => message.pointer("/osc/limits").map(|l| {
                    Reply::Done(Ok(Outcome::Value {
                        value: parse_limits(l).unwrap_or(Value::Null),
                    }))
                }),
                expect => at_path(message, &p.path).map(|v| match (expect, v) {
                    (Expect::Equals(want), got) if got == want => Reply::Done(Ok(Outcome::Ack)),
                    (Expect::Equals(want), got) if by_xid => {
                        Reply::Done(Err(CommandError::DeviceError {
                            code: None,
                            message: format!("device reports {got} after setting {want}"),
                        }))
                    }
                    // Without an xid this may be a subscription notification
                    // carrying the old value; keep waiting for the reply.
                    (Expect::Equals(_), _) => Reply::Wait,
                    (Expect::Present, _) => Reply::Done(Ok(Outcome::Ack)),
                    (_, value) => Reply::Done(Ok(Outcome::Value {
                        value: value.clone(),
                    })),
                }),
            };
            match reply {
                Some(Reply::Done(result)) => {
                    let p = self.pending.remove(i);
                    cx.round_trip(cx.now().saturating_sub(p.sent_at));
                    cx.complete(p.id, result);
                }
                Some(Reply::Wait) | None => i += 1,
            }
        }
        self.arm_reply_timer(cx);
    }
}

/// The ten sync settings of §8.29-8.38.
const SYNC_KEYS: [&str; 10] = [
    "low_cut_frequency",
    "ignore_low_cut_frequency",
    "gain",
    "ignore_gain",
    "display",
    "ignore_display",
    "cable_emulation",
    "ignore_cable_emulation",
    "auto_lock",
    "ignore_auto_lock",
];

/// `{"00": null, ..., "99": null}`: every channel of a bank.
fn bank_query() -> Value {
    let mut bank = Map::new();
    for i in 0..100 {
        bank.insert(format!("{i:02}"), Value::Null);
    }
    Value::Object(bank)
}

/// Wrap `value` in objects along `path`: ["rx1","audio_mute"] -> {"rx1":{"audio_mute":v}}.
fn nest(path: &[&str], value: Value) -> Value {
    path.iter()
        .rev()
        .fold(value, |acc, key| json!({ *key: acc }))
}

fn at_path<'a>(message: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter().try_fold(message, |node, key| node.get(key))
}

/// Walk an SSC error entry to its `[code, {desc}]` leaves, keeping their paths.
/// Also accepts the flat `{code, desc}` form.
fn error_leaves(
    node: &Value,
    path: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, Option<i64>, String)>,
) {
    match node {
        Value::Array(items) if items.first().is_some_and(Value::is_i64) => {
            let desc = items
                .get(1)
                .and_then(|d| d.get("desc"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            out.push((path.clone(), items[0].as_i64(), desc));
        }
        Value::Object(map) if map.contains_key("code") => {
            let desc = map
                .get("desc")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            out.push((path.clone(), map.get("code").and_then(Value::as_i64), desc));
        }
        Value::Object(map) => {
            for (k, v) in map {
                path.push(k.clone());
                error_leaves(v, path, out);
                path.pop();
            }
        }
        _ => {}
    }
}

fn subscription_messages(channels: u32) -> [Value; 2] {
    let meter = json!({"osc": {"state": {"subscribe": [{
        "#": {"min": METER_INTERVAL_MS, "max": METER_INTERVAL_MS, "lifetime": LIFETIME_S, "count": 1000},
        "mm": null,
    }]}}});

    let mut tree = Map::new();
    tree.insert("#".into(), json!({"lifetime": LIFETIME_S, "count": 1000}));
    for ch in 1..=channels {
        tree.insert(
            format!("rx{ch}"),
            json!({
                "name": null,
                "carrier": null,
                "audio_mute": null,
                "active_warnings": null,
                "active_status": null,
                "wsm_master_cnt": null,
                "skx": {
                    "battery": null, "name": null, "type": null, "lowcut": null, "gain": null,
                    "display": null, "capsule": null, "cable_emulation": null, "autolock": null,
                },
            }),
        );
    }
    tree.insert("sys".into(), json!({"wsm_master_cnt": null}));
    let state = json!({"osc": {"state": {"subscribe": [Value::Object(tree)]}}});
    [meter, state]
}

fn identity_messages() -> [Value; 1] {
    [
        json!({"device": {"identity": {"version": null, "vendor": null, "product": null}, "name": null}}),
    ]
}

/// `{"rx1":{"carrier":[{"min":470100,"max":713900,"inc":25}]}}` inside /osc/limits.
fn parse_limits(limits: &Value) -> Option<Value> {
    limits.as_array()?.iter().find_map(|entry| {
        let carrier = entry.pointer("/rx1/carrier")?;
        let spec = carrier
            .as_array()
            .and_then(|a| a.first())
            .unwrap_or(carrier);
        let (min, max) = (spec.get("min")?.as_i64()?, spec.get("max")?.as_i64()?);
        let step = spec.get("inc").and_then(Value::as_i64);
        Some(json!({"min_khz": min, "max_khz": max, "step_khz": step}))
    })
}

/// "x:xx" -> minutes; "-:--" or anything else -> None.
fn parse_minutes(time: &str) -> Option<i64> {
    let (h, m) = time.trim().split_once(':')?;
    if m.len() != 2 {
        return None;
    }
    Some(h.parse::<i64>().ok()? * 60 + m.parse::<i64>().ok()?)
}

/// A transmitter string: empty means no transmitter or no valid information.
fn tx_text(v: &Value) -> Option<Value> {
    let s = v.as_str()?.trim();
    Some(if s.is_empty() { Value::Null } else { json!(s) })
}

/// The sync settings as the state names them.
fn sync_patch(sync: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    let index = |key: &str| sync.get(key).and_then(Value::as_i64);
    if let Some(i) = index("low_cut_frequency") {
        out.insert("low_cut_hz".into(), json!(LOW_CUT_HZ.get(i as usize)));
    }
    if let Some(g) = index("gain") {
        out.insert("gain_db".into(), json!(g));
    }
    if let Some(i) = index("display") {
        out.insert(
            "display".into(),
            json!(match i {
                0 => Some("frequency"),
                1 => Some("name"),
                _ => None,
            }),
        );
    }
    if let Some(i) = index("cable_emulation") {
        out.insert(
            "cable_emulation".into(),
            json!(CABLE_EMULATION.get(i as usize)),
        );
    }
    if let Some(i) = index("auto_lock") {
        out.insert("auto_lock".into(), json!(i == 1));
    }
    for (wire, name) in [
        ("ignore_low_cut_frequency", "ignore_low_cut"),
        ("ignore_gain", "ignore_gain"),
        ("ignore_display", "ignore_display"),
        ("ignore_cable_emulation", "ignore_cable_emulation"),
        ("ignore_auto_lock", "ignore_auto_lock"),
    ] {
        if let Some(b) = sync.get(wire).and_then(Value::as_bool) {
            out.insert(name.into(), json!(b));
        }
    }
    out
}

fn channel_patch(block: &Map<String, Value>) -> Map<String, Value> {
    let mut ch = Map::new();
    if let Some(v) = block.get("name").and_then(Value::as_str) {
        ch.insert("name".into(), json!(v));
    }
    if let Some(v) = block.get("carrier").and_then(Value::as_i64) {
        ch.insert("frequency_khz".into(), json!(v));
    }
    if let Some(v) = block.get("audio_mute").and_then(Value::as_bool) {
        ch.insert("mute".into(), json!(v));
    }
    if let Some(v) = block.get("encryption").and_then(Value::as_bool) {
        ch.insert("encryption".into(), json!(v));
    }
    if let Some(v) = block.get("active_warnings").filter(|v| v.is_array()) {
        ch.insert("warnings".into(), v.clone());
    }
    if let Some(status) = block.get("active_status").and_then(Value::as_array) {
        let text = |i: usize| {
            status
                .get(i)
                .and_then(Value::as_str)
                .filter(|s| *s != "None")
        };
        ch.insert("status".into(), json!(text(0)));
        ch.insert("status_detail".into(), json!(text(1)));
    }
    if let Some([bank, number]) = block
        .get("active_bank_channel")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    {
        // "--" is manual tuning, where the channel number means nothing.
        let bank = bank.as_str().map(str::to_ascii_lowercase);
        let manual = bank.as_deref() == Some("--");
        ch.insert(
            "bank".into(),
            json!(if manual {
                Some("manual".to_string())
            } else {
                bank
            }),
        );
        ch.insert(
            "bank_channel".into(),
            if manual { Value::Null } else { number.clone() },
        );
    }
    if let Some(v) = block.get("testtone").and_then(Value::as_i64) {
        // -60 to 0 dBFS when on, 1 when off.
        // Off is test_tone false, and the level is removed.
        ch.insert("test_tone".into(), json!(v <= 0));
        ch.insert("test_tone_dbfs".into(), json!((v <= 0).then_some(v)));
    }
    if let Some(sync) = block.get("sync_settings").and_then(Value::as_object) {
        let s = sync_patch(sync);
        if !s.is_empty() {
            ch.insert("sync".into(), Value::Object(s));
        }
    }
    if let Some(freq) = block.get("freq").and_then(Value::as_object) {
        let mut banks = Map::new();
        for (bank, entries) in freq {
            if let Some(entries) = entries.as_object() {
                let table: Map<String, Value> = entries
                    .iter()
                    .filter(|(_, v)| v.is_i64())
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if !table.is_empty() {
                    banks.insert(bank.to_ascii_lowercase(), Value::Object(table));
                }
            }
        }
        if !banks.is_empty() {
            ch.insert("presets".into(), Value::Object(banks));
        }
    }
    if let Some(skx) = block.get("skx").and_then(Value::as_object) {
        let mut tx = Map::new();
        if let Some(v) = skx.get("name").and_then(Value::as_str) {
            tx.insert("name".into(), json!(v));
        }
        if let Some(v) = skx.get("type").filter(|v| v.is_array()) {
            tx.insert("type".into(), v.clone());
        }
        for (wire, name) in [
            ("lowcut", "low_cut"),
            ("gain", "gain"),
            ("display", "display"),
            ("capsule", "capsule"),
            ("cable_emulation", "cable_emulation"),
            ("autolock", "auto_lock"),
        ] {
            if let Some(v) = skx.get(wire).and_then(tx_text) {
                tx.insert(name.into(), v);
            }
        }
        if let Some(battery) = skx.get("battery").and_then(Value::as_array) {
            // Empty: no transmitter, or no valid battery information. Absent,
            // never zero.
            let value = match battery.first().and_then(Value::as_str) {
                Some(state) => {
                    let minutes = battery
                        .get(1)
                        .and_then(Value::as_str)
                        .and_then(parse_minutes);
                    json!({"state": state.trim(), "minutes": minutes})
                }
                None => Value::Null,
            };
            tx.insert("battery".into(), value);
        }
        if !tx.is_empty() {
            ch.insert("transmitter".into(), Value::Object(tx));
        }
    }
    ch
}

/// One row of the metering array, per TI 1109 §8.100.
fn meter_patch(row: &[Value]) -> Option<Value> {
    if row.len() < 9 {
        // A truncated row must not read as a dead link on a working channel.
        return None;
    }
    let n: Vec<f64> = row.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
    Some(json!({
        "rf": {
            "antenna_a_dbm": (n[0] - 255.0) / 2.0,
            "antenna_a_peak": n[1] == 1.0,
            "antenna_b_dbm": (n[2] - 255.0) / 2.0,
            "antenna_b_peak": n[3] == 1.0,
            "antenna_a_on": n[4] == 1.0,
            "antenna_b_on": n[5] == 1.0,
            "lqi": n[6] as i64,
        },
        "af": {
            "level_dbfs": (n[7] + 1.0) / 2.0 - 128.0,
            "peak": n[8] == 1.0,
        },
    }))
}

/// The IPv4 settings of one interface, as the state names them.
fn network_patch(ipv4: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for (wire, name) in [
        ("auto", "auto"),
        ("mdns", "mdns"),
        ("ipaddr", "address"),
        ("netmask", "subnet_mask"),
        ("gateway", "gateway"),
        ("static_ipaddr", "static_address"),
        ("static_netmask", "static_subnet_mask"),
        ("static_gateway", "static_gateway"),
    ] {
        // Read as plain strings or one-element arrays.
        let value = match ipv4.get(wire) {
            Some(Value::Array(a)) => a.first().cloned(),
            Some(v) if !v.is_null() => Some(v.clone()),
            _ => None,
        };
        if let Some(v) = value {
            out.insert(name.into(), v);
        }
    }
    out
}

fn state_patch(message: &Value, channels: u32) -> Option<Value> {
    let mut patch = Map::new();
    let mut chans = Map::new();

    if let Some(rows) = message.get("mm").and_then(Value::as_array) {
        for (i, row) in rows.iter().enumerate() {
            if let Some(m) = row.as_array().and_then(|r| meter_patch(r)) {
                chans.insert((i + 1).to_string(), m);
            }
        }
    }
    for ch in 1..=channels {
        let mut p = message
            .get(format!("rx{ch}"))
            .and_then(Value::as_object)
            .map(channel_patch)
            .unwrap_or_default();
        if let Some(level) = message
            .pointer(&format!("/audio/out{ch}/level_db"))
            .and_then(Value::as_i64)
        {
            p.insert("af_out_db".into(), json!(level));
        }
        if !p.is_empty() {
            // Copied, not merged: a null here is a removal the snapshot
            // must see, and merging would apply it to this patch instead.
            let entry = chans.entry(ch.to_string()).or_insert_with(|| json!({}));
            entry.as_object_mut().unwrap().extend(p);
        }
    }
    if !chans.is_empty() {
        patch.insert("channels".into(), Value::Object(chans));
    }

    let mut device = Map::new();
    if let Some(identity) = message
        .pointer("/device/identity")
        .and_then(Value::as_object)
    {
        for key in ["version", "vendor", "product"] {
            if let Some(v) = identity.get(key).and_then(Value::as_str) {
                device.insert(key.into(), json!(v));
            }
        }
    }
    if let Some(v) = message.pointer("/device/name").and_then(Value::as_str) {
        device.insert("name".into(), json!(v));
    }
    if let Some(sys) = message.get("sys").and_then(Value::as_object) {
        if let Some(v) = sys.get("booster").and_then(Value::as_bool) {
            device.insert("booster".into(), json!(v));
        }
        if let Some(v) = sys.get("brightness").and_then(Value::as_i64) {
            device.insert("brightness_percent".into(), json!(v));
        }
        if let Some(v) = sys.get("clock").and_then(Value::as_i64) {
            let mode = usize::try_from(v - 1).ok().and_then(|i| CLOCK.get(i));
            device.insert("clock".into(), json!(mode));
        }
        if let Some(v) = sys.get("clock_frequency_measured").and_then(Value::as_str) {
            device.insert("clock_measured".into(), json!(v));
        }
        if let Some(dante) = sys.get("dante").and_then(Value::as_object) {
            let mut d = Map::new();
            for key in ["name", "version"] {
                if let Some(v) = dante.get(key).and_then(Value::as_str) {
                    d.insert(key.into(), json!(v));
                }
            }
            if !d.is_empty() {
                device.insert("dante".into(), Value::Object(d));
            }
        }
    }
    let mut network = Map::new();
    for (wire, name) in [("ipv4", "control"), ("ipv4_dante", "dante")] {
        if let Some(ipv4) = message
            .pointer(&format!("/device/network/{wire}"))
            .and_then(Value::as_object)
        {
            let n = network_patch(ipv4);
            if !n.is_empty() {
                network.insert(name.into(), Value::Object(n));
            }
        }
    }
    if !network.is_empty() {
        device.insert("network".into(), Value::Object(network));
    }
    if !device.is_empty() {
        patch.insert("device".into(), Value::Object(device));
    }

    (!patch.is_empty()).then_some(Value::Object(patch))
}

fn invalid(cx: &mut Cx, id: CommandId, message: impl Into<String>) {
    cx.complete(
        id,
        Err(CommandError::InvalidParams {
            message: message.into(),
        }),
    );
}

/// A dotted IPv4 parameter, as the receiver writes it.
fn ipv4(params: &Params, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<Ipv4Addr>().ok())
        .map(|a| a.to_string())
}

impl Module for D6000 {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.connected {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        let ch = params.get("channel").and_then(Value::as_i64).unwrap_or(1);
        let rx = format!("rx{ch}");
        let flag = |key: &str| params.get(key).and_then(Value::as_bool).unwrap_or(true);
        let text = |key: &str| params.get(key).and_then(Value::as_str).unwrap_or("");
        match name {
            "mute" => {
                let muted = flag("muted");
                self.request(
                    cx,
                    id,
                    &[&rx, "audio_mute"],
                    json!(muted),
                    Expect::Equals(json!(muted)),
                );
            }
            "set_frequency" => {
                let khz = params.get("frequency_khz").cloned().unwrap_or(Value::Null);
                self.request(cx, id, &[&rx, "carrier"], khz, Expect::Applied);
            }
            "set_af_out" => {
                let out = format!("out{ch}");
                let level = params.get("level_db").cloned().unwrap_or(Value::Null);
                self.request(cx, id, &["audio", &out, "level_db"], level, Expect::Applied);
            }
            "set_channel_name" => {
                let name = json!(text("name"));
                self.request(cx, id, &[&rx, "name"], name, Expect::Applied);
            }
            "set_device_name" => {
                // The receiver appends its MAC address, so the reply differs.
                let name = json!(text("name"));
                self.request(cx, id, &["device", "name"], name, Expect::Applied);
            }
            "set_bank_channel" => {
                let bank = text("bank");
                let number = params
                    .get("bank_channel")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let value = json!([bank, number]);
                self.request(
                    cx,
                    id,
                    &[&rx, "active_bank_channel"],
                    value,
                    Expect::Applied,
                );
            }
            "set_encryption" => {
                let on = flag("enabled");
                self.request(
                    cx,
                    id,
                    &[&rx, "encryption"],
                    json!(on),
                    Expect::Equals(json!(on)),
                );
            }
            "set_booster" => {
                let on = flag("enabled");
                self.request(
                    cx,
                    id,
                    &["sys", "booster"],
                    json!(on),
                    Expect::Equals(json!(on)),
                );
            }
            "set_brightness" => {
                let percent = params.get("percent").cloned().unwrap_or(Value::Null);
                self.request(cx, id, &["sys", "brightness"], percent, Expect::Applied);
            }
            "set_clock" => {
                let Some(i) = CLOCK.iter().position(|c| *c == text("mode")) else {
                    return invalid(cx, id, "unknown clock mode");
                };
                let value = json!(i + 1);
                self.request(
                    cx,
                    id,
                    &["sys", "clock"],
                    value.clone(),
                    Expect::Equals(value),
                );
            }
            "set_sync_settings" => {
                let mut sync = Map::new();
                if let Some(hz) = params.get("low_cut_hz").and_then(Value::as_str) {
                    let Some(i) = LOW_CUT_HZ.iter().position(|f| f.to_string() == hz) else {
                        return invalid(cx, id, "low cut is 30, 60, 80, 100 or 120 Hz");
                    };
                    sync.insert("low_cut_frequency".into(), json!(i));
                }
                if let Some(gain) = params.get("gain_db").and_then(Value::as_i64) {
                    if gain % 3 != 0 {
                        return invalid(cx, id, "sync gain goes in 3 dB steps");
                    }
                    sync.insert("gain".into(), json!(gain));
                }
                if let Some(display) = params.get("display").and_then(Value::as_str) {
                    sync.insert("display".into(), json!(u8::from(display == "name")));
                }
                if let Some(cable) = params.get("cable_emulation").and_then(Value::as_str) {
                    let i = CABLE_EMULATION
                        .iter()
                        .position(|c| *c == cable)
                        .unwrap_or(0);
                    sync.insert("cable_emulation".into(), json!(i));
                }
                if let Some(lock) = params.get("auto_lock").and_then(Value::as_bool) {
                    sync.insert("auto_lock".into(), json!(u8::from(lock)));
                }
                for (param, wire) in [
                    ("ignore_low_cut", "ignore_low_cut_frequency"),
                    ("ignore_gain", "ignore_gain"),
                    ("ignore_display", "ignore_display"),
                    ("ignore_cable_emulation", "ignore_cable_emulation"),
                    ("ignore_auto_lock", "ignore_auto_lock"),
                ] {
                    if let Some(b) = params.get(param).and_then(Value::as_bool) {
                        sync.insert(wire.into(), json!(b));
                    }
                }
                if sync.is_empty() {
                    return invalid(cx, id, "give at least one sync setting");
                }
                self.request(
                    cx,
                    id,
                    &[&rx, "sync_settings"],
                    Value::Object(sync),
                    Expect::Present,
                );
            }
            "set_user_frequency" => {
                let bank = text("bank");
                let index = params.get("index").and_then(Value::as_i64).unwrap_or(0);
                let slot = format!("{index:02}");
                let khz = params.get("frequency_khz").cloned().unwrap_or(Value::Null);
                self.request(cx, id, &[&rx, "freq", bank, &slot], khz, Expect::Applied);
            }
            "get_bank" => {
                // The bank's hundred channels in one read; the reply is the
                // table, which also lands in the state.
                let bank = text("bank");
                self.request(cx, id, &[&rx, "freq", bank], bank_query(), Expect::Applied);
            }
            "set_network" | "set_dante_network" => {
                let dante = name == "set_dante_network";
                if dante && !self.dante {
                    return cx.complete(
                        id,
                        Err(CommandError::UnsupportedForModel {
                            command: name.into(),
                            model: "em-6000".into(),
                        }),
                    );
                }
                let wire = if dante { "ipv4_dante" } else { "ipv4" };
                let auto = text("mode") == "auto";
                let value = if auto {
                    json!({"auto": true})
                } else {
                    let (Some(ip), Some(mask)) =
                        (ipv4(params, "address"), ipv4(params, "subnet_mask"))
                    else {
                        return invalid(
                            cx,
                            id,
                            "manual addressing needs a dotted IPv4 address and subnet mask",
                        );
                    };
                    let gateway = ipv4(params, "gateway").unwrap_or_else(|| "0.0.0.0".into());
                    if dante {
                        json!({"settings": ["manual", ip, mask, gateway]})
                    } else {
                        let mut v = json!({"auto": false, "static_ipaddr": ip,
                                           "static_netmask": mask, "static_gateway": gateway});
                        if let Some(mdns) = params.get("mdns").and_then(Value::as_bool) {
                            v["mdns"] = json!(mdns);
                        }
                        v
                    }
                };
                self.request(cx, id, &["device", "network", wire], value, Expect::Present);
            }
            // A read of a read-only method (TI 1109 §8.41).
            "identify" => self.request(cx, id, &[&rx, "identify"], Value::Null, Expect::Present),
            "get_frequency_limits" => {
                let query = json!([{"rx1": {"carrier": null}}]);
                self.request(cx, id, &["osc", "limits"], query, Expect::Limits);
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, _from: SocketAddr, data: &[u8]) {
        cx.set_timer(SILENCE, SILENCE_TIMEOUT);
        cx.alive();
        if !self.connected {
            self.connected = true;
            cx.connection(Connection::Connected);
        }

        let Ok(message) = serde_json::from_slice::<Value>(data) else {
            return;
        };
        if message.pointer("/osc/error").is_some() {
            self.handle_errors(cx, &message);
            return;
        }
        // State first, so a caller reading the snapshot after its command
        // completes sees what the reply carried.
        if let Some(patch) = state_patch(&message, self.channels) {
            cx.state(patch);
        }
        self.resolve_replies(cx, &message);
        if let Some(n) = message
            .pointer("/sys/wsm_master_cnt")
            .and_then(Value::as_i64)
        {
            self.counter(cx, 0, n);
        }
        for ch in 1..=self.channels {
            if let Some(n) = message
                .pointer(&format!("/rx{ch}/wsm_master_cnt"))
                .and_then(Value::as_i64)
            {
                self.counter(cx, ch, n);
            }
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, _socket: Key, message: &str) {
        self.open = false;
        cx.log(Level::Debug, format!("socket error: {message}"));
        self.close(cx, "socket error");
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RENEW => {
                self.subscribe(cx);
                cx.set_timer(RENEW, RENEW_EVERY);
            }
            SILENCE => {
                cx.log(Level::Warning, "stopped sending; reconnecting");
                self.close(cx, "no data for 15 s");
            }
            RECONNECT => self.open(cx),
            REPLY => {
                let now = cx.now();
                let (expired, waiting): (Vec<_>, Vec<_>) =
                    self.pending.drain(..).partition(|p| p.deadline <= now);
                self.pending = waiting;
                for p in expired {
                    let result = match p.expect {
                        Expect::Limits => Err(CommandError::Timeout),
                        _ => Ok(Outcome::Unverified),
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
    use crate::module::{Action, CommandResult};
    use std::net::{IpAddr, Ipv4Addr};

    fn device() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 60)), 45)
    }

    fn connected() -> D6000 {
        let mut d = D6000::for_device(device(), 2);
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        feed(&mut d, 1, json!({"osc": {"state": {"subscribe": []}}}));
        d
    }

    fn feed(d: &mut D6000, now: Millis, message: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        d.datagram(&mut cx, SOCKET, device(), message.to_string().as_bytes());
        cx.take()
    }

    fn sent(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, .. } => serde_json::from_slice(data).ok(),
                _ => None,
            })
            .collect()
    }

    fn completed(actions: &[Action]) -> Vec<(CommandId, CommandResult)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Complete { id, result } => Some((*id, result.clone())),
                _ => None,
            })
            .collect()
    }

    fn patch(actions: &[Action]) -> Value {
        actions
            .iter()
            .find_map(|a| match a {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .unwrap_or(Value::Null)
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn start_sends_both_subscriptions_and_identity() {
        let mut d = D6000::for_device(device(), 2);
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let actions = cx.take();
        assert!(actions.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Ephemeral
        }));
        let sent = sent(&actions);
        assert_eq!(
            sent[0],
            json!({"osc": {"state": {"subscribe": [{
                "#": {"min": 480, "max": 480, "lifetime": 20, "count": 1000}, "mm": null
            }]}}})
        );
        let tree = &sent[1]["osc"]["state"]["subscribe"][0];
        assert_eq!(tree["#"], json!({"lifetime": 20, "count": 1000}));
        assert!(tree["rx2"]["skx"]["battery"].is_null());
        assert!(sent[2]["device"]["identity"].is_object());
        assert!(actions.contains(&Action::SetTimer {
            key: RENEW,
            after: 6_666
        }));
    }

    #[test]
    fn metering_uses_the_documented_formulae() {
        // TI 1109 §8.100's own example.
        let mut d = connected();
        let p = patch(&feed(
            &mut d,
            10,
            json!({"mm": [[0,0,0,0,0,0,0,0,0],[83,0,53,0,1,1,128,165,0]]}),
        ));
        let ch2 = &p["channels"]["2"];
        assert_eq!(ch2["rf"]["antenna_a_dbm"], -86.0);
        assert_eq!(ch2["rf"]["antenna_b_dbm"], -101.0);
        assert_eq!(ch2["rf"]["antenna_a_on"], true);
        assert_eq!(ch2["rf"]["lqi"], 128);
        assert_eq!(ch2["af"]["level_dbfs"], -45.0);
    }

    #[test]
    fn short_metering_rows_are_skipped() {
        let mut d = connected();
        let p = patch(&feed(&mut d, 10, json!({"mm": [[1, 2, 3]]})));
        assert!(p.is_null());
    }

    #[test]
    fn battery_states_and_absence() {
        let mut d = connected();
        let p = patch(&feed(
            &mut d,
            10,
            json!({"rx1": {"skx": {"battery": ["70%", "5:12"]}}}),
        ));
        assert_eq!(
            p["channels"]["1"]["transmitter"]["battery"],
            json!({"state": "70%", "minutes": 312})
        );
        let p = patch(&feed(
            &mut d,
            20,
            json!({"rx1": {"skx": {"battery": ["low", "-:--"]}}}),
        ));
        assert_eq!(
            p["channels"]["1"]["transmitter"]["battery"],
            json!({"state": "low", "minutes": null})
        );
        let p = patch(&feed(&mut d, 30, json!({"rx1": {"skx": {"battery": []}}})));
        assert_eq!(p["channels"]["1"]["transmitter"]["battery"], Value::Null);
    }

    #[test]
    fn mute_carries_an_xid_and_acks_on_the_matching_reply() {
        let mut d = connected();
        let mut cx = Cx::new(10);
        d.command(
            &mut cx,
            5,
            "mute",
            &params(json!({"channel": 2, "muted": true})),
        );
        let sent = sent(&cx.take());
        assert_eq!(
            sent[0],
            json!({"osc": {"xid": 1}, "rx2": {"audio_mute": true}})
        );

        let actions = feed(
            &mut d,
            20,
            json!({"osc": {"xid": 1}, "rx2": {"audio_mute": true}}),
        );
        assert_eq!(completed(&actions), [(5, Ok(Outcome::Ack))]);
        assert_eq!(patch(&actions)["channels"]["2"]["mute"], true);
    }

    #[test]
    fn a_reply_without_xid_is_matched_by_path() {
        let mut d = connected();
        let mut cx = Cx::new(10);
        d.command(
            &mut cx,
            5,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        // A stale notification carrying the old value does not settle it...
        let actions = feed(&mut d, 20, json!({"rx1": {"audio_mute": false}}));
        assert!(completed(&actions).is_empty());
        // ...the reply stating the new value does.
        let actions = feed(&mut d, 30, json!({"rx1": {"audio_mute": true}}));
        assert_eq!(completed(&actions), [(5, Ok(Outcome::Ack))]);
    }

    #[test]
    fn adapted_values_are_returned() {
        let mut d = connected();
        let mut cx = Cx::new(10);
        d.command(
            &mut cx,
            9,
            "set_af_out",
            &params(json!({"channel": 1, "level_db": 18})),
        );
        let actions = feed(
            &mut d,
            20,
            json!({"osc": {"xid": 1}, "audio": {"out1": {"level_db": 6}}}),
        );
        assert_eq!(
            completed(&actions),
            [(9, Ok(Outcome::Value { value: json!(6) }))]
        );
    }

    #[test]
    fn identify_is_a_read() {
        let mut d = connected();
        let mut cx = Cx::new(10);
        d.command(&mut cx, 2, "identify", &params(json!({"channel": 1})));
        assert_eq!(
            sent(&cx.take())[0],
            json!({"osc": {"xid": 1}, "rx1": {"identify": null}})
        );
    }

    #[test]
    fn errors_fail_the_matching_command() {
        let mut d = connected();
        let mut cx = Cx::new(10);
        d.command(
            &mut cx,
            4,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        let actions = feed(
            &mut d,
            20,
            json!({"osc": {"error": [{"rx1": {"audio_mute": [406, {"desc": "not acceptable"}]}}]}}),
        );
        assert_eq!(
            completed(&actions),
            [(
                4,
                Err(CommandError::DeviceError {
                    code: Some("406".into()),
                    message: "not acceptable".into()
                })
            )]
        );
    }

    #[test]
    fn terminated_subscription_is_renewed_immediately() {
        let mut d = connected();
        let actions = feed(
            &mut d,
            10,
            json!({"osc": {"error": [{"mm": [310, {"desc": "subscription terminated"}]}]}}),
        );
        assert_eq!(sent(&actions).len(), 2);
    }

    #[test]
    fn frequency_limits_skip_entries_without_a_carrier() {
        let limits = json!([{"rx1": {"name": null}}, {"rx1": {"carrier": [{"min": 470100, "max": 713900, "inc": 25}]}}]);
        assert_eq!(
            parse_limits(&limits),
            Some(json!({"min_khz": 470100, "max_khz": 713900, "step_khz": 25}))
        );
    }

    #[test]
    fn commands_need_a_connection() {
        let mut d = D6000::for_device(device(), 2);
        let mut cx = Cx::new(0);
        d.command(&mut cx, 1, "mute", &params(json!({"channel": 1})));
        assert_eq!(
            completed(&cx.take()),
            [(1, Err(CommandError::NotConnected))]
        );
    }

    #[test]
    fn silence_closes_and_reconnects() {
        let mut d = connected();
        let mut cx = Cx::new(15_001);
        d.timer(&mut cx, SILENCE);
        let a = cx.take();
        assert!(a.contains(&Action::UdpClose { socket: SOCKET }));
        assert!(a.contains(&Action::SetTimer {
            key: RECONNECT,
            after: RECONNECT_AFTER
        }));
        let mut cx = Cx::new(20_001);
        d.timer(&mut cx, RECONNECT);
        let a = cx.take();
        assert!(a.contains(&Action::UdpOpen {
            socket: SOCKET,
            bind: Bind::Ephemeral
        }));
        // Not connected again yet, so identity is asked again.
        assert!(sent(&a)
            .iter()
            .any(|m| m.pointer("/device/identity").is_some()));
    }

    #[test]
    fn opened_for_commands_only_it_subscribes_to_nothing() {
        let mut d = D6000::for_device(device(), 2);
        d.monitor = false;
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        // No subscription and no identity read: only the liveness query.
        assert_eq!(
            sent(&cx.take()),
            [json!({"osc": {"xid": 1}, "device": {"name": null}})]
        );
        let a = feed(
            &mut d,
            15,
            json!({"osc": {"xid": 1}, "device": {"name": "EM 6000"}}),
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(15)));
        assert_eq!(patch(&a)["device"]["name"], "EM 6000");

        // Renewal asks again; nothing is subscribed.
        let mut cx = Cx::new(RENEW_EVERY);
        d.timer(&mut cx, RENEW);
        assert_eq!(
            sent(&cx.take()),
            [json!({"osc": {"xid": 2}, "device": {"name": null}})]
        );

        // Commands work.
        let mut cx = Cx::new(7_000);
        d.command(
            &mut cx,
            4,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        assert_eq!(
            sent(&cx.take()),
            [json!({"osc": {"xid": 3}, "rx1": {"audio_mute": true}})]
        );
        let a = feed(
            &mut d,
            7_020,
            json!({"osc": {"xid": 3}, "rx1": {"audio_mute": true}}),
        );
        assert_eq!(completed(&a), [(4, Ok(Outcome::Ack))]);
        assert!(a.contains(&Action::RoundTrip(20)));
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let mut d = connected();
        // Subscription notifications are pushed, not answers.
        let a = feed(&mut d, 5, json!({"rx1": {"name": "Vox"}}));
        assert!(!a.iter().any(|x| matches!(x, Action::RoundTrip(_))));

        let mut cx = Cx::new(10);
        d.command(
            &mut cx,
            9,
            "set_af_out",
            &params(json!({"channel": 1, "level_db": 6})),
        );
        let a = feed(
            &mut d,
            42,
            json!({"osc": {"xid": 1}, "audio": {"out1": {"level_db": 6}}}),
        );
        assert!(a.contains(&Action::RoundTrip(32)));

        let mut cx = Cx::new(100);
        d.command(
            &mut cx,
            10,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        let a = feed(
            &mut d,
            107,
            json!({"osc": {"xid": 2, "error": [{"rx1": {"audio_mute": [406, {"desc": "no"}]}}]}}),
        );
        assert!(a.contains(&Action::RoundTrip(7)));
    }

    #[test]
    fn unanswered_set_is_unverified() {
        let mut d = connected();
        let mut cx = Cx::new(10);
        d.command(
            &mut cx,
            1,
            "set_frequency",
            &params(json!({"channel": 1, "frequency_khz": 500000})),
        );
        let mut cx = Cx::new(10 + REPLY_WAIT);
        d.timer(&mut cx, REPLY);
        assert_eq!(completed(&cx.take()), [(1, Ok(Outcome::Unverified))]);
    }

    fn run(d: &mut D6000, now: Millis, id: CommandId, name: &str, p: Value) -> Vec<Value> {
        let mut cx = Cx::new(now);
        d.command(&mut cx, id, name, &params(p));
        sent(&cx.take())
    }

    #[test]
    fn connecting_reads_the_settings_no_subscription_reports() {
        let mut d = D6000::for_device(device(), 2);
        d.dante = true;
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let out = sent(&cx.take());
        let tree = &out[1]["osc"]["state"]["subscribe"][0];
        // The on-change counters and the rest of the transmitter are
        // subscribed.
        assert!(tree["rx1"]["wsm_master_cnt"].is_null());
        assert!(tree["sys"]["wsm_master_cnt"].is_null());
        assert!(tree["rx2"]["skx"].get("capsule").is_some());
        let has = |pointer: &str| out.iter().any(|m| m.pointer(pointer).is_some());
        for p in [
            "/sys/booster",
            "/sys/clock",
            "/sys/dante/name",
            "/device/network/ipv4/static_ipaddr",
            "/device/network/ipv4_dante/ipaddr",
            "/rx1/active_bank_channel",
            "/rx2/encryption",
            "/rx1/sync_settings/ignore_auto_lock",
            "/audio/out2/level_db",
            "/rx1/freq/u6/99",
        ] {
            assert!(has(p), "{p}");
        }
        // One user bank per datagram.
        let banks = out
            .iter()
            .filter(|m| m.pointer("/rx1/freq").is_some())
            .count();
        assert_eq!(banks, 6);
        // Renewing once connected subscribes again but reads nothing.
        feed(&mut d, 1, json!({"osc": {"state": {"subscribe": []}}}));
        let mut cx = Cx::new(RENEW_EVERY);
        d.timer(&mut cx, RENEW);
        assert_eq!(sent(&cx.take()).len(), 2);
    }

    #[test]
    fn a_change_on_the_receiver_is_read_again() {
        let mut d = connected();
        // The first counter value is only remembered.
        let mut cx = Cx::new(10);
        d.datagram(
            &mut cx,
            SOCKET,
            device(),
            json!({"rx2": {"wsm_master_cnt": 4}, "sys": {"wsm_master_cnt": 1}})
                .to_string()
                .as_bytes(),
        );
        assert!(sent(&cx.take()).is_empty());
        // A change on channel 2 reads channel 2 again, not channel 1.
        let a = feed(&mut d, 20, json!({"rx2": {"wsm_master_cnt": 5}}));
        let s = sent(&a);
        assert!(s.iter().any(|m| m.pointer("/rx2/sync_settings").is_some()));
        assert!(!s.iter().any(|m| m.get("rx1").is_some()));
        // A system change reads the system settings.
        let a = feed(&mut d, 30, json!({"sys": {"wsm_master_cnt": 2}}));
        assert!(sent(&a).iter().any(|m| m.pointer("/sys/booster").is_some()));
    }

    #[test]
    fn settings_become_state() {
        let mut d = connected();
        let p = patch(&feed(
            &mut d,
            10,
            json!({"rx1": {
                "active_bank_channel": ["U2", 63], "encryption": true, "testtone": 1,
                "active_status": ["SyncFail", "SyncResultTimeout"],
                "sync_settings": {"low_cut_frequency": 1, "ignore_low_cut_frequency": false,
                    "gain": 12, "ignore_gain": true, "display": 1, "ignore_display": false,
                    "cable_emulation": 2, "ignore_cable_emulation": false, "auto_lock": 1,
                    "ignore_auto_lock": true},
                "freq": {"u1": {"00": 470200, "01": 470400}},
                "skx": {"capsule": "MMD 935", "lowcut": "60 Hz", "gain": "", "autolock": "Off"},
            }, "audio": {"out1": {"level_db": 6}}}),
        ));
        let ch = &p["channels"]["1"];
        assert_eq!(ch["bank"], "u2");
        assert_eq!(ch["bank_channel"], 63);
        assert_eq!(ch["encryption"], true);
        assert_eq!(ch["test_tone_dbfs"], Value::Null);
        assert_eq!(ch["test_tone"], false);
        assert_eq!(ch["status"], "SyncFail");
        assert_eq!(ch["status_detail"], "SyncResultTimeout");
        assert_eq!(ch["af_out_db"], 6);
        assert_eq!(
            ch["sync"],
            json!({"low_cut_hz": 60, "ignore_low_cut": false, "gain_db": 12, "ignore_gain": true,
                   "display": "name", "ignore_display": false, "cable_emulation": "type2",
                   "ignore_cable_emulation": false, "auto_lock": true, "ignore_auto_lock": true})
        );
        assert_eq!(ch["presets"]["u1"], json!({"00": 470200, "01": 470400}));
        assert_eq!(
            ch["transmitter"],
            json!({"capsule": "MMD 935", "low_cut": "60 Hz", "gain": null, "auto_lock": "Off"})
        );

        // Manual tuning: no bank channel.
        let p = patch(&feed(
            &mut d,
            20,
            json!({"rx2": {"active_bank_channel": ["--", 0], "testtone": -52}}),
        ));
        assert_eq!(p["channels"]["2"]["bank"], "manual");
        assert_eq!(p["channels"]["2"]["bank_channel"], Value::Null);
        assert_eq!(p["channels"]["2"]["test_tone_dbfs"], -52);
        assert_eq!(p["channels"]["2"]["test_tone"], true);

        let p = patch(&feed(
            &mut d,
            30,
            json!({"sys": {"booster": true, "brightness": 80, "clock": 2,
                           "clock_frequency_measured": "48000 Hz", "dante": {"name": "EM6000"}},
                   "device": {"network": {"ipv4": {"auto": false, "ipaddr": ["10.0.0.5"],
                                                  "static_ipaddr": "10.0.0.5"}}}}),
        ));
        assert_eq!(
            p["device"],
            json!({"booster": true, "brightness_percent": 80, "clock": "internal_96k",
                   "clock_measured": "48000 Hz", "dante": {"name": "EM6000"},
                   "network": {"control": {"auto": false, "address": "10.0.0.5",
                                           "static_address": "10.0.0.5"}}})
        );
    }

    #[test]
    fn setters_are_written_and_settled_by_their_reply() {
        let mut d = connected();
        assert_eq!(
            run(
                &mut d,
                10,
                1,
                "set_bank_channel",
                json!({"channel": 1, "bank": "u2", "bank_channel": 63})
            ),
            [json!({"osc": {"xid": 1}, "rx1": {"active_bank_channel": ["u2", 63]}})]
        );
        let a = feed(
            &mut d,
            20,
            json!({"osc": {"xid": 1}, "rx1": {"active_bank_channel": ["u2", 63]}}),
        );
        assert_eq!(
            completed(&a),
            [(
                1,
                Ok(Outcome::Value {
                    value: json!(["u2", 63])
                })
            )]
        );

        assert_eq!(
            run(
                &mut d,
                30,
                2,
                "set_encryption",
                json!({"channel": 2, "enabled": true})
            ),
            [json!({"osc": {"xid": 2}, "rx2": {"encryption": true}})]
        );
        let a = feed(
            &mut d,
            40,
            json!({"osc": {"xid": 2}, "rx2": {"encryption": false}}),
        );
        assert!(matches!(
            &completed(&a)[..],
            [(2, Err(CommandError::DeviceError { .. }))]
        ));

        assert_eq!(
            run(&mut d, 50, 3, "set_booster", json!({"enabled": false})),
            [json!({"osc": {"xid": 3}, "sys": {"booster": false}})]
        );
        assert_eq!(
            run(&mut d, 50, 4, "set_brightness", json!({"percent": 40})),
            [json!({"osc": {"xid": 4}, "sys": {"brightness": 40}})]
        );
        assert_eq!(
            run(&mut d, 50, 5, "set_clock", json!({"mode": "external"})),
            [json!({"osc": {"xid": 5}, "sys": {"clock": 3}})]
        );
        assert_eq!(
            run(
                &mut d,
                50,
                6,
                "set_channel_name",
                json!({"channel": 1, "name": "Vox"})
            ),
            [json!({"osc": {"xid": 6}, "rx1": {"name": "Vox"}})]
        );
        assert_eq!(
            run(&mut d, 50, 7, "set_device_name", json!({"name": "Rack1"})),
            [json!({"osc": {"xid": 7}, "device": {"name": "Rack1"}})]
        );
        assert_eq!(
            run(
                &mut d,
                50,
                8,
                "set_user_frequency",
                json!({"channel": 2, "bank": "u3", "index": 7, "frequency_khz": 512300})
            ),
            [json!({"osc": {"xid": 8}, "rx2": {"freq": {"u3": {"07": 512300}}}})]
        );
        let a = feed(
            &mut d,
            60,
            json!({"osc": {"xid": 8}, "rx2": {"freq": {"u3": {"07": 512300}}}}),
        );
        assert_eq!(
            completed(&a),
            [(
                8,
                Ok(Outcome::Value {
                    value: json!(512300)
                })
            )]
        );
        assert_eq!(patch(&a)["channels"]["2"]["presets"]["u3"]["07"], 512300);

        let s = run(
            &mut d,
            70,
            9,
            "get_bank",
            json!({"channel": 1, "bank": "b2"}),
        );
        assert_eq!(s[0]["rx1"]["freq"]["b2"].as_object().unwrap().len(), 100);
    }

    #[test]
    fn sync_settings_send_only_what_is_given() {
        let mut d = connected();
        assert_eq!(
            run(
                &mut d,
                10,
                1,
                "set_sync_settings",
                json!({"channel": 1, "low_cut_hz": "80", "gain_db": -6, "display": "name",
                       "cable_emulation": "type3", "auto_lock": true, "ignore_gain": true})
            ),
            [json!({"osc": {"xid": 1}, "rx1": {"sync_settings": {
                "low_cut_frequency": 2, "gain": -6, "display": 1, "cable_emulation": 3,
                "auto_lock": 1, "ignore_gain": true}}})]
        );
        let a = feed(
            &mut d,
            20,
            json!({"osc": {"xid": 1}, "rx1": {"sync_settings": {"gain": -6}}}),
        );
        assert_eq!(completed(&a), [(1, Ok(Outcome::Ack))]);
        // Off the 3 dB grid, or nothing at all, is refused before sending.
        assert!(run(
            &mut d,
            30,
            2,
            "set_sync_settings",
            json!({"channel": 1, "gain_db": 4})
        )
        .is_empty());
        assert!(run(&mut d, 30, 3, "set_sync_settings", json!({"channel": 1})).is_empty());
        // An error on one leaf fails the command.
        run(
            &mut d,
            40,
            4,
            "set_sync_settings",
            json!({"channel": 2, "gain_db": 60}),
        );
        let a = feed(
            &mut d,
            50,
            json!({"osc": {"error": [{"rx2": {"sync_settings": {"gain": [406, {"desc": "no"}]}}}]}}),
        );
        assert!(matches!(
            &completed(&a)[..],
            [(4, Err(CommandError::DeviceError { .. }))]
        ));
    }

    #[test]
    fn network_settings() {
        let mut d = connected();
        assert_eq!(
            run(
                &mut d,
                10,
                1,
                "set_network",
                json!({"mode": "manual", "address": "10.0.0.20", "subnet_mask": "255.255.255.0"})
            ),
            [json!({"osc": {"xid": 1}, "device": {"network": {"ipv4": {
                "auto": false, "static_ipaddr": "10.0.0.20", "static_netmask": "255.255.255.0",
                "static_gateway": "0.0.0.0"}}}})]
        );
        assert!(run(
            &mut d,
            20,
            2,
            "set_network",
            json!({"mode": "manual", "address": "x"})
        )
        .is_empty());
        // Dante only on the Dante model.
        assert!(run(&mut d, 30, 3, "set_dante_network", json!({"mode": "auto"})).is_empty());
        d.dante = true;
        assert_eq!(
            run(
                &mut d,
                40,
                4,
                "set_dante_network",
                json!({"mode": "manual", "address": "192.168.1.203", "subnet_mask": "255.255.0.0",
                       "gateway": "192.168.0.1"})
            ),
            [
                json!({"osc": {"xid": 2}, "device": {"network": {"ipv4_dante": {
                "settings": ["manual", "192.168.1.203", "255.255.0.0", "192.168.0.1"]}}}})
            ]
        );
    }
}
