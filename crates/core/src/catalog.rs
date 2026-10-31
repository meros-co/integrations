//! The catalogue: every device spec, parsed once from the embedded YAML, plus
//! the parameter validation every command passes through before a module sees
//! it. SPEC.md §4 defines the rules; this is their only implementation.

use std::collections::BTreeMap;

use crate::value::{Map, Value};
use regex::Regex;
use serde::{Deserialize, Serialize};

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
    /// `tcp`, `udp`, `http`, `https`, `ws`, `wss`, `tls`, `ssh` or `icmp`
    /// (ICMP echo, with no number).
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
    /// The selection settings (`select`) the command's messages name: it
    /// fails until each is chosen. Worked out from the spec, not declared.
    #[serde(default, skip_deserializing, skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
    /// How long each of the command's messages may wait for its answer, in
    /// place of the transport's timeout: a firmware upload or a slow
    /// operation the vendor documents (SPEC.md §4, "Timeouts").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
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
    /// A float that also takes the string `"-inf"`: a level a native module
    /// can set to silence, as its state reports it (SPEC.md §4).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub negative_infinity: bool,
    /// A setting that picks one of the account's things (a broadcast, a
    /// channel, a plan): it may be left empty when the device is opened, and
    /// `command` lists the choices (SPEC.md §2, "Settings chosen after
    /// signing in").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<Select>,
}

/// A selection setting's choices (SPEC.md §2, "Settings chosen after
/// signing in"): the command listing them, and where each choice's id and
/// name are in its result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Select {
    /// The command that lists the choices; it does not name the setting, so
    /// it works before the setting is chosen.
    pub command: String,
    /// The JSON path of the array of choices in the command's result; the
    /// result itself when left out (an array, or one object, one choice).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<String>,
    /// The JSON path of a choice's id, the setting's value, in each choice.
    pub id: String,
    /// A regex whose first group is the id within the text at `id`, for a
    /// service that names a thing by a path (Vimeo's `/live_events/123`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_match: Option<String>,
    /// The JSON path of the name a person chooses by, in each element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The command's parameters that take the value of another setting,
    /// chosen first: parameter name to setting name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
    /// State that belongs to the thing chosen and is not keyed by it: each
    /// path, and everything under it, leaves state when the choice changes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clears: Vec<String>,
}

/// Whether a `send` item, probe or message names `{settings.<name>}`.
pub(crate) fn names_setting(item: &Value, name: &str) -> bool {
    let text = match item {
        Value::String(s) => std::borrow::Cow::Borrowed(s.as_str()),
        other => std::borrow::Cow::Owned(other.to_string()),
    };
    let needle = format!("{{settings.{name}");
    text.match_indices(&needle)
        .any(|(at, _)| matches!(text.as_bytes().get(at + needle.len()), Some(b'}' | b':')))
}

/// Each poll group's key under `polled` in state (SPEC.md §8, "When each
/// poll group was last answered"): its `name`, or its place from 1.
pub(crate) fn poll_group_names(telemetry: Option<&Value>) -> Vec<String> {
    match telemetry.and_then(|t| t.get("poll")) {
        Some(Value::Array(groups)) => groups
            .iter()
            .enumerate()
            .map(|(i, g)| {
                g.get("name")
                    .and_then(Value::as_str)
                    .map_or_else(|| (i + 1).to_string(), str::to_string)
            })
            .collect(),
        Some(_) => vec!["1".into()],
        None => Vec::new(),
    }
}

/// Whether a spec-driven transport waits for the answer to each poll item,
/// so a poll group's answers can be timed: HTTP and OSC over TCP always, a
/// line or OSC transport whose device answers, a websocket whose replies
/// `reply_match` tells from its pushes.
pub(crate) fn polls_answered(transport: &Value) -> bool {
    let reply = transport.get("reply").and_then(Value::as_str);
    match transport.get("type").and_then(Value::as_str) {
        Some("http" | "osc-tcp") => true,
        Some("line-tcp") => reply != Some("none"),
        Some("osc-udp" | "line-udp") => reply == Some("to-source"),
        Some("ws") => reply != Some("none") && transport.get("reply_match").is_some(),
        _ => false,
    }
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
    /// A host file a response body is saved to, given as `{"file":
    /// "<path>"}`: a recording, a snapshot, a configuration backup. The
    /// body is streamed to disk as it arrives (SPEC.md §4, "Saving a
    /// response to a file").
    Save,
}

