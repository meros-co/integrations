//! Sennheiser Spectera Base Station over SSCv2: HTTPS requests plus a
//! server-sent event stream, through the shared client in [`super::sscv2`].
//!
//! Resources, fields and ranges are Sennheiser's published Spectera OpenAPI
//! (API 18.1, Base Station firmware 1.4.1 and later). The Base Station's
//! collections (RF channels, antennas, audio links, inputs, outputs, paired
//! and pairable mobile devices) are SSCv2 control resources: subscribing to
//! the collection delivers the whole list first and then each changed,
//! created (`{}`) or deleted (`null`) item, keyed by the item's path or, for
//! the fixed lists, as the changed item alone on the collection's path.
//!
//! State mirrors the resources with their field names in snake_case, keyed
//! by the API's own ids (RF channels 0-1, antenna ports a-d, inputs and
//! outputs 0-31, audio link ids, mobile device mtUid). Meters (60 Hz when
//! subscribed) are polled at the `metering_poll_ms` interval instead, when
//! set.
//!
//! Spectera does not yet implement SSCv2's third-party user `api`; requests
//! authenticate as `controlSennheiser` with the device password, unless the
//! `username` setting says otherwise.
//!
//! Opened for commands only, it opens no subscription stream (so takes none
//! of the Base Station's connections) and reads nothing on connecting: the
//! version request that finds the device, repeated whenever it has been
//! quiet for 3 s, is the liveness check. A command to a mobile device needs
//! the device's type (SEK or SKM), which is then taken from the command's
//! `device_type` or read from the device first.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use super::sscv2::{object, Call, Returns, SscDevice, Sscv2, REQUEST_TIMEOUT};
use crate::catalog::Params;
use crate::module::{CommandError, Cx, Level, Millis, OpenContext};

const WRITE_TIMEOUT: Millis = 5_000;

/// The subscribed resources, in the order they are added.
const RESOURCES: &[&str] = &[
    "/api/device/identity",
    "/api/device/state",
    "/api/device/site",
    "/api/device/identification",
    "/api/device/entitlement",
    "/api/ssc/usagemark",
    "/api/health/psu",
    "/api/health/tempstateoverall",
    "/api/health/fan/FAN_1/errorstate",
    "/api/health/fan/FAN_2/errorstate",
    "/api/health/fan/FAN_3/errorstate",
    "/api/firmware/update/state",
    "/api/rf/channels",
    "/api/rf/antennas",
    "/api/rf/restrictions",
    "/api/audio/links",
    "/api/audio/inputs",
    "/api/audio/outputs",
    "/api/audio/pendingactions",
    "/api/audio/interface/aoip/status",
    "/api/audio/interface/madi1/status",
    "/api/audio/interface/madi2/status",
    "/api/audio/interface/wordclock/status",
    "/api/audio/interface/madi1/output",
    "/api/audio/interface/madi2/output",
    "/api/audio/interface/wordclock/output",
    "/api/mts/pairing",
    "/api/mts/pairable",
    "/api/mts/paired/all",
    "/api/mts/firmware/update/state",
];

const METERING: &str = "/api/audio/metering";

/// Collections: API path, state path, and the item's id field.
const COLLECTIONS: &[(&str, &[&str], &str)] = &[
    ("/api/rf/channels", &["rf", "channels"], "rfChannelId"),
    ("/api/rf/antennas", &["rf", "antennas"], "antennaPortId"),
    ("/api/audio/links", &["audio", "links"], "audiolinkId"),
    ("/api/audio/inputs", &["audio", "inputs"], "inputId"),
    ("/api/audio/outputs", &["audio", "outputs"], "outputId"),
    ("/api/mts/paired/all", &["mobile_devices"], "mtUid"),
    ("/api/mts/pairable", &["pairable_devices"], "mtUid"),
];

/// Collections whose items come and go; the others have a fixed set.
const DYNAMIC: &[&str] = &[
    "/api/audio/links",
    "/api/mts/paired/all",
    "/api/mts/pairable",
];

/// Single resources: API path and state path.
const SINGLES: &[(&str, &[&str])] = &[
    ("/api/device/identity", &["device", "identity"]),
    ("/api/device/state", &["device", "status"]),
    ("/api/device/site", &["device", "site"]),
    ("/api/device/identification", &["device", "identification"]),
    ("/api/device/entitlement", &["device", "entitlement"]),
    ("/api/device/time", &["device", "time"]),
    ("/api/ssc/usagemark", &["device", "usage_mark"]),
    ("/api/health/psu", &["health", "psu"]),
    ("/api/health/tempstateoverall", &["health", "temperature"]),
    ("/api/firmware/update/state", &["firmware", "base_station"]),
    (
        "/api/mts/firmware/update/state",
        &["firmware", "mobile_devices"],
    ),
    ("/api/rf/restrictions", &["rf", "restrictions"]),
    ("/api/audio/pendingactions", &["audio", "pending_actions"]),
    ("/api/audio/metering", &["audio", "metering"]),
    ("/api/mts/pairing", &["pairing"]),
];

/// Audio link modes: the operator's name and the API's modeId.
const MODES: &[(&str, i64)] = &[
    ("max_range", 1),
    ("max_link_density", 2),
    ("live_link_density_mono", 3),
    ("live_mono", 4),
    ("live_low_latency_mono", 5),
    ("live_link_density_stereo", 6),
    ("live_stereo", 7),
    ("live_low_latency_stereo", 8),
    ("live_ultra_low_latency_stereo", 9),
    ("raw", 10),
    ("raw_low_latency", 11),
    ("empty_mono", 1001),
    ("empty_stereo", 1002),
];

