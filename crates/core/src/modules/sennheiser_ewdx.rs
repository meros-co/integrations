//! Sennheiser EW-DX over SSCv2: HTTPS requests plus a server-sent event stream.
//!
//! Resources, ranges and enumerations from Sennheiser's EW-DX 3rd-party
//! OpenAPI 1.7; the subscription lifecycle from its SSCv2 specification. The
//! resources RFDeck exercised on real receivers (/api/channel/{id}, its
//! signalQualityIndicator, level and warnings, /api/rf/channels/{id} and
//! /api/transmitters/{id}/battery) are subscribed first; the rest of the
//! documented surface follows, and a batch the device refuses does not stop
//! the later ones, so older firmware keeps what it has. The SSCv2 session
//! itself (probe, authentication, subscriptions, liveness) is the shared
//! client in [`super::sscv2`].
//!
//! Opened for commands only, it opens no subscription stream (so takes none of
//! the device's subscription sessions) and reads nothing on connecting: the
//! version request that finds the device, repeated whenever it has been quiet
//! for 3 s, is the liveness check.

use std::net::IpAddr;

use serde_json::{json, Map, Value};

use super::sscv2::{object, Call, SscDevice, Sscv2};
use crate::catalog::Params;
use crate::module::{CommandError, Cx, Level, Millis, OpenContext};

const WRITE_TIMEOUT: Millis = 3_000;
/// Link Density and encryption changes make the receiver busy for a while.
const SLOW_WRITE_TIMEOUT: Millis = 10_000;

/// The EW-DX receiver's resources, state and commands.
pub(crate) struct EwdxDevice {
    channels: u32,
}

pub(crate) type Ewdx = Sscv2<EwdxDevice>;

impl Sscv2<EwdxDevice> {
    pub(crate) fn from_context(ctx: OpenContext) -> Ewdx {
        let password = ctx
            .settings
            .get("password")
            .and_then(Value::as_str)
            .unwrap_or("");
        let port = ctx.port.unwrap_or(443);
        let mut d = Ewdx::for_device(ctx.host, port, password, ctx.channels.unwrap_or(2));
        d.monitor = ctx.monitor;
        d
    }

    fn for_device(host: IpAddr, port: u16, password: &str, channels: u32) -> Ewdx {
        Sscv2::new(host, port, password, EwdxDevice { channels })
    }
}

/// Device-wide resources, after the channels' own.
const DEVICE_RESOURCES: [&str; 8] = [
    "/api/device/site",
    "/api/device/state",
    "/api/device/identification",
    "/api/rf",
    "/api/rf/transmission",
    "/api/rf/encryption",
    "/api/ssc/legacyMode",
    "/api/firmware/update/state",
];

impl SscDevice for EwdxDevice {
    fn resources(&self) -> Vec<String> {
        let mut paths = Vec::new();
        // Six per channel, the ones tested on hardware, so they fill whole
        // batches of their own on a two- or four-channel receiver.
        for id in 0..self.channels {
            paths.push(format!("/api/channel/{id}"));
            paths.push(format!("/api/channel/{id}/signalQualityIndicator"));
            paths.push(format!("/api/channel/{id}/level"));
            paths.push(format!("/api/channel/{id}/warnings"));
            paths.push(format!("/api/rf/channels/{id}"));
            paths.push(format!("/api/transmitters/{id}/battery"));
        }
        for id in 0..self.channels {
            paths.push(format!("/api/channel/{id}/identify"));
            paths.push(format!("/api/channel/{id}/signalStrengthIndicator"));
            paths.push(format!("/api/channel/{id}/diversityIndicator"));
            paths.push(format!("/api/rf/presets/user/channels/{id}/banks/0"));
            paths.push(format!("/api/syncSettings/{id}"));
            paths.push(format!("/api/syncSettings/{id}/ignore"));
            paths.push(format!("/api/transmitters/{id}"));
            paths.push(format!("/api/transmitters/{id}/warnings"));
        }
        paths.extend(DEVICE_RESOURCES.iter().map(|p| p.to_string()));
        paths
    }

    fn subscribe_past_refusals(&self) -> bool {
        true
    }

    fn apply_resource(&self, path: &str, value: &Value, patch: &mut Map<String, Value>) {
        apply_resource(path, value, patch);
    }

    fn read_failed(&self, cx: &mut Cx, path: &str, status: u16) {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match (status, parts.as_slice()) {
            // No transmitter linked: its resources answer 422.
            (422, ["api", "transmitters", id, ..]) => {
                if let Ok(id) = id.parse::<u32>() {
                    cx.state(json!({"channels": {(id + 1).to_string(): {"transmitter": null}}}));
                }
            }
            (404, ["api", "channel", _]) => cx.log(
                Level::Warning,
                format!("{path} does not exist; the device has fewer channels than this model"),
            ),
            _ => cx.log(
                Level::Debug,
                format!("{path} answered HTTP {status}; this firmware may not have it"),
            ),
        }
    }

