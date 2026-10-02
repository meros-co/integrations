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
    /// The frame encoding: `jpeg`.
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
    pub fn model(&self, id: &str) -> Option<&ModelSpec> {
        self.models.iter().find(|m| m.id == id)
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
            let spec: DeviceSpec = serde_yaml::from_str(text)
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
                let spec: DeviceSpec =
                    serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap())
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
            Some(Value::Null) | None => match &spec.default {
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
            if s.chars().any(|c| (c as u32) < 0x20) {
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
        serde_yaml::from_str(yaml).unwrap()
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
