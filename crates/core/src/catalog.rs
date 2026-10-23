//! The catalogue: every device spec, parsed once from the embedded YAML, plus
//! the parameter validation every command passes through before a module sees
//! it. SPEC.md §4 defines the rules; this is their only implementation.

use std::collections::BTreeMap;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

include!(concat!(env!("OUT_DIR"), "/embedded_specs.rs"));

/// Command parameters or settings after validation: every declared value that
/// is present or has a default, and nothing else.
pub type Params = Map<String, Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSpec {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub category: String,
    #[serde(default)]
    pub implementation: Implementation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// A native addition to a spec-driven device, for what the format cannot
    /// express (a notification channel), named so it is never hidden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension: Option<String>,
    pub source: Vec<Source>,
    /// Every port the integration uses, with its default: the device's
    /// control port (which `OpenRequest::port` overrides), and any push,
    /// feedback, notification or discovery ports. A `port` of `None` means
    /// there is no default and one must be given.
    #[serde(default)]
    pub ports: Vec<PortSpec>,
    #[serde(default)]
    pub settings: BTreeMap<String, ParamSpec>,
    pub models: Vec<ModelSpec>,
    #[serde(default)]
    pub commands: BTreeMap<String, CommandSpec>,
    #[serde(default)]
    pub quirks: Vec<Quirk>,
    /// Telemetry fields, keyed by dotted path with `*` for a channel number.
    #[serde(default)]
    pub state: BTreeMap<String, StateField>,
    /// Continuous media the device's module publishes, such as a camera's
    /// live view, keyed by stream name. Watched with `Core::open_stream`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub streams: BTreeMap<String, StreamSpec>,
    /// Wire behaviour for spec-driven devices, kept verbatim for the spec engine.
    #[serde(default, skip_serializing)]
    pub transport: Option<Value>,
    #[serde(default, skip_serializing)]
    pub on_connect: Vec<Value>,
    /// Subscriptions, polls and update rules for spec-driven telemetry.
    #[serde(default, skip_serializing)]
    pub telemetry: Option<Value>,
    /// Numeric response code to failure message, used with `code_range`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub codes: BTreeMap<String, String>,
    /// Named value conversions (a fader law), used by the `to.` and `from.`
    /// template directives; kept verbatim for the spec engine.
    #[serde(default, skip_serializing)]
    pub conversions: Option<Value>,
    /// Named value tables (an enumeration's wire words), used by the `map.`
    /// template directive; kept verbatim for the spec engine.
    #[serde(default, skip_serializing)]
    pub maps: Option<Value>,
}