/// The Base Station's resources, state and commands.
pub(crate) struct SpecteraDevice {
    /// Meters are read at this interval when set.
    metering_poll: Option<Millis>,
    /// Ids known in each dynamic collection, so a full list read again can
    /// remove the ones that went while the device was not watched.
    known: RefCell<BTreeMap<&'static str, BTreeSet<String>>>,
    /// Each mobile device's type (SEK or SKM), which every write to it
    /// must name.
    types: RefCell<BTreeMap<String, String>>,
}

pub(crate) type Spectera = Sscv2<SpecteraDevice>;

impl Sscv2<SpecteraDevice> {
    pub(crate) fn from_context(ctx: OpenContext) -> Spectera {
        let setting = |name: &str| {
            ctx.settings
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let mut user = setting("username");
        if user.is_empty() {
            user = "controlSennheiser".into();
        }
        let metering_poll = ctx
            .settings
            .get("metering_poll_ms")
            .and_then(Value::as_u64)
            .filter(|ms| *ms > 0);
        let port = ctx.port.unwrap_or(443);
        let mut d = Sscv2::new_as(
            ctx.host,
            port,
            &user,
            &setting("password"),
            SpecteraDevice::new(metering_poll),
        );
        d.monitor = ctx.monitor;
        d
    }
}

impl SpecteraDevice {
    fn new(metering_poll: Option<Millis>) -> SpecteraDevice {
        SpecteraDevice {
            metering_poll,
            known: RefCell::new(BTreeMap::new()),
            types: RefCell::new(BTreeMap::new()),
        }
    }

    fn remember_type(&self, uid: &str, value: &Value) {
        if let Some(t) = value.get("type").and_then(Value::as_str) {
            self.types
                .borrow_mut()
                .insert(uid.to_string(), t.to_string());
        }
    }

    fn mobile_type(&self, uid: i64, params: &Params) -> Result<String, CommandError> {
        if let Some(t) = params.get("device_type").and_then(Value::as_str) {
            return Ok(t.to_string());
        }
        self.types
            .borrow()
            .get(&uid.to_string())
            .cloned()
            .ok_or_else(|| CommandError::InvalidParams {
                message: format!(
                    "mobile device {uid} is not known as paired yet: give device_type (SEK or SKM)"
                ),
            })
    }

    /// A PUT to a paired mobile device, naming its id and type as the API
    /// requires, with the given fields.
    fn mobile_put(&self, params: &Params, fields: Value) -> Result<Call, CommandError> {
        let uid = int(params, "mt_uid")?;
        let kind = self.mobile_type(uid, params)?;
        let mut body = json!({"mtUid": uid, "type": kind});
        if let (Some(body), Value::Object(fields)) = (body.as_object_mut(), fields) {
            body.extend(fields);
        }
        Ok(Call::put(
            format!("/api/mts/paired/all/{uid}"),
            body,
            WRITE_TIMEOUT,
        ))
    }

    fn require_type(&self, params: &Params, wanted: &str, what: &str) -> Result<(), CommandError> {
        let uid = int(params, "mt_uid")?;
        let kind = self.mobile_type(uid, params)?;
        if kind != wanted {
            return Err(CommandError::InvalidParams {
                message: format!(
                    "{what} exists only on an {wanted}; mobile device {uid} is an {kind}"
                ),
            });
        }
        Ok(())
    }

    /// Apply a collection's value: the whole list, or one changed item.
    fn apply_collection(
        &self,
        api: &'static str,
        at: &[&str],
        id_field: &str,
        value: &Value,
        patch: &mut Map<String, Value>,
    ) {
        let target = descend(patch, at);
        match value {
            Value::Array(items) => {
                let mut ids = BTreeSet::new();
                for item in items {
                    if let Some(id) = item.get(id_field).and_then(id_text) {
                        if api == "/api/mts/paired/all" {
                            self.remember_type(&id, item);
                        }
                        target.insert(id.clone(), snake(item));
                        ids.insert(id);
                    }
                }
                if DYNAMIC.contains(&api) {
                    let mut known = self.known.borrow_mut();
                    let before = known.insert(api, ids.clone()).unwrap_or_default();
                    for gone in before.difference(&ids) {
                        target.insert(gone.clone(), Value::Null);
                    }
                }
            }
            Value::Object(_) => {
                if let Some(id) = value.get(id_field).and_then(id_text) {
                    self.apply_item(api, &id, value, target);
                }
            }
            _ => {}
        }
    }

    /// Apply one item of a collection: created (`{}`), changed, or deleted
    /// (`null`).
    fn apply_item(
        &self,
        api: &'static str,
        id: &str,
        value: &Value,
        target: &mut Map<String, Value>,
    ) {
        if api == "/api/mts/paired/all" {
            self.remember_type(id, value);
        }
        if DYNAMIC.contains(&api) {
            let mut known = self.known.borrow_mut();
            let ids = known.entry(api).or_default();
            if value.is_null() {
                ids.remove(id);
                if api == "/api/mts/paired/all" {
                    self.types.borrow_mut().remove(id);
                }
            } else {
                ids.insert(id.to_string());
            }
        }
        target.insert(id.to_string(), snake(value));
    }
}

/// An id as a state key: a number or a string.
fn id_text(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// The object at a path of keys under the patch, created as needed.
fn descend<'a>(patch: &'a mut Map<String, Value>, at: &[&str]) -> &'a mut Map<String, Value> {
    let mut node = patch;
    for key in at {
        node = object(node, key);
    }
    node
}

