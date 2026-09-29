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
    /// Wire behaviour for spec-driven devices, kept verbatim for the spec engine.
    #[serde(default, skip_serializing)]
    pub transport: Option<Value>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateField {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub description: String,
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

/// Every embedded spec, keyed by id.
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
        assert!(catalog.devices.len() >= 14);
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
