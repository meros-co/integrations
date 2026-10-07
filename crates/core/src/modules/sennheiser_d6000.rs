//! Sennheiser Digital 6000 (EM 6000, EM 6000 Dante) over SSC: JSON over UDP.
//!
//! Protocol from Sennheiser TI 1109 v2.2. Lifecycle from a consumer's Digital 6000
//! client: two subscriptions (a fixed-rate metering array and an on-change
//! channel tree), renewed at a third of their lifetime, with a silence timeout
//! underneath because a lapsed subscription is otherwise indistinguishable from
//! an idle receiver.
//!
//! Opened for commands only, it subscribes to nothing and reads no identity.
//! A receiver sends nothing unsubscribed, so its name is asked at the renewal
//! cadence instead, as the liveness check.

use std::net::SocketAddr;

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

#[derive(Debug)]
enum Expect {
    /// Mute: the reply states the resulting value, which should be this.
    Bool(bool),
    /// A numeric setter: the reply is the applied value, possibly adapted.
    Applied,
    Identify,
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
    /// Subscribe and read identity; false for commands only.
    monitor: bool,
    /// The unanswered liveness query of a commands-only device: its xid and
    /// when it went.
    liveness: Option<(u64, Millis)>,
    connected: bool,
    open: bool,
    next_xid: u64,
    pending: Vec<Pending>,
}

impl D6000 {
    pub(crate) fn new(ctx: OpenContext) -> D6000 {
        let port = ctx.port.unwrap_or(45);
        let mut d = D6000::for_device(SocketAddr::new(ctx.host, port), ctx.channels.unwrap_or(2));
        d.monitor = ctx.monitor;
        d
    }

    fn for_device(device: SocketAddr, channels: u32) -> D6000 {
        D6000 {
            device,
            channels,
            monitor: true,
            liveness: None,
            connected: false,
            open: false,
            next_xid: 1,
            pending: Vec::new(),
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
        cx.connection(Connection::Disconnected {
            reason: reason.into(),
        });
        cx.set_timer(RECONNECT, RECONNECT_AFTER);
    }

    /// Both subscriptions, and identity until connected. Opened for commands
    /// only, the liveness query instead: an unsubscribed receiver sends
    /// nothing, so without it a lost device could not be told from an idle one.
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
                let index = self
                    .pending
                    .iter()
                    .position(|p| xid == Some(p.xid) || (!path.is_empty() && p.path == path));
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
            let reply = match p.expect {
                Expect::Limits => message.pointer("/osc/limits").map(|l| {
                    Reply::Done(Ok(Outcome::Value {
                        value: parse_limits(l).unwrap_or(Value::Null),
                    }))
                }),
                _ => at_path(message, &p.path).map(|v| match (&p.expect, v) {
                    (Expect::Bool(want), Value::Bool(got)) if got == want => {
                        Reply::Done(Ok(Outcome::Ack))
                    }
                    (Expect::Bool(want), got) if by_xid => {
                        Reply::Done(Err(CommandError::DeviceError {
                            code: None,
                            message: format!("device reports {got} after setting {want}"),
                        }))
                    }
                    // Without an xid this may be a subscription notification
                    // carrying the old value; keep waiting for the reply.
                    (Expect::Bool(_), _) => Reply::Wait,
                    (Expect::Identify, _) => Reply::Done(Ok(Outcome::Ack)),
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
                "skx": {"battery": null, "name": null, "type": null},
            }),
        );
    }
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
    if let Some(v) = block.get("active_warnings").filter(|v| v.is_array()) {
        ch.insert("warnings".into(), v.clone());
    }
    if let Some(skx) = block.get("skx").and_then(Value::as_object) {
        let mut tx = Map::new();
        if let Some(v) = skx.get("name").and_then(Value::as_str) {
            tx.insert("name".into(), json!(v));
        }
        if let Some(v) = skx.get("type").filter(|v| v.is_array()) {
            tx.insert("type".into(), v.clone());
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
        if let Some(block) = message.get(format!("rx{ch}")).and_then(Value::as_object) {
            let p = channel_patch(block);
            if !p.is_empty() {
                // Copied, not merged: a null here is a removal the snapshot
                // must see, and merging would apply it to this patch instead.
                let entry = chans.entry(ch.to_string()).or_insert_with(|| json!({}));
                entry.as_object_mut().unwrap().extend(p);
            }
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
    if !device.is_empty() {
        patch.insert("device".into(), Value::Object(device));
    }

    (!patch.is_empty()).then_some(Value::Object(patch))
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
        match name {
            "mute" => {
                let muted = params.get("muted").and_then(Value::as_bool).unwrap_or(true);
                self.request(
                    cx,
                    id,
                    &[&rx, "audio_mute"],
                    json!(muted),
                    Expect::Bool(muted),
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
            // A read of a read-only method (TI 1109 §8.41).
            "identify" => self.request(cx, id, &[&rx, "identify"], Value::Null, Expect::Identify),
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
        assert_eq!(sent(&a).len(), 3);
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
}