/// camelCase to snake_case: `rfChannelId` is `rf_channel_id`, `aoIpIn` is
/// `ao_ip_in`, `madi1In` is `madi1_in`.
fn snake_key(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for (i, c) in key.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A resource's value with every object key in snake_case.
fn snake(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (snake_key(k), snake(v))).collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(snake).collect()),
        other => other.clone(),
    }
}

fn int(params: &Params, name: &str) -> Result<i64, CommandError> {
    params
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| missing(name))
}

fn float(params: &Params, name: &str) -> Result<f64, CommandError> {
    params
        .get(name)
        .and_then(Value::as_f64)
        .ok_or_else(|| missing(name))
}

fn boolean(params: &Params, name: &str, default: bool) -> bool {
    params.get(name).and_then(Value::as_bool).unwrap_or(default)
}

fn text<'a>(params: &'a Params, name: &str) -> Result<&'a str, CommandError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| missing(name))
}

fn missing(name: &str) -> CommandError {
    CommandError::InvalidParams {
        message: format!("'{name}' is required"),
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn mode_id(params: &Params) -> Result<i64, CommandError> {
    let name = text(params, "mode")?;
    MODES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, id)| *id)
        .ok_or_else(|| invalid(format!("unknown audio link mode '{name}'")))
}

/// The API's name for an audio interface in output field names.
fn interface_field(interface: &str) -> Result<&'static str, CommandError> {
    match interface {
        "aoip" => Ok("aoIp"),
        "madi1" => Ok("madi1"),
        "madi2" => Ok("madi2"),
        other => Err(invalid(format!("unknown interface '{other}'"))),
    }
}

/// A frequency in kHz must be a whole MHz (multipleOf 1000 in the API).
fn frequency(params: &Params) -> Result<i64, CommandError> {
    let khz = int(params, "frequency_khz")?;
    if khz % 1000 != 0 {
        return Err(invalid(
            "the frequency must be a whole number of MHz (a multiple of 1000 kHz)",
        ));
    }
    Ok(khz)
}

impl SscDevice for SpecteraDevice {
    fn resources(&self) -> Vec<String> {
        RESOURCES.iter().map(|p| p.to_string()).collect()
    }

    /// Subscribing delivers each resource's current value at once (SSCv2
    /// 2.3), so nothing is read separately but the identity.
    fn initial_reads(&self) -> Vec<String> {
        Vec::new()
    }

    /// API 17.0 (firmware 1.3.x) lacks some resources of 18.1; the rest are
    /// still subscribed.
    fn subscribe_past_refusals(&self) -> bool {
        true
    }

    fn poll(&self) -> Option<(Vec<String>, Millis)> {
        self.metering_poll
            .map(|every| (vec![METERING.to_string()], every))
    }

    fn apply_resource(&self, path: &str, value: &Value, patch: &mut Map<String, Value>) {
        for (api, at) in SINGLES {
            if path == *api {
                let target = descend(patch, &at[..at.len() - 1]);
                target.insert(at[at.len() - 1].to_string(), snake(value));
                return;
            }
        }
        for (api, at, id_field) in COLLECTIONS {
            if path == *api {
                self.apply_collection(api, at, id_field, value, patch);
                return;
            }
            if let Some(id) = path
                .strip_prefix(api)
                .and_then(|rest| rest.strip_prefix('/'))
                .filter(|id| !id.is_empty() && !id.contains('/'))
            {
                let target = descend(patch, at);
                self.apply_item(api, id, value, target);
                return;
            }
        }
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match parts.as_slice() {
            ["api", "health", "fan", fan, "errorstate"] => {
                descend(patch, &["health", "fans"]).insert(fan.to_string(), snake(value));
            }
            ["api", "audio", "interface", interface, "status"] => {
                descend(patch, &["audio", "interfaces", interface])
                    .insert("status".into(), snake(value));
            }
            ["api", "audio", "interface", interface, "output"] => {
                descend(patch, &["audio", "interfaces", interface])
                    .insert("output".into(), snake(value));
            }
            ["api", "rf", "scan", antenna, sub, kind @ ("config" | "results")] => {
                descend(patch, &["rf", "scan", antenna, sub])
                    .insert(kind.to_string(), snake(value));
            }
            _ => {}
        }
    }

    fn read_failed(&self, cx: &mut Cx, path: &str, status: u16) {
        // Spectera answers an unknown id with 422, not 404.
        if status == 422 || status == 404 {
            cx.log(
                Level::Warning,
                format!("{path} does not exist on this Base Station"),
            );
        }
    }