    fn command(&self, name: &str, params: &Params) -> Result<Call, CommandError> {
        let channel = || self.channel(params);
        match name {
            "mute" => {
                let id = channel()?;
                // "Per SSCv2, a write is a PUT of that resource carrying only the
                // properties to change" (RFDeck, verified on OpenAPI 1.7).
                let muted = boolean(params, "muted").unwrap_or(true);
                Ok(put(format!("/api/channel/{id}"), json!({"mute": muted})))
            }
            "set_channel_name" => {
                let id = channel()?;
                let name = text(params, "name")?;
                Ok(put(format!("/api/channel/{id}"), json!({"name": name})))
            }
            "set_gain" => {
                let id = channel()?;
                let gain = stepped(params, "gain_db", 3, "3 dB")?;
                Ok(put(format!("/api/channel/{id}"), json!({"gain": gain})))
            }
            "set_af_out" => {
                let id = channel()?;
                let level = stepped(params, "level_db", 6, "6 dB")?;
                Ok(put(
                    format!("/api/channel/{id}"),
                    json!({"outputLevel": level}),
                ))
            }
            "identify" => {
                let id = channel()?;
                let on = boolean(params, "enabled").unwrap_or(true);
                Ok(put(
                    format!("/api/channel/{id}/identify"),
                    json!({"enabled": on}),
                ))
            }
            "identify_device" => {
                let on = boolean(params, "enabled").unwrap_or(true);
                Ok(put("/api/device/identification", json!({"visual": on})))
            }
            "restore_channel_audio_defaults" => {
                let id = channel()?;
                Ok(put(
                    format!("/api/channel/{id}/restore"),
                    json!({"mode": "AudioDefault"}),
                ))
            }
            "acknowledge_channel_sorting" => {
                let id = channel()?;
                Ok(put(
                    format!("/api/channel/{id}/channelSorting"),
                    json!({"sorted": true}),
                ))
            }
            "set_frequency" => {
                let id = channel()?;
                let khz = stepped(params, "frequency_khz", 25, "25 kHz")?;
                Ok(put(
                    format!("/api/rf/channels/{id}/frequency"),
                    json!({"frequency": khz}),
                ))
            }
            "select_preset" => {
                let id = channel()?;
                let kind = match text(params, "bank_type")? {
                    "factory" => "Factory",
                    "user" => "User",
                    other => return Err(invalid(format!("unknown bank type '{other}'"))),
                };
                let bank = params.get("bank").and_then(Value::as_i64).unwrap_or(0);
                let preset = int(params, "preset")?;
                Ok(put(
                    format!("/api/rf/channels/{id}/preset"),
                    json!({"type": kind, "bank": bank, "channel": preset}),
                ))
            }
            "set_user_presets" => {
                let id = channel()?;
                let list = user_presets(params)?;
                Ok(put(
                    format!("/api/rf/presets/user/channels/{id}/banks/0"),
                    list,
                ))
            }
            "get_factory_presets" => {
                let bank = params.get("bank").and_then(Value::as_i64).unwrap_or(0);
                Ok(Call::get(format!("/api/rf/presets/factory/banks/{bank}")))
            }
            "set_link_density" => {
                let on = boolean(params, "enabled").ok_or_else(|| missing("enabled"))?;
                let mode = if on { "LinkDensity" } else { "Standard" };
                Ok(Call::put(
                    "/api/rf/transmission",
                    json!({ "mode": mode }),
                    SLOW_WRITE_TIMEOUT,
                ))
            }
            "set_encryption" => {
                let on = boolean(params, "enabled").ok_or_else(|| missing("enabled"))?;
                Ok(Call::put(
                    "/api/rf/encryption",
                    json!({ "enabled": on }),
                    SLOW_WRITE_TIMEOUT,
                ))
            }
            "set_transmitter_sync" => {
                let id = channel()?;
                Ok(put(format!("/api/syncSettings/{id}"), sync_body(params)?))
            }
            "set_transmitter_sync_ignore" => {
                let id = channel()?;
                let mut body = Map::new();
                for (param, field) in IGNORE_FIELDS {
                    if let Some(v) = boolean(params, param) {
                        body.insert(field.into(), json!(v));
                    }
                }
                if body.is_empty() {
                    return Err(invalid("give at least one setting to change"));
                }
                Ok(put(
                    format!("/api/syncSettings/{id}/ignore"),
                    Value::Object(body),
                ))
            }
            other => Err(CommandError::UnknownCommand {
                command: other.into(),
            }),
        }
    }
}

impl EwdxDevice {
    /// The API's 0-based id of the 1-based channel parameter.
    fn channel(&self, params: &Params) -> Result<i64, CommandError> {
        let channel = params.get("channel").and_then(Value::as_i64).unwrap_or(1);
        if channel < 1 || channel > self.channels as i64 {
            return Err(invalid(format!(
                "this model has {} channels",
                self.channels
            )));
        }
        Ok(channel - 1)
    }
}

fn put(path: impl Into<String>, body: Value) -> Call {
    Call::put(path, body, WRITE_TIMEOUT)
}

fn boolean(params: &Params, name: &str) -> Option<bool> {
    params.get(name).and_then(Value::as_bool)
}

fn int(params: &Params, name: &str) -> Result<i64, CommandError> {
    params
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| missing(name))
}

fn text<'a>(params: &'a Params, name: &str) -> Result<&'a str, CommandError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| missing(name))
}

/// An integer that must be a multiple of the API's step.
fn stepped(params: &Params, name: &str, step: i64, what: &str) -> Result<i64, CommandError> {
    let v = int(params, name)?;
    if v % step != 0 {
        return Err(invalid(format!("{name} must be in steps of {what}")));
    }
    Ok(v)
}

fn missing(name: &str) -> CommandError {
    invalid(format!("'{name}' is required"))
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

/// The user bank: 1 to 100 frequencies in kHz, each a multiple of 25.
fn user_presets(params: &Params) -> Result<Value, CommandError> {
    let list = params
        .get("frequencies_khz")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("frequencies_khz must be a JSON array of frequencies in kHz"))?;
    if list.is_empty() || list.len() > 100 {
        return Err(invalid("a user bank holds 1 to 100 frequencies"));
    }
    for f in list {
        match f.as_i64() {
            Some(khz) if khz > 0 && khz % 25 == 0 => {}
            _ => {
                return Err(invalid(format!(
                    "{f} is not a frequency in kHz in steps of 25 kHz"
                )))
            }
        }
    }
    Ok(Value::Array(list.clone()))
}

/// The sync settings the parameters change, in the API's words.
fn sync_body(params: &Params) -> Result<Value, CommandError> {
    let mut body = Map::new();
    let pick = |name: &str, allowed: &[&str]| -> Result<Option<&'static str>, CommandError> {
        let Some(v) = params.get(name).and_then(Value::as_str) else {
            return Ok(None);
        };
        WIRE_WORDS
            .iter()
            .find(|(ours, wire)| *ours == v && allowed.contains(wire))
            .map(|(_, wire)| Some(*wire))
            .ok_or_else(|| invalid(format!("unknown {name} '{v}'")))
    };
    if let Some(w) = pick("mute_config", &["Off", "RfMute", "AfMute"])? {
        body.insert("muteConfig".into(), json!(w));
    }
    if let Some(w) = pick("cable_emulation", &["Off", "Type1", "Type2", "Type3"])? {
        body.insert("cableEmulation".into(), json!(w));
    }
    if let Some(w) = pick("lowcut", &["Off", "30Hz", "60Hz", "80Hz", "100Hz", "120Hz"])? {
        body.insert("lowcut".into(), json!(w));
    }
    if let Some(v) = boolean(params, "lock") {
        body.insert("lock".into(), json!(v));
    }
    if let Some(v) = params.get("trim_db").and_then(Value::as_i64) {
        body.insert("trim".into(), json!(v));
    }
    if let Some(v) = boolean(params, "led") {
        body.insert("led".into(), json!(v));
    }
    if let Some(w) = pick("mute_config_table_stand", &["AfMute", "Off", "PTT", "PTM"])? {
        body.insert("muteConfigTs".into(), json!(w));
    }
    if body.is_empty() {
        return Err(invalid("give at least one setting to change"));
    }
    Ok(Value::Object(body))
}