/// One port an integration uses (SPEC.md §2, Ports).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortSpec {
    /// The default; `None` when there is none and it must be given.
    pub port: Option<u16>,
    /// `tcp`, `udp`, `http`, `https`, `ws`, `wss`, `tls`, `ssh`, `serial`
    /// (a serial port on the host, named by its `setting`, with no number),
    /// `midi` (a MIDI port on the host, likewise) or `icmp` (ICMP echo, with
    /// no number).
    pub protocol: String,
    /// `control` (where commands go; `OpenRequest::port` overrides it),
    /// `push`, `feedback`, `notification` or `discovery`.
    pub role: String,
    /// Who listens on it: `device` (the default) or `core`.
    #[serde(default = "device_listener")]
    pub listener: String,
    /// The setting that changes it, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setting: Option<String>,
    /// When it applies, if not always: a setting's value or a model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn device_listener() -> String {
    "device".into()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Implementation {
    #[default]
    Spec,
    Native,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSpec {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dcas: Option<u32>,
    pub supports: Vec<String>,
    pub verification: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, ParamSpec>,
    #[serde(default = "default_returns")]
    pub returns: String,
    /// Wire behaviour for spec-driven commands, kept verbatim for the spec engine.
    #[serde(default, skip_serializing)]
    pub send: Option<Value>,
    #[serde(default, skip_serializing)]
    pub expect: Option<Value>,
}

fn default_returns() -> String {
    "ack".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSpec {
    #[serde(rename = "type")]
    pub kind: ParamType,
    /// A short name for a person: what a form or button shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// What the value means, its unit and numbering, in a sentence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub secret: bool,
    /// A string that may hold line breaks, tabs and other control
    /// characters (a script, a note): accepted only because every template
    /// sends it through `:json`, which escapes them (SPEC.md §4).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub multiline: bool,
    /// The default for the models named here, in place of `default`: a
    /// serial line setting whose factory value differs by model (a `serial`
    /// block's `by_model`, SPEC.md §2). Listed in the catalogue; a spec does
    /// not declare it on a setting itself.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub default_by_model: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    Int,
    Float,
    Bool,
    Enum,
    String,
    /// Any JSON value, for a request body whose shape the device's own API
    /// documents and validates. Sent as compact JSON.
    Json,
    /// Bytes, given as a base64 string (RFC 4648, padding optional) or as a
    /// host file, `{"file": "<path>"}`, and sent as they are: an HTTP request
    /// body or a multipart part (an image, a show file). A file is streamed
    /// from disk as it is sent. `max_length` limits the bytes.
    Binary,
}

/// The path of a `binary` parameter given as a host file: an object whose
/// only member is `file`, a non-empty string.
pub(crate) fn binary_file(value: &Value) -> Option<&str> {
    let map = value.as_object()?;
    match (map.len(), map.get("file")) {
        (1, Some(Value::String(path))) if !path.is_empty() => Some(path),
        _ => None,
    }
}

/// The length of the host file a `binary` parameter names, once it has been
/// opened for reading. Its contents are never read here, nor quoted.
pub(crate) fn binary_file_len(name: &str, path: &str) -> Result<u64, String> {
    let meta = std::fs::File::open(path)
        .and_then(|file| file.metadata())
        .map_err(|e| format!("'{name}': the file {path} cannot be read: {e}"))?;
    if !meta.is_file() {
        return Err(format!("'{name}': {path} is not a file"));
    }
    Ok(meta.len())
}

/// A `binary` parameter's bytes: standard base64, with or without padding.
pub(crate) fn decode_binary(text: &str) -> Option<Vec<u8>> {
    use base64::engine::{general_purpose, DecodePaddingMode, GeneralPurpose};
    use base64::Engine;
    const B64: GeneralPurpose = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        general_purpose::PAD.with_decode_padding_mode(DecodePaddingMode::Indifferent),
    );
    B64.decode(text).ok()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateField {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub description: String,
}

/// A stream a native module publishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSpec {
    /// The frame encoding: `jpeg`, or `meros-preview` (a Meros Camera's
    /// preview frame as the camera frames it).
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// The models that have it; every model when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quirk {
    pub models: Vec<String>,
    pub severity: String,
    pub text: String,
}

impl DeviceSpec {
    /// Whether the core only listens, on every port the integration uses, so
    /// it reaches out to no device and can be opened without a host.
    pub fn core_listens_only(&self) -> bool {
        !self.ports.is_empty() && self.ports.iter().all(|p| p.listener == "core")
    }

    /// Whether the device can be reached on a serial port of the host, so it
    /// may be opened without a network host (SPEC.md §2, Ports).
    pub fn uses_serial(&self) -> bool {
        self.ports.iter().any(|p| p.protocol == "serial")
    }

    /// Whether the device can be reached on a MIDI port of the host, so it
    /// may be opened without a network host (SPEC.md §2, Ports).
    pub fn uses_midi(&self) -> bool {
        self.ports.iter().any(|p| p.protocol == "midi")
    }

    pub fn model(&self, id: &str) -> Option<&ModelSpec> {
        self.models.iter().find(|m| m.id == id)
    }

    /// Parse a spec, and add the settings its transport's `serial` block
    /// stands for (SPEC.md §2, "The same protocol on a serial port").
    pub(crate) fn from_yaml(text: &str) -> Result<DeviceSpec, String> {
        let mut spec: DeviceSpec = crate::yaml::from_str(text).map_err(|e| e.to_string())?;
        spec.expand_serial()?;
        Ok(spec)
    }

    /// The `serial` block of a `line-tcp` transport: the same protocol on a
    /// serial port of the host, chosen per device by the `link` setting.
    pub(crate) fn serial_block(&self) -> Option<&Map<String, Value>> {
        let t = self.transport.as_ref()?;
        if t.get("type").and_then(Value::as_str) != Some("line-tcp") {
            return None;
        }
        t.get("serial").and_then(Value::as_object)
    }

    /// Whether a device of `model` with these (validated) settings uses the
    /// serial port: `None` for a spec with no `serial` block. With no `link`
    /// given, a model in `serial_only` uses its serial port, any other the
    /// network.
    pub(crate) fn serial_link(&self, model: &str, settings: &Params) -> Option<bool> {
        let block = self.serial_block()?;
        Some(match settings.get("link").and_then(Value::as_str) {
            Some(link) => link == "serial",
            None => names_model(block.get("serial_only"), model),
        })
    }

    /// Whether `model` has the serial port of the `serial` block: every model
    /// unless `models` names some.
    pub(crate) fn serial_model(&self, model: &str) -> bool {
        self.serial_block().is_none_or(|b| match b.get("models") {
            None => true,
            some => names_model(some, model),
        })
    }

    /// The transport keys that differ on the serial port for `model`: every
    /// entry of the block's `overrides` that is for the model (one naming no
    /// `models` is for all), laid over one another in order. Empty for a
    /// spec with no `serial` block or no `overrides`.
    pub(crate) fn serial_overrides(&self, model: &str) -> Map<String, Value> {
        let mut out = Map::new();
        let entries = match self.serial_block().and_then(|b| b.get("overrides")) {
            Some(Value::Array(list)) => list.clone(),
            Some(one) => vec![one.clone()],
            None => return out,
        };
        for entry in entries.iter().filter_map(Value::as_object) {
            if entry.contains_key("models") && !names_model(entry.get("models"), model) {
                continue;
            }
            for (k, v) in entry {
                if k != "models" {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
        out
    }

    /// Add the `link` setting and the six serial settings a `serial` block
    /// stands for. A spec declaring any of them itself is refused, as is a
    /// block that is not one the core can open a port with.
    fn expand_serial(&mut self) -> Result<(), String> {
        let Some(t) = self.transport.as_ref() else {
            return Ok(());
        };
        let Some(block) = t.get("serial") else {
            return Ok(());
        };
        if t.get("type").and_then(Value::as_str) != Some("line-tcp") {
            return Err("transport.serial is for a line-tcp transport".into());
        }
        let block = block
            .as_object()
            .ok_or("transport.serial must be a mapping")?
            .clone();
        for key in block.keys() {
            if ![
                "link",
                "serial_only",
                "models",
                "baud_rate",
                "data_bits",
                "parity",
                "stop_bits",
                "flow_control",
                "overrides",
            ]
            .contains(&key.as_str())
            {
                return Err(format!("transport.serial.{key} is not a serial block key"));
            }
        }
        for key in ["serial_only", "models"] {
            if let Some(list) = block.get(key) {
                let ids = list
                    .as_array()
                    .filter(|a| !a.is_empty())
                    .ok_or(format!("transport.serial.{key} must list model ids"))?;
                for id in ids {
                    let id = id.as_str().unwrap_or_default();
                    if self.model(id).is_none() {
                        return Err(format!("transport.serial.{key}: no model '{id}'"));
                    }
                }
            }
        }
        if let Some(overrides) = block.get("overrides") {
            let entries = match overrides {
                Value::Array(list) if !list.is_empty() => list.clone(),
                Value::Object(_) => vec![overrides.clone()],
                _ => {
                    return Err(
                        "transport.serial.overrides must be a mapping or a list of them".into(),
                    )
                }
            };
            for entry in &entries {
                let entry = entry
                    .as_object()
                    .ok_or("transport.serial.overrides: each entry is a mapping")?;
                for key in entry.keys() {
                    if !SERIAL_OVERRIDES.contains(&key.as_str()) && key != "models" {
                        return Err(format!(
                            "transport.serial.overrides.{key}: not a framing, reply or pacing key a \
                             serial port can change"
                        ));
                    }
                }
                if let Some(list) = entry.get("models") {
                    let ids = list
                        .as_array()
                        .filter(|a| !a.is_empty())
                        .ok_or("transport.serial.overrides.models must list model ids")?;
                    for id in ids {
                        let id = id.as_str().unwrap_or_default();
                        if self.model(id).is_none() {
                            return Err(format!(
                                "transport.serial.overrides.models: no model '{id}'"
                            ));
                        }
                    }
                }
            }
        }
        let link = match block.get("link") {
            None => "tcp",
            Some(v) => match v.as_str() {
                Some(l @ ("tcp" | "serial")) => l,
                _ => return Err("transport.serial.link must be tcp or serial".into()),
            },
        };
        let only = block.contains_key("serial_only");
        if link == "serial" && only {
            return Err(
                "transport.serial: serial_only names the models that default to \
                        serial, so link stays tcp"
                    .into(),
            );
        }
        let mut add = BTreeMap::new();
        add.insert(
            "link".to_string(),
            ParamSpec {
                values: Some(vec!["tcp".into(), "serial".into()]),
                default: (!only).then(|| Value::from(link)),
                ..setting(
                    ParamType::Enum,
                    "Link",
                    if only {
                        "tcp reaches the device over the network at its host and port; serial \
                         speaks the same protocol on a serial port of the host. Left out, the \
                         models reached only on a serial port use serial, the others tcp."
                    } else {
                        "tcp reaches the device over the network at its host and port; serial \
                         speaks the same protocol on a serial port of the host."
                    },
                )
            },
        );
        add.insert(
            "serial_port".to_string(),
            ParamSpec {
                max_length: Some(256),
                ..setting(
                    ParamType::String,
                    "Serial port",
                    "With link serial, the host's serial port wired to the device: COM3 on \
                     Windows, /dev/ttyUSB0 on Linux, /dev/cu.usbserial-1420 on macOS.",
                )
            },
        );
        let serial_models = block.get("models").cloned();
        let line = |name: &str| -> Result<Line, String> {
            match block.get(name) {
                None => Err(format!(
                    "transport.serial.{name}: give the device's documented value"
                )),
                Some(Value::Object(m)) => {
                    for key in m.keys() {
                        if !["default", "values", "by_model"].contains(&key.as_str()) {
                            return Err(format!("transport.serial.{name}.{key}: not a key here"));
                        }
                    }
                    let default = m
                        .get("default")
                        .cloned()
                        .ok_or(format!("transport.serial.{name}: no default"))?;
                    let values = m
                        .get("values")
                        .and_then(Value::as_array)
                        .cloned()
                        .ok_or(format!("transport.serial.{name}: no values"))?;
                    if !values.contains(&default) {
                        return Err(format!(
                            "transport.serial.{name}: the default is not one of the values"
                        ));
                    }
                    let entries = match m.get("by_model") {
                        None => Vec::new(),
                        Some(Value::Array(list)) if !list.is_empty() => list.clone(),
                        Some(_) => {
                            return Err(format!(
                                "transport.serial.{name}.by_model: a list of {{models, default}}"
                            ))
                        }
                    };
                    let mut by_model = Vec::new();
                    let mut seen = std::collections::BTreeSet::new();
                    for entry in &entries {
                        let Some(entry) = entry.as_object().filter(|e| {
                            e.len() == 2 && e.contains_key("models") && e.contains_key("default")
                        }) else {
                            return Err(format!(
                                "transport.serial.{name}.by_model: each entry is {{models, default}}"
                            ));
                        };
                        let value = entry["default"].clone();
                        if !values.contains(&value) {
                            return Err(format!(
                                "transport.serial.{name}.by_model: {value} is not one of the values"
                            ));
                        }
                        let ids =
                            entry["models"]
                                .as_array()
                                .filter(|a| !a.is_empty())
                                .ok_or(format!(
                                    "transport.serial.{name}.by_model: models must list model ids"
                                ))?;
                        let mut names = Vec::new();
                        for id in ids {
                            let id = id.as_str().unwrap_or_default();
                            if self.model(id).is_none() {
                                return Err(format!(
                                    "transport.serial.{name}.by_model: no model '{id}'"
                                ));
                            }
                            if serial_models.is_some() && !names_model(serial_models.as_ref(), id) {
                                return Err(format!(
                                    "transport.serial.{name}.by_model: model '{id}' has no \
                                     serial port"
                                ));
                            }
                            if !seen.insert(id.to_string()) {
                                return Err(format!(
                                    "transport.serial.{name}.by_model: model '{id}' is named \
                                     twice"
                                ));
                            }
                            names.push(id.to_string());
                        }
                        by_model.push((names, value));
                    }
                    Ok((default, values, by_model))
                }
                Some(one) => Ok((one.clone(), vec![one.clone()], Vec::new())),
            }
        };
        // Each entry's default for each model it names, as the setting holds
        // its values, and the words the description adds for them.
        let per_model = |by_model: &[(Vec<String>, Value)], show: &dyn Fn(&Value) -> Value| {
            let map: BTreeMap<String, Value> = by_model
                .iter()
                .flat_map(|(ids, v)| ids.iter().map(move |id| (id.clone(), show(v))))
                .collect();
            let words: String = by_model
                .iter()
                .map(|(ids, v)| {
                    let v = show(v);
                    let v = v.as_str().map_or_else(|| v.to_string(), str::to_string);
                    format!(", {v} on {}", ids.join(", "))
                })
                .collect();
            ((!map.is_empty()).then_some(map), words)
        };
        let (default, values, by_model) = line("baud_rate")?;
        if values.iter().any(|v| v.as_u64().is_none_or(|n| n == 0)) {
            return Err("transport.serial.baud_rate: bits per second, above 0".into());
        }
        add.insert(
            "baud_rate".to_string(),
            if values.len() == 1 {
                ParamSpec {
                    min: default.as_f64(),
                    max: default.as_f64(),
                    default: Some(default.clone()),
                    ..setting(
                        ParamType::Int,
                        "Baud rate",
                        &format!(
                            "With link serial, bits per second: the device's port runs at \
                             {default}."
                        ),
                    )
                }
            } else {
                let (default_by_model, words) =
                    per_model(&by_model, &|v| Value::from(v.to_string()));
                ParamSpec {
                    values: Some(values.iter().map(|v| v.to_string()).collect()),
                    default: Some(Value::from(default.to_string())),
                    default_by_model,
                    ..setting(
                        ParamType::Enum,
                        "Baud rate",
                        &format!(
                            "With link serial, bits per second, as set on the device's port \
                             ({default} unless changed there{words})."
                        ),
                    )
                }
            },
        );
        for (name, label, low, high, what) in [
            ("data_bits", "Data bits", 5, 8, "bits per character"),
            ("stop_bits", "Stop bits", 1, 2, "stop bits"),
        ] {
            let (default, values, by_model) = line(name)?;
            let mut n: Vec<u64> = values.iter().filter_map(Value::as_u64).collect();
            n.sort_unstable();
            n.dedup();
            if n.len() != values.len()
                || n.iter().any(|v| !(low..=high).contains(v))
                || n[n.len() - 1] - n[0] + 1 != n.len() as u64
            {
                return Err(format!(
                    "transport.serial.{name}: whole numbers from {low} to {high}, with no gaps"
                ));
            }
            let (default_by_model, words) = per_model(&by_model, &|v| v.clone());
            add.insert(
                name.to_string(),
                ParamSpec {
                    min: Some(n[0] as f64),
                    max: Some(n[n.len() - 1] as f64),
                    default: Some(default.clone()),
                    default_by_model,
                    ..setting(
                        ParamType::Int,
                        label,
                        &format!(
                            "With link serial, {what}, as set on the device's port ({default} \
                             by default{words})."
                        ),
                    )
                },
            );
        }
        for (name, label, choices, what) in [
            (
                "parity",
                "Parity",
                &["none", "odd", "even"][..],
                "the parity",
            ),
            (
                "flow_control",
                "Flow control",
                &["none", "xon-xoff", "rts-cts"][..],
                "how the port paces what is sent",
            ),
        ] {
            let (default, values, by_model) = line(name)?;
            let values: Vec<String> = values
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect();
            if values.iter().any(|v| !choices.contains(&v.as_str())) {
                return Err(format!(
                    "transport.serial.{name}: one of {}",
                    choices.join(", ")
                ));
            }
            let (default_by_model, words) = per_model(&by_model, &|v| v.clone());
            add.insert(
                name.to_string(),
                ParamSpec {
                    values: Some(values),
                    default: Some(default.clone()),
                    default_by_model,
                    ..setting(
                        ParamType::Enum,
                        label,
                        &format!(
                            "With link serial, {what}, as set on the device's port ({} by \
                             default{words}).",
                            default.as_str().unwrap_or_default()
                        ),
                    )
                },
            );
        }
        for (name, spec) in add {
            if self.settings.contains_key(&name) {
                return Err(format!(
                    "settings.{name}: transport.serial declares it; leave it out"
                ));
            }
            self.settings.insert(name, spec);
        }
        Ok(())
    }
}

/// A serial line setting from a `serial` block: its default, its values,
/// and the defaults of the models named in `by_model`.
type Line = (Value, Vec<Value>, Vec<(Vec<String>, Value)>);

/// Whether a list of model ids holds `model`.
/// The transport keys a `serial` block's `overrides` may change: how a
/// message is framed, how replies are told apart, and how messages are
/// paced (SPEC.md §2, "The same protocol on a serial port").
pub(crate) const SERIAL_OVERRIDES: &[&str] = &[
    "framing",
    "open",
    "close",
    "terminator",
    "reply_framing",
    "reply_terminators",
    "reply_fence",
    "reply_match",
    "error_match",
    "pacing",
];

fn names_model(list: Option<&Value>, model: &str) -> bool {
    list.and_then(Value::as_array)
        .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(model)))
}

/// A setting a `serial` block adds, before its type's own fields.
fn setting(kind: ParamType, label: &str, description: &str) -> ParamSpec {
    ParamSpec {
        kind,
        label: Some(label.into()),
        description: Some(description.into()),
        min: None,
        max: None,
        values: None,
        max_length: None,
        pattern: None,
        default: None,
        required: false,
        secret: false,
        multiline: false,
        default_by_model: None,
    }
}

/// Specs keyed by id: every embedded one, or a core's selection of them.
#[derive(Debug, Clone, Serialize)]
pub struct Catalog {
    pub devices: BTreeMap<String, DeviceSpec>,
}

impl Catalog {
    /// Parse the embedded specs. A spec that fails to parse is a build defect,
    /// caught by the test suite, so this panics rather than returning a partial
    /// catalogue that silently lacks a device.
    pub fn embedded() -> Catalog {
        let mut devices = BTreeMap::new();
        for (file, text) in EMBEDDED_SPECS {
            let spec = DeviceSpec::from_yaml(text)
                .unwrap_or_else(|e| panic!("embedded spec {file}.yaml does not parse: {e}"));
            devices.insert(spec.id.clone(), spec);
        }
        Catalog { devices }
    }

    pub fn device(&self, id: &str) -> Option<&DeviceSpec> {
        self.devices.get(id)
    }

    /// Keep only the named specs, for a product that uses only some devices.
    /// Each name is a spec id, a vendor group (`vendor-sennheiser`: those of
    /// the vendor's integrations this build includes) or `all`. A name this
    /// build cannot meet is an error that says why: unknown, or an integration
    /// whose feature the build left out.
    pub fn select(mut self, ids: &[String]) -> Result<Catalog, String> {
        if ids.is_empty() {
            return Err(
                "the device selection is empty: leave it out to include every device".into(),
            );
        }
        let mut keep = std::collections::BTreeSet::new();
        for id in ids {
            if id == "all" {
                keep.extend(self.devices.keys().cloned());
                continue;
            }
            if let Some((group, specs)) = VENDOR_GROUPS.iter().find(|(group, _)| group == id) {
                let built: Vec<&str> = specs
                    .iter()
                    .copied()
                    .filter(|spec| self.devices.contains_key(*spec))
                    .collect();
                if built.is_empty() {
                    return Err(format!(
                        "vendor group '{group}' has no integration in this build: enable the \
                         '{group}' feature, or the feature of one of its integrations ({})",
                        specs.join(", ")
                    ));
                }
                keep.extend(built.into_iter().map(String::from));
                continue;
            }
            if self.devices.contains_key(id) {
                keep.insert(id.clone());
                continue;
            }
            return Err(match feature(id) {
                Some(feature) => {
                    format!("device '{id}' is not in this build: it needs the '{feature}' feature")
                }
                None => format!("unknown device '{id}' in the selection"),
            });
        }
        self.devices.retain(|id, _| keep.contains(id));
        Ok(self)
    }

    /// Every spec in `specs/`, read from the source tree whatever the build's
    /// features, for tests of the spec engine itself.
    #[cfg(test)]
    pub(crate) fn source_tree() -> Catalog {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../specs");
        let mut devices = BTreeMap::new();
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                let spec = DeviceSpec::from_yaml(&std::fs::read_to_string(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()));
                devices.insert(spec.id.clone(), spec);
            }
        }
        Catalog { devices }
    }
}