/// The path of a `save` parameter: an object whose only member is `file`, a
/// non-empty string naming a file (not a folder) in a folder that exists.
pub(crate) fn save_file(name: &str, value: &Value) -> Result<std::path::PathBuf, String> {
    let path =
        binary_file(value).ok_or_else(|| format!("'{name}' must be {{\"file\": \"<path>\"}}"))?;
    let path = std::path::PathBuf::from(path);
    if path.is_dir() {
        return Err(format!("'{name}': {} is a folder", path.display()));
    }
    if path.file_name().is_none() {
        return Err(format!("'{name}': {} names no file", path.display()));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    if !parent.is_dir() {
        return Err(format!(
            "'{name}': the folder {} does not exist",
            parent.display()
        ));
    }
    Ok(path)
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

    /// Whether these (validated) settings have the core host a session for
    /// any host to join (`link` `rtp_host`, the MIDI integrations' hosted
    /// RTP-MIDI session), so the device can be opened without a host.
    pub fn hosts_session(&self, settings: &Params) -> bool {
        settings.get("link").and_then(Value::as_str) == Some("rtp_host")
    }

    pub fn model(&self, id: &str) -> Option<&ModelSpec> {
        self.models.iter().find(|m| m.id == id)
    }

    /// Parse a spec, and add what its settings and telemetry imply.
    pub(crate) fn from_yaml(text: &str) -> Result<DeviceSpec, String> {
        let mut spec: DeviceSpec = crate::yaml::from_str(text).map_err(|e| e.to_string())?;
        spec.expand_select()?;
        spec.expand_polled()?;
        Ok(spec)
    }

    /// The settings chosen after signing in (`select`): each is checked, and
    /// each command lists those its messages name in `needs`.
    fn expand_select(&mut self) -> Result<(), String> {
        let selects: Vec<(String, Select)> = self
            .settings
            .iter()
            .filter_map(|(n, s)| s.select.clone().map(|sel| (n.clone(), sel)))
            .collect();
        for (name, select) in &selects {
            let setting = &self.settings[name];
            if setting.required || setting.default.is_some() {
                return Err(format!(
                    "settings.{name}: a select setting is chosen after opening, so it is \
                     neither required nor given a default"
                ));
            }
            let command = self.commands.get(&select.command).ok_or_else(|| {
                format!("settings.{name}.select: no command '{}'", select.command)
            })?;
            if command.returns != "value" {
                return Err(format!(
                    "settings.{name}.select: '{}' must return the list (returns: value)",
                    select.command
                ));
            }
            if command
                .send
                .as_ref()
                .is_some_and(|s| names_setting(s, name))
            {
                return Err(format!(
                    "settings.{name}.select: '{}' names the setting it lists the choices for",
                    select.command
                ));
            }
            if let Some(pattern) = &select.id_match {
                let re = Regex::new(pattern)
                    .map_err(|e| format!("settings.{name}.select.id_match: {e}"))?;
                if re.captures_len() < 2 {
                    return Err(format!(
                        "settings.{name}.select.id_match: the id is its first group"
                    ));
                }
            }
            for path in &select.clears {
                let declared = self.state.keys().any(|k| {
                    k == path
                        || k.strip_prefix(path.as_str())
                            .is_some_and(|rest| rest.starts_with('.'))
                });
                if !declared {
                    return Err(format!(
                        "settings.{name}.select.clears: '{path}' is no declared state"
                    ));
                }
            }
            for (param, from) in &select.params {
                if !command.params.contains_key(param) {
                    return Err(format!(
                        "settings.{name}.select.params: '{}' has no parameter '{param}'",
                        select.command
                    ));
                }
                if !self.settings.contains_key(from) || from == name {
                    return Err(format!(
                        "settings.{name}.select.params: '{from}' is not another setting"
                    ));
                }
            }
        }
        for command in self.commands.values_mut() {
            let Some(send) = &command.send else { continue };
            command.needs = selects
                .iter()
                .filter(|(n, _)| names_setting(send, n))
                .map(|(n, _)| n.clone())
                .collect();
        }
        Ok(())
    }

    /// The `polled.<group>` state of a spec-driven device whose transport
    /// waits for each poll item's answer (SPEC.md §8).
    fn expand_polled(&mut self) -> Result<(), String> {
        if self.transport.is_some()
            && self
                .state
                .keys()
                .any(|k| k == "polled" || k.starts_with("polled."))
        {
            return Err("state: polled.* is the core's own; leave it out".into());
        }
        let Some(transport) = self.transport.as_ref() else {
            return Ok(());
        };
        if !polls_answered(transport) {
            return Ok(());
        }
        let telemetry = self.telemetry.as_ref();
        let groups: Vec<Value> = match telemetry.and_then(|t| t.get("poll")) {
            Some(Value::Array(g)) => g.clone(),
            Some(one) => vec![one.clone()],
            None => Vec::new(),
        };
        let names = poll_group_names(telemetry);
        for (name, group) in names.iter().zip(&groups) {
            let every = match group.get("every_ms").and_then(Value::as_u64) {
                Some(ms) => format!("polled every {ms} ms"),
                None => "read once on connecting".into(),
            };
            let key = format!("polled.{name}");
            if self.state.contains_key(&key) {
                return Err(format!("telemetry.poll: two groups named '{name}'"));
            }
            self.state.insert(
                key,
                StateField {
                    kind: "int".into(),
                    unit: Some("s".into()),
                    description: format!(
                        "When the poll group {name} ({every}) was last answered, in Unix \
                         seconds: its data is as old as this"
                    ),
                },
            );
        }
        Ok(())
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
    for name in given.keys() {
        if !declared.contains_key(name) {
            return Err(format!("unknown parameter '{name}'"));
        }
    }

    let mut out = Params::new();
    for (name, spec) in declared {
        let value = match given.get(name) {
            Some(Value::Null) | None => match spec.default.as_ref() {
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
        ParamType::Float if spec.negative_infinity && value.as_str() == Some("-inf") => Ok(()),
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
        ParamType::Save => save_file(name, value).map(|_| ()),
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
    use crate::json;

    fn spec(yaml: &str) -> BTreeMap<String, ParamSpec> {
        crate::yaml::from_str(yaml).unwrap()
    }

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn a_float_takes_minus_infinity_only_when_declared() {
        let decl = spec(concat!(
            "gain: { type: float, min: -60.0, max: 6.0, negative_infinity: true }
",
            "level: { type: float, min: -60.0, max: 6.0 }",
        ));
        assert!(validate(&decl, &params(json!({"gain": "-inf"}))).is_ok());
        assert!(validate(&decl, &params(json!({"gain": -6.0}))).is_ok());
        assert!(validate(&decl, &params(json!({"gain": "inf"}))).is_err());
        assert!(validate(&decl, &params(json!({"level": "-inf"}))).is_err());
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
        // Parsed once: parsing every spec for each check takes minutes.
        let embedded = Catalog::embedded();
        let ids: Vec<String> = embedded.devices.keys().take(2).cloned().collect();
        if let Some(first) = ids.first() {
            let selected = embedded.clone().select(&ids).unwrap();
            assert_eq!(selected.devices.keys().cloned().collect::<Vec<_>>(), ids);
            assert!(selected.device(first).is_some());
        }
        let err = embedded
            .clone()
            .select(&["no-such-device".into()])
            .unwrap_err();
        assert!(err.contains("unknown device 'no-such-device'"), "{err}");
        assert!(embedded.clone().select(&[]).is_err());
        if let Some((id, _, _)) = ALL_SPECS.iter().find(|(_, built, _)| !built) {
            let err = embedded.clone().select(&[id.to_string()]).unwrap_err();
            assert!(err.contains(&format!("the '{id}' feature")), "{err}");
        }
        let every = embedded.clone().select(&["all".into()]).unwrap();
        assert_eq!(every.devices.len(), EMBEDDED_SPECS.len());
        for (group, specs) in VENDOR_GROUPS {
            let built: Vec<&str> = specs
                .iter()
                .copied()
                .filter(|s| embedded.device(s).is_some())
                .collect();
            match embedded.clone().select(&[group.to_string()]) {
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