/// The sync-ignore parameters and their API fields.
const IGNORE_FIELDS: [(&str, &str); 8] = [
    ("mute_config", "muteConfig"),
    ("cable_emulation", "cableEmulation"),
    ("lowcut", "lowcut"),
    ("lock", "lock"),
    ("trim", "trim"),
    ("led", "led"),
    ("name", "name"),
    ("frequency", "frequency"),
];

/// Our words for the API's enumeration values, both ways.
const WIRE_WORDS: [(&str, &str); 22] = [
    ("off", "Off"),
    ("rf_mute", "RfMute"),
    ("af_mute", "AfMute"),
    ("type1", "Type1"),
    ("type2", "Type2"),
    ("type3", "Type3"),
    ("30hz", "30Hz"),
    ("60hz", "60Hz"),
    ("80hz", "80Hz"),
    ("100hz", "100Hz"),
    ("120hz", "120Hz"),
    ("push_to_talk", "PTT"),
    ("push_to_mute", "PTM"),
    ("push_to_talk", "PushToTalk"),
    ("push_to_mute", "PushToMute"),
    ("battery", "Battery"),
    ("primary_cell", "PrimaryCell"),
    ("no_battery", "NoBattery"),
    ("normal", "Normal"),
    ("identifying", "Identifying"),
    ("firmware_update", "FirmwareUpdate"),
    ("busy", "Busy"),
];

/// An enumeration value in our words; one we do not know is kept as sent.
fn word(value: &Value) -> Option<Value> {
    let wire = value.as_str()?;
    let ours = WIRE_WORDS
        .iter()
        .find(|(_, w)| *w == wire)
        .map(|(o, _)| (*o).to_string())
        .unwrap_or_else(|| wire.to_string());
    Some(json!(ours))
}

/// Copy `from[src]` into `to[dst]`, converted, when present and convertible.
fn copy(
    to: &mut Map<String, Value>,
    dst: &str,
    from: &Value,
    src: &str,
    convert: fn(&Value) -> Option<Value>,
) {
    if let Some(v) = from.get(src).and_then(convert) {
        to.insert(dst.into(), v);
    }
}

fn as_bool(v: &Value) -> Option<Value> {
    v.as_bool().map(Value::Bool)
}

fn as_number(v: &Value) -> Option<Value> {
    v.is_number().then(|| v.clone())
}

fn as_string(v: &Value) -> Option<Value> {
    v.is_string().then(|| v.clone())
}

fn as_array(v: &Value) -> Option<Value> {
    v.is_array().then(|| v.clone())
}

/// The first element of a one-element list (`mates`, `audio`).
fn first_string(v: &Value) -> Option<Value> {
    v.as_array()?.first().filter(|f| f.is_string()).cloned()
}

/// The transmitter fields shared by `/api/transmitters` and its items.
fn transmitter_fields(value: &Value, tx: &mut Map<String, Value>) {
    copy(tx, "type", value, "type", as_string);
    copy(tx, "capsule", value, "capsule", as_string);
    copy(tx, "version", value, "version", as_string);
    copy(tx, "name", value, "name", as_string);
    copy(tx, "mute", value, "mute", as_bool);
    copy(tx, "identifying", value, "identification", as_bool);
    copy(tx, "lowcut", value, "lowcut", word);
    copy(tx, "trim_db", value, "trim", as_number);
    copy(tx, "cable_emulation", value, "cableEmulation", word);
    copy(tx, "led", value, "led", as_bool);
    copy(tx, "lock", value, "lock", as_bool);
    copy(tx, "mute_config", value, "muteConfig", word);
    copy(
        tx,
        "mute_config_table_stand",
        value,
        "muteConfigTableStand",
        word,
    );
}

/// Map one device-wide resource onto the state tree.
fn apply_device(path: &str, value: &Value, patch: &mut Map<String, Value>) {
    if path == "/api/device/identity" {
        object(patch, "device").insert("identity".into(), value.clone());
        return;
    }
    let device = object(patch, "device");
    match path {
        "/api/device/site" => {
            copy(device, "name", value, "deviceName", as_string);
            copy(device, "location", value, "location", as_string);
        }
        "/api/device/state" => {
            copy(device, "state", value, "state", word);
            copy(device, "warnings", value, "warnings", as_array);
        }
        "/api/device/identification" => copy(device, "identifying", value, "visual", as_bool),
        "/api/rf" => {
            let rf = object(device, "rf");
            copy(rf, "code", value, "code", as_string);
            if let Some(ranges) = value.get("ranges").and_then(Value::as_array) {
                let ranges: Vec<Value> = ranges
                    .iter()
                    .map(|r| {
                        // The document's example spells it stepsize, its
                        // schema stepSize.
                        let step = r.get("stepSize").or_else(|| r.get("stepsize"));
                        json!({"start_khz": r.get("start"), "end_khz": r.get("end"), "step_khz": step})
                    })
                    .collect();
                rf.insert("ranges".into(), Value::Array(ranges));
            }
        }
        "/api/rf/transmission" => {
            if let Some(mode) = value.get("mode").and_then(Value::as_str) {
                device.insert("link_density".into(), json!(mode == "LinkDensity"));
            }
            copy(
                device,
                "preset_spacing_khz",
                value,
                "presetSpacing",
                as_number,
            );
        }
        "/api/rf/encryption" => copy(device, "encryption", value, "enabled", as_bool),
        "/api/ssc/legacyMode" => copy(device, "legacy_mode", value, "enabled", as_bool),
        "/api/firmware/update/state" => {
            let fw = object(device, "firmware");
            copy(fw, "version", value, "deviceVersion", as_string);
            copy(fw, "dante_version", value, "danteVersion", as_string);
            if let Some(s) = value.get("state").and_then(Value::as_str) {
                fw.insert("update_state".into(), json!(s.to_ascii_lowercase()));
            }
            copy(fw, "update_progress_pct", value, "progress", as_number);
            copy(fw, "last_status", value, "lastStatus", as_string);
        }
        _ => {}
    }
}

/// Map one resource onto the state tree. Channels are 1-based in the state and
/// 0-based in the API.
fn apply_resource(path: &str, value: &Value, patch: &mut Map<String, Value>) {
    apply_one(path, value, patch);
    prune(patch);
}