/// The Cargo feature that builds an integration in, whether or not this build
/// has it: the spec id itself. `None` for an id no spec has.
pub fn feature(id: &str) -> Option<&'static str> {
    ALL_SPECS
        .iter()
        .find(|(spec, _, _)| *spec == id)
        .map(|(spec, _, _)| *spec)
}

/// Every integration in the source tree: its spec id (also its feature
/// name), whether this build includes it, and its vendor group.
pub fn integrations() -> &'static [(&'static str, bool, &'static str)] {
    ALL_SPECS
}

/// Every vendor group feature and the integrations it enables. A vendor
/// group's name can also be given in a core's `devices` selection.
pub fn vendor_groups() -> &'static [(&'static str, &'static [&'static str])] {
    VENDOR_GROUPS
}

/// Validate `given` against `declared`, applying defaults.
///
/// Rejects unknown names, missing required values, wrong types, out-of-range
/// values, strings over length, strings failing their pattern, and strings
/// containing control characters. Never clamps or strips: a corrected value
/// would produce a device state the caller did not ask for.
pub fn validate(declared: &BTreeMap<String, ParamSpec>, given: &Params) -> Result<Params, String> {
    validate_inner(declared, given, None)
}

/// [`validate`] for a device's settings: a setting whose default differs by
/// model (`default_by_model`) takes the default of `model`.
pub fn validate_settings(
    declared: &BTreeMap<String, ParamSpec>,
    given: &Params,
    model: &str,
) -> Result<Params, String> {
    validate_inner(declared, given, Some(model))
}