    fn command(&self, name: &str, p: &Params) -> Result<Call, CommandError> {
        let put = |path: String, body: Value| Ok(Call::put(path, body, WRITE_TIMEOUT));
        let rf = |p: &Params| int(p, "rf_channel");
        match name {
            // Base Station
            "identify" => put(
                "/api/device/identification".into(),
                json!({"visual": boolean(p, "enabled", true)}),
            ),
            "set_usage_mark" => put(
                "/api/ssc/usagemark".into(),
                json!({"usedBy": text(p, "used_by")?, "timer": int(p, "timer_s")?}),
            ),
            "clear_firmware_update_status" => {
                Ok(bodiless("PUT", "/api/firmware/update/state/clear"))
            }

            // RF channels
            "set_rf_active" => {
                let id = rf(p)?;
                let state = if boolean(p, "active", true) {
                    "RfActive"
                } else {
                    "RfMuted"
                };
                put(
                    format!("/api/rf/channels/{id}"),
                    json!({"rfChannelId": id, "rfState": state}),
                )
            }
            "set_rf_frequency" => {
                let id = rf(p)?;
                put(
                    format!("/api/rf/channels/{id}"),
                    json!({"rfChannelId": id, "frequency": frequency(p)?}),
                )
            }
            "set_rf_tx_power" => {
                let id = rf(p)?;
                let mw: i64 = text(p, "power_mw")?
                    .parse()
                    .map_err(|_| invalid("power_mw is one of 10, 20, 30, 50, 100"))?;
                put(
                    format!("/api/rf/channels/{id}"),
                    json!({"rfChannelId": id, "txPower": mw}),
                )
            }
            "set_rf_bandwidth" => {
                let id = rf(p)?;
                let khz: i64 = text(p, "bandwidth_khz")?
                    .parse()
                    .map_err(|_| invalid("bandwidth_khz is 6000 or 8000"))?;
                put(
                    format!("/api/rf/channels/{id}"),
                    json!({"rfChannelId": id, "bandwidthMode": khz}),
                )
            }
            "set_rf_state_on_startup" => {
                let id = rf(p)?;
                let state = match text(p, "state")? {
                    "active" => "RfActive",
                    "muted" => "RfMuted",
                    "last" => "RfLastState",
                    other => return Err(invalid(format!("unknown startup state '{other}'"))),
                };
                put(
                    format!("/api/rf/channels/{id}"),
                    json!({"rfChannelId": id, "rfStateOnStartup": state}),
                )
            }

            // Antennas
            "set_antenna_binding" => {
                let port = text(p, "antenna")?;
                put(
                    format!("/api/rf/antennas/{port}"),
                    json!({"antennaPortId": port,
                           "bindings": [{"subAntennaId": 0, "binding": text(p, "binding")?}]}),
                )
            }
            "identify_antenna" => {
                let port = text(p, "antenna")?;
                put(
                    format!("/api/rf/antennas/{port}"),
                    json!({"antennaPortId": port, "identify": boolean(p, "enabled", true)}),
                )
            }
            "set_antenna_led_colors" => {
                let port = text(p, "antenna")?;
                put(
                    format!("/api/rf/antennas/{port}"),
                    json!({"antennaPortId": port,
                           "ledColors": {"rfActive": text(p, "rf_active_color")?,
                                         "rfMuted": text(p, "rf_muted_color")?}}),
                )
            }
            "set_scan_config" => {
                let port = text(p, "antenna")?;
                put(
                    format!("/api/rf/scan/{port}/0/config"),
                    json!({"refLevel": int(p, "ref_level_dbm")?,
                           "sweepTime": int(p, "sweep_time_s")?,
                           "rbw": text(p, "rbw")?}),
                )
            }
            "get_scan_results" => Ok(Call::get(format!(
                "/api/rf/scan/{}/0/results",
                text(p, "antenna")?
            ))),
            "get_rf_restrictions" => Ok(Call::get("/api/rf/restrictions")),

            // Audio links
            "create_audio_link" => {
                let mut body = json!({"rfChannelId": rf(p)?, "modeId": mode_id(p)?});
                if let Some(link) = p.get("link").and_then(Value::as_i64) {
                    body["audiolinkId"] = json!(link);
                }
                Ok(Call {
                    method: "POST",
                    path: "/api/audio/links".into(),
                    body: Some(body),
                    timeout: WRITE_TIMEOUT,
                    returns: Returns::Value,
                })
            }
            "set_audio_link_mode" => {
                let link = int(p, "link")?;
                put(
                    format!("/api/audio/links/{link}"),
                    json!({"audiolinkId": link, "modeId": mode_id(p)?}),
                )
            }
            "delete_audio_link" => Ok(bodiless(
                "DELETE",
                format!("/api/audio/links/{}", int(p, "link")?),
            )),

            // Inputs and outputs
            "route_input" => {
                let input = int(p, "input")?;
                put(
                    format!("/api/audio/inputs/{input}"),
                    json!({"inputId": input, "iemAudiolinkId": int(p, "link")?}),
                )
            }
            "set_input_name" => {
                let input = int(p, "input")?;
                put(
                    format!("/api/audio/inputs/{input}"),
                    json!({"inputId": input, "name": text(p, "name")?}),
                )
            }
            "set_input_source" => {
                let input = int(p, "input")?;
                put(
                    format!("/api/audio/inputs/{input}"),
                    json!({"inputId": input, "inputSource": text(p, "source")?}),
                )
            }
            "route_output" => {
                let output = int(p, "output")?;
                put(
                    format!("/api/audio/outputs/{output}"),
                    json!({"outputId": output, "micAudiolinkId": int(p, "link")?}),
                )
            }
            "set_output_interface" => {
                let output = int(p, "output")?;
                let interface = interface_field(text(p, "interface")?)?;
                let value = text(p, "value")?;
                let when = text(p, "command")?;
                let field = match when {
                    "disabled" => {
                        if !matches!(value, "On" | "Off") {
                            return Err(invalid(
                                "with the command function disabled an interface is only On or Off",
                            ));
                        }
                        format!("{interface}EnableIfCommandIsDisabled")
                    }
                    "enabled" => format!("{interface}EnableIfCommandIsEnabled"),
                    other => return Err(invalid(format!("unknown command state '{other}'"))),
                };
                let mut body = json!({"outputId": output});
                body[field] = json!(value);
                put(format!("/api/audio/outputs/{output}"), body)
            }
            "set_clock_output" => {
                let interface = text(p, "interface")?;
                put(
                    format!("/api/audio/interface/{interface}/output"),
                    json!({"outputClockSource": text(p, "source")?}),
                )
            }
            "get_metering" => Ok(Call::get(METERING)),

            // Pairing
            "set_pairing" => put(
                "/api/mts/pairing".into(),
                json!({"enable": boolean(p, "enabled", true)}),
            ),
            "pair_device" => Ok(Call {
                method: "POST",
                path: "/api/mts/paired/all".into(),
                body: Some(json!({"mtUid": int(p, "mt_uid")?})),
                timeout: WRITE_TIMEOUT,
                returns: Returns::Ack,
            }),
            "unpair_device" => Ok(bodiless(
                "DELETE",
                format!("/api/mts/paired/all/{}", int(p, "mt_uid")?),
            )),
            "identify_pairable_device" => {
                let uid = int(p, "mt_uid")?;
                put(
                    format!("/api/mts/pairable/{uid}"),
                    json!({"mtUid": uid, "identify": boolean(p, "enabled", true)}),
                )
            }
            "dismiss_pairable_device" => Ok(bodiless(
                "DELETE",
                format!("/api/mts/pairable/{}", int(p, "mt_uid")?),
            )),

            // Mobile devices (SEK bodypacks, SKM handhelds)
            "identify_mobile_device" => {
                self.mobile_put(p, json!({"identify": boolean(p, "enabled", true)}))
            }
            "set_mobile_device_name" => self.mobile_put(p, json!({"name": text(p, "name")?})),
            "set_mobile_device_rf_channel" => self.mobile_put(p, json!({"rfChannelId": rf(p)?})),
            "set_mobile_device_sleep" => {
                self.mobile_put(p, json!({"sleep": boolean(p, "enabled", true)}))
            }
            "set_mobile_device_color" => {
                self.mobile_put(p, json!({"connectedStateColor": text(p, "color")?}))
            }
            "assign_mic_link" => self.mobile_put(p, json!({"micAudiolinkId": int(p, "link")?})),
            "assign_iem_link" => {
                self.require_type(p, "SEK", "an IEM link")?;
                self.mobile_put(p, json!({"iemAudiolinkId": int(p, "link")?}))
            }
            "set_headphone_volume" => {
                self.require_type(p, "SEK", "a headphone output")?;
                self.mobile_put(p, json!({"headphoneVolume": half_db(p, "volume_db")?}))
            }
            "set_headphone_volume_limits" => {
                self.require_type(p, "SEK", "a headphone output")?;
                let (min, max) = (half_db(p, "min_db")?, half_db(p, "max_db")?);
                if min > max {
                    return Err(invalid("min_db must not be above max_db"));
                }
                self.mobile_put(
                    p,
                    json!({"headphoneVolumeMin": min, "headphoneVolumeMax": max}),
                )
            }
            "set_headphone_balance" => {
                self.require_type(p, "SEK", "a headphone output")?;
                self.mobile_put(p, json!({"headphoneBalance": int(p, "balance")?}))
            }
            "set_headphone_mono" => {
                self.require_type(p, "SEK", "a headphone output")?;
                self.mobile_put(
                    p,
                    json!({"headphoneMonoBalanced": boolean(p, "enabled", true)}),
                )
            }
            "set_mic_gain" => {
                let gain = int(p, "gain_db")?;
                let uid = int(p, "mt_uid")?;
                let min = if self.mobile_type(uid, p)? == "SKM" {
                    -10
                } else {
                    -6
                };
                if gain < min {
                    return Err(invalid(format!(
                        "the lowest gain on this device is {min} dB"
                    )));
                }
                self.mobile_put(p, json!({"micPreampGain": gain}))
            }
            "set_mic_low_cut" => {
                let hz: i64 = text(p, "frequency_hz")?
                    .parse()
                    .map_err(|_| invalid("frequency_hz is one of 20, 30, 60, 80, 100, 120"))?;
                let uid = int(p, "mt_uid")?;
                if self.mobile_type(uid, p)? == "SKM" && hz < 60 {
                    return Err(invalid("an SKM's low cut is 60, 80, 100 or 120 Hz"));
                }
                self.mobile_put(p, json!({"micLowCutHz": hz}))
            }
            "set_test_tone" => {
                let mut fields = json!({"micTestToneEnabled": boolean(p, "enabled", true)});
                if let Some(level) = p.get("level_dbfs").and_then(Value::as_i64) {
                    fields["micTestToneLevel"] = json!(level);
                }
                self.mobile_put(p, fields)
            }
            "set_mic_line_selection" => {
                self.require_type(p, "SEK", "a mic/line input")?;
                self.mobile_put(p, json!({"micLineSelection": text(p, "selection")?}))
            }
            "set_cable_emulation" => {
                self.require_type(p, "SEK", "cable emulation")?;
                self.mobile_put(p, json!({"cableEmulation": text(p, "emulation")?}))
            }
            "set_command_behavior" => {
                self.mobile_put(p, json!({"commandBehavior": text(p, "behavior")?}))
            }
            "update_mobile_device_firmware" => Ok(bodiless("POST", "/api/mts/firmware/update")),
            "clear_mobile_device_firmware_update_status" => {
                Ok(bodiless("PUT", "/api/mts/firmware/update/state/clear"))
            }

            // Reads
            "get_device_time" => Ok(Call::get("/api/device/time")),
            "get_resource" => {
                let path = text(p, "path")?;
                if !path.starts_with("/api/") || path.contains("..") {
                    return Err(invalid("path must be an API resource path, from /api/"));
                }
                let mut call = Call::get(path);
                call.timeout = REQUEST_TIMEOUT;
                Ok(call)
            }
            other => Err(CommandError::UnknownCommand {
                command: other.into(),
            }),
        }
    }
}