fn apply_one(path: &str, value: &Value, patch: &mut Map<String, Value>) {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let (id, field): (Option<u32>, &str) = match parts.as_slice() {
        ["api", "channel", id] => (id.parse().ok(), "channel"),
        ["api", "channel", id, field] => (id.parse().ok(), field),
        ["api", "rf", "channels", id] => (id.parse().ok(), "rf"),
        ["api", "rf", "presets", "user", "channels", id, "banks", "0"] => {
            (id.parse().ok(), "user_presets")
        }
        ["api", "syncSettings", id] => (id.parse().ok(), "sync"),
        ["api", "syncSettings", id, "ignore"] => (id.parse().ok(), "sync_ignore"),
        ["api", "transmitters", id] => (id.parse().ok(), "transmitter"),
        ["api", "transmitters", id, "battery"] => (id.parse().ok(), "battery"),
        ["api", "transmitters", id, "warnings"] => (id.parse().ok(), "tx_warnings"),
        // The transmitter list: every linked channel's transmitter.
        ["api", "transmitters"] => {
            for item in value.as_array().into_iter().flatten() {
                if let (Some(id), Some(tx)) = (
                    item.get("id").and_then(Value::as_u64),
                    item.get("transmitter"),
                ) {
                    apply_one(&format!("/api/transmitters/{id}"), tx, patch);
                }
            }
            return;
        }
        _ => {
            apply_device(path, value, patch);
            return;
        }
    };
    let Some(id) = id else { return };
    let channel = object(object(patch, "channels"), &(id + 1).to_string());

    match field {
        "channel" => {
            copy(channel, "name", value, "name", as_string);
            copy(channel, "mute", value, "mute", as_bool);
            copy(channel, "gain_db", value, "gain", as_number);
            copy(channel, "af_out_db", value, "outputLevel", as_number);
            copy(channel, "mate", value, "mates", first_string);
            copy(channel, "output", value, "audio", first_string);
            copy(
                channel,
                "sorting_event_counter",
                value,
                "eventCounter",
                as_number,
            );
        }
        "identify" => copy(channel, "identifying", value, "enabled", as_bool),
        "signalQualityIndicator" => copy(
            object(channel, "rf"),
            "quality_pct",
            value,
            "value",
            as_number,
        ),
        "signalStrengthIndicator" => copy(
            object(channel, "rf"),
            "strength_dbm",
            value,
            "value",
            as_number,
        ),
        "diversityIndicator" => {
            let antenna = match value.get("value").and_then(Value::as_i64) {
                Some(0) => "none",
                Some(1) => "a",
                Some(2) => "b",
                _ => return,
            };
            object(channel, "rf").insert("antenna".into(), json!(antenna));
        }
        "level" => copy(
            object(channel, "af"),
            "level_dbfs",
            value,
            "value",
            as_number,
        ),
        "warnings" if value.is_array() => {
            channel.insert("warnings".into(), value.clone());
        }
        "rf" => {
            copy(channel, "frequency_khz", value, "frequency", as_number);
            if let Some(presets) = value.get("presets").filter(|p| p.is_object()) {
                let preset = object(channel, "preset");
                if let Some(t) = presets.get("type").and_then(Value::as_str) {
                    preset.insert("type".into(), json!(t.to_ascii_lowercase()));
                    // Bank and channel are left out when the type is None.
                    preset.insert(
                        "bank".into(),
                        presets.get("bank").cloned().unwrap_or(Value::Null),
                    );
                    preset.insert(
                        "channel".into(),
                        presets.get("channel").cloned().unwrap_or(Value::Null),
                    );
                }
            }
        }
        "user_presets" if value.is_array() => {
            channel.insert("user_presets_khz".into(), value.clone());
        }
        "sync" => {
            let sync = object(channel, "sync");
            copy(sync, "mute_config", value, "muteConfig", word);
            copy(sync, "cable_emulation", value, "cableEmulation", word);
            copy(sync, "lowcut", value, "lowcut", word);
            copy(sync, "lock", value, "lock", as_bool);
            copy(sync, "trim_db", value, "trim", as_number);
            copy(sync, "led", value, "led", as_bool);
            copy(sync, "mute_config_table_stand", value, "muteConfigTs", word);
        }
        "sync_ignore" => {
            let ignore = object(object(channel, "sync"), "ignore");
            for (param, field) in IGNORE_FIELDS {
                copy(ignore, param, value, field, as_bool);
            }
        }
        "transmitter" => transmitter_fields(value, object(channel, "transmitter")),
        "battery" => {
            let tx = object(channel, "transmitter");
            copy(tx, "battery_percent", value, "gauge", as_number);
            copy(tx, "battery_type", value, "type", word);
            copy(tx, "battery_lifetime_min", value, "lifetime", as_number);
        }
        "tx_warnings" if value.is_array() => {
            object(channel, "transmitter").insert("warnings".into(), value.clone());
        }
        _ => {}
    }
}

/// Remove the empty objects a resource carrying nothing we keep left behind.
/// A null is kept: it is a removal.
fn prune(map: &mut Map<String, Value>) {
    map.retain(|_, v| match v {
        Value::Object(child) => {
            prune(child);
            !child.is_empty()
        }
        _ => true,
    });
}

#[cfg(test)]
mod tests {
    use super::super::sscv2::{LIVENESS, QUIET_AFTER, RETRY, RETRY_AFTER, STREAM};
    use super::*;
    use crate::module::{
        Action, CommandId, CommandResult, Connection, HttpRequest, HttpResponse, Module, Outcome,
        RequestId, SseInput,
    };
    use crate::sse::SseEvent;
    use std::net::Ipv4Addr;

