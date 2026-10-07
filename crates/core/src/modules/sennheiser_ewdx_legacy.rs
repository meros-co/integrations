//! The EW-DX's SSCv1 legacy mode: JSON over UDP port 45, beside SSCv2.
//!
//! Firmware 4 receivers speak SSCv1 too once "Legacy" third-party access is
//! enabled for them (in Control Cockpit or Wireless Systems Manager). It is
//! unencrypted and has no password, so the core uses it only for what SSCv2
//! cannot do: the device name and location (read-only over SSCv2), display
//! brightness, the receiver's auto lock, the EM 4 Dante's antenna booster
//! supply, restart, factory reset, and the network and Dante network
//! settings. Methods and their values are from Sennheiser's SSC developer's
//! guide for EW-DX (firmware 3.0.x, 03/2024); the message shapes and
//! replies are as for the Digital 6000 module, which speaks the same
//! protocol.
//!
//! Every request carries an `/osc/xid` and is answered with it. While
//! monitored, the settings above are subscribed with a 60 s lifetime and the
//! subscription is renewed every 20 s; a `310` (subscription terminated)
//! renews it at once. SSCv2 decides whether the device is connected: a
//! receiver with legacy mode off simply never answers here, and a command
//! then times out with a hint in the log.
//!
//! Opened for commands only, it subscribes to nothing and reads nothing.

use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{Bind, CommandError, CommandId, Cx, Key, Level, Millis, Outcome};

pub(crate) const SOCKET: Key = "ssc1";
pub(crate) const RENEW: Key = "ssc1-renew";
pub(crate) const REPLY: Key = "ssc1-reply";

const LIFETIME_S: u64 = 60;
pub(crate) const RENEW_EVERY: Millis = 20_000;
pub(crate) const REPLY_WAIT: Millis = 2_000;
/// "310 – subscription terminated", sent when a lifetime or count expires.
const SUBSCRIPTION_TERMINATED: i64 = 310;

/// The commands only legacy mode can carry.
pub(crate) const COMMANDS: [&str; 11] = [
    "set_device_name",
    "set_location",
    "set_brightness",
    "set_auto_lock",
    "set_booster",
    "restart",
    "restore_factory_defaults",
    "set_network",
    "set_mdns",
    "set_dante_network",
    "set_dante_interface_mapping",
];

#[derive(Debug)]
struct Pending {
    id: CommandId,
    xid: u64,
    sent_at: Millis,
    deadline: Millis,
}

pub(crate) struct Legacy {
    device: SocketAddr,
    monitor: bool,
    /// The model has Dante network settings (the Dante models).
    dante: bool,
    /// The model has an antenna booster supply (EM 4 Dante).
    booster: bool,
    open: bool,
    next_xid: u64,
    pending: Vec<Pending>,
    /// Whether the missing-answer hint was logged already.
    hinted: bool,
}

impl Legacy {
    pub(crate) fn new(device: SocketAddr, model: &str, monitor: bool) -> Legacy {
        Legacy {
            device,
            monitor,
            dante: model.ends_with("-dante"),
            booster: model == "em-4-dante",
            open: false,
            next_xid: 1,
            pending: Vec::new(),
            hinted: false,
        }
    }

    pub(crate) fn start(&mut self, cx: &mut Cx) {
        cx.udp_open(SOCKET, Bind::Ephemeral);
        self.open = true;
        if self.monitor {
            self.subscribe(cx);
            self.send(cx, &self.constants());
            cx.set_timer(RENEW, RENEW_EVERY);
        }
    }

    pub(crate) fn stop(&mut self, cx: &mut Cx) {
        if self.open {
            cx.udp_close(SOCKET);
            self.open = false;
        }
        cx.cancel_timer(RENEW);
        cx.cancel_timer(REPLY);
        for p in self.pending.drain(..) {
            cx.complete(p.id, Err(CommandError::Closed));
        }
    }

    fn send(&self, cx: &mut Cx, message: &Value) {
        cx.udp_send(SOCKET, self.device, message.to_string());
    }