fn validate_inner(
    declared: &BTreeMap<String, ParamSpec>,
    given: &Params,
    model: Option<&str>,
) -> Result<Params, String> {
    for name in given.keys() {
        if !declared.contains_key(name) {
            return Err(format!("unknown parameter '{name}'"));
        }
    }

    let mut out = Params::new();
    for (name, spec) in declared {
        let for_model = model.and_then(|m| spec.default_by_model.as_ref()?.get(m));
        let value = match given.get(name) {
            Some(Value::Null) | None => match for_model.or(spec.default.as_ref()) {
                Some(default) => default.clone(),
                None if spec.required => return Err(format!("'{name}' is required")),
                None => continue,
            },
            Some(v) => v.clone(),
        };
        check_value(name, spec, &value)?;
        out.insert(name.clone(), value);
    }
    Ok(out)
}

fn check_value(name: &str, spec: &ParamSpec, value: &Value) -> Result<(), String> {
    let in_range = |n: f64| -> Result<(), String> {
        if let Some(min) = spec.min {
            if n < min {
                return Err(format!("'{name}' is {n}, below the minimum {min}"));
            }
        }
        if let Some(max) = spec.max {
            if n > max {
                return Err(format!("'{name}' is {n}, above the maximum {max}"));
            }
        }
        Ok(())
    };

    match spec.kind {
        ParamType::Int => {
            let n = value
                .as_i64()
                .ok_or_else(|| format!("'{name}' must be an integer"))?;
            in_range(n as f64)
        }
        ParamType::Float => {
            let n = value
                .as_f64()
                .ok_or_else(|| format!("'{name}' must be a number"))?;
            in_range(n)
        }
        ParamType::Bool => value
            .as_bool()
            .map(|_| ())
            .ok_or_else(|| format!("'{name}' must be true or false")),
        ParamType::Json => Ok(()),
        ParamType::Binary => {
            let len = match (value.as_str(), binary_file(value)) {
                (Some(s), _) => decode_binary(s)
                    .ok_or_else(|| format!("'{name}' is not base64"))?
                    .len() as u64,
                (None, Some(path)) => binary_file_len(name, path)?,
                (None, None) => {
                    return Err(format!(
                        "'{name}' must be a base64 string or {{\"file\": \"<path>\"}}"
                    ))
                }
            };
            if let Some(max) = spec.max_length {
                if len > max as u64 {
                    return Err(format!("'{name}' is longer than {max} bytes"));
                }
            }
            Ok(())
        }
        ParamType::Enum => {
            let s = value
                .as_str()
                .ok_or_else(|| format!("'{name}' must be a string"))?;
            let values = spec.values.as_deref().unwrap_or_default();
            if values.iter().any(|v| v == s) {
                Ok(())
            } else {
                Err(format!("'{name}' must be one of {values:?}"))
            }
        }
        ParamType::String => {
            let s = value
                .as_str()
                .ok_or_else(|| format!("'{name}' must be a string"))?;
            if !spec.multiline && s.chars().any(|c| (c as u32) < 0x20) {
                return Err(format!("'{name}' contains a control character"));
            }
            if let Some(max) = spec.max_length {
                if s.chars().count() > max {
                    return Err(format!("'{name}' is longer than {max} characters"));
                }
            }
            if let Some(pattern) = &spec.pattern {
                let re = Regex::new(pattern)
                    .map_err(|e| format!("'{name}' has an invalid pattern in its spec: {e}"))?;
                if !re.is_match(s) {
                    return Err(format!("'{name}' does not match {pattern}"));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec(yaml: &str) -> BTreeMap<String, ParamSpec> {
        crate::yaml::from_str(yaml).unwrap()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn every_embedded_spec_parses() {
        let catalog = Catalog::embedded();
        assert_eq!(catalog.devices.len(), EMBEDDED_SPECS.len());
        if cfg!(feature = "all") {
            assert_eq!(catalog.devices.len(), ALL_SPECS.len());
        }
        for (file, _) in EMBEDDED_SPECS {
            assert!(
                catalog.devices.contains_key(*file),
                "{file}.yaml's id is not {file}"
            );
        }
        for spec in catalog.devices.values() {
            for model in &spec.models {
                for cmd in &model.supports {
                    assert!(
                        spec.commands.contains_key(cmd),
                        "{}: {} lists unknown {cmd}",
                        spec.id,
                        model.id
                    );
                }
            }
        }
    }

    /// The features in Cargo.toml's [features], each with its list.
    fn manifest_features() -> BTreeMap<String, Vec<String>> {
        let manifest = include_str!("../Cargo.toml").replace("\r\n", "\n");
        let section = manifest
            .split("\n[features]\n")
            .nth(1)
            .and_then(|rest| rest.split("\n[").next())
            .expect("a [features] section");
        let mut features = BTreeMap::new();
        let mut current: Option<(String, String)> = None;
        for line in section.lines() {
            let line = line.split(" #").next().unwrap().trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            match &mut current {
                Some((_, list)) => list.push_str(line),
                None => {
                    let (name, list) = line.split_once(" = ").expect("name = [...]");
                    current = Some((name.to_string(), list.to_string()));
                }
            }
            if current
                .as_ref()
                .is_some_and(|(_, list)| list.ends_with(']'))
            {
                let (name, list) = current.take().unwrap();
                let items = list
                    .trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                features.insert(name, items);
            }
        }
        features
    }

    #[test]
    fn the_integration_table_matches_the_cargo_features() {
        let features = manifest_features();
        assert!(
            features["default"].is_empty(),
            "the default is no integration"
        );
        let groups: Vec<String> = VENDOR_GROUPS.iter().map(|(g, _)| g.to_string()).collect();
        assert_eq!(features["all"], groups, "'all' enables every vendor group");
        for (group, specs) in VENDOR_GROUPS {
            let listed = features
                .get(*group)
                .unwrap_or_else(|| panic!("vendor group '{group}' has no feature"));
            assert_eq!(
                listed, *specs,
                "'{group}' must enable exactly its integrations"
            );
        }
        for (spec, built, group) in ALL_SPECS {
            let own = features
                .get(*spec)
                .unwrap_or_else(|| panic!("integration '{spec}' has no feature"));
            assert!(
                own.iter().all(|f| f.starts_with("dep:")),
                "'{spec}' may enable only its own optional dependencies, not {own:?}"
            );
            assert!(VENDOR_GROUPS
                .iter()
                .any(|(g, s)| g == group && s.contains(spec)));
            assert_eq!(
                *built,
                EMBEDDED_SPECS.iter().any(|(file, _)| file == spec),
                "{spec}"
            );
            if cfg!(feature = "all") {
                assert!(built, "'all' is on but '{spec}' is not built");
            }
        }
        for name in features.keys() {
            assert!(
                name == "default"
                    || name == "all"
                    || groups.contains(name)
                    || ALL_SPECS.iter().any(|(spec, _, _)| spec == name),
                "feature '{name}' is neither an integration, a vendor group nor 'all'"
            );
        }
    }

    #[test]
    fn a_selection_keeps_only_its_devices_and_names_what_is_missing() {
        let ids: Vec<String> = Catalog::embedded()
            .devices
            .keys()
            .take(2)
            .cloned()
            .collect();
        if let Some(first) = ids.first() {
            let selected = Catalog::embedded().select(&ids).unwrap();
            assert_eq!(selected.devices.keys().cloned().collect::<Vec<_>>(), ids);
            assert!(selected.device(first).is_some());
        }
        let err = Catalog::embedded()
            .select(&["no-such-device".into()])
            .unwrap_err();
        assert!(err.contains("unknown device 'no-such-device'"), "{err}");
        assert!(Catalog::embedded().select(&[]).is_err());
        if let Some((id, _, _)) = ALL_SPECS.iter().find(|(_, built, _)| !built) {
            let err = Catalog::embedded().select(&[id.to_string()]).unwrap_err();
            assert!(err.contains(&format!("the '{id}' feature")), "{err}");
        }
        let every = Catalog::embedded().select(&["all".into()]).unwrap();
        assert_eq!(every.devices.len(), EMBEDDED_SPECS.len());
        for (group, specs) in VENDOR_GROUPS {
            let built: Vec<&str> = specs
                .iter()
                .copied()
                .filter(|s| Catalog::embedded().device(s).is_some())
                .collect();
            match Catalog::embedded().select(&[group.to_string()]) {
                Ok(selected) => assert_eq!(selected.devices.keys().collect::<Vec<_>>(), built),
                Err(err) => {
                    assert!(built.is_empty(), "{group}: {err}");
                    assert!(err.contains(&format!("the '{group}' feature")), "{err}");
                }
            }
        }
    }

    #[test]
    fn applies_defaults_and_rejects_unknown() {
        let decl = spec("channel: { type: int, min: 1, max: 4, required: true }\nmuted: { type: bool, default: true }");
        let out = validate(&decl, &params(json!({"channel": 2}))).unwrap();
        assert_eq!(out["muted"], json!(true));
        assert!(validate(&decl, &params(json!({"channel": 2, "extra": 1}))).is_err());
        assert!(validate(&decl, &params(json!({}))).is_err());
    }

    #[test]
    fn rejects_rather_than_clamps() {
        let decl = spec("gain: { type: int, min: -18, max: 42, required: true }");
        assert!(validate(&decl, &params(json!({"gain": 43}))).is_err());
        assert!(validate(&decl, &params(json!({"gain": 1.5}))).is_err());
    }

    #[test]
    fn rejects_control_characters_and_pattern_failures() {
        let decl =
            spec("cue: { type: string, max_length: 8, pattern: \"^[0-9.]+$\", required: true }");
        assert!(validate(&decl, &params(json!({"cue": "1.5"}))).is_ok());
        assert!(validate(&decl, &params(json!({"cue": "1\r\nDelete"}))).is_err());
        assert!(validate(&decl, &params(json!({"cue": "1A"}))).is_err());
        assert!(validate(&decl, &params(json!({"cue": "123456789"}))).is_err());
    }
}