    fn device() -> Ewdx {
        Ewdx::for_device(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)), 443, "secret", 2)
    }

    fn requests(actions: &[Action]) -> Vec<(RequestId, HttpRequest)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Http { id, request } => Some((*id, request.clone())),
                _ => None,
            })
            .collect()
    }

    fn ok(body: Value) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            headers: Vec::new(),
            status: 200,
            body: body.to_string().into_bytes(),
        })
    }

    fn status(code: u16) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            headers: Vec::new(),
            status: code,
            body: Vec::new(),
        })
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

    /// Start, answer the probe, open the stream and deliver the session id.
    fn streaming() -> (Ewdx, Vec<Action>) {
        let mut d = device();
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(
            &mut cx,
            probe,
            ok(json!({"protocol": "2.0", "schema": "1.5"})),
        );
        cx.take();
        let mut cx = Cx::new(20);
        d.sse(&mut cx, STREAM, SseInput::Opened);
        d.sse(
            &mut cx,
            STREAM,
            SseInput::Event(SseEvent {
                event: "open".into(),
                data: json!({"path": "/api/ssc/state/subscriptions/abc", "sessionUUID": "abc"})
                    .to_string(),
            }),
        );
        let actions = cx.take();
        (d, actions)
    }

    #[test]
    fn requests_are_authenticated_as_api() {
        let mut d = device();
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let (_, probe) = requests(&cx.take()).remove(0);
        assert_eq!(probe.url, "https://10.0.0.5:443/api/ssc/version");
        // base64("api:secret")
        assert!(probe
            .headers
            .contains(&("Authorization".into(), "Basic YXBpOnNlY3JldA==".into())));
        assert!(probe.accept_invalid_certs);
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn unauthorized(actions: &[Action]) -> bool {
        actions
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. })))
    }

    fn sends_anything(actions: &[Action]) -> bool {
        actions.iter().any(|a| {
            matches!(
                a,
                Action::Http { .. } | Action::SseOpen { .. } | Action::SetTimer { .. }
            )
        })
    }

    /// Commands after a refusal fail with Auth and send nothing.
    fn assert_refused(d: &mut Ewdx) {
        let mut cx = Cx::new(90_000);
        d.command(
            &mut cx,
            7,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        let a = cx.take();
        assert!(!sends_anything(&a));
        assert!(matches!(
            completed(&a)[0],
            (7, Err(CommandError::Auth { .. }))
        ));
    }

    #[test]
    fn a_rejected_password_is_terminal() {
        let mut d = device();
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(&mut cx, probe, status(401));
        let a = cx.take();
        assert!(unauthorized(&a));
        // No retry on any schedule: repeated failures can lock the device's
        // third-party access (RFDeck review item O).
        assert!(!sends_anything(&a));
        assert!(a.contains(&Action::CancelTimer { key: RETRY }));
        // A stray retry timer does nothing either.
        let mut cx = Cx::new(60_000);
        d.timer(&mut cx, RETRY);
        assert!(!sends_anything(&cx.take()));
        assert_refused(&mut d);
    }

    #[test]
    fn a_refusal_on_the_stream_is_terminal() {
        for code in [401, 403] {
            let (mut d, _) = streaming();
            let mut cx = Cx::new(30);
            d.sse(
                &mut cx,
                STREAM,
                SseInput::Closed {
                    status: Some(code),
                    reason: format!("HTTP {code}"),
                },
            );
            let a = cx.take();
            assert!(unauthorized(&a), "HTTP {code}");
            assert!(!sends_anything(&a), "HTTP {code}");
            assert_refused(&mut d);
        }
    }

    #[test]
    fn a_refused_liveness_check_is_not_proof_of_life() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(20 + QUIET_AFTER + 1);
        d.timer(&mut cx, LIVENESS);
        let (check, _) = requests(&cx.take())
            .into_iter()
            .find(|(_, r)| r.url.ends_with("/api/ssc/version"))
            .unwrap();
        let mut cx = Cx::new(20 + QUIET_AFTER + 2);
        d.http_response(&mut cx, check, status(401));
        let a = cx.take();
        assert!(!a.contains(&Action::Alive));
        assert!(unauthorized(&a));
        assert!(!sends_anything(&a));
        assert_refused(&mut d);
    }

    #[test]
    fn a_refused_subscription_or_fetch_is_terminal() {
        let (mut d, actions) = streaming();
        let reqs = requests(&actions);
        let (subscribe, _) = reqs.iter().find(|(_, r)| r.method == "PUT").unwrap();
        let (fetch, _) = reqs.iter().find(|(_, r)| r.method == "GET").unwrap();
        let mut cx = Cx::new(30);
        d.http_response(&mut cx, *subscribe, status(403));
        let a = cx.take();
        assert!(unauthorized(&a));
        assert!(a.contains(&Action::SseClose { stream: STREAM }));
        assert!(!sends_anything(&a));
        // Responses still in flight report nothing further.
        let mut cx = Cx::new(31);
        d.http_response(&mut cx, *fetch, status(401));
        assert!(cx.take().is_empty());
        assert_refused(&mut d);
    }

    #[test]
    fn a_mute_refused_mid_session_fails_with_auth_and_stops() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(50);
        d.command(
            &mut cx,
            3,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        let (put, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(60);
        d.http_response(&mut cx, put, status(401));
        let a = cx.take();
        assert!(matches!(
            completed(&a)[0],
            (3, Err(CommandError::Auth { .. }))
        ));
        assert!(unauthorized(&a));
        assert_refused(&mut d);
    }

    #[test]
    fn subscribes_every_channel_in_batches_after_the_session_id() {
        let (mut d, actions) = streaming();
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        let reqs = requests(&actions);
        let (first_id, first) = &reqs[0];
        assert_eq!(first.method, "PUT");
        assert_eq!(
            first.url,
            "https://10.0.0.5:443/api/ssc/state/subscriptions/abc/add"
        );
        let body: Value = serde_json::from_slice(first.body.as_ref().unwrap()).unwrap();
        assert_eq!(body.as_array().unwrap().len(), 4);

        // The first three batches are the six resources tested on hardware,
        // per channel.
        assert_eq!(
            body,
            json!([
                "/api/channel/0",
                "/api/channel/0/signalQualityIndicator",
                "/api/channel/0/level",
                "/api/channel/0/warnings"
            ])
        );

        // The next batch follows the first's answer, refused or not, until all
        // 36 paths (six tested and eight more per channel, two channels, and
        // eight of the device) are subscribed.
        let mut id = *first_id;
        let mut total = 4;
        let mut answer = 0;
        loop {
            let mut cx = Cx::new(30);
            // A firmware lacking a resource refuses its batch; the rest go on.
            answer += 1;
            let code = if answer == 5 { 400 } else { 200 };
            d.http_response(&mut cx, id, status(code));
            let next: Vec<_> = requests(&cx.take())
                .into_iter()
                .filter(|(_, r)| r.method == "PUT")
                .collect();
            let Some((next_id, req)) = next.into_iter().next() else {
                break;
            };
            let body: Value = serde_json::from_slice(req.body.as_ref().unwrap()).unwrap();
            total += body.as_array().unwrap().len();
            id = next_id;
        }
        assert_eq!(total, 36);
    }

    fn patch_of(d: &Ewdx, data: Value) -> Value {
        let mut cx = Cx::new(40);
        d.apply(&mut cx, &data);
        cx.take()
            .into_iter()
            .find_map(|a| match a {
                Action::State(p) => Some(p),
                _ => None,
            })
            .unwrap_or(Value::Null)
    }

    #[test]
    fn channel_settings_and_rf_detail_map_onto_channels() {
        let (d, _) = streaming();
        let patch = patch_of(
            &d,
            json!({
                "/api/channel/0": {"name": "VOX", "mute": true, "gain": 12, "outputLevel": -6,
                                   "mates": ["TX1"], "audio": ["Out1"], "eventCounter": 3},
                "/api/channel/0/identify": {"enabled": true},
                "/api/channel/0/signalStrengthIndicator": {"value": -61.5},
                "/api/channel/0/signalQualityIndicator": {"value": 97},
                "/api/channel/0/diversityIndicator": {"value": 2},
                "/api/channel/0/warnings": ["LowSignal"],
                "/api/rf/channels/0": {"frequency": 520000,
                                       "presets": {"type": "User", "bank": 0, "channel": 4}},
                "/api/rf/channels/1": {"frequency": 530000, "presets": {"type": "None"}},
                "/api/rf/presets/user/channels/0/banks/0": [549200, 549800],
            }),
        );
        let ch = &patch["channels"]["1"];
        assert_eq!(ch["name"], "VOX");
        assert_eq!(ch["gain_db"], 12);
        assert_eq!(ch["af_out_db"], -6);
        assert_eq!(ch["mate"], "TX1");
        assert_eq!(ch["output"], "Out1");
        assert_eq!(ch["sorting_event_counter"], 3);
        assert_eq!(ch["identifying"], true);
        // Several meters of one channel in one notification all arrive.
        assert_eq!(
            ch["rf"],
            json!({"strength_dbm": -61.5, "quality_pct": 97, "antenna": "b"})
        );
        assert_eq!(ch["warnings"], json!(["LowSignal"]));
        assert_eq!(ch["frequency_khz"], 520000);
        assert_eq!(
            ch["preset"],
            json!({"type": "user", "bank": 0, "channel": 4})
        );
        assert_eq!(ch["user_presets_khz"], json!([549200, 549800]));
        // No preset: bank and channel are cleared, not left stale.
        assert_eq!(
            patch["channels"]["2"]["preset"],
            json!({"type": "none", "bank": null, "channel": null})
        );
    }

    #[test]
    fn sync_settings_and_transmitter_detail_map_onto_channels() {
        let (d, _) = streaming();
        let patch = patch_of(
            &d,
            json!({
                "/api/syncSettings/1": {"muteConfig": "RfMute", "cableEmulation": "Type2",
                    "lowcut": "80Hz", "lock": true, "trim": -3, "led": false, "muteConfigTs": "PTT"},
                "/api/syncSettings/1/ignore": {"name": true, "frequency": false},
                "/api/transmitters/1": {"type": "EW_DX_SKM", "capsule": "MMD_935",
                    "version": "3.1.0", "name": "LEAD", "mute": false, "identification": false,
                    "lowcut": "100Hz", "trim": 2, "led": true, "lock": false},
                "/api/transmitters/1/battery": {"type": "PrimaryCell", "lifetime": 312, "gauge": 80},
                "/api/transmitters/1/warnings": ["LowBattery"],
            }),
        );
        let ch = &patch["channels"]["2"];
        assert_eq!(
            ch["sync"],
            json!({"mute_config": "rf_mute", "cable_emulation": "type2", "lowcut": "80hz",
                   "lock": true, "trim_db": -3, "led": false, "mute_config_table_stand": "push_to_talk",
                   "ignore": {"name": true, "frequency": false}})
        );
        assert_eq!(
            ch["transmitter"],
            json!({"type": "EW_DX_SKM", "capsule": "MMD_935", "version": "3.1.0", "name": "LEAD",
                   "mute": false, "identifying": false, "lowcut": "100hz", "trim_db": 2,
                   "led": true, "lock": false, "battery_percent": 80, "battery_type": "primary_cell",
                   "battery_lifetime_min": 312, "warnings": ["LowBattery"]})
        );
    }

    #[test]
    fn device_resources_map_onto_device() {
        let (d, _) = streaming();
        let patch = patch_of(
            &d,
            json!({
                "/api/device/site": {"deviceName": "EWDXEM2", "location": "Stage left", "position": ""},
                "/api/device/state": {"state": "Busy", "warnings": []},
                "/api/device/identification": {"visual": false},
                "/api/rf": {"code": "Q1-9", "ranges": [{"start": 470200, "end": 550000, "stepSize": 25}]},
                "/api/rf/transmission": {"mode": "LinkDensity", "presetSpacing": 300},
                "/api/rf/encryption": {"enabled": true},
                "/api/ssc/legacyMode": {"enabled": false},
                "/api/firmware/update/state": {"deviceVersion": "4.1.0", "danteVersion": "4.2.5",
                    "state": "Idle", "progress": 0, "lastStatus": "None"},
            }),
        );
        assert_eq!(
            patch["device"],
            json!({
                "name": "EWDXEM2", "location": "Stage left", "state": "busy", "warnings": [],
                "identifying": false,
                "rf": {"code": "Q1-9", "ranges": [{"start_khz": 470200, "end_khz": 550000, "step_khz": 25}]},
                "link_density": true, "preset_spacing_khz": 300, "encryption": true,
                "legacy_mode": false,
                "firmware": {"version": "4.1.0", "dante_version": "4.2.5", "update_state": "idle",
                             "update_progress_pct": 0, "last_status": "None"},
            })
        );
    }

    #[test]
    fn an_unlinked_transmitter_is_null() {
        // The read of channel 2's transmitter answers 422.
        let (mut d, actions) = streaming();
        let reads = requests(&actions);
        let (id, _) = reads
            .iter()
            .find(|(_, r)| r.method == "GET" && r.url.ends_with("/api/transmitters/1"))
            .unwrap();
        let mut cx = Cx::new(60);
        d.http_response(&mut cx, *id, status(422));
        let a = cx.take();
        assert!(a.contains(&Action::State(
            json!({"channels": {"2": {"transmitter": null}}})
        )));
    }

    /// Run a command on a streaming device and return its request.
    fn sent(name: &str, p: Value) -> Result<HttpRequest, CommandError> {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(50);
        d.command(&mut cx, 1, name, &params(p));
        let a = cx.take();
        if let Some((_, Err(e))) = completed(&a).into_iter().next() {
            return Err(e);
        }
        Ok(requests(&a).remove(0).1)
    }

    fn body(r: &HttpRequest) -> Value {
        serde_json::from_slice(r.body.as_ref().unwrap()).unwrap()
    }

    #[test]
    fn commands_write_the_documented_resources() {
        let cases = [
            (
                "set_channel_name",
                json!({"channel": 2, "name": "VOX 1"}),
                "PUT",
                "/api/channel/1",
                json!({"name": "VOX 1"}),
            ),
            (
                "set_gain",
                json!({"channel": 1, "gain_db": 21}),
                "PUT",
                "/api/channel/0",
                json!({"gain": 21}),
            ),
            (
                "set_af_out",
                json!({"channel": 1, "level_db": -12}),
                "PUT",
                "/api/channel/0",
                json!({"outputLevel": -12}),
            ),
            (
                "identify",
                json!({"channel": 2}),
                "PUT",
                "/api/channel/1/identify",
                json!({"enabled": true}),
            ),
            (
                "identify",
                json!({"channel": 2, "enabled": false}),
                "PUT",
                "/api/channel/1/identify",
                json!({"enabled": false}),
            ),
            (
                "identify_device",
                json!({}),
                "PUT",
                "/api/device/identification",
                json!({"visual": true}),
            ),
            (
                "restore_channel_audio_defaults",
                json!({"channel": 1}),
                "PUT",
                "/api/channel/0/restore",
                json!({"mode": "AudioDefault"}),
            ),
            (
                "acknowledge_channel_sorting",
                json!({"channel": 1}),
                "PUT",
                "/api/channel/0/channelSorting",
                json!({"sorted": true}),
            ),
            (
                "set_frequency",
                json!({"channel": 2, "frequency_khz": 606525}),
                "PUT",
                "/api/rf/channels/1/frequency",
                json!({"frequency": 606525}),
            ),
            (
                "select_preset",
                json!({"channel": 1, "bank_type": "factory", "preset": 7}),
                "PUT",
                "/api/rf/channels/0/preset",
                json!({"type": "Factory", "bank": 0, "channel": 7}),
            ),
            (
                "set_user_presets",
                json!({"channel": 1, "frequencies_khz": [549200, 549800]}),
                "PUT",
                "/api/rf/presets/user/channels/0/banks/0",
                json!([549200, 549800]),
            ),
            (
                "set_link_density",
                json!({"enabled": true}),
                "PUT",
                "/api/rf/transmission",
                json!({"mode": "LinkDensity"}),
            ),
            (
                "set_link_density",
                json!({"enabled": false}),
                "PUT",
                "/api/rf/transmission",
                json!({"mode": "Standard"}),
            ),
            (
                "set_encryption",
                json!({"enabled": true}),
                "PUT",
                "/api/rf/encryption",
                json!({"enabled": true}),
            ),
            (
                "set_transmitter_sync",
                json!({"channel": 1, "mute_config": "af_mute", "lowcut": "120hz", "trim_db": -12, "mute_config_table_stand": "push_to_mute"}),
                "PUT",
                "/api/syncSettings/0",
                json!({"muteConfig": "AfMute", "lowcut": "120Hz", "trim": -12, "muteConfigTs": "PTM"}),
            ),
            (
                "set_transmitter_sync",
                json!({"channel": 2, "cable_emulation": "type3", "lock": false, "led": true}),
                "PUT",
                "/api/syncSettings/1",
                json!({"cableEmulation": "Type3", "lock": false, "led": true}),
            ),
            (
                "set_transmitter_sync_ignore",
                json!({"channel": 1, "name": true, "frequency": false}),
                "PUT",
                "/api/syncSettings/0/ignore",
                json!({"name": true, "frequency": false}),
            ),
        ];
        for (name, p, method, path, want) in cases {
            let r = sent(name, p).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(r.method, method, "{name}");
            assert_eq!(r.url, format!("https://10.0.0.5:443{path}"), "{name}");
            assert_eq!(body(&r), want, "{name}");
        }
        let r = sent("get_factory_presets", json!({})).unwrap();
        assert_eq!(r.method, "GET");
        assert_eq!(r.url, "https://10.0.0.5:443/api/rf/presets/factory/banks/0");
    }

    #[test]
    fn values_off_the_api_step_are_refused_before_sending() {
        for (name, p) in [
            ("set_gain", json!({"channel": 1, "gain_db": 4})),
            ("set_af_out", json!({"channel": 1, "level_db": 3})),
            (
                "set_frequency",
                json!({"channel": 1, "frequency_khz": 606510}),
            ),
            (
                "set_user_presets",
                json!({"channel": 1, "frequencies_khz": [549210]}),
            ),
            (
                "set_user_presets",
                json!({"channel": 1, "frequencies_khz": []}),
            ),
            (
                "set_user_presets",
                json!({"channel": 1, "frequencies_khz": "549200"}),
            ),
            ("set_transmitter_sync", json!({"channel": 1})),
            ("set_transmitter_sync_ignore", json!({"channel": 1})),
            ("mute", json!({"channel": 3})),
        ] {
            assert!(
                matches!(
                    sent(name, p.clone()),
                    Err(CommandError::InvalidParams { .. })
                ),
                "{name} {p}"
            );
        }
    }

    #[test]
    fn a_factory_bank_read_returns_the_list() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(50);
        d.command(
            &mut cx,
            5,
            "get_factory_presets",
            &params(json!({"bank": 0})),
        );
        let (id, _) = requests(&cx.take()).remove(0);
        let mut cx = Cx::new(60);
        d.http_response(&mut cx, id, ok(json!([470200, 471000])));
        assert_eq!(
            completed(&cx.take()),
            [(
                5,
                Ok(Outcome::Value {
                    value: json!([470200, 471000])
                })
            )]
        );
    }

    #[test]
    fn notifications_map_onto_channels() {
        let (d, _) = streaming();
        let mut cx = Cx::new(40);
        d.apply(
            &mut cx,
            &json!({
                "/api/channel/1": {"name": "Vox", "mute": false},
                "/api/channel/1/signalQualityIndicator": {"value": 87},
                "/api/channel/1/level": {"value": -42.5},
                "/api/rf/channels/1": {"frequency": 606500},
                "/api/transmitters/1/battery": {"gauge": 65},
            }),
        );
        let patch = cx.take().into_iter().find_map(|a| match a {
            Action::State(p) => Some(p),
            _ => None,
        });
        let ch = &patch.unwrap()["channels"]["2"];
        assert_eq!(ch["name"], "Vox");
        assert_eq!(ch["rf"]["quality_pct"], 87);
        assert_eq!(ch["af"]["level_dbfs"], -42.5);
        assert_eq!(ch["frequency_khz"], 606500);
        // A percentage, per EW-DX SSC §8.106 (RFDeck review item A).
        assert_eq!(ch["transmitter"]["battery_percent"], 65);
    }

    #[test]
    fn path_value_notifications_are_accepted() {
        let (d, _) = streaming();
        let mut cx = Cx::new(40);
        d.apply(
            &mut cx,
            &json!({"path": "/api/channel/0", "value": {"mute": true}}),
        );
        let patch = cx.take().into_iter().find_map(|a| match a {
            Action::State(p) => Some(p),
            _ => None,
        });
        assert_eq!(patch.unwrap()["channels"]["1"]["mute"], true);
    }

    #[test]
    fn mute_puts_the_channel_resource() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(50);
        let params = json!({"channel": 2, "muted": true})
            .as_object()
            .unwrap()
            .clone();
        d.command(&mut cx, 9, "mute", &params);
        let (id, req) = requests(&cx.take()).remove(0);
        assert_eq!(req.method, "PUT");
        assert_eq!(req.url, "https://10.0.0.5:443/api/channel/1");
        assert_eq!(req.body.as_deref(), Some(br#"{"mute":true}"#.as_slice()));

        let mut cx = Cx::new(60);
        d.http_response(&mut cx, id, status(200));
        assert_eq!(completed(&cx.take()), [(9, Ok(Outcome::Ack))]);
    }

    #[test]
    fn mute_failures_are_classified() {
        let (mut d, _) = streaming();
        let params = json!({"channel": 1}).as_object().unwrap().clone();
        let mut cx = Cx::new(50);
        d.command(&mut cx, 1, "mute", &params);
        d.command(&mut cx, 2, "mute", &params);
        let ids: Vec<_> = requests(&cx.take()).into_iter().map(|(i, _)| i).collect();
        let mut cx = Cx::new(60);
        d.http_response(&mut cx, ids[0], status(404));
        d.http_response(&mut cx, ids[1], Err("connection reset".into()));
        let done = completed(&cx.take());
        assert!(
            matches!(&done[0].1, Err(CommandError::DeviceError { code: Some(c), .. }) if c == "404")
        );
        assert!(matches!(&done[1].1, Err(CommandError::Transport { .. })));
    }

    #[test]
    fn a_quiet_stream_is_probed_and_dropped_after_two_failures() {
        let (mut d, _) = streaming();
        let mut failures = 0;
        for now in [3_100, 6_200] {
            let mut cx = Cx::new(now);
            d.timer(&mut cx, LIVENESS);
            let (id, req) = requests(&cx.take()).remove(0);
            assert!(req.url.ends_with("/api/ssc/version"));
            let mut cx = Cx::new(now + 10);
            d.http_response(&mut cx, id, Err("timed out".into()));
            failures += 1;
            let a = cx.take();
            let dropped = a.contains(&Action::SseClose { stream: STREAM });
            assert_eq!(dropped, failures == 2);
        }
    }

    #[test]
    fn opened_for_commands_only_it_opens_no_stream() {
        let mut d = device();
        d.monitor = false;
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(&mut cx, probe, ok(json!({"protocol": "2.0"})));
        let a = cx.take();
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // No stream, no subscription, no identity or state read.
        assert!(!a.iter().any(|x| matches!(x, Action::SseOpen { .. })));
        assert!(requests(&a).is_empty());

        // Quiet for 3 s: the version is asked as the liveness check.
        let mut cx = Cx::new(10 + QUIET_AFTER);
        d.timer(&mut cx, LIVENESS);
        let (check, req) = requests(&cx.take()).remove(0);
        assert!(req.url.ends_with("/api/ssc/version"));
        let mut cx = Cx::new(10 + QUIET_AFTER + 8);
        d.http_response(&mut cx, check, ok(json!({"protocol": "2.0"})));
        let a = cx.take();
        assert!(a.contains(&Action::Alive));
        assert!(a.contains(&Action::RoundTrip(8)));

        // Commands work.
        let mut cx = Cx::new(5_000);
        d.command(
            &mut cx,
            4,
            "mute",
            &params(json!({"channel": 1, "muted": true})),
        );
        let (put, req) = requests(&cx.take()).remove(0);
        assert_eq!(req.url, "https://10.0.0.5:443/api/channel/0");
        let mut cx = Cx::new(5_030);
        d.http_response(&mut cx, put, status(200));
        let a = cx.take();
        assert_eq!(completed(&a), [(4, Ok(Outcome::Ack))]);
        assert!(a.contains(&Action::RoundTrip(30)));

        // Losing it closes no stream, since none was opened.
        let mut cx = Cx::new(9_000);
        d.lost(&mut cx, "gone".into());
        assert!(!cx.take().contains(&Action::SseClose { stream: STREAM }));
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let mut d = device();
        let mut cx = Cx::new(100);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(140);
        d.http_response(&mut cx, probe, ok(json!({"protocol": "2.0"})));
        assert!(cx.take().contains(&Action::RoundTrip(40)));

        // A request that fails in transport was not answered.
        let (mut d, _) = streaming();
        let mut cx = Cx::new(50);
        d.command(&mut cx, 1, "mute", &params(json!({"channel": 1})));
        let (id, _) = requests(&cx.take())[0].clone();
        let mut cx = Cx::new(2_000);
        d.http_response(&mut cx, id, Err("timed out".into()));
        assert!(!cx.take().iter().any(|x| matches!(x, Action::RoundTrip(_))));
    }

    #[test]
    fn a_closed_stream_reconnects() {
        let (mut d, _) = streaming();
        let mut cx = Cx::new(70);
        d.sse(
            &mut cx,
            STREAM,
            SseInput::Closed {
                status: None,
                reason: "stream ended".into(),
            },
        );
        let a = cx.take();
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Disconnected { .. }))));
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_AFTER
        }));
        // The last known state stays; the connection status says it is stale.
        assert!(!a.iter().any(|x| matches!(x, Action::State(_))));
    }
}