/// A request with no body, acknowledged by any 2xx.
fn bodiless(method: &'static str, path: impl Into<String>) -> Call {
    Call {
        method,
        path: path.into(),
        body: None,
        timeout: WRITE_TIMEOUT,
        returns: Returns::Ack,
    }
}

/// A level in dB on the API's 0.5 dB grid.
fn half_db(p: &Params, name: &str) -> Result<f64, CommandError> {
    let db = float(p, name)?;
    if (db * 2.0).fract() != 0.0 {
        return Err(invalid(format!("{name} is in 0.5 dB steps")));
    }
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::super::sscv2::{QUIET_AFTER, STREAM};
    use super::*;
    use crate::module::{
        Action, CommandId, CommandResult, Connection, HttpRequest, HttpResponse, Module, Outcome,
        RequestId, SseInput,
    };
    use crate::sse::SseEvent;
    use std::net::{IpAddr, Ipv4Addr};

    fn context(monitor: bool, settings: Value) -> OpenContext {
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)),
            host_name: None,
            port: None,
            model: "base-station".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor,
        }
    }

    fn device(monitor: bool) -> Spectera {
        Spectera::from_context(context(monitor, json!({"password": "secret"})))
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

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    fn body(r: &HttpRequest) -> Value {
        serde_json::from_slice(r.body.as_ref().unwrap()).unwrap()
    }

    fn patches(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    /// Start, answer the probe, open the stream and deliver the session id.
    fn streaming(d: &mut Spectera) -> Vec<Action> {
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(
            &mut cx,
            probe,
            ok(json!({"protocol": "2.3", "schema": "18.1"})),
        );
        cx.take();
        let mut cx = Cx::new(20);
        d.sse(&mut cx, STREAM, SseInput::Opened);
        d.sse(
            &mut cx,
            STREAM,
            SseInput::Event(SseEvent {
                event: "open".into(),
                data: json!({"path": "/api/ssc/state/subscriptions/s1", "sessionUUID": "s1"})
                    .to_string(),
            }),
        );
        cx.take()
    }

    fn event(d: &mut Spectera, data: Value) -> Vec<Value> {
        let mut cx = Cx::new(30);
        d.sse(
            &mut cx,
            STREAM,
            SseInput::Event(SseEvent {
                event: "message".into(),
                data: data.to_string(),
            }),
        );
        patches(&cx.take())
    }

    fn command(d: &mut Spectera, name: &str, p: Value) -> Result<HttpRequest, CommandResult> {
        let mut cx = Cx::new(40);
        d.command(&mut cx, 1, name, &params(p));
        let a = cx.take();
        match requests(&a).into_iter().next() {
            Some((_, r)) => Ok(r),
            None => Err(completed(&a).remove(0).1),
        }
    }

    #[test]
    fn authenticates_as_control_sennheiser_unless_told_otherwise() {
        let mut d = device(true);
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let (_, probe) = requests(&cx.take()).remove(0);
        assert_eq!(probe.url, "https://10.0.0.9:443/api/ssc/version");
        // base64("controlSennheiser:secret")
        assert!(probe.headers.contains(&(
            "Authorization".into(),
            "Basic Y29udHJvbFNlbm5oZWlzZXI6c2VjcmV0".into()
        )));
        assert!(probe.accept_invalid_certs);

        let mut d = Spectera::from_context(context(
            true,
            json!({"password": "secret", "username": "api"}),
        ));
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let (_, probe) = requests(&cx.take()).remove(0);
        assert!(probe
            .headers
            .contains(&("Authorization".into(), "Basic YXBpOnNlY3JldA==".into())));
    }

    #[test]
    fn subscribes_every_resource_in_batches_and_reads_only_the_identity() {
        let mut d = device(true);
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(&mut cx, probe, ok(json!({"protocol": "2.3"})));
        let reads = requests(&cx.take());
        assert_eq!(reads.len(), 1);
        assert!(reads[0].1.url.ends_with("/api/device/identity"));

        let mut d = device(true);
        let actions = streaming(&mut d);
        let reqs = requests(&actions);
        // Only the first batch: the rest follow each success.
        assert_eq!(reqs.len(), 1);
        assert!(reqs[0]
            .1
            .url
            .ends_with("/api/ssc/state/subscriptions/s1/add"));
        let mut subscribed: Vec<Value> = body(&reqs[0].1).as_array().unwrap().clone();
        let mut id = reqs[0].0;
        loop {
            let mut cx = Cx::new(30);
            d.http_response(&mut cx, id, status(200));
            let Some((next, req)) = requests(&cx.take()).into_iter().next() else {
                break;
            };
            subscribed.extend(body(&req).as_array().unwrap().clone());
            id = next;
        }
        assert_eq!(subscribed.len(), RESOURCES.len());
        assert!(subscribed.contains(&json!("/api/mts/paired/all")));
        // Meters are not subscribed: they would arrive at up to 60 Hz.
        assert!(!subscribed.contains(&json!(METERING)));
    }

    #[test]
    fn collections_map_onto_state_and_follow_creation_and_deletion() {
        let mut d = device(true);
        streaming(&mut d);
        // The first notification of a fixed list is the whole list.
        let p = event(
            &mut d,
            json!({"/api/rf/channels": [
                {"rfChannelId": 0, "frequency": 550000, "rfState": "RfActive", "txPower": 50},
                {"rfChannelId": 1, "frequency": 600000, "rfState": "RfMuted"}
            ]}),
        );
        assert_eq!(p[0]["rf"]["channels"]["0"]["frequency"], 550000);
        assert_eq!(p[0]["rf"]["channels"]["1"]["rf_state"], "RfMuted");
        assert_eq!(p[0]["rf"]["channels"]["0"]["tx_power"], 50);

        // Later, the changed item alone on the collection's path.
        let p = event(
            &mut d,
            json!({"/api/rf/channels": {"rfChannelId": 1, "rfState": "RfActive"}}),
        );
        assert_eq!(
            p[0],
            json!({"rf": {"channels": {"1": {"rf_channel_id": 1, "rf_state": "RfActive"}}}})
        );

        // A paired mobile device, then its deletion.
        let p = event(
            &mut d,
            json!({"/api/mts/paired/all": [
                {"mtUid": 42, "type": "SEK", "name": "Vox", "batteryFillLevel": 80, "micLqi": 4}
            ]}),
        );
        assert_eq!(p[0]["mobile_devices"]["42"]["battery_fill_level"], 80);
        assert_eq!(p[0]["mobile_devices"]["42"]["mic_lqi"], 4);
        let p = event(&mut d, json!({"/api/mts/paired/all/42": null}));
        assert_eq!(p[0], json!({"mobile_devices": {"42": null}}));

        // An audio link created ({}), filled in, then a full list without it.
        event(&mut d, json!({"/api/audio/links/7": {}}));
        let p = event(
            &mut d,
            json!({"/api/audio/links/7": {"audiolinkId": 7, "rfChannelId": 0, "modeId": 4}}),
        );
        assert_eq!(p[0]["audio"]["links"]["7"]["mode_id"], 4);
        let p = event(&mut d, json!({"/api/audio/links": []}));
        assert_eq!(p[0], json!({"audio": {"links": {"7": null}}}));

        // Singles, fans and interfaces.
        let p = event(
            &mut d,
            json!({
                "/api/health/fan/FAN_2/errorstate": {"value": "Ok"},
                "/api/audio/interface/madi1/status": {"moduleType": "Coax"},
                "/api/device/state": {"state": "Normal", "warnings": []},
                "/api/audio/outputs": {"outputId": 3, "aoIpEnableIfCommandIsDisabled": "Off"}
            }),
        );
        assert_eq!(p[0]["health"]["fans"]["FAN_2"]["value"], "Ok");
        assert_eq!(
            p[0]["audio"]["interfaces"]["madi1"]["status"]["module_type"],
            "Coax"
        );
        assert_eq!(p[0]["device"]["status"]["state"], "Normal");
        assert_eq!(
            p[0]["audio"]["outputs"]["3"]["ao_ip_enable_if_command_is_disabled"],
            "Off"
        );
    }

    #[test]
    fn writes_name_their_ids_as_the_api_requires() {
        let mut d = device(true);
        streaming(&mut d);
        let r = command(
            &mut d,
            "set_rf_active",
            json!({"rf_channel": 1, "active": false}),
        )
        .unwrap();
        assert_eq!(r.method, "PUT");
        assert!(r.url.ends_with("/api/rf/channels/1"));
        assert_eq!(body(&r), json!({"rfChannelId": 1, "rfState": "RfMuted"}));

        let r = command(
            &mut d,
            "set_output_interface",
            json!({"output": 5, "interface": "madi2", "command": "enabled", "value": "Talk"}),
        )
        .unwrap();
        assert_eq!(
            body(&r),
            json!({"outputId": 5, "madi2EnableIfCommandIsEnabled": "Talk"})
        );
        let refused = command(
            &mut d,
            "set_output_interface",
            json!({"output": 5, "interface": "aoip", "command": "disabled", "value": "Mute"}),
        );
        assert!(matches!(
            refused,
            Err(Err(CommandError::InvalidParams { .. }))
        ));

        let r = command(
            &mut d,
            "create_audio_link",
            json!({"rf_channel": 0, "mode": "live_stereo"}),
        )
        .unwrap();
        assert_eq!(r.method, "POST");
        assert_eq!(body(&r), json!({"rfChannelId": 0, "modeId": 7}));

        let r = command(
            &mut d,
            "set_antenna_binding",
            json!({"antenna": "c", "binding": "RfChannel1"}),
        )
        .unwrap();
        assert_eq!(
            body(&r),
            json!({"antennaPortId": "c", "bindings": [{"subAntennaId": 0, "binding": "RfChannel1"}]})
        );

        let bad = command(
            &mut d,
            "set_rf_frequency",
            json!({"rf_channel": 0, "frequency_khz": 550500}),
        );
        assert!(matches!(bad, Err(Err(CommandError::InvalidParams { .. }))));
    }

    #[test]
    fn mobile_device_writes_carry_the_type_learnt_from_state() {
        let mut d = device(true);
        streaming(&mut d);
        // Unknown device: the type must be given.
        let unknown = command(&mut d, "set_mic_gain", json!({"mt_uid": 9, "gain_db": 12}));
        assert!(matches!(
            unknown,
            Err(Err(CommandError::InvalidParams { .. }))
        ));
        let r = command(
            &mut d,
            "set_mic_gain",
            json!({"mt_uid": 9, "gain_db": 12, "device_type": "SKM"}),
        )
        .unwrap();
        assert_eq!(
            body(&r),
            json!({"mtUid": 9, "type": "SKM", "micPreampGain": 12})
        );

        event(
            &mut d,
            json!({"/api/mts/paired/all/42": {"mtUid": 42, "type": "SKM", "name": "HH"}}),
        );
        let r = command(&mut d, "identify_mobile_device", json!({"mt_uid": 42})).unwrap();
        assert!(r.url.ends_with("/api/mts/paired/all/42"));
        assert_eq!(
            body(&r),
            json!({"mtUid": 42, "type": "SKM", "identify": true})
        );
        // A handheld has no headphone output.
        let no = command(
            &mut d,
            "set_headphone_volume",
            json!({"mt_uid": 42, "volume_db": -10.0}),
        );
        assert!(matches!(no, Err(Err(CommandError::InvalidParams { .. }))));
        let low = command(
            &mut d,
            "set_mic_low_cut",
            json!({"mt_uid": 42, "frequency_hz": "20"}),
        );
        assert!(matches!(low, Err(Err(CommandError::InvalidParams { .. }))));
    }

    #[test]
    fn a_created_link_is_returned_and_a_read_updates_state() {
        let mut d = device(true);
        streaming(&mut d);
        let mut cx = Cx::new(40);
        d.command(
            &mut cx,
            3,
            "create_audio_link",
            &params(json!({"rf_channel": 0, "mode": "live_mono", "link": 12})),
        );
        let (id, r) = requests(&cx.take()).remove(0);
        assert_eq!(body(&r)["audiolinkId"], 12);
        let mut cx = Cx::new(60);
        d.http_response(
            &mut cx,
            id,
            Ok(HttpResponse {
                headers: Vec::new(),
                status: 201,
                body: br#"{"audiolinkId":12,"rfChannelId":0,"modeId":4}"#.to_vec(),
            }),
        );
        let a = cx.take();
        assert_eq!(
            completed(&a),
            [(
                3,
                Ok(Outcome::Value {
                    value: json!({"audiolinkId": 12, "rfChannelId": 0, "modeId": 4})
                })
            )]
        );
        assert!(a.contains(&Action::RoundTrip(20)));

        let mut cx = Cx::new(70);
        d.command(&mut cx, 4, "get_device_time", &params(json!({})));
        let (id, _) = requests(&cx.take()).remove(0);
        let mut cx = Cx::new(80);
        d.http_response(&mut cx, id, ok(json!({"time": 1000, "valid": true})));
        let p = patches(&cx.take());
        assert_eq!(p[0]["device"]["time"]["valid"], true);
    }

    #[test]
    fn meters_are_polled_when_asked_and_not_otherwise() {
        let mut d = Spectera::from_context(context(
            true,
            json!({"password": "secret", "metering_poll_ms": 250}),
        ));
        let a = streaming(&mut d);
        assert!(a.contains(&Action::SetTimer {
            key: "poll",
            after: 250
        }));
        let mut cx = Cx::new(300);
        d.timer(&mut cx, "poll");
        let a = cx.take();
        let polls = requests(&a);
        assert_eq!(polls.len(), 1);
        assert!(polls[0].1.url.ends_with(METERING));
        // Not asked again until answered.
        let mut cx = Cx::new(550);
        d.timer(&mut cx, "poll");
        assert!(requests(&cx.take()).is_empty());
        let mut cx = Cx::new(560);
        d.http_response(
            &mut cx,
            polls[0].0,
            ok(json!({"aoIpIn": {"peak": [-20.0], "rms": [-30.0]}, "updateCounter": 3})),
        );
        let p = patches(&cx.take());
        assert_eq!(p[0]["audio"]["metering"]["ao_ip_in"]["peak"][0], -20.0);

        let mut d = device(true);
        let a = streaming(&mut d);
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: "poll", .. })));
    }

    #[test]
    fn opened_for_commands_only_it_subscribes_and_reads_nothing() {
        let mut d = device(false);
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(&mut cx, probe, ok(json!({"protocol": "2.3"})));
        let a = cx.take();
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(!a.iter().any(|x| matches!(x, Action::SseOpen { .. })));
        assert!(requests(&a).is_empty());

        let mut cx = Cx::new(10 + QUIET_AFTER);
        d.timer(&mut cx, "liveness");
        let (check, req) = requests(&cx.take()).remove(0);
        assert!(req.url.ends_with("/api/ssc/version"));
        let mut cx = Cx::new(10 + QUIET_AFTER + 5);
        d.http_response(&mut cx, check, ok(json!({"protocol": "2.3"})));
        assert!(cx.take().contains(&Action::RoundTrip(5)));

        let r = command(&mut d, "identify", json!({"enabled": true})).unwrap();
        assert!(r.url.ends_with("/api/device/identification"));
        assert_eq!(body(&r), json!({"visual": true}));
    }

    #[test]
    fn a_refused_password_is_terminal() {
        let mut d = device(true);
        let mut cx = Cx::new(0);
        d.start(&mut cx);
        let probe = requests(&cx.take())[0].0;
        let mut cx = Cx::new(10);
        d.http_response(&mut cx, probe, status(401));
        assert!(cx
            .take()
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        let mut cx = Cx::new(20);
        d.command(&mut cx, 5, "identify", &params(json!({})));
        let a = cx.take();
        assert!(requests(&a).is_empty());
        assert!(matches!(
            completed(&a)[0],
            (5, Err(CommandError::Auth { .. }))
        ));
    }

    #[test]
    fn a_batch_refused_for_a_missing_resource_does_not_stop_the_rest() {
        let mut d = device(true);
        let actions = streaming(&mut d);
        let (first, _) = requests(&actions).remove(0);
        let mut cx = Cx::new(30);
        // API 17.0 answers 400 for a resource it does not have.
        d.http_response(&mut cx, first, status(400));
        let next = requests(&cx.take());
        assert_eq!(next.len(), 1);
        assert!(next[0].1.url.ends_with("/s1/add"));
    }

    #[test]
    fn snake_case_conversion() {
        assert_eq!(snake_key("rfChannelId"), "rf_channel_id");
        assert_eq!(snake_key("aoIpIn"), "ao_ip_in");
        assert_eq!(
            snake_key("madi1EnableIfCommandIsDisabled"),
            "madi1_enable_if_command_is_disabled"
        );
        assert_eq!(snake_key("psu1"), "psu1");
        assert_eq!(snake_key("micLowCutHz"), "mic_low_cut_hz");
    }
}