    /// The settings kept current, as one SSC address tree.
    fn tree(&self) -> Value {
        let mut device = json!({
            "name": null,
            "location": null,
            "brightness": null,
            "lock": null,
            "network": {
                "ipv4": {
                    "auto": null, "ipaddr": null, "netmask": null, "gateway": null,
                    "manual_ipaddr": null, "manual_netmask": null, "manual_gateway": null,
                },
                "mdns": null,
            },
        });
        if self.booster {
            device["booster"] = Value::Null;
        }
        if self.dante {
            device["network"]["dante"] = json!({
                "identity": {"version": null},
                "ipv4": {
                    "auto": null, "ipaddr": null, "netmask": null, "gateway": null,
                    "manual_ipaddr": null, "manual_netmask": null, "manual_gateway": null,
                },
                "macs": null,
                "interface_mapping": null,
            });
        }
        json!({ "device": device })
    }

    fn subscribe(&self, cx: &mut Cx) {
        let mut tree = self.tree();
        tree.as_object_mut()
            .unwrap()
            .insert("#".into(), json!({"lifetime": LIFETIME_S}));
        self.send(cx, &json!({"osc": {"state": {"subscribe": [tree]}}}));
    }

    /// Values that never change, which cannot be subscribed: read once.
    fn constants(&self) -> Value {
        json!({"device": {"network": {"ether": {"macs": null}}}})
    }

    pub(crate) fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RENEW if self.open && self.monitor => {
                self.subscribe(cx);
                cx.set_timer(RENEW, RENEW_EVERY);
            }
            REPLY => {
                let now = cx.now();
                let (late, kept): (Vec<_>, Vec<_>) =
                    self.pending.drain(..).partition(|p| p.deadline <= now);
                self.pending = kept;
                if !late.is_empty() && !self.hinted {
                    self.hinted = true;
                    cx.log(
                        Level::Warning,
                        "no SSCv1 answer on UDP port 45: is Legacy third-party access enabled for the receiver?",
                    );
                }
                for p in late {
                    cx.complete(p.id, Err(CommandError::Timeout));
                }
                self.arm_reply_timer(cx);
            }
            _ => {}
        }
    }

    fn arm_reply_timer(&self, cx: &mut Cx) {
        match self.pending.iter().map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    /// Send a command's message, or say why it cannot be made.
    pub(crate) fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match self.message(name, params) {
            Ok(mut message) => {
                let xid = self.next_xid;
                self.next_xid += 1;
                message
                    .as_object_mut()
                    .unwrap()
                    .insert("osc".into(), json!({"xid": xid}));
                self.send(cx, &message);
                self.pending.push(Pending {
                    id,
                    xid,
                    sent_at: cx.now(),
                    deadline: cx.now() + REPLY_WAIT,
                });
                self.arm_reply_timer(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn message(&self, name: &str, params: &Params) -> Result<Value, CommandError> {
        let device = |v: Value| Ok(json!({ "device": v }));
        match name {
            "set_device_name" => device(json!({"name": text(params, "name")?})),
            "set_location" => device(json!({"location": text(params, "location")?})),
            "set_brightness" => device(json!({"brightness": int(params, "level")?})),
            "set_auto_lock" => device(json!({"lock": boolean(params, "locked")?})),
            "set_booster" if self.booster => {
                device(json!({"booster": boolean(params, "enabled")?}))
            }
            "set_booster" => Err(invalid("only the EM 4 Dante has an antenna booster supply")),
            "restart" => device(json!({"restart": true})),
            "restore_factory_defaults" => device(json!({"restore": "FACTORY_DEFAULTS"})),
            "set_mdns" => device(json!({"network": {"mdns": boolean(params, "enabled")?}})),
            "set_network" => {
                let mut ipv4 = Map::new();
                ipv4.insert("auto".into(), json!(boolean(params, "auto")?));
                for (param, field) in MANUAL {
                    if let Some(v) = params.get(param).and_then(Value::as_str) {
                        ipv4.insert(field.into(), json!(v));
                    }
                }
                if params.get("auto") == Some(&json!(false)) && ipv4.len() < 4 {
                    return Err(invalid(
                        "a manual address needs ip, netmask and gateway together",
                    ));
                }
                device(json!({"network": {"ipv4": ipv4}}))
            }
            "set_dante_network" | "set_dante_interface_mapping" if !self.dante => Err(invalid(
                "only the EM 2 Dante and EM 4 Dante have Dante network settings",
            )),
            "set_dante_network" => {
                // [primary, secondary]; null leaves the other one as it is.
                let slot = match text(params, "interface")? {
                    "primary" => 0,
                    "secondary" => 1,
                    other => return Err(invalid(format!("unknown Dante interface '{other}'"))),
                };
                let pair = |v: Value| {
                    let mut a = vec![Value::Null, Value::Null];
                    a[slot] = v;
                    Value::Array(a)
                };
                let mut ipv4 = Map::new();
                ipv4.insert("auto".into(), pair(json!(boolean(params, "auto")?)));
                for (param, field) in MANUAL {
                    if let Some(v) = params.get(param).and_then(Value::as_str) {
                        ipv4.insert(field.into(), pair(json!(v)));
                    }
                }
                if params.get("auto") == Some(&json!(false)) && ipv4.len() < 4 {
                    return Err(invalid(
                        "a manual address needs ip, netmask and gateway together",
                    ));
                }
                device(json!({"network": {"dante": {"ipv4": ipv4}}}))
            }
            "set_dante_interface_mapping" => {
                let mapping = match text(params, "mapping")? {
                    "single_cable" => "SINGLE_CABLE",
                    "split1" => "SPLIT1",
                    "split2" => "SPLIT2",
                    "split" => "SPLIT",
                    "audio_redundancy" => "AUDIO_REDUNDANCY",
                    other => return Err(invalid(format!("unknown interface mapping '{other}'"))),
                };
                device(json!({"network": {"dante": {"interface_mapping": mapping}}}))
            }
            other => Err(CommandError::UnknownCommand {
                command: other.into(),
            }),
        }
    }

    /// A datagram from the receiver: a reply, an error, or a notification.
    pub(crate) fn datagram(&mut self, cx: &mut Cx, from: SocketAddr, data: &[u8]) {
        if from.ip() != self.device.ip() {
            return;
        }
        let Ok(message) = serde_json::from_slice::<Value>(data) else {
            return;
        };
        self.hinted = false;
        let xid = message.pointer("/osc/xid").and_then(Value::as_u64);
        let errors = message.pointer("/osc/error").and_then(Value::as_array);
        if let Some(errors) = errors {
            let mut codes = Vec::new();
            for entry in errors {
                error_codes(entry, &mut codes);
            }
            if codes.contains(&SUBSCRIPTION_TERMINATED) && self.monitor {
                cx.log(Level::Debug, "SSCv1 subscription terminated; renewing");
                self.subscribe(cx);
            }
            let failed = codes.iter().find(|c| **c >= 400).map(|c| c.to_string());
            if let (Some(xid), Some(code)) = (xid, failed.clone()) {
                if let Some(i) = self.pending.iter().position(|p| p.xid == xid) {
                    let p = self.pending.remove(i);
                    cx.round_trip(cx.now().saturating_sub(p.sent_at));
                    cx.complete(
                        p.id,
                        Err(CommandError::DeviceError {
                            code: Some(code),
                            message: Value::Array(errors.clone()).to_string(),
                        }),
                    );
                    self.arm_reply_timer(cx);
                }
            } else if failed.is_some() {
                cx.log(
                    Level::Warning,
                    format!("the receiver refused an SSCv1 request: {message}"),
                );
            }
        }
        if let Some(xid) = xid {
            if let Some(i) = self.pending.iter().position(|p| p.xid == xid) {
                let p = self.pending.remove(i);
                cx.round_trip(cx.now().saturating_sub(p.sent_at));
                cx.complete(p.id, Ok(Outcome::Ack));
                self.arm_reply_timer(cx);
            }
        }
        if let Some(patch) = state_patch(&message) {
            cx.state(patch);
        }
    }
}

/// The manual address parameters and their SSCv1 methods.
const MANUAL: [(&str, &str); 3] = [
    ("ip", "manual_ipaddr"),
    ("netmask", "manual_netmask"),
    ("gateway", "manual_gateway"),
];

/// The address fields read back, and their state names.
const ADDRESSES: [(&str, &str); 7] = [
    ("auto", "auto"),
    ("ipaddr", "ip"),
    ("netmask", "netmask"),
    ("gateway", "gateway"),
    ("manual_ipaddr", "manual_ip"),
    ("manual_netmask", "manual_netmask"),
    ("manual_gateway", "manual_gateway"),
];

/// Every SSC error code under an error entry: `[code, {desc}]` leaves, or
/// the flat `{code, desc}` form.
fn error_codes(node: &Value, out: &mut Vec<i64>) {
    match node {
        Value::Array(items) if items.first().is_some_and(Value::is_i64) => {
            out.extend(items[0].as_i64());
        }
        Value::Number(n) => out.extend(n.as_i64()),
        Value::Object(map) if map.contains_key("code") => {
            out.extend(map.get("code").and_then(Value::as_i64));
        }
        Value::Object(map) => map.values().for_each(|v| error_codes(v, out)),
        _ => {}
    }
}

/// A one-element list (the document's `count: 1`) or a plain string, as the
/// receiver may send either.
fn single(v: &Value) -> Option<Value> {
    match v {
        Value::Array(items) if items.len() == 1 => single(&items[0]),
        Value::String(_) | Value::Bool(_) => Some(v.clone()),
        _ => None,
    }
}

/// What a message says about the settings legacy mode keeps current.
fn state_patch(message: &Value) -> Option<Value> {
    let device = message.get("device")?.as_object()?;
    let mut out = Map::new();
    for (key, name) in [("name", "name"), ("location", "location")] {
        if let Some(v) = device.get(key).filter(|v| v.is_string()) {
            out.insert(name.into(), v.clone());
        }
    }
    if let Some(v) = device.get("brightness").filter(|v| v.is_i64()) {
        out.insert("brightness".into(), v.clone());
    }
    if let Some(v) = device.get("lock").filter(|v| v.is_boolean()) {
        out.insert("auto_lock".into(), v.clone());
    }
    if let Some(v) = device.get("booster").filter(|v| v.is_boolean()) {
        out.insert("booster".into(), v.clone());
    }
    let mut network = Map::new();
    if let Some(ipv4) = message.pointer("/device/network/ipv4") {
        for (wire, name) in ADDRESSES {
            if let Some(v) = ipv4.get(wire).and_then(single) {
                network.insert(name.into(), v);
            }
        }
    }
    if let Some(v) = message
        .pointer("/device/network/mdns")
        .filter(|v| v.is_boolean())
    {
        network.insert("mdns".into(), v.clone());
    }
    if let Some(v) = message
        .pointer("/device/network/ether/macs")
        .filter(|v| v.is_array())
    {
        network.insert("macs".into(), v.clone());
    }
    if !network.is_empty() {
        out.insert("network".into(), Value::Object(network));
    }
    if let Some(d) = message.pointer("/device/network/dante") {
        let mut dante = Map::new();
        if let Some(v) = d.pointer("/identity/version").filter(|v| v.is_string()) {
            dante.insert("version".into(), v.clone());
        }
        if let Some(ipv4) = d.get("ipv4") {
            for (wire, name) in ADDRESSES {
                // [primary, secondary]
                if let Some(v) = ipv4.get(wire).filter(|v| v.is_array()) {
                    dante.insert(name.into(), v.clone());
                }
            }
        }
        if let Some(v) = d.get("macs").filter(|v| v.is_array()) {
            dante.insert("macs".into(), v.clone());
        }
        if let Some(v) = d.get("interface_mapping").and_then(Value::as_str) {
            dante.insert("interface_mapping".into(), json!(v.to_ascii_lowercase()));
        }
        if !dante.is_empty() {
            out.insert("dante".into(), Value::Object(dante));
        }
    }
    (!out.is_empty()).then(|| json!({ "device": out }))
}

fn boolean(params: &Params, name: &str) -> Result<bool, CommandError> {
    params
        .get(name)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("'{name}' is required")))
}

fn int(params: &Params, name: &str) -> Result<i64, CommandError> {
    params
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(format!("'{name}' is required")))
}

fn text<'a>(params: &'a Params, name: &str) -> Result<&'a str, CommandError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{name}' is required")))
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::{IpAddr, Ipv4Addr};

    const DEVICE: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)), 45);

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn sent(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { socket, to, data } if *socket == SOCKET && *to == DEVICE => {
                    serde_json::from_slice(data).ok()
                }
                _ => None,
            })
            .collect()
    }

    fn completed(actions: &[Action]) -> Vec<(CommandId, Result<Outcome, CommandError>)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Complete { id, result } => Some((*id, result.clone())),
                _ => None,
            })
            .collect()
    }

    /// The message a command sends, without its xid.
    fn message(model: &str, name: &str, p: Value) -> Result<Value, CommandError> {
        let mut l = Legacy::new(DEVICE, model, false);
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(10);
        l.command(&mut cx, 1, name, &params(p));
        let a = cx.take();
        if let Some((_, Err(e))) = completed(&a).into_iter().next() {
            return Err(e);
        }
        let mut m = sent(&a).remove(0);
        assert_eq!(m["osc"]["xid"], 1);
        m.as_object_mut().unwrap().remove("osc");
        Ok(m)
    }

    #[test]
    fn commands_send_the_documented_methods() {
        let cases = [
            (
                "set_device_name",
                json!({"name": "EM2STAGE"}),
                json!({"device": {"name": "EM2STAGE"}}),
            ),
            (
                "set_location",
                json!({"location": "Rack 3"}),
                json!({"device": {"location": "Rack 3"}}),
            ),
            (
                "set_brightness",
                json!({"level": 4}),
                json!({"device": {"brightness": 4}}),
            ),
            (
                "set_auto_lock",
                json!({"locked": true}),
                json!({"device": {"lock": true}}),
            ),
            ("restart", json!({}), json!({"device": {"restart": true}})),
            (
                "restore_factory_defaults",
                json!({}),
                json!({"device": {"restore": "FACTORY_DEFAULTS"}}),
            ),
            (
                "set_mdns",
                json!({"enabled": false}),
                json!({"device": {"network": {"mdns": false}}}),
            ),
            (
                "set_network",
                json!({"auto": true}),
                json!({"device": {"network": {"ipv4": {"auto": true}}}}),
            ),
            (
                "set_network",
                json!({"auto": false, "ip": "192.168.1.20", "netmask": "255.255.255.0", "gateway": "192.168.1.1"}),
                json!({"device": {"network": {"ipv4": {"auto": false, "manual_ipaddr": "192.168.1.20",
                    "manual_netmask": "255.255.255.0", "manual_gateway": "192.168.1.1"}}}}),
            ),
        ];
        for (name, p, want) in cases {
            assert_eq!(message("em-2", name, p).unwrap(), want, "{name}");
        }
        assert_eq!(
            message("em-4-dante", "set_booster", json!({"enabled": true})).unwrap(),
            json!({"device": {"booster": true}})
        );
        assert_eq!(
            message(
                "em-2-dante",
                "set_dante_network",
                json!({"interface": "secondary", "auto": false, "ip": "10.1.0.2",
                       "netmask": "255.255.0.0", "gateway": "10.1.0.1"})
            )
            .unwrap(),
            json!({"device": {"network": {"dante": {"ipv4": {
                "auto": [null, false], "manual_ipaddr": [null, "10.1.0.2"],
                "manual_netmask": [null, "255.255.0.0"], "manual_gateway": [null, "10.1.0.1"]}}}}})
        );
        assert_eq!(
            message(
                "em-4-dante",
                "set_dante_interface_mapping",
                json!({"mapping": "audio_redundancy"})
            )
            .unwrap(),
            json!({"device": {"network": {"dante": {"interface_mapping": "AUDIO_REDUNDANCY"}}}})
        );
    }

    #[test]
    fn model_and_address_limits_are_refused_before_sending() {
        for (model, name, p) in [
            ("em-2-dante", "set_booster", json!({"enabled": true})),
            (
                "em-2",
                "set_dante_network",
                json!({"interface": "primary", "auto": true}),
            ),
            (
                "em-2",
                "set_dante_interface_mapping",
                json!({"mapping": "split"}),
            ),
            (
                "em-2",
                "set_network",
                json!({"auto": false, "ip": "192.168.1.20"}),
            ),
        ] {
            assert!(
                matches!(
                    message(model, name, p),
                    Err(CommandError::InvalidParams { .. })
                ),
                "{model} {name}"
            );
        }
    }

    #[test]
    fn replies_complete_by_xid_and_update_state() {
        let mut l = Legacy::new(DEVICE, "em-2", false);
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        // Commands only: nothing subscribed or read.
        assert!(sent(&cx.take()).is_empty());
        let mut cx = Cx::new(10);
        l.command(&mut cx, 7, "set_brightness", &params(json!({"level": 2})));
        l.command(&mut cx, 8, "set_brightness", &params(json!({"level": 5})));
        cx.take();
        let mut cx = Cx::new(25);
        l.datagram(
            &mut cx,
            DEVICE,
            json!({"osc": {"xid": 1}, "device": {"brightness": 2}})
                .to_string()
                .as_bytes(),
        );
        l.datagram(
            &mut cx,
            DEVICE,
            json!({"osc": {"xid": 2, "error": [{"device": {"brightness": [406, {"desc": "not acceptable"}]}}]}})
                .to_string()
                .as_bytes(),
        );
        let a = cx.take();
        let done = completed(&a);
        assert_eq!(done[0], (7, Ok(Outcome::Ack)));
        assert!(
            matches!(&done[1], (8, Err(CommandError::DeviceError { code: Some(c), .. })) if c == "406")
        );
        assert!(a.contains(&Action::RoundTrip(15)));
        assert!(a.contains(&Action::State(json!({"device": {"brightness": 2}}))));
    }

    #[test]
    fn an_unanswered_command_times_out_with_a_hint() {
        let mut l = Legacy::new(DEVICE, "em-2", false);
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        l.command(&mut cx, 3, "restart", &params(json!({})));
        cx.take();
        let mut cx = Cx::new(REPLY_WAIT);
        l.timer(&mut cx, REPLY);
        let a = cx.take();
        assert_eq!(completed(&a), [(3, Err(CommandError::Timeout))]);
        assert!(a.iter().any(|x| matches!(x, Action::Log { .. })));
    }

    #[test]
    fn monitored_it_subscribes_renews_and_maps_notifications() {
        let mut l = Legacy::new(DEVICE, "em-4-dante", true);
        let mut cx = Cx::new(0);
        l.start(&mut cx);
        let a = cx.take();
        let msgs = sent(&a);
        let tree = &msgs[0]["osc"]["state"]["subscribe"][0];
        assert_eq!(tree["#"]["lifetime"], 60);
        assert!(tree["device"].get("booster").is_some());
        assert!(tree["device"]["network"]["dante"]
            .get("interface_mapping")
            .is_some());
        assert_eq!(
            msgs[1],
            json!({"device": {"network": {"ether": {"macs": null}}}})
        );
        assert!(a.contains(&Action::SetTimer {
            key: RENEW,
            after: RENEW_EVERY
        }));

        let mut cx = Cx::new(RENEW_EVERY);
        l.timer(&mut cx, RENEW);
        assert!(sent(&cx.take())[0]["osc"]["state"]["subscribe"].is_array());

        // 310: renewed at once.
        let mut cx = Cx::new(30_000);
        l.datagram(
            &mut cx,
            DEVICE,
            json!({"osc": {"error": [{"device": {"name": [310]}}]}})
                .to_string()
                .as_bytes(),
        );
        assert_eq!(sent(&cx.take()).len(), 1);

        let mut cx = Cx::new(31_000);
        l.datagram(
            &mut cx,
            DEVICE,
            json!({"device": {
                "name": "EM4", "location": "FOH", "brightness": 5, "lock": false, "booster": true,
                "network": {
                    "ipv4": {"auto": true, "ipaddr": ["192.168.1.20"], "netmask": "255.255.255.0"},
                    "mdns": true, "ether": {"macs": ["00:1B:66:00:00:01"]},
                    "dante": {"identity": {"version": "4.2.5"}, "ipv4": {"auto": [true, false]},
                              "macs": ["00:1B:66:00:00:02", "00:1B:66:00:00:03"],
                              "interface_mapping": "SPLIT"}}}})
            .to_string()
            .as_bytes(),
        );
        assert_eq!(
            cx.take(),
            [Action::State(json!({"device": {
                "name": "EM4", "location": "FOH", "brightness": 5, "auto_lock": false, "booster": true,
                "network": {"auto": true, "ip": "192.168.1.20", "netmask": "255.255.255.0",
                            "mdns": true, "macs": ["00:1B:66:00:00:01"]},
                "dante": {"version": "4.2.5", "auto": [true, false],
                          "macs": ["00:1B:66:00:00:02", "00:1B:66:00:00:03"],
                          "interface_mapping": "split"}}}))]
        );

        // Another host's datagram is ignored.
        let mut cx = Cx::new(32_000);
        l.datagram(
            &mut cx,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)), 45),
            json!({"device": {"name": "X"}}).to_string().as_bytes(),
        );
        assert!(cx.take().is_empty());
    }
}
